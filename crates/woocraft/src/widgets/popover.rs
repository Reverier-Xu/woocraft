//! popover panel anchored to a trigger, styled by the woocraft design system.
//!
//! this is a styled wrapper over [`gpui_base::Popover`], which owns the full
//! behavior surface: pointer or keyboard activation, controlled and
//! uncontrolled open state, outside-click and Escape dismissal, focus capture
//! and restoration, and the deferred-paint registration that keeps dialogs and
//! overlays aware of the open popover. positioning rides on the base popup
//! host: trigger measurement, anchor corners, first-frame sync, and
//! window-edge snapping.
//!
//! the wrapper contributes the themed surface — popover colors, large radius,
//! the theme hairline border, one elevation shadow, base typography — and a
//! short fade-in sampled from the persistent wrapper node. the surface
//! unmounts with the base popover, so only the entrance animates; exits are
//! immediate. motion honors the system reduce-motion preference through the
//! base motion system.
//!
//! ```rust,ignore
//! use gpui::Anchor;
//! use woocraft::{Button, Popover, v_flex};
//!
//! Popover::new("export")
//!     .trigger(Button::new("export-trigger").label("Export…"))
//!     .anchor(Anchor::BottomRight)
//!     .content(|state, _, cx| {
//!         // the state handle lets controls inside dismiss the panel.
//!         let popover = cx.entity();
//!         v_flex().gap_2()
//!             .child(Label::new("pick a format"))
//!             .child(Button::new("close").on_click(move |_, window, cx| {
//!                 popover.update(cx, |state, cx| state.dismiss(window, cx))
//!             }))
//!     });
//! ```

use std::rc::Rc;

use gpui::{
  Anchor, AnyElement, App, Context, ElementId, IntoElement, MouseButton, ParentElement, RenderOnce,
  StyleRefinement, Styled, Window, black, div, px, rems,
};
use gpui_base::{
  Popover as BasePopover, PopoverState, Selectable, StyledExt as _, box_shadow,
  motion::{Transition, transition},
};

use crate::theme::{ActiveTheme, duration, with_alpha};

type OpenChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type ContentBuilder =
  Box<dyn FnOnce(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> AnyElement>;

/// popover element styled by the woocraft design system.
///
/// open semantics follow the base popover: `open` pins the state (controlled),
/// `default_open` seeds it (uncontrolled), and every uncontrolled transition
/// announces through `on_open_change`.
#[derive(IntoElement)]
pub struct Popover {
  id: ElementId,
  base: BasePopover,
  default_open: bool,
  open: Option<bool>,
  content: Option<ContentBuilder>,
  on_open_change: Option<OpenChangeHandler>,
  style: StyleRefinement,
}

impl Popover {
  /// creates a closed popover with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BasePopover::new(id.clone()),
      id,
      default_open: false,
      open: None,
      content: None,
      on_open_change: None,
      style: StyleRefinement::default(),
    }
  }

  /// sets the anchor corner of the surface relative to the trigger.
  pub fn anchor(mut self, anchor: impl Into<Anchor>) -> Self {
    self.base = self.base.anchor(anchor);
    self
  }

  /// sets the pointer button that toggles the popover; left by default.
  pub fn mouse_button(mut self, mouse_button: MouseButton) -> Self {
    self.base = self.base.mouse_button(mouse_button);
    self
  }

  /// sets the trigger element; its selected state tracks the popover.
  pub fn trigger<T>(mut self, trigger: T) -> Self
  where
    T: Selectable + IntoElement + 'static, {
    self.base = self.base.trigger(trigger);
    self
  }

  /// supplies a raw trigger builder for higher-level presentation facades.
  #[doc(hidden)]
  pub fn trigger_with(
    mut self, trigger: impl FnOnce(bool, &Window, &App) -> AnyElement + 'static,
  ) -> Self {
    self.base = self.base.trigger_with(trigger);
    self
  }

  /// seeds the uncontrolled open state.
  pub fn default_open(mut self, open: bool) -> Self {
    self.default_open = open;
    self
  }

  /// pins the open state, turning the popover controlled.
  pub fn open(mut self, open: bool) -> Self {
    self.open = Some(open);
    self
  }

  /// moves focus into `handle` on open instead of the surface's own focus
  /// handle.
  pub fn track_focus(mut self, handle: &gpui::FocusHandle) -> Self {
    self.base = self.base.track_focus(handle);
    self
  }

  /// keeps the popover open when the pointer goes down outside it.
  pub fn overlay_closable(mut self, closable: bool) -> Self {
    self.base = self.base.overlay_closable(closable);
    self
  }

  /// subscribes to every open-state transition.
  pub fn on_open_change(
    mut self, callback: impl Fn(&bool, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_open_change = Some(Rc::new(callback));
    self
  }

  /// sets the content builder; it receives the popover state so controls
  /// inside the surface can dismiss the panel.
  pub fn content<F, E>(mut self, content: F) -> Self
  where
    E: IntoElement,
    F: FnOnce(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> E + 'static, {
    self.content = Some(Box::new(move |state, window, cx| {
      content(state, window, cx).into_any_element()
    }));
    self
  }
}

impl Styled for Popover {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Popover {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // theme values are copied out before any mutable borrow of `cx`; they
    // travel into the content closure, which base runs during its render.
    let (font_family, text_size, foreground, background, border_color, border_width, radius) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.popover_foreground,
        theme.popover,
        theme.border,
        theme.border_width,
        theme.radius_lg,
      )
    };

    // the base surface unmounts on close, so the entrance fade is sampled
    // here, on the persistent wrapper node. a keyed mirror of the open
    // state feeds the transition: controlled mode copies the prop on every
    // render, uncontrolled toggles arrive through the wrapped handler.
    let id = self.id.clone();
    let mirror =
      window.use_keyed_state((id.clone(), "woocraft-open"), cx, |_, _| self.default_open);
    let open = self.open.unwrap_or_else(|| *mirror.read(cx));
    mirror.update(cx, |open_mirror, _| *open_mirror = open);

    let progress = transition(
      (id, "woocraft-surface"),
      if open { 1.0 } else { 0.0 },
      Transition::new(duration::REVEAL),
      window,
      cx,
    );

    let mut base = self.base;
    base = base.default_open(self.default_open);
    if let Some(open) = self.open {
      base = base.open(open);
    }
    if let Some(on_open_change) = self.on_open_change {
      let mirror = mirror.clone();
      base = base.on_open_change(move |open, window, cx| {
        mirror.update(cx, |open_mirror, _| *open_mirror = *open);
        on_open_change(open, window, cx);
      });
    }

    let shadow_offset = rems(0.25).to_pixels(window.rem_size());
    let shadow_blur = rems(1.).to_pixels(window.rem_size());
    let style = self.style;

    if let Some(content) = self.content {
      base.content(move |state, window, cx| {
        let content = content(state, window, cx);
        div()
          .font_family(font_family)
          .text_size(text_size)
          .text_color(foreground)
          .bg(background)
          .rounded(radius)
          .border(border_width)
          .border_color(border_color)
          .shadow(vec![box_shadow(
            px(0.),
            shadow_offset,
            shadow_blur,
            px(0.),
            with_alpha(black(), 0.2),
          )])
          .p(rems(1.))
          .opacity(progress)
          .refine_style(&style)
          .child(content)
      })
    } else {
      base
    }
  }
}

#[cfg(test)]
mod tests {
  use super::Popover;

  #[test]
  fn default_state_is_closed() {
    let popover = Popover::new("test");
    assert!(!popover.default_open);
    assert!(popover.open.is_none());
    assert!(popover.content.is_none());
  }

  #[test]
  fn open_setter_pins_the_controlled_state() {
    let popover = Popover::new("test").open(true);
    assert_eq!(popover.open, Some(true));
    assert!(!Popover::new("test").open(true).open(false).open.unwrap());
  }

  #[test]
  fn content_builder_is_stored() {
    let popover = Popover::new("test").content(|_, _, _| gpui::div());
    assert!(popover.content.is_some());
  }
}
