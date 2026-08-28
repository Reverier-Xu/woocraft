//! Integration tests for PTY-backed terminal sessions.
//!
//! These run fully headless: they spawn real child processes through the
//! cross-platform PTY and assert on snapshots and events.

use std::time::Duration;

use woocraft_terminal::{
  SpawnOptions, TerminalBounds,
  control::{spawn_with_events, wait_for_exit_blocking, wait_for_text_blocking},
};

const TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_BOUNDS: TerminalBounds = TerminalBounds::new(20.0, 8.0, 80, 24);

fn shell_options(program: &str, args: &[&str]) -> SpawnOptions {
  SpawnOptions::with_shell((
    program.to_string(),
    args.iter().map(|arg| arg.to_string()).collect(),
  ))
}

#[cfg(unix)]
#[test]
fn echo_marker_then_exit() {
  let (session, events) = spawn_with_events(
    shell_options("sh", &["-c", "echo woocraft-marker-42"]),
    DEFAULT_BOUNDS,
  )
  .expect("failed to spawn session");

  assert!(session.pid().is_some(), "the child pid should be exposed");
  assert!(session.is_alive());

  let text =
    wait_for_text_blocking(&events, &session, "woocraft-marker-42", TIMEOUT).expect("marker");
  assert!(text.contains("woocraft-marker-42"));

  let code = wait_for_exit_blocking(&events, &session, TIMEOUT).expect("exit");
  assert_eq!(code, 0);
  assert!(!session.is_alive());
  assert_eq!(session.child_exit_status().expect("status").code(), 0);
}

#[cfg(windows)]
#[test]
fn echo_marker_then_exit() {
  let (session, events) = spawn_with_events(
    shell_options("cmd.exe", &["/C", "echo woocraft-marker-42"]),
    DEFAULT_BOUNDS,
  )
  .expect("failed to spawn session");

  assert!(session.pid().is_some());
  let text =
    wait_for_text_blocking(&events, &session, "woocraft-marker-42", TIMEOUT).expect("marker");
  assert!(text.contains("woocraft-marker-42"));
  wait_for_exit_blocking(&events, &session, TIMEOUT).expect("exit");
  assert!(!session.is_alive());
}

#[cfg(unix)]
#[test]
fn interactive_echo_round_trip() {
  let (session, events) =
    spawn_with_events(shell_options("cat", &[]), DEFAULT_BOUNDS).expect("failed to spawn session");

  // Wait until the child is attached: `cat` echoes the pty's line discipline
  // feedback, so typed characters show up as cells.
  session.input(b"hi\r".as_slice());
  let text = wait_for_text_blocking(&events, &session, "hi", TIMEOUT).expect("echo");
  assert!(text.contains("hi"));

  // Send EOF (ctrl-d) to terminate `cat`.
  session.input(b"\x04".as_slice());
  let code = wait_for_exit_blocking(&events, &session, TIMEOUT).expect("exit");
  assert_eq!(code, 0);
}

#[cfg(unix)]
#[test]
fn resize_updates_grid_and_reflows() {
  let (session, events) = spawn_with_events(
    shell_options("sh", &["-c", "printf 'A%.0s' $(seq 1 120)"]),
    TerminalBounds::new(20.0, 8.0, 80, 24),
  )
  .expect("failed to spawn session");

  // At 80 columns the 120 characters wrap across two lines.
  wait_for_text_blocking(&events, &session, &"A".repeat(80), TIMEOUT).expect("first chunk");
  let content = session.snapshot();
  assert_eq!(content.columns, 80);
  assert_eq!(content.screen_lines, 24);

  // After growing the viewport the content reflows into a single line.
  session.resize(TerminalBounds::new(20.0, 8.0, 200, 24));
  let text =
    wait_for_text_blocking(&events, &session, &"A".repeat(120), TIMEOUT).expect("reflowed");
  assert_eq!(session.snapshot().columns, 200);
  assert!(text.contains(&"A".repeat(120)));
}

#[cfg(windows)]
#[test]
fn resize_updates_grid() {
  let (session, events) = spawn_with_events(
    shell_options(
      "powershell.exe",
      &[
        "-NoProfile",
        "-Command",
        "Write-Host -NoNewline ('A' * 120)",
      ],
    ),
    TerminalBounds::new(20.0, 8.0, 80, 24),
  )
  .expect("failed to spawn session");

  wait_for_text_blocking(&events, &session, &"A".repeat(80), TIMEOUT).expect("first chunk");
  session.resize(TerminalBounds::new(20.0, 8.0, 200, 24));
  let text =
    wait_for_text_blocking(&events, &session, &"A".repeat(120), TIMEOUT).expect("reflowed");
  assert_eq!(session.snapshot().columns, 200);
  assert!(text.contains(&"A".repeat(120)));
}

#[cfg(unix)]
#[test]
fn kill_is_idempotent_and_terminates_child() {
  let (session, events) =
    spawn_with_events(shell_options("sh", &[]), DEFAULT_BOUNDS).expect("failed to spawn session");

  session.kill();
  session.kill();

  // The child is terminated through the closed PTY; the exact status depends
  // on the platform (signal vs code), so only require it to be reported.
  wait_for_exit_blocking(&events, &session, TIMEOUT).expect("exit after kill");
  assert!(!session.is_alive());
}

#[cfg(unix)]
#[test]
fn text_and_lines_helpers_reflect_output() {
  let (session, events) = spawn_with_events(
    shell_options("sh", &["-c", "printf 'first\\nsecond\\n'"]),
    DEFAULT_BOUNDS,
  )
  .expect("failed to spawn session");

  wait_for_text_blocking(&events, &session, "second", TIMEOUT).expect("output");

  let lines = session.last_n_non_empty_lines(10);
  assert!(lines.contains(&"first".to_string()));
  assert!(lines.contains(&"second".to_string()));

  let text = session.text();
  assert!(text.contains("first"));
  assert!(text.contains("second"));
}

#[cfg(unix)]
#[test]
fn paste_respects_line_endings() {
  let (session, events) =
    spawn_with_events(shell_options("cat", &[]), DEFAULT_BOUNDS).expect("failed to spawn session");

  // Plain mode: newlines are converted to carriage returns.
  session.paste("a\nb\r\nc");
  let text = wait_for_text_blocking(&events, &session, "c", TIMEOUT).expect("pasted");
  assert!(text.contains('a') && text.contains('b') && text.contains('c'));

  // Finish the current line, then send EOF (ctrl-d) to terminate `cat`.
  session.input(b"\r".as_slice());
  session.input(b"\x04".as_slice());
  wait_for_exit_blocking(&events, &session, TIMEOUT).expect("exit");
}

#[cfg(unix)]
#[test]
fn selection_captures_text() {
  let (session, events) = spawn_with_events(
    shell_options("sh", &["-c", "echo hello-woocraft"]),
    DEFAULT_BOUNDS,
  )
  .expect("failed to spawn session");
  wait_for_text_blocking(&events, &session, "hello-woocraft", TIMEOUT).expect("output");

  // Select the full first line.
  session.select(
    woocraft_terminal::Point::new(0, 0),
    woocraft_terminal::Point::new(0, 14),
    woocraft_terminal::SelectionKind::Characters,
  );
  let copied = session.copy_selection().expect("selection text");
  assert_eq!(copied.trim_end(), "hello-woocraft");

  session.clear();
  assert!(!session.text().contains("hello-woocraft"));
}

#[cfg(unix)]
#[test]
fn scroll_reaches_history_top() {
  let (session, events) = spawn_with_events(
    shell_options("sh", &["-c", "seq 1 200"]),
    TerminalBounds::new(20.0, 8.0, 80, 10),
  )
  .expect("failed to spawn session");
  wait_for_text_blocking(&events, &session, "200", TIMEOUT).expect("output");

  // The first line scrolled out of the viewport.
  assert!(!session.snapshot().scrolled_to_top);
  session.scroll(woocraft_terminal::ScrollKind::Top);
  let content = session.snapshot();
  assert!(content.scrolled_to_top);
  assert!(content.display_offset > 0);
  assert!(session.text().contains("1"));

  session.scroll(woocraft_terminal::ScrollKind::Bottom);
  assert!(session.snapshot().scrolled_to_bottom);
}
