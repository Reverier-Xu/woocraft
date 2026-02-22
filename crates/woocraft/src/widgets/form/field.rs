use std::rc::Rc;

use gpui::{
  AlignItems, AnyElement, AnyView, App, Axis, Div, Element, ElementId, InteractiveElement as _,
  IntoElement, ParentElement, Pixels, Rems, RenderOnce, SharedString, Styled, Window, div,
  prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme as _, AxisExt, Size, StyledExt, h_flex, v_flex};

type FieldElementBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyElement>;

#[derive(Clone, Copy)]
pub(super) struct FieldProps {
  pub(super) size: Size,
  pub(super) layout: Axis,
  pub(super) columns: usize,
  pub(super) label_width: Option<Pixels>,
  pub(super) label_text_size: Option<Rems>,
}

impl Default for FieldProps {
  fn default() -> Self {
    Self {
      size: Size::default(),
      layout: Axis::Vertical,
      columns: 1,
      label_width: Some(px(140.0)),
      label_text_size: None,
    }
  }
}

pub enum FieldBuilder {
  String(SharedString),
  Element(FieldElementBuilder),
  View(AnyView),
}

impl Default for FieldBuilder {
  fn default() -> Self {
    Self::String(SharedString::default())
  }
}

impl From<AnyView> for FieldBuilder {
  fn from(view: AnyView) -> Self {
    Self::View(view)
  }
}

impl From<&'static str> for FieldBuilder {
  fn from(value: &'static str) -> Self {
    Self::String(value.into())
  }
}

impl From<String> for FieldBuilder {
  fn from(value: String) -> Self {
    Self::String(value.into())
  }
}

impl From<SharedString> for FieldBuilder {
  fn from(value: SharedString) -> Self {
    Self::String(value)
  }
}

impl RenderOnce for FieldBuilder {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    match self {
      FieldBuilder::String(value) => value.into_any_element(),
      FieldBuilder::Element(builder) => builder(window, cx),
      FieldBuilder::View(view) => view.into_any(),
    }
  }
}

#[derive(IntoElement)]
pub struct Field {
  id: ElementId,
  props: FieldProps,
  label: Option<FieldBuilder>,
  label_indent: bool,
  description: Option<FieldBuilder>,
  children: Vec<AnyElement>,
  visible: bool,
  required: bool,
  align_items: Option<AlignItems>,
  col_span: u16,
  col_start: Option<i16>,
  col_end: Option<i16>,
}

impl Default for Field {
  fn default() -> Self {
    Self::new()
  }
}

impl Field {
  pub fn new() -> Self {
    Self {
      id: 0.into(),
      props: FieldProps::default(),
      label: None,
      label_indent: true,
      description: None,
      children: Vec::new(),
      visible: true,
      required: false,
      align_items: None,
      col_span: 1,
      col_start: None,
      col_end: None,
    }
  }

  pub fn label(mut self, label: impl Into<FieldBuilder>) -> Self {
    self.label = Some(label.into());
    self
  }

  pub fn label_indent(mut self, indent: bool) -> Self {
    self.label_indent = indent;
    self
  }

  pub fn label_fn<F, E>(mut self, label: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut Window, &mut App) -> E + 'static,
  {
    self.label = Some(FieldBuilder::Element(Rc::new(move |window, cx| {
      label(window, cx).into_any_element()
    })));
    self
  }

  pub fn description(mut self, description: impl Into<FieldBuilder>) -> Self {
    self.description = Some(description.into());
    self
  }

  pub fn description_fn<F, E>(mut self, description: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut Window, &mut App) -> E + 'static,
  {
    self.description = Some(FieldBuilder::Element(Rc::new(move |window, cx| {
      description(window, cx).into_any_element()
    })));
    self
  }

  pub fn visible(mut self, visible: bool) -> Self {
    self.visible = visible;
    self
  }

  pub fn required(mut self, required: bool) -> Self {
    self.required = required;
    self
  }

  pub(super) fn props(mut self, ix: usize, props: FieldProps) -> Self {
    self.id = ix.into();
    self.props = props;
    self
  }

  pub fn items_start(mut self) -> Self {
    self.align_items = Some(AlignItems::Start);
    self
  }

  pub fn items_end(mut self) -> Self {
    self.align_items = Some(AlignItems::End);
    self
  }

  pub fn items_center(mut self) -> Self {
    self.align_items = Some(AlignItems::Center);
    self
  }

  pub fn col_span(mut self, col_span: u16) -> Self {
    self.col_span = col_span.max(1);
    self
  }

  pub fn col_start(mut self, col_start: i16) -> Self {
    self.col_start = Some(col_start);
    self
  }

  pub fn col_end(mut self, col_end: i16) -> Self {
    self.col_end = Some(col_end);
    self
  }
}

impl ParentElement for Field {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Field {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    if !self.visible {
      return div().into_any_element();
    }

    let layout = self.props.layout;
    let label_width = if layout.is_vertical() {
      None
    } else {
      self.props.label_width
    };
    let has_label = self.label.is_some();
    let reserve_label_space = layout.is_horizontal() && (has_label || self.label_indent);

    #[inline]
    fn wrap_div(layout: Axis) -> Div {
      if layout.is_vertical() {
        v_flex()
      } else {
        h_flex()
      }
    }

    #[inline]
    fn wrap_label(label_width: Option<Pixels>) -> Div {
      div().when_some(label_width, |this, width| this.w(width).flex_shrink_0())
    }

    let outer_gap = match self.props.size {
      Size::Small => px(6.0),
      Size::Medium => px(8.0),
      Size::Large => px(12.0),
    };
    let inner_gap = if layout.is_horizontal() {
      outer_gap / 2.0
    } else {
      outer_gap / 4.0
    };
    let default_label_text_size = self.props.size.smaller().text_size();

    v_flex()
      .flex_1()
      .gap(inner_gap)
      .col_span(self.col_span)
      .when_some(self.col_start, |this, start| this.col_start(start))
      .when_some(self.col_end, |this, end| this.col_end(end))
      .child(
        wrap_div(layout)
          .id(self.id)
          .gap(inner_gap)
          .map(|this| match self.align_items {
            Some(AlignItems::Start) => this.items_start(),
            Some(AlignItems::End) => this.items_end(),
            Some(AlignItems::Center) => this.items_center(),
            Some(AlignItems::Baseline) => this.items_baseline(),
            _ => this,
          })
          .when(reserve_label_space || has_label, |this| {
            this.child(
              wrap_label(label_width)
                .text_size(default_label_text_size)
                .when_some(self.props.label_text_size, |this, size| {
                  this.text_size(size)
                })
                .text_color(cx.theme().foreground)
                .font_medium()
                .gap_1()
                .items_center()
                .when_some(self.label, |this, builder| {
                  this.child(
                    h_flex()
                      .gap_1()
                      .child(div().overflow_x_hidden().child(builder.render(window, cx)))
                      .when(self.required, |this| {
                        this.child(div().text_color(cx.theme().danger).child("*"))
                      }),
                  )
                }),
            )
          })
          .child(
            div()
              .w_full()
              .flex_1()
              .overflow_x_hidden()
              .children(self.children),
          ),
      )
      .child(
        wrap_div(layout)
          .gap(inner_gap)
          .when(reserve_label_space, |this| {
            this.child(wrap_label(label_width))
          })
          .when_some(self.description, |this, builder| {
            this.child(
              div()
                .text_size(default_label_text_size)
                .text_color(cx.theme().muted_foreground)
                .child(builder.render(window, cx)),
            )
          }),
      )
      .into_any_element()
  }
}
