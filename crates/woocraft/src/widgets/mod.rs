//! styled widgets built on the gpui-base behavioral foundations.

use gpui::{App, ElementId, FocusHandle, Window};

/// resolves the focus handle the base control actually tracks: the
/// caller-provided handle when `track_focus` set one, otherwise the keyed
/// handle the base primitive creates under the control's element id.
/// reading the same keyed slot is what lets the styled layer draw focus
/// indication that follows the real focus target.
pub(crate) fn base_focus_handle(
  id: &ElementId, provided: Option<&FocusHandle>, window: &mut Window, cx: &mut App,
) -> FocusHandle {
  provided.cloned().unwrap_or_else(|| {
    window
      .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone()
  })
}

pub mod alert_dialog;
pub mod avatar;
pub mod badge;
pub mod button;
pub mod checkbox;
pub mod collapsible;
pub mod dialog;
pub mod dialog_stack;
pub mod divider;
pub mod icon_label;
pub mod kbd;
pub mod label;
pub mod link;
pub mod popover;
pub mod progress;
pub mod radio;
pub mod radio_group;
pub mod spinner;
pub mod switch;
pub mod tag;
pub mod title_bar;
pub mod toast;
pub mod toggle;
pub mod toggle_group;
pub mod tooltip;
pub mod window_border;
