//! switch control for boolean on/off selection.
//!
//! this is a styled wrapper over [`gpui_base::Switch`], which owns every
//! behavioral concern — toggle, focus, keyboard activation, and
//! accessibility. the wrapper adds the design-system vocabulary: a pill
//! track with a sliding thumb animated over the theme switch-toggle
//! duration through the base motion system (which honors the system
//! reduce-motion preference), an optional text label, and a semantic
//! accent color.

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, FocusHandle, Hsla, InteractiveElement as _,
  IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, rems,
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

  /// resolves the focus handle the base control tracks, keyed by the
  /// shared element identity.
  fn focus_handle(&self, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone()
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
    let focused = !disabled && self.focus_handle(window, cx).is_focused(window);

    // geometry is rem-driven: a 2.5rem × 1.25rem pill, a 1rem thumb, and a
    // 0.125rem inset on both rest positions. the thumb travels through the
    // base motion transition, which retargets mid-flight and short-circuits
    // under the system reduce-motion preference.
    let rem = window.rem_size();
    let track_w = rems(2.5).to_pixels(rem);
    let track_h = rems(1.25).to_pixels(rem);
    let thumb = rems(1.).to_pixels(rem);
    let inset = rems(0.125).to_pixels(rem);
    let travel = track_w - thumb - inset * 2.;
    let rest = inset;
    let thumb_x = transition(
      self.id.clone(),
      if checked { rest + travel } else { rest },
      Transition::new(duration::SWITCH_TOGGLE),
      window,
      cx,
    );

    let theme = cx.theme();

    let accent = self.color.unwrap_or(theme.primary);
    let track_bg = if disabled {
      with_alpha(accent, opacity::DISABLED)
    } else {
      accent
    };
    let track_border = if disabled {
      theme.border
    } else if focused {
      theme.ring
    } else if checked {
      track_bg
    } else {
      theme.border
    };

    let (foreground, muted_foreground, border_width) =
      (theme.foreground, theme.muted_foreground, theme.border_width);
    let mut base = self.base;

    base = base
      .flex()
      .items_center()
      .gap(rems(0.5))
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(if disabled {
        muted_foreground
      } else {
        foreground
      })
      .when(!disabled, |this| {
        this
          .cursor_pointer()
          .hover(|s| s.opacity(opacity::solid::HOVER))
          .active(|s| s.opacity(opacity::solid::ACTIVE))
      })
      .when(disabled, |this| {
        this.cursor(CursorStyle::OperationNotAllowed)
      })
      .child(
        div()
          .flex_none()
          .relative()
          .w(track_w)
          .h(track_h)
          .rounded_full()
          .border(border_width)
          .border_color(track_border)
          .bg(if checked { track_bg } else { theme.muted })
          .child(
            div()
              .absolute()
              .top((track_h - thumb) / 2.)
              .left(thumb_x)
              .size(thumb)
              .rounded_full()
              .border(border_width)
              .border_color(if checked {
                gpui::transparent_black()
              } else {
                theme.border
              })
              .bg(theme.background),
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
