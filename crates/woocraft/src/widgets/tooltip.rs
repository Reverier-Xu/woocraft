use gpui::{
  Action, AnyElement, AnyView, App, AppContext, Context, IntoElement, ParentElement, Render,
  SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder,
};

use crate::{
  ActiveTheme, CardStyle, Easing, Kbd, Presence, Size, StyleSized, StyledExt, Transition, duration,
  h_flex,
};

type TooltipElementBuilder = Box<dyn Fn(&mut Window, &mut App) -> AnyElement>;

enum TooltipContent {
  Text(SharedString),
  Element(TooltipElementBuilder),
}

pub struct Tooltip {
  style: StyleRefinement,
  content: TooltipContent,
  key_binding: Option<Kbd>,
  action: Option<(Box<dyn Action>, Option<SharedString>)>,
  size: Size,
}

impl Tooltip {
  pub fn new(text: impl Into<SharedString>) -> Self {
    Self {
      style: StyleRefinement::default(),
      content: TooltipContent::Text(text.into()),
      key_binding: None,
      action: None,
      size: Size::Medium,
    }
  }

  pub fn element<E, F>(builder: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut Window, &mut App) -> E + 'static, {
    Self {
      style: StyleRefinement::default(),
      key_binding: None,
      action: None,
      content: TooltipContent::Element(Box::new(move |window, cx| {
        builder(window, cx).into_any_element()
      })),
      size: Size::Medium,
    }
  }

  pub fn key_binding(mut self, key_binding: Option<Kbd>) -> Self {
    self.key_binding = key_binding;
    self
  }

  pub fn action(mut self, action: &dyn Action, context: Option<&str>) -> Self {
    self.action = Some((action.boxed_clone(), context.map(SharedString::new)));
    self
  }

  /// Render the tooltip as a view for gpui's tooltip API.
  ///
  /// The signature has to hand out an `AnyView`: gpui's tooltip element only
  /// consumes entity-backed views, so *a* per-hover entity is unavoidable.
  /// Everything expensive is resolved here instead of inside that entity: the
  /// boxed action is consumed for a one-time keymap scan and then dropped, so
  /// the backing [`TooltipView`] is an immutable shell that merely paints
  /// precomputed data on the (rare) frames it re-renders.
  pub fn build(mut self, window: &mut Window, cx: &mut App) -> AnyView {
    // Resolve the key binding eagerly: the tooltip content is immutable for
    // its lifetime, so `binding_for_action`'s keymap scan does not need to be
    // repeated on every re-render of the backing entity.
    if self.key_binding.is_none() {
      if let Some((action, context)) = &self.action {
        self.key_binding = Kbd::binding_for_action(
          action.as_ref(),
          context.as_ref().map(|s| s.as_ref()),
          window,
        );
      }
      self.action = None;
    }

    cx.new(|_| TooltipView {
      style: self.style,
      content: self.content,
      key_binding: self.key_binding,
      size: self.size,
    })
    .into()
  }
}

impl FluentBuilder for Tooltip {}

impl_sizable!(Tooltip);
impl_styled!(Tooltip);

/// The immutable shell entity held by gpui's tooltip API. See
/// [`Tooltip::build`]; it owns only precomputed render inputs.
struct TooltipView {
  style: StyleRefinement,
  content: TooltipContent,
  key_binding: Option<Kbd>,
  size: Size,
}

impl Render for TooltipView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // The effective binding was resolved once in `build`; render only reads
    // the cached value.
    let key_binding = self.key_binding.clone();

    // Fade in when the tooltip mounts. The view only exists while the tooltip
    // is shown, so its keyed presence state drops on hide and every show
    // replays the entrance.
    let enter = Presence::new((cx.entity_id().as_u64() as usize, "tooltip-enter"), true)
      .transition(Transition::new(duration::TOOLTIP_ENTER).easing(Easing::EaseOut))
      .sample(window, cx)
      .progress;

    h_flex()
      .m_3()
      .tooltip_style(cx.theme())
      .justify_between()
      .component_padding(self.size)
      .component_gap(self.size)
      .opacity(enter)
      .refine_style(&self.style)
      .map(|this| match self.content {
        TooltipContent::Text(ref text) => this.child(text.clone()),
        TooltipContent::Element(ref builder) => this.child(builder(window, cx)),
      })
      .when_some(key_binding, |this, kbd| {
        this.child(
          div()
            .text_sm()
            .flex_shrink_0()
            .text_color(cx.theme().muted_foreground)
            .child(kbd.outline()),
        )
      })
  }
}
