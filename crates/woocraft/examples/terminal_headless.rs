//! External terminal control demo — no GUI required.
//!
//! Shows how a host process (an editor, an agent, a CI job, ...) can drive a
//! terminal session headlessly through the `woocraft-terminal` core crate:
//!
//! 1. spawn the platform default shell over a PTY,
//! 2. write commands into it,
//! 3. wait for output by watching the screen,
//! 4. read snapshot text / recent lines,
//! 5. resize the PTY,
//! 6. collect the exit code.
//!
//! Note that a PTY echoes typed input back, so the echoed command line
//! contains every marker we send. The demo therefore waits for a marker to
//! appear as a *standalone* line: command output renders on its own line,
//! while the echo is always part of a longer prompt line.
//!
//! Run with: `cargo run -p woocraft --example terminal_headless`

use std::{thread, time::Duration};

use anyhow::{Result, bail};
use woocraft_terminal::{TerminalEvent, TerminalSession};

/// Timeout for the waits below.
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);

/// How often the polling loop re-checks the terminal screen.
const POLL_INTERVAL: Duration = Duration::from_millis(5);

/// Blocking wait until one of the last `tail` non-empty screen lines is
/// exactly `line` (ignoring surrounding whitespace).
fn wait_for_line(
  events: &async_channel::Receiver<TerminalEvent>, session: &TerminalSession, line: &str,
  tail: usize, timeout: Duration,
) -> Result<()> {
  let deadline = std::time::Instant::now() + timeout;
  loop {
    if session
      .last_n_non_empty_lines(tail)
      .iter()
      .any(|text| text.trim() == line)
    {
      return Ok(());
    }
    if std::time::Instant::now() >= deadline {
      bail!("timed out waiting for output line {line:?}");
    }
    match events.try_recv() {
      Ok(TerminalEvent::Exit) => bail!("terminal exited while waiting for output line {line:?}"),
      Err(async_channel::TryRecvError::Closed) => {
        bail!("terminal event stream closed while waiting for output line {line:?}")
      }
      _ => thread::sleep(POLL_INTERVAL),
    }
  }
}

fn main() -> Result<()> {
  let (session, events) = woocraft_terminal::control::spawn_with_events(
    woocraft_terminal::SpawnOptions::default_shell_options(),
    woocraft_terminal::TerminalBounds::default(),
  )?;
  println!("spawned shell (pid: {:?})", session.pid());

  // Type a command into the interactive shell and wait for its output.
  // `\r` is the carriage-return `Enter` sends over a PTY. The tail of 2
  // covers `[command output, next prompt]` once the shell finishes.
  session.input_str("echo hello-woocraft\r");
  wait_for_line(&events, &session, "hello-woocraft", 2, WAIT_TIMEOUT)?;

  // Snapshots expose the live grid: cursor, modes and recent lines.
  let snapshot = session.snapshot();
  println!(
    "snapshot: {}x{} cells, {} lines of scrollback, cursor at ({}, {})",
    snapshot.columns,
    snapshot.screen_lines,
    snapshot.total_lines - snapshot.screen_lines,
    snapshot.cursor.point.line,
    snapshot.cursor.point.column,
  );
  println!(
    "last non-empty lines: {:?}",
    session.last_n_non_empty_lines(3)
  );

  // The PTY can be resized at any time; the emulator grid follows.
  session.resize(woocraft_terminal::TerminalBounds::new(24.0, 10.0, 120, 30));
  let resized = session.snapshot();
  println!(
    "resized to {}x{} cells",
    resized.columns, resized.screen_lines
  );

  // Leave the shell and collect its exit code.
  session.input_str("exit\r");
  let code = woocraft_terminal::control::wait_for_exit_blocking(&events, &session, WAIT_TIMEOUT)?;
  println!("shell exited with code {code}");

  // `kill` is idempotent and safe to call even after a normal exit; it
  // releases the PTY and stops the session's background threads.
  session.kill();
  Ok(())
}
