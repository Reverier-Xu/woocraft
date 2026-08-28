//! Headless helpers for externally controlling terminal sessions.
//!
//! These helpers work without any async runtime: the async variants drive
//! themselves through a small shared timer thread, while the `_blocking`
//! variants can be called from plain synchronous code.

use std::{
  future::Future,
  pin::Pin,
  sync::{
    Arc, Condvar, LazyLock, Mutex,
    mpsc::{self, Sender as MpscSender},
  },
  task::{Context, Poll, Waker},
  thread,
  time::{Duration, Instant},
};

use anyhow::{Result, anyhow, bail};
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
  loop {
    let text = session.text();
    if text.contains(needle) {
      return Ok(text);
    }
    check_not_expired(deadline, needle)?;
    match events.try_recv() {
      Ok(TerminalEvent::Exit) => bail!(exited_before_match(needle, session)),
      Ok(_) => {}
      Err(TryRecvError::Closed) => bail!(exited_before_match(needle, session)),
      Err(TryRecvError::Empty) => {}
    }
    sleep_until(deadline.min(Instant::now() + POLL_INTERVAL)).await;
  }
}

/// Blocking variant of [`wait_for_text`].
pub fn wait_for_text_blocking(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, needle: &str, timeout: Duration,
) -> Result<String> {
  let deadline = Instant::now() + timeout;
  loop {
    let text = session.text();
    if text.contains(needle) {
      return Ok(text);
    }
    check_not_expired(deadline, needle)?;
    match events.try_recv() {
      Ok(TerminalEvent::Exit) => bail!(exited_before_match(needle, session)),
      Ok(_) => {}
      Err(TryRecvError::Closed) => bail!(exited_before_match(needle, session)),
      Err(TryRecvError::Empty) => thread::sleep(POLL_INTERVAL),
    }
  }
}

/// Waits until the child process exits, returning its exit code
/// (`-1` when it was terminated by a signal or the code is unavailable).
pub async fn wait_for_exit(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, timeout: Duration,
) -> Result<i32> {
  let deadline = Instant::now() + timeout;
  loop {
    if let Some(status) = session.child_exit_status() {
      return Ok(status.code());
    }
    if Instant::now() >= deadline {
      bail!("timed out waiting for terminal exit");
    }
    match events.try_recv() {
      Ok(TerminalEvent::Exit | TerminalEvent::ChildExit(_)) => {
        if let Some(status) = session.child_exit_status() {
          return Ok(status.code());
        }
      }
      Ok(_) => {}
      Err(TryRecvError::Closed) => {
        if let Some(status) = session.child_exit_status() {
          return Ok(status.code());
        }
        bail!("terminal event stream closed before the child exited");
      }
      Err(TryRecvError::Empty) => {}
    }
    sleep_until(deadline.min(Instant::now() + POLL_INTERVAL)).await;
  }
}

/// Blocking variant of [`wait_for_exit`].
pub fn wait_for_exit_blocking(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, timeout: Duration,
) -> Result<i32> {
  let deadline = Instant::now() + timeout;
  loop {
    if let Some(status) = session.child_exit_status() {
      return Ok(status.code());
    }
    if Instant::now() >= deadline {
      bail!("timed out waiting for terminal exit");
    }
    match events.try_recv() {
      Ok(TerminalEvent::Exit | TerminalEvent::ChildExit(_)) => {
        if let Some(status) = session.child_exit_status() {
          return Ok(status.code());
        }
      }
      Ok(_) => {}
      Err(TryRecvError::Closed) => {
        if let Some(status) = session.child_exit_status() {
          return Ok(status.code());
        }
        bail!("terminal event stream closed before the child exited");
      }
      Err(TryRecvError::Empty) => thread::sleep(POLL_INTERVAL),
    }
  }
}

fn check_not_expired(deadline: Instant, needle: &str) -> Result<()> {
  if Instant::now() >= deadline {
    bail!("timed out waiting for text {needle:?}");
  }
  Ok(())
}

fn exited_before_match(needle: &str, session: &TerminalSession) -> anyhow::Error {
  anyhow!(
    "terminal exited before {needle:?} appeared; last output: {:?}",
    session.last_n_non_empty_lines(8)
  )
}

// ---------------------------------------------------------------------------
// Runtime-free timer
// ---------------------------------------------------------------------------

/// The shared timer thread backing [`sleep_until`].
struct Timer {
  schedule: MpscSender<(Instant, Waker)>,
}

static TIMER: LazyLock<Timer> = LazyLock::new(|| {
  let (schedule_tx, schedule_rx) = mpsc::channel::<(Instant, Waker)>();
  thread::Builder::new()
    .name("woocraft-terminal-timer".into())
    .spawn(move || run_timer(schedule_rx))
    .expect("failed to spawn timer thread");
  Timer {
    schedule: schedule_tx,
  }
});

fn run_timer(schedule_rx: mpsc::Receiver<(Instant, Waker)>) {
  struct Pending {
    heap: Mutex<Vec<(Instant, Waker)>>,
    signal: Condvar,
  }
  let pending = Arc::new(Pending {
    heap: Mutex::new(Vec::new()),
    signal: Condvar::new(),
  });
  let push_pending = pending.clone();
  thread::Builder::new()
    .name("woocraft-terminal-timer-schedule".into())
    .spawn(move || {
      for (deadline, waker) in schedule_rx {
        push_pending.heap.lock().unwrap().push((deadline, waker));
        push_pending.signal.notify_all();
      }
    })
    .expect("failed to spawn timer scheduler thread");

  let mut heap = pending.heap.lock().unwrap();
  loop {
    // Drain everything that is due right now.
    let now = Instant::now();
    heap.sort_by_key(|(deadline, _)| *deadline);
    let due = heap
      .iter()
      .take_while(|(deadline, _)| *deadline <= now)
      .count();
    for (_, waker) in heap.drain(..due) {
      waker.wake();
    }
    match heap.first() {
      Some(&(next, _)) => {
        let wait = next.saturating_duration_since(Instant::now());
        // A new, earlier deadline may arrive while waiting; the scheduler
        // signals the condvar and we recompute.
        let (guard, _) = pending
          .signal
          .wait_timeout_while(heap, wait, |heap| {
            !heap.iter().any(|(deadline, _)| *deadline <= Instant::now())
          })
          .unwrap();
        heap = guard;
      }
      None => {
        heap = pending
          .signal
          .wait(heap)
          .unwrap_or_else(|poisoned| poisoned.into_inner());
      }
    }
  }
}

/// A future that completes at (or shortly after) `deadline`.
///
/// Backed by the shared timer thread; this intentionally contains no async
/// runtime so the crate stays runtime-free.
pub struct Sleep {
  deadline: Instant,
  scheduled: bool,
}

impl Sleep {
  fn new(deadline: Instant) -> Self {
    Self {
      deadline,
      scheduled: false,
    }
  }
}

impl Unpin for Sleep {}

impl Future for Sleep {
  type Output = ();

  fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
    let this = &mut *self;
    if Instant::now() >= this.deadline {
      return Poll::Ready(());
    }
    if !this.scheduled {
      TIMER
        .schedule
        .send((this.deadline, cx.waker().clone()))
        .expect("timer thread died");
      this.scheduled = true;
    }
    Poll::Pending
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
    let mut future = Box::pin(future);
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
}
