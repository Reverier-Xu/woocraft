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
//! - **Indeterminate State**: Visual "partially checked" state for mixed
//!   selections (e.g. a parent checkbox in a tree)
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

use std::{rc::Rc, time::Duration};

use gpui::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, MouseButton,
  ParentElement, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{
  ActiveTheme, Easing, Icon, IconName, Sizable, Size, StyleSized, StyledExt, Transition, duration,
  h_flex, transition,
};

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
  indeterminate: bool,
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
      indeterminate: false,
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

  /// Set the indeterminate ("partially checked") state.
  ///
  /// When true, the indicator shows a dash instead of a checkmark and uses
  /// the checked color scheme. Clicking still toggles `checked`; the caller
  /// is responsible for deriving `indeterminate` from its data (e.g. some,
  /// but not all, children of a tree node are checked).
  pub fn indeterminate(mut self, indeterminate: bool) -> Self {
    self.indeterminate = indeterminate;
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
    let indeterminate = self.indeterminate;
    let marked = checked || indeterminate;
    let focus_handle = window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone();

    // Check transitions: the checkmark fades in and the indicator's fill and
    // border cross-fade to the primary color, all interruptible and reduced
    // motion aware. Disabled checkboxes resolve without animation.
    let policy = Transition::new(if self.disabled {
      Duration::ZERO
    } else {
      duration::CHECKBOX_TOGGLE
    })
    .easing(Easing::EaseOut);
    let check_opacity = transition(
      (self.id.clone(), "check"),
      if marked { 1.0 } else { 0.0 },
      policy.clone(),
      window,
      cx,
    );
    let indicator_color = transition(
      (self.id.clone(), "fill"),
      if marked {
        cx.theme().primary
      } else {
        cx.theme().background
      },
      policy.clone(),
      window,
      cx,
    );
    let border_color = transition(
      (self.id.clone(), "border"),
      if marked {
        cx.theme().primary
      } else {
        cx.theme().input
      },
      policy,
      window,
      cx,
    );
    let mark_icon = if indeterminate {
      IconName::SubtractFilled
    } else {
      IconName::CheckmarkFilled
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
          .border(self.size.em(0.125))
          .border_color(border_color)
          .bg(indicator_color)
          .child(
            h_flex()
              .size_full()
              .items_center()
              .justify_center()
              .border(self.size.em(0.125))
              .border_color(cx.theme().background)
              .child(
                Icon::new(mark_icon)
                  .with_size(self.size.smaller())
                  .text_color(cx.theme().background)
                  .opacity(check_opacity),
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
