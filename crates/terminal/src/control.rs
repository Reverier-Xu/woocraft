//! Headless helpers for externally controlling terminal sessions.
//!
//! These helpers work without any async runtime: they poll the session from the
//! calling thread, so they can be used from a plain synchronous context (a
//! test, an example, a worker thread).

use std::{
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
enum Step<T> {
  /// Polling finished.
  Done(T),
  /// Poll again after this delay.
  Retry(Duration),
}

/// Drives a polling step to completion, sleeping on the calling thread.
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
pub fn wait_for_text_blocking(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, needle: &str, timeout: Duration,
) -> Result<String> {
  let deadline = Instant::now() + timeout;
  poll_blocking(|| poll_text(events, session, needle, deadline))
}

/// One [`wait_for_text_blocking`] iteration.
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
pub fn wait_for_exit_blocking(
  events: &Receiver<TerminalEvent>, session: &TerminalSession, timeout: Duration,
) -> Result<i32> {
  let deadline = Instant::now() + timeout;
  poll_blocking(|| poll_exit(events, session, deadline))
}

/// One [`wait_for_exit_blocking`] iteration.
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
