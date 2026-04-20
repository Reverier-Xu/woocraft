use gpuim::{Action, App, KeyBinding, actions};
use serde::Deserialize;

pub const POPOVER_CONTEXT: &str = "Popover";
pub const DIALOG_CONTEXT: &str = "Dialog";

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

pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("escape", Cancel, Some(POPOVER_CONTEXT)),
    KeyBinding::new("escape", Cancel, Some(DIALOG_CONTEXT)),
  ]);
}
