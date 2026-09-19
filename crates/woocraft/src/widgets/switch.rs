//! switch control for boolean on/off selection.
//!
//! this is a styled wrapper over [`gpui_base::Switch`], which owns every
//! behavioral concern — toggle, focus, keyboard activation, and
//! accessibility. the wrapper adds the design-system vocabulary: a `2rem`
//! block whose centered `0.25rem` line carries the state color (accent
//! when checked, muted otherwise) and a square knob that slides between
//! rest positions and grows under hover — both animated through the base
//! motion system, which honors the system reduce-motion preference.

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, Hsla, InteractiveElement as _, IntoElement,
  ParentElement, Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, rems,
};
use gpui_base::{
  Disableable, Switch as BaseSwitch,
  motion::{Transition, transition},
};

use crate::{
  ActiveTheme,
  theme::{duration, opacity, with_alpha},
};

/// interactive switch element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Switch;
///
/// Switch::new("dark-mode")
///   .label("dark mode")
///   .checked(true)
///   .on_change(|checked, _event, _window, _cx| {
///     println!("switched on: {checked}");
///   });
/// ```
#[derive(IntoElement)]
pub struct Switch {
  id: ElementId,
  base: BaseSwitch,
  checked: bool,
  disabled: bool,
  color: Option<Hsla>,
  label: Option<SharedString>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Switch {
  /// creates an unchecked switch with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseSwitch::new(id.clone()),
      id,
      checked: false,
      disabled: false,
      color: None,
      label: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the application-controlled checked value.
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

  /// sets the accent color of the checked track. defaults to the theme
  /// primary.
  pub fn color(mut self, color: Hsla) -> Self {
    self.color = Some(color);
    self
  }

  /// sets the visible text label rendered to the right of the track.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the name exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }

  /// handles activation with the next checked value and its input event.
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

  /// sets whether the switch participates in keyboard focus traversal.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.base = self.base.tab_stop(tab_stop);
    self
  }
}

impl Disableable for Switch {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }
}

impl Styled for Switch {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Switch {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Switch {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let disabled = self.disabled;

    // hover is tracked through a keyed slot so the knob morph animates
    // through the same motion transition as the slide; a hover style hook
    // could only snap.
    let hovered_slot = window.use_keyed_state((self.id.clone(), "hover"), cx, |_, _| false);
    let hovered = !disabled && *hovered_slot.read(cx);

    // geometry is rem-driven: a 2rem-wide, 1rem-tall block; the line runs
    // centered, 0.25rem thick; the knob spans the full block height as a
    // 0.25 × 1rem capsule and grows to a 0.5 × 1rem square under hover.
    let rem = window.rem_size();
    let block_w = rems(2.).to_pixels(rem);
    let block_h = rems(1.).to_pixels(rem);
    let line_h = rems(0.25).to_pixels(rem);
    let knob_h = rems(1.).to_pixels(rem);
    let inset = rems(0.25).to_pixels(rem);
    let rest_w = rems(0.25).to_pixels(rem);
    let hover_w = rems(0.5).to_pixels(rem);

    let knob_w = transition(
      (self.id.clone(), "knob-w"),
      if hovered { hover_w } else { rest_w },
      Transition::new(duration::CONTROL),
      window,
      cx,
    );
    let knob_center = transition(
      (self.id.clone(), "knob-x"),
      if checked {
        block_w - inset - rest_w / 2.
      } else {
        inset + rest_w / 2.
      },
      Transition::new(duration::SWITCH_TOGGLE),
      window,
      cx,
    );

    let theme = cx.theme();
    let accent = self.color.unwrap_or(theme.primary);
    let (muted, foreground, muted_foreground, radius, font_size) = (
      theme.muted,
      theme.foreground,
      theme.muted_foreground,
      theme.radius,
      theme.font_size,
    );
    let font_family = theme.font_family.clone();

    let line_color = if disabled {
      if checked {
        with_alpha(accent, opacity::DISABLED)
      } else {
        muted
      }
    } else if checked {
      accent
    } else {
      muted
    };
    let knob_color = if disabled {
      with_alpha(foreground, opacity::DISABLED)
    } else {
      foreground
    };

    let mut track = div()
      .id((self.id.clone(), "track"))
      .relative()
      .flex_none()
      .w(block_w)
      .h(block_h)
      .child(
        div()
          .absolute()
          .top((block_h - line_h) / 2.)
          .left_0()
          .w(block_w)
          .h(line_h)
          .rounded_full()
          .bg(line_color),
      )
      .child(
        div()
          .absolute()
          .top((block_h - knob_h) / 2.)
          .left(knob_center - knob_w / 2.)
          .w(knob_w)
          .h(knob_h)
          .rounded(radius)
          .bg(knob_color),
      );
    if !disabled {
      let hovered_slot = hovered_slot.clone();
      track = track.on_hover(move |entered: &bool, _, cx| {
        hovered_slot.update(cx, |slot, _| *slot = *entered);
      });
    }

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
      .child(track)
      .when_some(self.label, |this, label| this.child(div().child(label)))
      .children(self.children);

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base
  }
}

#[cfg(test)]
mod tests {
  use gpui::hsla;

  use super::Switch;

  #[test]
  fn default_switch_is_unchecked_and_enabled() {
    let switch = Switch::new("test");
    assert!(!switch.checked);
    assert!(!switch.disabled);
  }

  #[test]
  fn checked_setter_stores_the_boolean_value() {
    assert!(Switch::new("test").checked(true).checked);
    assert!(!Switch::new("test").checked(true).checked(false).checked);
  }

  #[test]
  fn color_defaults_to_none_until_set() {
    let switch = Switch::new("test");
    assert!(switch.color.is_none());

    let accent = hsla(0.4, 0.7, 0.5, 1.0);
    assert_eq!(Switch::new("test").color(accent).color, Some(accent));
  }

  #[test]
  fn disabled_flag_is_stored() {
    assert!(Switch::new("test").disabled(true).disabled);
  }
}
