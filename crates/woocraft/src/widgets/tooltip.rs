use gpui::{
  Action, AnyElement, AnyView, App, AppContext, Context, IntoElement, ParentElement, Render,
  SharedString, StyleRefinement, Styled, Window, div, prelude::FluentBuilder,
};

use crate::{ActiveTheme, Kbd, StyledExt, h_flex};

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
}

impl Tooltip {
  pub fn new(text: impl Into<SharedString>) -> Self {
    Self {
      style: StyleRefinement::default(),
      content: TooltipContent::Text(text.into()),
      key_binding: None,
      action: None,
    }
  }

  pub fn element<E, F>(builder: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut Window, &mut App) -> E + 'static,
  {
    Self {
      style: StyleRefinement::default(),
      key_binding: None,
      action: None,
      content: TooltipContent::Element(Box::new(move |window, cx| {
        builder(window, cx).into_any_element()
      })),
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

  pub fn build(self, _: &mut Window, cx: &mut App) -> AnyView {
    cx.new(|_| self).into()
  }
}

impl FluentBuilder for Tooltip {}

impl Styled for Tooltip {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl Render for Tooltip {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let key_binding = if let Some(key_binding) = &self.key_binding {
      Some(key_binding.clone())
    } else if let Some((action, context)) = &self.action {
      Kbd::binding_for_action(
        action.as_ref(),
        context.as_ref().map(|s| s.as_ref()),
        window,
      )
    } else {
      None
    };

    div().child(
      h_flex()
        .m_3()
        .bg(cx.theme().popover)
        .text_color(cx.theme().popover_foreground)
        .border_1()
        .border_color(cx.theme().border)
        .shadow_sm()
        .rounded(cx.theme().radius)
        .justify_between()
        .py_1()
        .px_2()
        .gap_3()
        .refine_style(&self.style)
        .map(|this| {
          this.child(div().map(|this| match self.content {
            TooltipContent::Text(ref text) => this.child(text.clone()),
            TooltipContent::Element(ref builder) => this.child(builder(window, cx)),
          }))
        })
        .when_some(key_binding, |this, kbd| {
          this.child(
            div()
              .text_sm()
              .flex_shrink_0()
              .text_color(cx.theme().muted_foreground)
              .child(kbd.outline()),
          )
        }),
    )
  }
}
