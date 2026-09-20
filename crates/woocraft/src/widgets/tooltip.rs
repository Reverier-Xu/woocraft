//! managed tooltips over the base overlay stack, styled by the woocraft
//! design system.
//!
//! the base tooltip stack is a per-window provider: an overlay view owns the
//! show delay, the cross-trigger grace period, mobile suppression, and
//! deferred paint above every other layer; triggers only report hover
//! transitions and content requests. this module wires that stack into the
//! design system with three pieces:
//!
//! - [`host`] creates and registers the themed overlay for one window; mount it
//!   once near the window root, before any tooltip fires.
//! - [`Tooltip`] is the themed content card (text or custom element, with an
//!   optional key-binding hint) handed to requests as an [`AnyView`].
//! - [`TooltipExt`] attaches hover behavior to any stateful element.
//!
//! ```rust,ignore
//! use woocraft::{Tooltip, TooltipExt, tooltip};
//!
//! // once per window, near the root:
//! window_root.tooltip_host = Some(tooltip::host(window, cx));
//!
//! // anywhere else:
//! div().id("save")
//!     .tooltip(|window, cx| Tooltip::new("save the current file").build(window, cx));
//! ```

use std::{cell::Cell, collections::HashMap, rc::Rc};

use gpui::{
  Action, AnyElement, AnyView, App, AppContext as _, Bounds, Entity, Global, IntoElement,
  ParentElement, Pixels, Render, SharedString, StatefulInteractiveElement, Styled, WeakEntity,
  Window, div, prelude::FluentBuilder as _, rems,
};
use gpui_base::{
  Easing, ElementExt, Keyframe, Keyframes, StyledExt as _, Timing, Tooltip as BaseTooltip,
  TooltipOverlay as BaseTooltipOverlay, TooltipRequest, TooltipTransition, animate_keyframes,
};

use crate::{
  theme::{ActiveTheme, duration},
  widgets::kbd::Kbd,
};

/// per-window registry of mounted tooltip overlays.
#[derive(Default)]
struct TooltipHosts(HashMap<gpui::WindowId, WeakEntity<BaseTooltipOverlay>>);

impl Global for TooltipHosts {}

/// creates the themed tooltip overlay for `window` and registers it.
///
/// mount the returned entity once per window, near the root and outside any
/// conditionals — every [`TooltipExt::tooltip`] trigger routes through the
/// registry entry created here, and requests are dropped while no overlay is
/// mounted.
pub fn host(window: &Window, cx: &mut App) -> Entity<BaseTooltipOverlay> {
  let overlay = cx.new(|_| BaseTooltipOverlay::new().render_with(themed_renderer()));
  if !cx.has_global::<TooltipHosts>() {
    cx.set_global(TooltipHosts::default());
  }
  cx.global_mut::<TooltipHosts>()
    .0
    .insert(window.window_handle().window_id(), overlay.downgrade());
  overlay
}

/// returns the overlay registered for `window`, if it still lives.
fn host_for(window: &Window, cx: &App) -> Option<Entity<BaseTooltipOverlay>> {
  cx.try_global::<TooltipHosts>()?
    .0
    .get(&window.window_handle().window_id())?
    .upgrade()
}

/// fade applied to enter transitions; grace-period switches stay seamless.
fn themed_renderer() -> impl Fn(AnyView, TooltipTransition, &mut Window, &mut App) -> AnyElement {
  |view, transition, window, cx| {
    let opacity = match transition {
      TooltipTransition::Enter { epoch } => {
        // a two-frame 0→1 track with endpoint offsets is statically valid;
        // the error arm is unreachable and renders instantly instead.
        let track = Keyframes::try_new([Keyframe::new(0.0, 0.0_f32), Keyframe::new(1.0, 1.0)]);
        match &track {
          Ok(track) => {
            let fade_id = gpui::ElementId::from(("woocraft-tooltip-fade", epoch));
            animate_keyframes(
              fade_id,
              track,
              Timing::new(duration::REVEAL).ease(Easing::EaseOut),
              window,
              cx,
            )
            .value
          }
          Err(_) => 1.0,
        }
      }
      TooltipTransition::Switch { .. } => 1.0,
    };
    div().opacity(opacity).child(view).into_any_element()
  }
}

type ElementBuilder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

enum TooltipContent {
  Text(SharedString),
  Element(ElementBuilder),
}

/// a themed tooltip card, handed to requests as a view.
///
/// the card carries one line of text or a custom element, plus an optional
/// key-binding hint rendered from an explicit [`Kbd`] or resolved from an
/// action's highest-precedence binding.
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

  /// builds the card view for a tooltip request.
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
    // theme values are copied out before the content builder borrows `cx`.
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

    BaseTooltip::new("woocraft-tooltip")
      .h_flex()
      .gap(rems(0.75))
      .px(rems(0.5))
      .py(rems(0.25))
      .rounded(radius)
      .border(border_width)
      .border_color(border_color)
      .bg(background)
      .text_color(foreground)
      .font_family(font_family)
      .text_size(text_size)
      .refine_style(&self.style)
      .map(|card| match &self.content {
        TooltipContent::Text(text) => card.child(text.clone()),
        TooltipContent::Element(builder) => card.child(builder(window, cx)),
      })
      .when_some(key_binding, |card, kbd| card.child(kbd.outline()))
  }
}

/// attaches managed tooltips to stateful elements.
///
/// the element's hover listener is taken over; do not combine with a manual
/// `on_hover` on the same element.
pub trait TooltipExt: StatefulInteractiveElement + ParentElement + ElementExt + Sized {
  /// shows the built tooltip card while the pointer hovers this element.
  ///
  /// the closure runs lazily on each hover, so captured state is read at
  /// show time.
  fn tooltip(self, build: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
    let build = Rc::new(build);
    self.tooltip_with(move |bounds| {
      let build = build.clone();
      TooltipRequest::new(bounds, move |window, cx| build(window, cx))
    })
  }

  /// full-control variant: shapes the request from the captured trigger
  /// bounds, for example to pin a preferred placement side.
  fn tooltip_with(self, request: impl Fn(Bounds<Pixels>) -> TooltipRequest + 'static) -> Self {
    let bounds = Rc::new(Cell::new(Bounds::default()));
    self
      .on_prepaint({
        let bounds = bounds.clone();
        move |prepaint_bounds, _, _| bounds.set(prepaint_bounds)
      })
      .on_hover(move |hovered, window, cx| {
        let Some(host) = host_for(window, cx) else {
          return;
        };
        if *hovered {
          let request = request(bounds.get());
          host.update(cx, |overlay, cx| overlay.request_show(request, window, cx));
        } else {
          host.update(cx, |overlay, cx| overlay.request_hide(window, cx));
        }
      })
  }
}

impl<T> TooltipExt for T where T: StatefulInteractiveElement + ParentElement + ElementExt {}

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
