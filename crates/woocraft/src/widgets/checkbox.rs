//! Checkbox component for boolean selection with optional label.
//!
//! Checkbox provides a clickable box that toggles between checked and unchecked
//! states. Optionally displays a label next to the checkbox. Fully keyboard
//! accessible with Tab navigation and Space/Enter to toggle. Common in forms,
//! settings, and multi-select lists where users need to opt in or out of
//! multiple independent options.
//!
//! # Features
//! - **Checked State**: Toggle between true/false with click or Space key
//! - **Optional Label**: Display text label to the right of checkbox
//! - **Keyboard Accessible**: Full keyboard support (Tab to focus, Space/Enter
//!   to toggle)
//! - **Disabled State**: Prevent interaction and dim appearance when disabled
//! - **Size Variants**: Small, Medium, Large sizing options
//! - **Click Handler**: Callback fires when user toggles state
//!
//! # Example
//! ```rust,ignore
//! Checkbox::new("agree_terms")
//!   .label("I agree to the terms")
//!   .checked(false)
//!   .on_click(|_is_checked, _window, _cx| {
//!     // Handle toggle
//!   })
//! ```

use std::rc::Rc;

use gpuim::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, MouseButton,
  ParentElement, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, Icon, IconName, Sizable, Size, StyleSized, StyledExt, h_flex};

type CheckboxClickHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
/// Checkbox input for boolean/toggled selection.
///
/// Checkbox renders a clickable, keyboard-accessible box that users interact
/// with to toggle a boolean state. Can display an optional label and supports
/// both click and keyboard (`Space`/`Enter`) interaction. Useful for yes/no
/// questions, feature toggles, and multi-select scenarios.
pub struct Checkbox {
  id: ElementId,
  style: StyleRefinement,
  label: Option<AnyElement>,
  children: Vec<AnyElement>,
  checked: bool,
  disabled: bool,
  size: Size,
  tab_stop: bool,
  tab_index: isize,
  on_click: Option<CheckboxClickHandler>,
}

impl Checkbox {
  /// Create a new unchecked checkbox with the given identifier.
  ///
  /// The ID is used for focus management and state tracking. Default state is
  /// unchecked.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      label: None,
      children: Vec::new(),
      checked: false,
      disabled: false,
      size: Size::default(),
      tab_stop: true,
      tab_index: 0,
      on_click: None,
    }
  }

  /// Set the label text or element displayed next to the checkbox.
  ///
  /// Label appears to the right of the checkbox box. Clicking the label also
  /// toggles the checkbox (improves hit target on touch devices).
  pub fn label(mut self, label: impl IntoElement) -> Self {
    self.label = Some(label.into_any_element());
    self
  }

  /// Set the initial checked state (default: unchecked/false).
  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  /// Attach a click/toggle handler.
  ///
  /// Handler receives the new checked state (after toggle). Called on both
  /// click and keyboard (Space/Enter) activation by the user.
  pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }

  /// Control whether the checkbox can receive focus via Tab key.
  ///
  /// Default: true. Set to false to skip this checkbox during keyboard
  /// navigation.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.tab_stop = tab_stop;
    self
  }

  /// Set the tab index for keyboard navigation order.
  ///
  /// Higher indices are focused after lower ones. Default: 0.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.tab_index = tab_index;
    self
  }
}

impl_disableable!(Checkbox);
impl_selectable!(Checkbox, checked);
impl_sizable!(Checkbox);
impl_styled!(Checkbox);
impl_parent_element!(Checkbox);

impl RenderOnce for Checkbox {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let focus_handle = window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone();
    let indicator_color = if checked {
      cx.theme().primary
    } else {
      cx.theme().background
    };
    let border_color = if checked {
      cx.theme().primary
    } else {
      cx.theme().input
    };

    h_flex()
      .id(self.id)
      .items_center()
      .component_gap(self.size)
      .text_color(if self.disabled {
        cx.theme().muted_foreground
      } else {
        cx.theme().foreground
      })
      .child(
        div()
          .flex_none()
          .size(self.size.component_height() * 0.5)
          .rounded(self.size.component_radius())
          .border_1()
          .border_color(border_color)
          .bg(indicator_color)
          .child(
            h_flex().size_full().items_center().justify_center().child(
              Icon::new(IconName::Checkmark)
                .with_size(self.size.smaller())
                .text_color(cx.theme().primary_foreground)
                .when(!checked, |this| this.opacity(0.0)),
            ),
          ),
      )
      .when_some(self.label, |this, label| this.child(label))
      .children(self.children)
      .when(!self.disabled, |this| {
        this.track_focus(
          &focus_handle
            .tab_stop(self.tab_stop)
            .tab_index(self.tab_index),
        )
      })
      .when(!self.disabled, |this| {
        this
          .cursor_pointer()
          .hover(|this| this.opacity(0.9))
          .active(|this| this.opacity(0.8))
      })
      .on_mouse_down(MouseButton::Left, |_, window, _| {
        window.prevent_default();
      })
      .when_some(
        self.on_click.filter(|_| !self.disabled),
        |this, on_click| {
          this.on_click(move |_event: &ClickEvent, window, cx| on_click(&!checked, window, cx))
        },
      )
      .refine_style(&self.style)
  }
}
