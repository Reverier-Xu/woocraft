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
  Anchor, AnyElement, App, ClickEvent, ElementId, FocusHandle, InteractiveElement as _,
  IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, Window, black, div, px, relative, rems,
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
  widgets::{
    button::{Button, ButtonVariants as _},
    divider::Divider,
    icon_label::IconLabel,
  },
};

type CloseHandler = Rc<dyn Fn(&mut Window, &mut App)>;
type ActionHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

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
///
/// layout contract, shared with the modal family: title — divider —
/// content — actions, each block owning its `0.25rem` chrome while the
/// card's own stack adds none. the title is an [`IconLabel`] row (intent
/// icon, ellipsizing title, right-aligned controls); the divider is the
/// timeout rail when [`Toast::timeout_progress`] is set, a hairline
/// otherwise; the description wraps to at most three lines and scrolls
/// beyond them; [`Toast::action`] right-aligns its button in the actions
/// block.
#[derive(IntoElement)]
pub struct Toast {
  id: ElementId,
  base: BaseToast,
  variant: ToastVariant,
  title: Option<SharedString>,
  description: Option<SharedString>,
  timeout_progress: Option<f32>,
  action: Option<(SharedString, ActionHandler)>,
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
      timeout_progress: None,
      action: None,
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

  /// sets the secondary line of text; it wraps to at most three lines and
  /// scrolls beyond them.
  pub fn description(mut self, description: impl Into<SharedString>) -> Self {
    self.description = Some(description.into());
    self
  }

  /// shows how much of the toast's lifetime remains: a hairline track
  /// between the title and the content, muted rail under a fill in the
  /// variant's intent color, draining from `1.0` to `0.0`. the application
  /// owns the clock and passes the remaining fraction on every render;
  /// sticky toasts simply never set it.
  pub fn timeout_progress(mut self, progress: f32) -> Self {
    self.timeout_progress = Some(progress.clamp(0., 1.));
    self
  }

  /// embeds an action button under the description, running `handler` when
  /// activated — sonner's "undo" pattern.
  pub fn action(
    mut self, label: impl Into<SharedString>,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.action = Some((label.into(), Rc::new(handler)));
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
      muted,
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
        theme.muted,
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
    // the timeout rail drains with the toast's remaining lifetime; the
    // motion transition just smooths the tick-to-tick steps.
    let timeout_track = self.timeout_progress.map(|progress| {
      let progress = gpui_base::transition(
        (self.id.clone(), "timeout"),
        progress,
        gpui_base::Transition::new(duration::CONTROL),
        window,
        cx,
      );
      div()
        .debug_selector(|| "woocraft-toast-timeout-track".into())
        .h(border_width)
        .w_full()
        .rounded_full()
        .bg(muted)
        .child(
          div()
            .debug_selector(|| "woocraft-toast-timeout-fill".into())
            .h_full()
            .w(relative(progress))
            .rounded_full()
            .bg(accent),
        )
        .into_any_element()
    });
    // the close control joins the title row's right-aligned controls.
    let close_button = self.on_close.map(|on_close| {
      div()
        .debug_selector(|| "woocraft-toast-close".into())
        .flex_shrink_0()
        .child(
          Button::new((self.id.clone(), "close"))
            .icon(Icon::new(IconName::Dismiss))
            .flat()
            .on_click(move |_, window, cx| on_close(window, cx)),
        )
        .into_any_element()
    });
    let action_button = self.action.map(|(label, on_action)| {
      Button::new((self.id.clone(), "action"))
        .debug_selector(|| "woocraft-toast-action".into())
        .label(label)
        .outline(true)
        .on_click(move |event, window, cx| on_action(event, window, cx))
        .into_any_element()
    });

    // title — divider — content — actions, each block owning its 0.25rem
    // chrome; the card's own stack adds nothing.
    let mut title_label = IconLabel::new((self.id.clone(), "title"))
      .icon(Icon::new(icon).text_color(accent))
      .w_full()
      .px(rems(0.))
      .py(rems(0.));
    if let Some(title) = self.title {
      title_label = title_label.label(title);
    }
    let title_row = super::dialog::title_row(
      title_label,
      close_button.into_iter().collect(),
      gpui::FontWeight::MEDIUM,
    )
    .into_any_element();

    let description = self.description.map(|description| {
      div()
        .id((self.id.clone(), "description"))
        .debug_selector(|| "woocraft-toast-description".into())
        .line_height(relative(1.25))
        // three wrapped lines, then scroll.
        .max_h(rems(3.75))
        .overflow_y_scroll()
        .text_color(muted_foreground)
        .child(description)
        .into_any_element()
    });
    let has_content = description.is_some() || !self.children.is_empty();

    // the timeout rail doubles as the title/content divider; without a
    // timeout, a plain hairline takes its place — but only when there is
    // content below to separate from.
    let divider = if timeout_track.is_some() {
      timeout_track
    } else if has_content || action_button.is_some() {
      Some(Divider::horizontal().into_any_element())
    } else {
      None
    };

    let content = has_content.then(|| {
      div()
        .flex()
        .flex_col()
        .gap(rems(0.25))
        .p(rems(0.25))
        .children(description)
        .children(self.children)
        .into_any_element()
    });
    let actions = action_button.map(|action| {
      div()
        .flex()
        .flex_row()
        .w_full()
        .justify_end()
        .p(rems(0.25))
        .child(action)
        .into_any_element()
    });

    self
      .base
      .refine_style(&style)
      .debug_selector(|| "woocraft-toast-card".into())
      .w_full()
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
      .child(
        div()
          .flex()
          .flex_col()
          .w_full()
          .child(title_row)
          .children(divider)
          .children(content)
          .children(actions),
      )
  }
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
  use std::{cell::Cell, rc::Rc};

  use gpui::{Context, IntoElement, ParentElement, Render, Styled, div, point, rems};

  use super::{Toast, ToastVariant};
  use crate::theme::ActiveTheme;

  #[test]
  fn toast_starts_neutral_without_copy() {
    let toast = Toast::new("test");
    assert_eq!(toast.variant, ToastVariant::Default);
    assert!(toast.title.is_none());
    assert!(toast.description.is_none());
    assert!(toast.action.is_none());
    assert!(toast.on_close.is_none());
  }

  #[test]
  fn variant_setters_update_the_intent() {
    let toast = Toast::new("test").success();
    assert_eq!(toast.variant, ToastVariant::Success);
  }

  struct Host {
    clicked: Rc<Cell<bool>>,
  }

  impl Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl IntoElement {
      let clicked = self.clicked.clone();
      div().w(rems(20.)).child(
        Toast::new("test-toast")
          .title("a title that runs well past the width the stack gives the card")
          .description(
            "a description long enough to wrap past three lines of body copy, so the card has \
             to clamp the block and let the rest scroll: one, two, three, four, five, six, \
             seven, eight, nine, ten, eleven, twelve, thirteen, fourteen, fifteen, sixteen",
          )
          .action("undo", move |_, _, _| clicked.set(true))
          .timeout_progress(0.5)
          .on_close(|_, _| {}),
      )
    }
  }

  fn toast_window(cx: &mut gpui::TestAppContext) -> (Rc<Cell<bool>>, &mut gpui::VisualTestContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let clicked = Rc::new(Cell::new(false));
    let host = clicked.clone();
    let (_view, cx) = cx.add_window_view(|_, _| Host { clicked: host });
    cx.update(|window, cx| {
      window.draw(cx).clear(cx);
      window.draw(cx).clear(cx);
    });
    (clicked, cx)
  }

  #[gpui::test]
  fn the_card_fills_the_stack_width(cx: &mut gpui::TestAppContext) {
    let (_clicked, cx) = toast_window(cx);
    let rem = cx.update(|window, _| window.rem_size());
    let card = cx
      .debug_bounds("woocraft-toast-card")
      .expect("the toast card paints");
    assert_eq!(card.size.width, rems(20.).to_pixels(rem));
  }

  #[gpui::test]
  fn the_description_clamps_to_three_lines(cx: &mut gpui::TestAppContext) {
    let (_clicked, cx) = toast_window(cx);
    let rem = cx.update(|window, _| window.rem_size());
    let description = cx
      .debug_bounds("woocraft-toast-description")
      .expect("the description paints");
    assert!(
      description.size.height <= rems(3.75).to_pixels(rem),
      "the description never exceeds three lines, got {:?}",
      description.size.height,
    );
  }

  #[gpui::test]
  fn the_close_control_sits_in_the_top_right(cx: &mut gpui::TestAppContext) {
    let (_clicked, cx) = toast_window(cx);
    let (rem, border) = cx.update(|window, cx| (window.rem_size(), cx.theme().border_width));
    let card = cx
      .debug_bounds("woocraft-toast-card")
      .expect("the toast card paints");
    let close = cx
      .debug_bounds("woocraft-toast-close")
      .expect("the close control paints");
    // the overlay insets from the card's padding box: the 0.25rem inset
    // plus the hairline border.
    let inset = rems(0.25).to_pixels(rem) + border;
    assert_eq!(close.origin.y, card.origin.y + inset);
    assert_eq!(
      close.origin.x + close.size.width,
      card.origin.x + card.size.width - inset
    );
  }

  #[gpui::test]
  fn the_timeout_rail_drains_with_the_remaining_fraction(cx: &mut gpui::TestAppContext) {
    let (_clicked, cx) = toast_window(cx);
    let track = cx
      .debug_bounds("woocraft-toast-timeout-track")
      .expect("the timeout track paints when progress is set");
    let fill = cx
      .debug_bounds("woocraft-toast-timeout-fill")
      .expect("the timeout fill paints");
    let expected = track.size.width / 2.;
    assert!(
      (fill.size.width - expected).abs() <= gpui::px(1.),
      "half the lifetime remains, got fill {:?} of track {:?}",
      fill.size.width,
      track.size.width,
    );
  }

  #[gpui::test]
  fn the_action_aligns_to_the_right_edge(cx: &mut gpui::TestAppContext) {
    let (_clicked, cx) = toast_window(cx);
    let (rem, border) = cx.update(|window, cx| (window.rem_size(), cx.theme().border_width));
    let card = cx
      .debug_bounds("woocraft-toast-card")
      .expect("the toast card paints");
    let action = cx
      .debug_bounds("woocraft-toast-action")
      .expect("the action button paints");
    assert_eq!(
      action.origin.x + action.size.width,
      card.origin.x + card.size.width - border - rems(0.25).to_pixels(rem),
      "the action block owns a 0.25rem chrome and right-aligns its button"
    );
  }

  #[gpui::test]
  fn the_action_runs_on_click(cx: &mut gpui::TestAppContext) {
    let (clicked, cx) = toast_window(cx);
    let action = cx
      .debug_bounds("woocraft-toast-action")
      .expect("the action button paints");
    cx.simulate_click(
      point(
        action.origin.x + action.size.width / 2.,
        action.origin.y + action.size.height / 2.,
      ),
      Default::default(),
    );
    assert!(clicked.get(), "the action handler ran");
  }
}
