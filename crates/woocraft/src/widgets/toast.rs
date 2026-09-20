//! toast notifications over the base stack model, styled by the woocraft
//! design system.
//!
//! the base toast module splits the problem in two: [`ToastManager`] owns
//! the ordered stack, auto-hide timers, and exit coordination as a pure
//! model, while the [`ToastStack`](base::ToastStack) element renders it with
//! deep stack motion — collapse peeks, hover expansion, and per-layer
//! springs. applications advance the manager on a timer and feed its
//! entries into the stack.
//!
//! this module contributes the design system's pieces: [`Toast`] is the
//! themed notification card (intent color, icon, title, description, and an
//! optional close control) whose enter and exit fades follow the lifecycle
//! status the base root carries, and [`Toaster`] is the themed stack with
//! rem-scaled motion defaults. the manager types are re-exported unchanged —
//! they have no presentation.
//!
//! ```rust,ignore
//! use woocraft::{Toast, ToastOptions, toast::{Toaster, ToastStackState}};
//!
//! // on every frame the application advances the manager and renders:
//! stack.item(
//!     "saved",
//!     Toast::new("saved")
//!         .success()
//!         .title("project saved")
//!         .on_close(|_, window, cx| manager.borrow_mut().dismiss(&"saved".into(), now))
//!         .transition_status(status),
//! );
//! ```

use std::rc::Rc;

use gpui::{
  Anchor, AnyElement, App, ElementId, FocusHandle, IntoElement, ParentElement, RenderOnce,
  SharedString, StyleRefinement, Styled, Window, black, div, prelude::FluentBuilder as _, px, rems,
};
use gpui_base::{
  Keyframe, Keyframes, StyledExt as _, Timing, Toast as BaseToast, ToastStack as BaseToastStack,
  animate_keyframes, box_shadow,
};
pub use gpui_base::{
  ToastAdvance, ToastManager, ToastMotion, ToastOptions, ToastStackState, ToastTransitionStatus,
};

use crate::{
  icon::{Icon, IconName},
  theme::{ActiveTheme, duration, with_alpha},
  widgets::button::{Button, ButtonVariants as _},
};

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;

/// intent color of a themed toast card.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ToastVariant {
  /// neutral confirmation.
  #[default]
  Default,
  /// positive outcome.
  Success,
  /// caution worth reading.
  Warning,
  /// failure or destructive outcome.
  Danger,
  /// informational aside.
  Info,
}

impl ToastVariant {
  /// the icon introducing the intent.
  fn icon(self) -> IconName {
    match self {
      ToastVariant::Default => IconName::Info,
      ToastVariant::Success => IconName::CheckmarkCircle,
      ToastVariant::Warning => IconName::Warning,
      ToastVariant::Danger => IconName::DismissCircle,
      ToastVariant::Info => IconName::Info,
    }
  }
}

/// themed notification card rendered inside a toaster stack.
#[derive(IntoElement)]
pub struct Toast {
  id: ElementId,
  base: BaseToast,
  variant: ToastVariant,
  title: Option<SharedString>,
  description: Option<SharedString>,
  on_close: Option<CloseHandler>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Toast {
  /// creates a neutral toast with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseToast::new(id.clone()),
      id,
      variant: ToastVariant::Default,
      title: None,
      description: None,
      on_close: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the intent color and icon.
  pub fn variant(mut self, variant: ToastVariant) -> Self {
    self.variant = variant;
    self
  }

  /// marks the toast as a positive outcome.
  pub fn success(mut self) -> Self {
    self.variant = ToastVariant::Success;
    self
  }

  /// marks the toast as a caution.
  pub fn warning(mut self) -> Self {
    self.variant = ToastVariant::Warning;
    self
  }

  /// marks the toast as a failure or destructive outcome.
  pub fn danger(mut self) -> Self {
    self.variant = ToastVariant::Danger;
    self
  }

  /// marks the toast as an informational aside.
  pub fn info(mut self) -> Self {
    self.variant = ToastVariant::Info;
    self
  }

  /// sets the primary line of text.
  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  /// sets the secondary line of text.
  pub fn description(mut self, description: impl Into<SharedString>) -> Self {
    self.description = Some(description.into());
    self
  }

  /// shows a close control running `handler`.
  pub fn on_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.on_close = Some(Rc::new(handler));
    self
  }

  /// sets the lifecycle phase driving the card's fade presentation.
  pub fn transition_status(mut self, status: ToastTransitionStatus) -> Self {
    self.base = self.base.transition_status(status);
    self
  }
}

impl Styled for Toast {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Toast {
  fn extend(&mut self, children: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(children);
  }
}

impl RenderOnce for Toast {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // theme values are copied out before the render borrows end.
    let (
      font_family,
      text_size,
      foreground,
      muted_foreground,
      card,
      border_color,
      border_width,
      radius,
      accent,
      icon,
    ) = {
      let theme = cx.theme();
      let accent = match self.variant {
        ToastVariant::Default => theme.muted_foreground,
        ToastVariant::Success => theme.success,
        ToastVariant::Warning => theme.warning,
        ToastVariant::Danger => theme.danger,
        ToastVariant::Info => theme.blue,
      };
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.foreground,
        theme.muted_foreground,
        theme.card,
        theme.border,
        theme.border_width,
        theme.radius,
        accent,
        self.variant.icon(),
      )
    };
    let status = self.base.status();
    let style = self.style;
    let id = self.id.clone();

    // enter: one fade per card lifetime, keyed by the toast id — the card
    // mounts exactly once. exit: a retargeting fade the status flip starts
    // while the base stack keeps the card mounted.
    let track = Keyframes::try_new([Keyframe::new(0.0, 0.0_f32), Keyframe::new(1.0, 1.0)]);
    let enter = match &track {
      Ok(track) => {
        animate_keyframes(
          (id.clone(), "woocraft-enter"),
          track,
          Timing::new(duration::REVEAL).ease(gpui_base::Easing::EaseOut),
          window,
          cx,
        )
        .value
      }
      Err(_) => 1.0,
    };
    let exit = gpui_base::transition(
      (id, "woocraft-exit"),
      if status == ToastTransitionStatus::Ending {
        0.0
      } else {
        1.0
      },
      gpui_base::Transition::new(duration::REVEAL),
      window,
      cx,
    );

    let shadow_blur = rems(1.).to_pixels(window.rem_size());
    let shadow_offset = rems(0.25).to_pixels(window.rem_size());
    let close_button = self.on_close.map(|on_close| {
      Button::new("toast-close")
        .icon(Icon::new(IconName::Dismiss))
        .flat()
        .on_click(move |_, window, cx| on_close(window, cx))
    });

    self
      .base
      .refine_style(&style)
      .h_flex()
      .items_start()
      .gap(rems(0.75))
      .px(rems(1.))
      .py(rems(0.75))
      .font_family(font_family)
      .text_size(text_size)
      .text_color(foreground)
      .bg(card)
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
      .opacity(enter * exit)
      .child(Icon::new(icon).text_color(accent))
      .child(
        v_flex_toast_body()
          .gap(rems(0.25))
          .when_some(self.title, |body, title| {
            body.child(div().font_weight(gpui::FontWeight::MEDIUM).child(title))
          })
          .when_some(self.description, |body, description| {
            body.child(div().text_color(muted_foreground).child(description))
          })
          .children(self.children),
      )
      .when_some(close_button, |card, button| card.child(button))
  }
}

// a column that stretches its rows; toasts are the one place the stack
// gesture needs the body taller than its content before text wraps.
fn v_flex_toast_body() -> gpui::Div {
  div().flex().flex_col()
}

/// themed toaster stack with rem-scaled motion defaults.
#[derive(IntoElement)]
pub struct Toaster {
  base: BaseToastStack,
  style: StyleRefinement,
}

impl Toaster {
  /// creates a stack anchored to the window's top-right corner by default.
  pub fn new(id: impl Into<ElementId>, state: ToastStackState) -> Self {
    Self {
      base: BaseToastStack::new(id, state),
      style: StyleRefinement::default(),
    }
  }

  /// anchors the stack to a viewport edge; the default is top-right.
  pub fn placement(mut self, placement: Anchor) -> Self {
    self.base = self.base.placement(placement);
    self
  }

  /// overrides the design system's rem-scaled motion tokens.
  pub fn motion(mut self, motion: ToastMotion) -> Self {
    self.base = self.base.motion(motion);
    self
  }

  /// sets the focus scope that expands the stack and pauses auto-hide.
  pub fn focus_handle(mut self, focus_handle: FocusHandle) -> Self {
    self.base = self.base.focus_handle(focus_handle);
    self
  }

  /// adds a stably keyed toast card to the stack.
  pub fn item(mut self, id: impl Into<ElementId>, child: impl IntoElement) -> Self {
    self.base = self.base.item(id, child);
    self
  }
}

impl Styled for Toaster {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Toaster {
  fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
    // sonner's 14px peek and gap expressed in rem so the stack scales
    // with the window root font size.
    let rem = window.rem_size();
    let motion = ToastMotion {
      collapsed_peek: rems(0.875).to_pixels(rem),
      expanded_gap: rems(0.875).to_pixels(rem),
      ..ToastMotion::sonner()
    };
    self
      .base
      .motion(motion)
      .w(rems(20.))
      .refine_style(&self.style)
  }
}

#[cfg(test)]
mod tests {
  use super::{Toast, ToastVariant};

  #[test]
  fn toast_starts_neutral_without_copy() {
    let toast = Toast::new("test");
    assert_eq!(toast.variant, ToastVariant::Default);
    assert!(toast.title.is_none());
    assert!(toast.description.is_none());
    assert!(toast.on_close.is_none());
  }

  #[test]
  fn variant_setters_update_the_intent() {
    let toast = Toast::new("test").success();
    assert_eq!(toast.variant, ToastVariant::Success);
  }
}
