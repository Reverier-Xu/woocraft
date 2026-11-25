//! Headless helpers for externally controlling terminal sessions.
//!
//! These helpers work without any async runtime: the async variants drive
//! themselves through a small shared timer thread, while the `_blocking`
//! variants can be called from plain synchronous code. Both share one polling
//! routine per query; only the way they wait between polls differs.

use std::{
  collections::BinaryHeap,
  future::Future,
  pin::Pin,
  sync::{
    Arc, Condvar, LazyLock, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
  },
  task::{Context, Poll, Waker},
  thread,
  time::{Duration, Instant},
};

use anyhow::{Result, anyhow};
use async_channel::{Receiver, TryRecvError};

use crate::{
  event::TerminalEvent, options::SpawnOptions, session::TerminalSession, types::TerminalBounds,
};

/// How often polling helpers re-check the terminal state.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Spawns a session and returns it paired with its event receiver.
pub fn spawn_with_events(
  options: SpawnOptions, bounds: TerminalBounds,
) -> Result<(TerminalSession, Receiver<TerminalEvent>)> {
  let session = TerminalSession::spawn(options, bounds)?;
  let events = session.events();
  Ok((session, events))
}

// ---------------------------------------------------------------------------
// Polling helpers
// ---------------------------------------------------------------------------

/// One iteration of a polling loop.
///
/// The query-specific steps ([`poll_text`], [`poll_exit`]) are shared by the
/// async and the blocking helpers; only the driver waiting between
/// [`Step::Retry`]s differs.
enum Step<T> {
  /// Polling finished.
  Done(T),
  /// Poll again after this delay.
  Retry(Duration),
}

/// Drives `step` to completion, awaiting the shared timer between polls.
async fn poll_async<T>(mut step: impl FnMut() -> Step<T>) -> T {
  loop {
    match step() {
      Step::Done(value) => return value,
      Step::Retry(delay) => sleep(delay).await,
    }
  }
}

/// Blocking counterpart of [`poll_async`]: sleeps on the calling thread.
fn poll_blocking<T>(mut step: impl FnMut() -> Step<T>) -> T {
  loop {
    match step() {
      Step::Done(value) => return value,
      Step::Retry(delay) => thread::sleep(delay),
    }
  }
}

/// The delay before the next poll: [`POLL_INTERVAL`], shortened so a poll
/// never sleeps past `deadline`. `None` once the deadline has passed.
fn next_poll_delay(deadline: Instant) -> Option<Duration> {
  deadline
    .checked_duration_since(Instant::now())
    .map(|remaining| remaining.min(POLL_INTERVAL))
}

/// Waits until the terminal output contains `needle`, returning the full
/// terminal text at that point.
///
/// Fails with a timeout error if the text does not appear within `timeout`,
/// and with an exit error if the terminal exits (or its event stream closes)
/// before a match is found.
pub async fn wait_for_text(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, needle: &str, timeout: Duration,
) -> Result<String> {
  let deadline = Instant::now() + timeout;
  poll_async(|| poll_text(events, session, needle, deadline)).await
}

/// Blocking variant of [`wait_for_text`].
pub fn wait_for_text_blocking(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, needle: &str, timeout: Duration,
) -> Result<String> {
  let deadline = Instant::now() + timeout;
  poll_blocking(|| poll_text(events, session, needle, deadline))
}

/// One [`wait_for_text`] iteration.
fn poll_text(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, needle: &str, deadline: Instant,
) -> Step<Result<String>> {
  let text = session.text();
  if text.contains(needle) {
    return Step::Done(Ok(text));
  }
  match events.try_recv() {
    // The PTY event loop applies every pending output byte before it queues
    // `Exit`, so a poll that raced the exit event only needs one last look
    // before reporting a failure. Without this, a fast `echo … && exit` child
    // makes CI flake with "terminal exited before output appeared" even though
    // the text is there.
    Ok(TerminalEvent::Exit) | Err(TryRecvError::Closed) => Step::Done(
      text_after_exit(session, needle).ok_or_else(|| exited_before_match(needle, session)),
    ),
    Ok(_) | Err(TryRecvError::Empty) => match next_poll_delay(deadline) {
      Some(delay) => Step::Retry(delay),
      None => Step::Done(Err(anyhow!("timed out waiting for text {needle:?}"))),
    },
  }
}

/// Final `needle` check once the terminal exited.
fn text_after_exit(session: &TerminalSession, needle: &str) -> Option<String> {
  let text = session.text();
  text.contains(needle).then_some(text)
}

fn exited_before_match(needle: &str, session: &TerminalSession) -> anyhow::Error {
  anyhow!(
    "terminal exited before {needle:?} appeared; last output: {:?}",
    session.last_n_non_empty_lines(8)
  )
}

/// Waits until the child process exits, returning its exit code
/// (`-1` when it was terminated by a signal or the code is unavailable).
pub async fn wait_for_exit(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, timeout: Duration,
) -> Result<i32> {
  let deadline = Instant::now() + timeout;
  poll_async(|| poll_exit(events, session, deadline)).await
}

/// Blocking variant of [`wait_for_exit`].
pub fn wait_for_exit_blocking(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, timeout: Duration,
) -> Result<i32> {
  let deadline = Instant::now() + timeout;
  poll_blocking(|| poll_exit(events, session, deadline))
}

/// One [`wait_for_exit`] iteration.
///
/// `ChildExit` and `Exit` are only hints that the status recorded on the
/// session is worth re-reading, so the status check at the top of every
/// iteration is what actually ends the wait.
fn poll_exit(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, deadline: Instant,
) -> Step<Result<i32>> {
  if let Some(status) = session.child_exit_status() {
    return Step::Done(Ok(status.code()));
  }
  if let Err(TryRecvError::Closed) = events.try_recv() {
    return Step::Done(Err(anyhow!(
      "terminal event stream closed before the child exited"
    )));
  }
  match next_poll_delay(deadline) {
    Some(delay) => Step::Retry(delay),
    None => Step::Done(Err(anyhow!("timed out waiting for terminal exit"))),
  }
}

// ---------------------------------------------------------------------------
// Runtime-free timer
// ---------------------------------------------------------------------------

/// The process-wide timer backing [`sleep_until`]: a single thread waking
/// futures whose deadline has passed. The crate stays runtime-free.
struct Timer {
  /// Pending sleeps, ordered so the earliest deadline is at the top.
  queue: Mutex<BinaryHeap<TimerEntry>>,
  /// Signalled whenever an entry is added or cancelled.
  signal: Condvar,
  next_sequence: AtomicU64,
}

static TIMER: LazyLock<&'static Timer> = LazyLock::new(|| {
  // Leaked on purpose: the timer thread and its queue live for as long as the
  // process does, so a `Sleep` can always register without a handle.
  let timer: &'static Timer = Box::leak(Box::new(Timer {
    queue: Mutex::new(BinaryHeap::new()),
    signal: Condvar::new(),
    next_sequence: AtomicU64::new(0),
  }));
  thread::Builder::new()
    .name("woocraft-terminal-timer".into())
    .spawn(move || timer.run())
    .expect("failed to spawn timer thread");
  timer
});

/// The wake bookkeeping shared between a [`Sleep`] and the timer thread.
#[derive(Default)]
struct SleepState {
  waker: Mutex<Option<Waker>>,
  /// Set when the [`Sleep`] was dropped before its deadline passed.
  canceled: AtomicBool,
}

impl SleepState {
  fn set_waker(&self, waker: &Waker) {
    let mut slot = self
      .waker
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    match &*slot {
      Some(current) if current.will_wake(waker) => {}
      _ => *slot = Some(waker.clone()),
    }
  }

  fn take_waker(&self) -> Option<Waker> {
    self
      .waker
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .take()
  }
}

/// A scheduled [`Sleep`].
struct TimerEntry {
  deadline: Instant,
  /// Total order for equal deadlines; without it the heap's `Ord` would not
  /// be consistent with `Eq`.
  sequence: u64,
  state: Arc<SleepState>,
}

impl PartialEq for TimerEntry {
  fn eq(&self, other: &Self) -> bool {
    self.cmp(other) == std::cmp::Ordering::Equal
  }
}

impl Eq for TimerEntry {}

impl PartialOrd for TimerEntry {
  fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for TimerEntry {
  fn cmp(&self, other: &Self) -> std::cmp::Ordering {
    // Reversed: `BinaryHeap` is a max-heap and the earliest deadline has to
    // come out first.
    other
      .deadline
      .cmp(&self.deadline)
      .then_with(|| other.sequence.cmp(&self.sequence))
  }
}

impl Timer {
  /// Queues a wake-up for `waker` at `deadline` and returns the state the
  /// timer thread uses to reach it.
  fn register(&self, deadline: Instant, waker: &Waker) -> Arc<SleepState> {
    let state = Arc::new(SleepState::default());
    state.set_waker(waker);
    self
      .queue
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner())
      .push(TimerEntry {
        deadline,
        sequence: self.next_sequence.fetch_add(1, Ordering::Relaxed),
        state: state.clone(),
      });
    // The new entry may be the earliest one, so the thread has to recompute
    // how long it may sleep.
    self.signal.notify_all();
    state
  }

  /// The timer thread: hands every due entry's waker to its executor.
  fn run(&'static self) {
    loop {
      // Waking outside the lock keeps a waker that re-polls synchronously
      // from deadlocking against the queue.
      let due = self.pop_due();
      for entry in due {
        if let Some(waker) = entry.state.take_waker() {
          waker.wake();
        }
      }
    }
  }

  /// Blocks until an entry is due, then removes every entry that is due.
  ///
  /// Cancelled entries are dropped lazily: a dropped [`Sleep`] keeps its slot
  /// until it reaches the head of the queue, at which point it is discarded
  /// without waking anyone. A cancelled sleep therefore costs one queue slot
  /// until its deadline passes, and never delays the entries behind it.
  fn pop_due(&'static self) -> Vec<TimerEntry> {
    let mut queue = self
      .queue
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    loop {
      let mut due = Vec::new();
      let now = Instant::now();
      while let Some(entry) = queue.peek() {
        let canceled = entry.state.canceled.load(Ordering::SeqCst);
        let is_due = entry.deadline <= now;
        if !canceled && !is_due {
          break;
        }
        let entry = queue.pop().expect("peeked entry");
        if !canceled {
          due.push(entry);
        }
      }
      if !due.is_empty() {
        return due;
      }
      let next = queue
        .peek()
        .map(|entry| entry.deadline.saturating_duration_since(Instant::now()));
      queue = match next {
        Some(wait) => {
          self
            .signal
            .wait_timeout(queue, wait)
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .0
        }
        None => self
          .signal
          .wait(queue)
          .unwrap_or_else(|poisoned| poisoned.into_inner()),
      };
    }
  }
}

/// A future that completes at (or shortly after) `deadline`.
///
/// Backed by the shared timer thread; this intentionally contains no async
/// runtime so the crate stays runtime-free. Dropping it before completion
/// cancels the wake-up.
pub struct Sleep {
  deadline: Instant,
  state: Option<Arc<SleepState>>,
}

impl Sleep {
  fn new(deadline: Instant) -> Self {
    Self {
      deadline,
      state: None,
    }
  }
}

impl Unpin for Sleep {}

impl Future for Sleep {
  type Output = ();

  fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    let this = self.get_mut();
    if Instant::now() >= this.deadline {
      return Poll::Ready(());
    }
    match &this.state {
      Some(state) => state.set_waker(cx.waker()),
      None => this.state = Some(TIMER.register(this.deadline, cx.waker())),
    }
    // The timer thread only pops an entry once its deadline passed, so the
    // waker may already be gone: re-check instead of sleeping forever.
    if Instant::now() >= this.deadline {
      return Poll::Ready(());
    }
    Poll::Pending
  }
}

impl Drop for Sleep {
  fn drop(&mut self) {
    let Some(state) = &self.state else {
      return;
    };
    state.canceled.store(true, Ordering::SeqCst);
    // Nudge the thread so it skips the cancelled head instead of waiting for
    // a deadline nobody is interested in anymore.
    TIMER.signal.notify_all();
  }
}

/// Completes at (or shortly after) `deadline`.
pub fn sleep_until(deadline: Instant) -> Sleep {
  Sleep::new(deadline)
}

/// Completes after `duration`.
pub fn sleep(duration: Duration) -> Sleep {
  Sleep::new(Instant::now() + duration)
}

#[cfg(test)]
mod tests {
  use super::*;

  /// Minimal executor for polling futures without a runtime.
  fn block_on<F: Future>(future: F) -> F::Output {
    struct Notify(Arc<std::sync::atomic::AtomicBool>, thread::Thread);
    impl std::task::Wake for Notify {
      fn wake(self: Arc<Self>) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        self.1.unpark();
      }
    }

    let notified = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let waker = Waker::from(Arc::new(Notify(notified.clone(), thread::current())));
    let mut cx = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
      if let Poll::Ready(output) = future.as_mut().poll(&mut cx) {
        return output;
      }
      while !notified.swap(false, std::sync::atomic::Ordering::SeqCst) {
        thread::park();
      }
    }
  }

  #[test]
  fn sleep_completes_after_duration() {
    let start = Instant::now();
    block_on(sleep(Duration::from_millis(30)));
    assert!(start.elapsed() >= Duration::from_millis(25));
  }

  #[test]
  fn sleep_until_past_deadline_is_ready() {
    let start = Instant::now();
    block_on(sleep_until(start - Duration::from_secs(1)));
    assert!(start.elapsed() < Duration::from_millis(100));
  }

  #[test]
  fn earlier_deadline_is_not_blocked_by_a_later_one() {
    // A far-future sleep registered first must not delay the shorter one that
    // follows it; a correct queue always waits on the earliest deadline.
    let mut late = std::pin::pin!(sleep(Duration::from_secs(5)));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(late.as_mut().poll(&mut cx).is_pending());

    let start = Instant::now();
    block_on(sleep(Duration::from_millis(20)));
    assert!(
      start.elapsed() < Duration::from_secs(2),
      "the later deadline delayed the earlier one"
    );
  }

  #[test]
  fn dropped_sleep_is_skipped() {
    // Cancelling must not leave the timer thread waiting for the dropped
    // deadline: the next sleep still fires on time.
    {
      let mut canceled = std::pin::pin!(sleep(Duration::from_secs(5)));
      let mut cx = Context::from_waker(Waker::noop());
      assert!(canceled.as_mut().poll(&mut cx).is_pending());
    }

    let start = Instant::now();
    block_on(sleep(Duration::from_millis(20)));
    assert!(
      start.elapsed() < Duration::from_secs(2),
      "the dropped sleep kept the timer thread busy"
    );
  }
}
