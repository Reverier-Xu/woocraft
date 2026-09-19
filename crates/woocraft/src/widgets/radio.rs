//! radio control for exclusive selection within a group.
//!
//! this is a styled wrapper over [`gpui_base::Radio`], which owns every
//! behavioral concern — activation, focus, keyboard support, and
//! accessibility. the wrapper adds the design-system vocabulary: a `1rem`
//! circular indicator whose primary dot scales in and out over the control
//! duration, an optional text label, and theme-driven state colors. pair it
//! with [`RadioGroup`](crate::RadioGroup) and report positions through
//! [`Radio::set_position`] so assistive technology can announce
//! "option 2 of 5".

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, FocusHandle, InteractiveElement as _,
  IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString, StyleRefinement, Styled,
  Window, div, prelude::FluentBuilder as _, px, rems,
};
use gpui_base::{
  Radio as BaseRadio,
  motion::{Transition, transition},
};

use crate::{
  ActiveTheme,
  theme::{duration, opacity, with_alpha},
};

/// interactive radio element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Radio;
///
/// Radio::new("plan-pro")
///   .label("pro plan")
///   .checked(self.plan == Plan::Pro)
///   .set_position(2, 3)
///   .on_change(|checked, _event, _window, _cx| {
///     println!("selected: {checked}");
///   });
/// ```
#[derive(IntoElement)]
pub struct Radio {
  id: ElementId,
  base: BaseRadio,
  checked: bool,
  disabled: bool,
  label: Option<SharedString>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Radio {
  /// creates an unchecked radio with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseRadio::new(id.clone()),
      id,
      checked: false,
      disabled: false,
      label: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// updates the element identity used when the radio is rendered.
  ///
  /// a group that assigns positional ids after construction needs this so
  /// each radio keeps a distinct element identity.
  pub fn id(mut self, id: impl Into<ElementId>) -> Self {
    self.id = id.into();
    self.base = self.base.id(self.id.clone());
    self
  }

  /// sets the application-controlled checked state.
  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self.base = self.base.checked(checked);
    self
  }

  /// sets whether pointer and keyboard activation are ignored.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }

  /// sets the visible text label rendered to the right of the circle.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the label exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }

  /// uses a caller-owned focus handle instead of creating keyed state.
  pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
    self.base = self.base.track_focus(focus_handle);
    self
  }

  /// sets this radio's one-based position and its group's total size.
  pub fn set_position(mut self, position: usize, size: usize) -> Self {
    self.base = self.base.set_position(position, size);
    self
  }

  /// handles a requested selection change. activating an already checked
  /// radio is a no-op because a radio cannot deselect itself.
  pub fn on_change(
    mut self, handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_change(handler);
    self
  }

  /// sets the focus traversal index. the default is `0`.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.base = self.base.tab_index(tab_index);
    self
  }

  /// sets whether the radio participates in keyboard focus traversal.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.base = self.base.tab_stop(tab_stop);
    self
  }
}

impl Styled for Radio {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Radio {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Radio {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let disabled = self.disabled;
    let theme = cx.theme();
    let (accent, border, foreground, muted_foreground) = (
      if disabled {
        with_alpha(theme.primary, opacity::DISABLED)
      } else {
        theme.primary
      },
      theme.border,
      theme.foreground,
      theme.muted_foreground,
    );
    let border_width = theme.border_width;
    let font_size = theme.font_size;
    let font_family = theme.font_family.clone();

    // the dot scales in and out over the control duration. colors snap
    // instead of easing: hue interpolation across near-neutral theme colors
    // reads as a flash.
    let indicator_border = if checked || disabled { accent } else { border };
    let dot_size = transition(
      (self.id.clone(), "dot"),
      if checked {
        rems(0.5).to_pixels(window.rem_size())
      } else {
        px(0.)
      },
      Transition::new(duration::CONTROL),
      window,
      cx,
    );

    let mut base = self.base;

    base = base
      .flex()
      .items_center()
      .gap(rems(0.5))
      .text_size(font_size)
      .font_family(font_family)
      .text_color(if disabled {
        muted_foreground
      } else {
        foreground
      })
      .when(!disabled, |this| this.cursor_pointer())
      .when(disabled, |this| {
        this.cursor(CursorStyle::OperationNotAllowed)
      })
      .child(
        // fixed-size slot keeps the row layout stable while the ring grows
        // under hover.
        div()
          .flex_none()
          .size(rems(1.))
          .flex()
          .items_center()
          .justify_center()
          .child(
            div()
              .id((self.id.clone(), "ring"))
              .flex()
              .items_center()
              .justify_center()
              .size(rems(1.))
              .rounded_full()
              .border(border_width)
              .border_color(indicator_border)
              .when(!disabled, |this| this.hover(|s| s.size(rems(1.1))))
              .child(div().size(dot_size).rounded_full().bg(accent)),
          ),
      )
      .when_some(self.label, |this, label| this.child(div().child(label)))
      .children(self.children);

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base
  }
}

#[cfg(test)]
mod tests {
  use super::Radio;

  #[test]
  fn default_radio_is_unchecked_and_enabled() {
    let radio = Radio::new("test");
    assert!(!radio.checked);
    assert!(!radio.disabled);
  }

  #[test]
  fn checked_setter_stores_the_boolean_value() {
    assert!(Radio::new("test").checked(true).checked);
    assert!(!Radio::new("test").checked(true).checked(false).checked);
  }

  #[test]
  fn id_setter_replaces_the_element_identity() {
    let radio = Radio::new("placeholder").id("renamed");
    assert_eq!(radio.id.to_string(), "renamed");
  }

  #[test]
  fn disabled_flag_is_stored() {
    assert!(Radio::new("test").disabled(true).disabled);
  }
}
