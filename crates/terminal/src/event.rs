//! Events delivered from a terminal session to its host.
//!
//! Rendering-related wakeups and lifecycle events flow through the same
//! channel so external controllers can drive sessions headlessly, while
//! clipboard/color requests are forwarded to the host with formatters that
//! produce the exact PTY response bytes.

use std::sync::Arc;

use vte::ansi::Rgb;

/// Formats the host clipboard content into the PTY response bytes for an
/// OSC 52 clipboard load request.
pub type ClipboardFormatter = Arc<dyn Fn(&str) -> Vec<u8> + Send + Sync + 'static>;

/// Formats a palette color into the PTY response bytes for a color request.
pub type ColorFormatter = Arc<dyn Fn(Rgb) -> Vec<u8> + Send + Sync + 'static>;

/// The exit status of a terminal child process.
///
/// On unix, `signal` is set when the child was terminated by a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChildStatus {
  pub code: Option<i32>,
  #[cfg(unix)]
  pub signal: Option<i32>,
}

impl ChildStatus {
  /// A single exit-code view: the raw code, or `-1` when unavailable.
  pub fn code(&self) -> i32 {
    self.code.unwrap_or(-1)
  }
}

#[cfg(unix)]
impl From<&std::process::ExitStatus> for ChildStatus {
  fn from(status: &std::process::ExitStatus) -> Self {
    use std::os::unix::process::ExitStatusExt as _;
    Self {
      code: status.code(),
      signal: status.signal(),
    }
  }
}

#[cfg(windows)]
impl From<&std::process::ExitStatus> for ChildStatus {
  fn from(status: &std::process::ExitStatus) -> Self {
    Self {
      code: status.code(),
    }
  }
}

/// An event emitted by a terminal session.
#[derive(Clone)]
pub enum TerminalEvent {
  /// New terminal content is available; the host should re-snapshot and redraw.
  Wakeup,
  /// The application set a new title (OSC 0/2).
  Title(String),
  /// The application reset the title.
  ResetTitle,
  /// The terminal bell was rung.
  Bell,
  /// The application stored text into the clipboard (OSC 52).
  ClipboardStore(String),
  /// The application requested clipboard contents (OSC 52).
  ///
  /// The host should read its clipboard and write the formatted response back
  /// via [`crate::TerminalSession::write_pty`]. Respond as soon as possible to
  /// preserve the ordering of PTY responses.
  ClipboardLoad(ClipboardFormatter),
  /// The application requested the RGB value of a palette entry.
  ///
  /// The host should resolve the color (usually from its theme) and write the
  /// formatted response back via [`crate::TerminalSession::write_pty`].
  ColorRequest {
    index: usize,
    formatter: ColorFormatter,
  },
  /// The cursor blinking state changed. The host should query
  /// [`crate::TerminalSession::cursor_blinking`] to read the new state; the
  /// query must happen on the host side because the session's emulator lock
  /// may be held by the PTY event loop when this event is emitted.
  CursorBlinkingChanged,
  /// The child process exited.
  ChildExit(ChildStatus),
  /// The session is shutting down; no further events will be emitted.
  Exit,
}

impl std::fmt::Debug for TerminalEvent {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      TerminalEvent::Wakeup => f.write_str("Wakeup"),
      TerminalEvent::Title(title) => write!(f, "Title({title:?})"),
      TerminalEvent::ResetTitle => f.write_str("ResetTitle"),
      TerminalEvent::Bell => f.write_str("Bell"),
      TerminalEvent::ClipboardStore(data) => write!(f, "ClipboardStore({data:?})"),
      TerminalEvent::ClipboardLoad(_) => f.write_str("ClipboardLoad"),
      TerminalEvent::ColorRequest { index, .. } => write!(f, "ColorRequest({index})"),
      TerminalEvent::CursorBlinkingChanged => f.write_str("CursorBlinkingChanged"),
      TerminalEvent::ChildExit(status) => write!(f, "ChildExit({status:?})"),
      TerminalEvent::Exit => f.write_str("Exit"),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn child_status_conversion() {
    #[cfg(unix)]
    let status = ChildStatus {
      code: Some(0),
      signal: None,
    };
    #[cfg(windows)]
    let status = ChildStatus { code: Some(0) };
    assert_eq!(status.code(), 0);
  }

  #[test]
  fn debug_formatting() {
    assert_eq!(format!("{:?}", TerminalEvent::Wakeup), "Wakeup");
    assert_eq!(format!("{:?}", TerminalEvent::Bell), "Bell");
    assert_eq!(
      format!("{:?}", TerminalEvent::Title("hi".to_string())),
      "Title(\"hi\")"
    );
  }
}
