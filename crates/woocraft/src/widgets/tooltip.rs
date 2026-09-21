//! themed tooltip cards rendered through gpui's native tooltip support.
//!
//! gpui ships the whole tooltip lifecycle per window: hover detection, the
//! show delay, positioning at the pointer, and dismissal. this module only
//! supplies the design system's card — [`Tooltip`] is a styled view handed
//! to the native `.tooltip()` builder as an [`AnyView`], carrying one line
//! of text or a custom element plus an optional key-binding hint resolved
//! from an explicit [`Kbd`] or an action's highest-precedence binding.
//!
//! ```rust,ignore
//! use woocraft::{Button, Tooltip};
//!
//! Button::new("save")
//!     .label("save")
//!     .tooltip(|window, cx| Tooltip::new("save the current file").build(window, cx));
//! ```

use std::time::Duration;

use gpui::{
  Action, AnyElement, AnyView, App, AppContext as _, IntoElement, ParentElement, Render,
  SharedString, Styled, Window, div, prelude::FluentBuilder as _, rems,
};
use gpui_base::{StyledExt as _, Tooltip as BaseTooltip};

use crate::{theme::ActiveTheme, widgets::kbd::Kbd};

type ElementBuilder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

enum TooltipContent {
  Text(SharedString),
  Element(ElementBuilder),
}

/// a themed tooltip card, handed to the native tooltip builder as a view.
pub struct Tooltip {
  style: gpui::StyleRefinement,
  content: TooltipContent,
  key_binding: Option<Kbd>,
  action: Option<(Box<dyn Action>, Option<SharedString>)>,
}

impl Tooltip {
  /// creates a card showing one line of text.
  pub fn new(text: impl Into<SharedString>) -> Self {
    Self {
      style: gpui::StyleRefinement::default(),
      content: TooltipContent::Text(text.into()),
      key_binding: None,
      action: None,
    }
  }

  /// creates a card rendering a custom element.
  pub fn element<E, F>(builder: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut Window, &mut App) -> E + 'static, {
    Self {
      style: gpui::StyleRefinement::default(),
      key_binding: None,
      action: None,
      content: TooltipContent::Element(Box::new(move |window, cx| {
        builder(window, cx).into_any_element()
      })),
    }
  }

  /// shows an explicit key hint next to the content.
  pub fn key_binding(mut self, key_binding: Option<Kbd>) -> Self {
    self.key_binding = key_binding;
    self
  }

  /// resolves the key hint from the action's highest-precedence binding,
  /// optionally filtered by key context.
  pub fn action(mut self, action: &dyn Action, context: Option<&str>) -> Self {
    self.action = Some((action.boxed_clone(), context.map(SharedString::new)));
    self
  }

  /// builds the card view for the native tooltip builder.
  pub fn build(self, _: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_| self).into()
  }
}

impl Styled for Tooltip {
  fn style(&mut self) -> &mut gpui::StyleRefinement {
    &mut self.style
  }
}

impl Render for Tooltip {
  fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
    let (font_family, text_size, foreground, background, border_color, border_width, radius) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.popover_foreground,
        theme.popover,
        theme.border,
        theme.border_width,
        theme.radius,
      )
    };
    let key_binding = self.key_binding.clone().or_else(|| {
      self.action.as_ref().and_then(|(action, context)| {
        Kbd::binding_for_action(action.as_ref(), context.as_deref(), window)
      })
    });

    // the margin keeps the card off the cursor: gpui anchors the tooltip
    // view at the pointer, and without margin the card hugs it.
    div().m(rems(0.75)).child(
      BaseTooltip::new("woocraft-tooltip")
        .h_flex()
        .max_w(rems(20.))
        .gap(rems(0.75))
        .px(rems(0.5))
        .py(rems(0.25))
        .font_family(font_family)
        .text_size(text_size)
        .text_color(foreground)
        .bg(background)
        .border(border_width)
        .border_color(border_color)
        .rounded(radius)
        .refine_style(&self.style)
        .map(|card| match &self.content {
          TooltipContent::Text(text) => card.child(text.clone()),
          TooltipContent::Element(builder) => card.child(builder(window, cx)),
        })
        .when_some(key_binding, |card, kbd| card.child(kbd.outline())),
    )
  }
}

/// the native tooltip show delay used by gpui, re-published for
/// applications tuning [`gpui::Window`] tooltip timing alongside the card.
pub const TOOLTIP_SHOW_DELAY: Duration = Duration::from_millis(500);

#[cfg(test)]
mod tests {
  use super::Tooltip;

  #[test]
  fn text_card_carries_no_hint_by_default() {
    let tooltip = Tooltip::new("hint");
    assert!(tooltip.key_binding.is_none());
    assert!(tooltip.action.is_none());
  }
}
