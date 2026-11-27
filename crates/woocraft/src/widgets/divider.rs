//! Visual separator line between content sections.
//!
//! Divider renders a horizontal or vertical line, typically used to visually
//! separate sections of content. Supports solid and dashed line styles,
//! optional center labels (for horizontal dividers), and customizable colors.

use gpui::{
  AnyElement, App, Axis, Div, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce,
  SharedString, StyleRefinement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _,
  px,
};

use crate::{ActiveTheme, Size, StyledExt, h_flex};

/// Line style for divider rendering.
#[derive(Clone, Copy, PartialEq, Default)]
pub enum DividerStyle {
  #[default]
  /// Solid continuous line.
  Solid,
  /// Dashed line with gaps.
  Dashed,
}

#[derive(IntoElement)]
/// Visual separator line (horizontal or vertical).
///
/// Divider renders a 1-pixel line to separate content sections. Can be single
/// horizontal/vertical line, optionally with a centered label (horizontal
/// only).
pub struct Divider {
  base: Div,
  style: StyleRefinement,
  label: Option<SharedString>,
  axis: Axis,
  color: Option<Hsla>,
  line_style: DividerStyle,
}

impl Divider {
  fn render_base(axis: Axis) -> Div {
    div().map(|this| match axis {
      Axis::Vertical => this.w(px(1.0)).h_full(),
      Axis::Horizontal => this.h(px(1.0)).w_full(),
    })
  }

  /// Creates a new vertical divider with default (solid) style.
  pub fn vertical() -> Self {
    Self {
      base: Self::render_base(Axis::Vertical),
      axis: Axis::Vertical,
      label: None,
      color: None,
      style: StyleRefinement::default(),
      line_style: DividerStyle::Solid,
    }
  }

  /// Creates a new horizontal divider with default (solid) style.
  pub fn horizontal() -> Self {
    Self {
      base: Self::render_base(Axis::Horizontal),
      axis: Axis::Horizontal,
      label: None,
      color: None,
      style: StyleRefinement::default(),
      line_style: DividerStyle::Solid,
    }
  }

  /// Creates a new vertical divider with dashed style.
  pub fn vertical_dashed() -> Self {
    Self::vertical().dashed()
  }

  /// Creates a new horizontal divider with dashed style.
  pub fn horizontal_dashed() -> Self {
    Self::horizontal().dashed()
  }

  /// Sets an optional centered label for the divider (horizontal only).
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// Sets the divider line color (defaults to theme border).
  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }

  /// Switches the line style to dashed.
  pub fn dashed(mut self) -> Self {
    self.line_style = DividerStyle::Dashed;
    self
  }

  /// Renders one line segment that fills the box it is given, so the same
  /// primitive works both for a standalone divider and for the segments that
  /// flank a divider label.
  fn render_line(axis: Axis, line_style: DividerStyle, color: Hsla) -> AnyElement {
    match line_style {
      DividerStyle::Solid => div().size_full().bg(color).into_any_element(),
      DividerStyle::Dashed => div()
        .size_full()
        .child(
          canvas(
            move |_, _, _| {},
            move |bounds, _, window, _| {
              let mut builder = PathBuilder::stroke(px(1.0)).dash_array(&[px(4.0), px(2.0)]);
              let (start, end) = match axis {
                Axis::Horizontal => {
                  let x = bounds.origin.x;
                  let y = bounds.origin.y + px(0.5);
                  (point(x, y), point(x + bounds.size.width, y))
                }
                Axis::Vertical => {
                  let x = bounds.origin.x + px(0.5);
                  let y = bounds.origin.y;
                  (point(x, y), point(x, y + bounds.size.height))
                }
              };

              builder.move_to(start);
              builder.line_to(end);
              if let Ok(line) = builder.build() {
                window.paint_path(line, color);
              }
            },
          )
          .size_full(),
        )
        .into_any_element(),
    }
  }
}

impl_styled!(Divider);

impl RenderOnce for Divider {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let color = self.color.unwrap_or(cx.theme().border);
    let axis = self.axis;
    let line_style = self.line_style;

    // A label is only meaningful for a horizontal divider. When present, the
    // line is split into two segments around the label so the label takes part
    // in layout; the fixed 1px-high base box of a plain divider would let the
    // label overflow and overlap whatever follows it.
    let label = self.label.filter(|_| matches!(axis, Axis::Horizontal));

    if let Some(label) = label {
      let segment = || {
        div()
          .flex_1()
          .min_w_0()
          .h(px(1.0))
          .child(Self::render_line(axis, line_style, color))
      };

      return h_flex()
        .w_full()
        .flex_shrink_0()
        .refine_style(&self.style)
        .child(segment())
        .child(
          div()
            .flex_shrink_0()
            .px_2()
            .py_1()
            .text_size(Size::Medium.text_size())
            .text_color(cx.theme().muted_foreground)
            .child(label),
        )
        .child(segment())
        .into_any_element();
    }

    self
      .base
      .flex_shrink_0()
      .refine_style(&self.style)
      .child(Self::render_line(axis, line_style, color))
      .into_any_element()
  }
}
