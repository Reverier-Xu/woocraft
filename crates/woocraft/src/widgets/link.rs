use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, MouseButton,
  ParentElement, RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement,
  Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, Disableable, StyledExt};

#[derive(IntoElement)]
pub struct Link {
  id: ElementId,
  style: StyleRefinement,
  href: Option<SharedString>,
  disabled: bool,
  on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
  children: Vec<AnyElement>,
}

impl Link {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      href: None,
      disabled: false,
      on_click: None,
      children: Vec::new(),
    }
  }

  pub fn href(mut self, href: impl Into<SharedString>) -> Self {
    self.href = Some(href.into());
    self
  }

  pub fn on_click(
    mut self,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }
}

impl Disableable for Link {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Styled for Link {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Link {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements)
  }
}

impl RenderOnce for Link {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let href = self.href.clone();
    let on_click = self.on_click.clone();
    let base_color = if self.disabled {
      cx.theme().muted_foreground
    } else {
      cx.theme().primary
    };

    div()
      .id(self.id)
      .text_color(base_color)
      .text_decoration_1()
      .text_decoration_color(base_color)
      .when(!self.disabled, |this| {
        this
          .cursor_pointer()
          .hover(|this| this.opacity(0.85))
          .active(|this| this.opacity(0.7))
      })
      .when(!self.disabled, |this| {
        this.on_mouse_down(MouseButton::Left, |_, _, cx| {
          cx.stop_propagation();
        })
      })
      .when(!self.disabled, |this| {
        this.on_click(move |event, window, cx| {
          if let Some(href) = href.as_ref() {
            cx.open_url(href);
          }

          if let Some(on_click) = on_click.as_ref() {
            on_click(event, window, cx);
          }
        })
      })
      .children(self.children)
      .refine_style(&self.style)
  }
}
