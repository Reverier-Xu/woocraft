//! Terminal widget: an embedded terminal emulator view backed by a
//! cross-platform PTY session.
//!
//! See `docs/terminal-design.md` for the architecture. The rendering and
//! input layers live here; the session/emulator core lives in the
//! `woocraft-terminal` crate, which can also be used headlessly for external
//! control.

mod colors;
mod element;
mod input;
mod link;
mod mouse;
mod view;

pub use colors::TerminalPalette;
use gpui::{App, KeyBinding};
pub use link::{GridLink, LineContext, LinkProvider, LinkSpan};
pub use view::{TerminalView, TerminalViewEvent, TerminalViewOptions};

// `TerminalCopy`, `TerminalPaste`, and `TerminalSelectAll` are prefixed to
// avoid clashing with the `input` widget's `Copy`, `Paste`, and `SelectAll`
// actions in the crate-root glob re-exports.
gpui::actions!(
  terminal,
  [
    Clear,
    TerminalCopy,
    TerminalPaste,
    TerminalSelectAll,
    ScrollLineUp,
    ScrollLineDown,
    ScrollPageUp,
    ScrollPageDown,
    ScrollToTop,
    ScrollToBottom,
  ]
);

/// The key context for terminal keybindings.
pub const CONTEXT: &str = "TerminalView";

/// Initializes the terminal widget: registers its keybindings.
pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("ctrl-shift-c", TerminalCopy, Some(CONTEXT)),
    KeyBinding::new("ctrl-shift-v", TerminalPaste, Some(CONTEXT)),
    KeyBinding::new("ctrl-shift-k", Clear, Some(CONTEXT)),
    KeyBinding::new("ctrl-shift-a", TerminalSelectAll, Some(CONTEXT)),
    KeyBinding::new("ctrl-shift-home", ScrollToTop, Some(CONTEXT)),
    KeyBinding::new("ctrl-shift-end", ScrollToBottom, Some(CONTEXT)),
    KeyBinding::new("shift-pageup", ScrollPageUp, Some(CONTEXT)),
    KeyBinding::new("shift-pagedown", ScrollPageDown, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-c", TerminalCopy, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-v", TerminalPaste, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-k", Clear, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-a", TerminalSelectAll, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-up", ScrollToTop, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-down", ScrollToBottom, Some(CONTEXT)),
  ]);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn context_is_stable() {
    assert_eq!(CONTEXT, "TerminalView");
  }
}
