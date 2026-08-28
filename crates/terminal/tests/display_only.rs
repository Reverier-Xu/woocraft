//! Integration tests for display-only sessions.
//!
//! Display-only sessions have no child process; output is fed directly into
//! the emulator, which makes escape-sequence handling assertable without a PTY.

use std::time::Duration;

use woocraft_terminal::{
  CellColor, CellFlags, NamedColor, SpawnOptions, TerminalBounds, TerminalEvent, TerminalSession,
};

fn display_session() -> TerminalSession {
  TerminalSession::spawn_display_only(
    SpawnOptions::default(),
    TerminalBounds::new(20.0, 8.0, 80, 24),
  )
}

fn wait_for_event(
  session: &TerminalSession, predicate: impl Fn(&TerminalEvent) -> bool,
) -> TerminalEvent {
  let events = session.events();
  let deadline = std::time::Instant::now() + Duration::from_secs(5);
  loop {
    match events.try_recv() {
      Ok(event) if predicate(&event) => return event,
      Ok(_) => {}
      Err(async_channel::TryRecvError::Closed) => panic!("event stream closed"),
      Err(async_channel::TryRecvError::Empty) => {}
    }
    if std::time::Instant::now() >= deadline {
      panic!("timed out waiting for event");
    }
    std::thread::sleep(Duration::from_millis(5));
  }
}

#[test]
fn sgr_attributes_are_parsed() {
  let session = display_session();
  session.feed_display(b"\x1b[1;31mR\x1b[0mN\x1b[38;5;196mC");

  let content = session.snapshot();
  assert_eq!(content.cells.len(), 80 * 24);

  let r = content.cells.iter().find(|c| c.cell.c == 'R').unwrap();
  assert!(r.cell.flags.contains(CellFlags::BOLD));
  assert_eq!(r.cell.fg, CellColor::Named(NamedColor::Red));
  assert_eq!(r.point, woocraft_terminal::Point::new(0, 0));

  let n = content.cells.iter().find(|c| c.cell.c == 'N').unwrap();
  assert_eq!(n.cell.fg, CellColor::Named(NamedColor::Foreground));
  assert_eq!(n.cell.flags, CellFlags::empty());
  assert_eq!(n.point, woocraft_terminal::Point::new(0, 1));

  let c = content.cells.iter().find(|c| c.cell.c == 'C').unwrap();
  assert_eq!(c.cell.fg, CellColor::Indexed(196));
  assert_eq!(c.point, woocraft_terminal::Point::new(0, 2));
}

#[test]
fn cursor_positioning_and_shape() {
  let session = display_session();
  session.feed_display(b"\x1b[3;5HX");

  let content = session.snapshot();
  let x = content.cells.iter().find(|c| c.cell.c == 'X').unwrap();
  assert_eq!(x.point, woocraft_terminal::Point::new(2, 4));
  assert_eq!(content.cursor.point, woocraft_terminal::Point::new(2, 5));
  assert_eq!(content.cursor.shape, woocraft_terminal::CursorShape::Block);
  assert_eq!(content.cursor_char, ' ');
}

#[test]
fn title_and_bell_events() {
  let session = display_session();

  session.feed_display(b"\x1b]2;my-title\x1b\\");
  let event = wait_for_event(
    &session,
    |event| matches!(event, TerminalEvent::Title(title) if title == "my-title"),
  );
  assert!(matches!(event, TerminalEvent::Title(_)));

  session.feed_display(b"\x07");
  wait_for_event(&session, |event| matches!(event, TerminalEvent::Bell));

  session.feed_display(b"\x1b]0;\x1b\\");
  // An empty OSC 0 title yields Title("") rather than a reset, matching the
  // emulator's native semantics.
  let event = wait_for_event(&session, |event| {
    matches!(event, TerminalEvent::ResetTitle)
      || matches!(event, TerminalEvent::Title(title) if title.is_empty())
  });
  assert!(matches!(
    event,
    TerminalEvent::ResetTitle | TerminalEvent::Title(_)
  ));
}

#[test]
fn modes_reflect_private_sequences() {
  let session = display_session();
  session.feed_display(b"\x1b[?1049h\x1b[?2004h");

  let content = session.snapshot();
  assert!(content.mode.contains(woocraft_terminal::Modes::ALT_SCREEN));
  assert!(
    content
      .mode
      .contains(woocraft_terminal::Modes::BRACKETED_PASTE)
  );

  session.feed_display(b"\x1b[?1049l\x1b[?2004l");
  let content = session.snapshot();
  assert!(!content.mode.contains(woocraft_terminal::Modes::ALT_SCREEN));
  assert!(
    !content
      .mode
      .contains(woocraft_terminal::Modes::BRACKETED_PASTE)
  );
}

#[test]
fn no_child_process() {
  let session = display_session();
  assert_eq!(session.pid(), None);
  assert!(!session.is_alive());
  assert_eq!(session.child_exit_status(), None);

  // Killing a display-only session is a no-op that must not panic.
  session.kill();
  session.kill();
}

#[test]
fn wakeup_events_are_emitted_for_feeds() {
  let session = display_session();
  session.feed_display(b"hello");
  wait_for_event(&session, |event| matches!(event, TerminalEvent::Wakeup));
}
