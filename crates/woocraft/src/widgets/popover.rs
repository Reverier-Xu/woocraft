use gpui::{
  AnyElement, App, Bounds, Deferred, ElementId, IntoElement, Length, ParentElement,
  Pixels, RenderOnce, StyleRefinement, Styled, Window, anchored, deferred, div, px,
  InteractiveElement as _, prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, Size, Sizable, StyleSized, StyledExt, v_flex};

#[derive(IntoElement)]
pub struct Popover {
  id: ElementId,
  children: Vec<AnyElement>,
  style: StyleRefinement,
  size: Size,
  trigger_bounds: Bounds<Pixels>,
  menu_width: Length,
}

impl Popover {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      children: Vec::new(),
      style: StyleRefinement::default(),
      size: Size::default(),
      trigger_bounds: Bounds::default(),
      menu_width: Length::Auto,
    }
  }

  pub fn trigger_bounds(mut self, bounds: Bounds<Pixels>) -> Self {
    self.trigger_bounds = bounds;
    self
  }

  pub fn menu_width(mut self, menu_width: impl Into<Length>) -> Self {
    self.menu_width = menu_width.into();
    self
  }

  fn render_overlay<E>(trigger_bounds: Bounds<Pixels>, menu_width: Length, content: E) -> Deferred
  where
    E: IntoElement + 'static,
  {
    deferred(
      anchored()
        .snap_to_window_with_margin(px(8.))
        .child(
          div()
            .relative()
            .map(|this| match menu_width {
              Length::Auto => this.w(trigger_bounds.size.width),
              Length::Definite(width) => this.w(width),
            })
            .child(content),
        ),
    )
    .with_priority(usize::MAX)
  }
}

impl Sizable for Popover {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for Popover {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Popover {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Popover {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {


    Self::render_overlay(
      self.trigger_bounds,
      self.menu_width,
      v_flex()
        .id(self.id)
        .occlude()
        .max_h(px(220.))
        .bg(cx.theme().background)
        .border_1()
        .border_color(cx.theme().border)
        .rounded(self.size.container_radius())
        .container_px(self.size)
        .container_py(self.size)
        .children(self.children)
        .refine_style(&self.style),
    )
  }
}
