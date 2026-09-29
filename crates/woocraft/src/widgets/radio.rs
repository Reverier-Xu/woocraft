//! Radio component for single-choice selection with optional label.
//!
//! Radio provides a clickable indicator that represents one mutually
//! exclusive choice. Optionally displays a label next to the indicator.
//! Fully keyboard accessible with Tab navigation and Space/Enter to select.
//! Visually identical to [`Checkbox`] except that the indicator center holds
//! no icon: the selected state is conveyed by the fill color alone. Common
//! in forms and settings where users pick exactly one option.
//!
//! # Features
//! - **Checked State**: Select with click or Space key (cannot deselect)
//! - **Optional Label**: Display text label to the right of the indicator
//! - **Keyboard Accessible**: Full keyboard support (Tab to focus, Space/Enter
//!   to select)
//! - **Disabled State**: Prevent interaction and dim appearance when disabled
//! - **Size Variants**: Small, Medium, Large sizing options
//! - **Click Handler**: Callback fires when user selects
//!
//! # Example
//! ```rust,ignore
//! Radio::new("plan_free")
//!   .label("Free plan")
//!   .checked(false)
//!   .on_click(|_is_selected, _window, _cx| {
//!     // Handle selection
//!   })
//! ```

use std::{rc::Rc, time::Duration};

use gpui::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, MouseButton,
  ParentElement, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{
  ActiveTheme, Easing, Size, StyleSized, StyledExt, Transition, duration, h_flex, transition,
};

type RadioClickHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
/// Radio input for single-choice selection.
///
/// Radio renders a clickable, keyboard-accessible indicator that users
/// interact with to make a choice. Can display an optional label and supports
/// both click and keyboard (`Space`/`Enter`) interaction. Visually identical
/// to [`Checkbox`] minus the center icon. Selection is the caller's
/// responsibility: wire `checked` and `on_click` (or use [`crate::RadioGroup`])
/// to enforce mutual exclusion.
pub struct Radio {
  id: ElementId,
  style: StyleRefinement,
  label: Option<AnyElement>,
  children: Vec<AnyElement>,
  checked: bool,
  disabled: bool,
  size: Size,
  tab_stop: bool,
  tab_index: isize,
  on_click: Option<RadioClickHandler>,
}

impl Radio {
  /// Create a new unselected radio with the given identifier.
  ///
  /// The ID is used for focus management and state tracking. Default state is
  /// unselected.
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

  /// Set the label text or element displayed next to the radio.
  ///
  /// Label appears to the right of the indicator. Clicking the label also
  /// selects the radio (improves hit target on touch devices).
  pub fn label(mut self, label: impl IntoElement) -> Self {
    self.label = Some(label.into_any_element());
    self
  }

  /// Set the selected state (default: unselected/false).
  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  /// Attach a click/select handler.
  ///
  /// Handler receives the new selected state, which is always `true` for a
  /// radio (clicking a radio selects it; it cannot be deselected). Called on
  /// both click and keyboard (Space/Enter) activation by the user.
  pub fn on_click(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }

  /// Control whether the radio can receive focus via Tab key.
  ///
  /// Default: true. Set to false to skip this radio during keyboard
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

impl_disableable!(Radio);
impl_selectable!(Radio, checked);
impl_sizable!(Radio);
impl_styled!(Radio);
impl_parent_element!(Radio);

impl RenderOnce for Radio {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let focus_handle = window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone();

    // Selection transitions: the indicator's fill and border cross-fade to
    // the primary color, interruptible and reduced motion aware. Disabled
    // radios resolve without animation.
    let policy = Transition::new(if self.disabled {
      Duration::ZERO
    } else {
      duration::RADIO_TOGGLE
    })
    .easing(Easing::EaseOut);
    let indicator_color = transition(
      (self.id.clone(), "fill"),
      if checked {
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
      if checked {
        cx.theme().primary
      } else {
        cx.theme().input
      },
      policy,
      window,
      cx,
    );

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
            div()
              .size_full()
              .border(self.size.em(0.125))
              .border_color(cx.theme().background),
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
          this.on_click(move |_event: &ClickEvent, window, cx| on_click(&true, window, cx))
        },
      )
      .refine_style(&self.style)
  }
}
