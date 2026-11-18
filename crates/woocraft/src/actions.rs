use gpui::{Action, App, KeyBinding, actions};
use serde::Deserialize;

pub const POPOVER_CONTEXT: &str = "Popover";
pub const DIALOG_CONTEXT: &str = "Dialog";
/// Key context of the markdown reader, so its copy/select-all bindings only
/// apply while a [`crate::Markdown`] has focus.
pub const MARKDOWN_CONTEXT: &str = "Markdown";

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = woocraft, no_json)]
pub struct Confirm {
  pub secondary: bool,
}

actions!(
  woocraft,
  [
    Cancel,
    SelectUp,
    SelectDown,
    SelectLeft,
    SelectRight,
    SelectFirst,
    SelectLast,
    SelectPrevColumn,
    SelectNextColumn,
    SelectPageUp,
    SelectPageDown
  ]
);

actions!(
  woocraft,
  [
    /// Copies the current markdown text selection.
    MarkdownCopy,
    /// Selects the whole markdown document.
    MarkdownSelectAll
  ]
);

pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("escape", Cancel, Some(POPOVER_CONTEXT)),
    KeyBinding::new("escape", Cancel, Some(DIALOG_CONTEXT)),
    KeyBinding::new("cmd-c", MarkdownCopy, Some(MARKDOWN_CONTEXT)),
    KeyBinding::new("ctrl-c", MarkdownCopy, Some(MARKDOWN_CONTEXT)),
    KeyBinding::new("cmd-a", MarkdownSelectAll, Some(MARKDOWN_CONTEXT)),
    KeyBinding::new("ctrl-a", MarkdownSelectAll, Some(MARKDOWN_CONTEXT)),
  ]);
}
