use gpui::{
  App, Axis, IntoElement, ParentElement, Pixels, Rems, RenderOnce, StyleRefinement, Styled, Window,
};

use super::{Field, FieldProps};
use crate::{Sizable, Size, StyledExt, v_flex};

#[derive(IntoElement)]
pub struct Form {
  style: StyleRefinement,
  fields: Vec<Field>,
  props: FieldProps,
}

impl Form {
  fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      fields: Vec::new(),
      props: FieldProps::default(),
    }
  }

  pub fn horizontal() -> Self {
    Self::new().layout(Axis::Horizontal)
  }

  pub fn vertical() -> Self {
    Self::new().layout(Axis::Vertical)
  }

  pub fn layout(mut self, layout: Axis) -> Self {
    self.props.layout = layout;
    self
  }

  pub fn label_width(mut self, width: Pixels) -> Self {
    self.props.label_width = Some(width);
    self
  }

  pub fn label_text_size(mut self, size: Rems) -> Self {
    self.props.label_text_size = Some(size);
    self
  }

  pub fn child(mut self, field: impl Into<Field>) -> Self {
    self.fields.push(field.into());
    self
  }

  pub fn children(mut self, fields: impl IntoIterator<Item = Field>) -> Self {
    self.fields.extend(fields);
    self
  }

  pub fn columns(mut self, columns: usize) -> Self {
    self.props.columns = columns.max(1);
    self
  }
}

impl Styled for Form {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl Sizable for Form {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.props.size = size.into();
    self
  }
}

impl RenderOnce for Form {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let props = self.props;
    let gap = props.size.component_gap();

    v_flex()
      .w_full()
      .grid()
      .grid_cols(props.columns as u16)
      .gap_x(gap * 2.0)
      .gap_y(gap)
      .children(
        self
          .fields
          .into_iter()
          .enumerate()
          .map(|(ix, field)| field.props(ix, props)),
      )
      .refine_style(&self.style)
  }
}
