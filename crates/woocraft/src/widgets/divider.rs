//! Visual separator line between content sections.
//!
//! Divider renders a horizontal or vertical line, typically used to visually
//! separate sections of content. Supports solid and dashed line styles,
//! optional center labels (for horizontal dividers), and customizable colors.

use gpui::{
  App, Axis, Div, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce, SharedString,
  StyleRefinement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _, px, relative,
};

use crate::{ActiveTheme, base::StyledExt};

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
/// Divider renders a 1-pixel line to separate content sections. Can be a
/// single horizontal/vertical line, optionally with a centered label
/// (horizontal only).
pub struct Divider {
  base: Div,
  style: StyleRefinement,
  label: Option<SharedString>,
  axis: Axis,
  color: Option<Hsla>,
  line_style: DividerStyle,
}

impl Divider {
  fn render_base(axis: Axis, labeled: bool) -> Div {
    div().map(|this| match axis {
      Axis::Vertical => this.w(px(1.0)).h_full(),
      // labeled horizontal dividers size themselves to the label chip so the
      // absolutely-positioned line behind it never overlaps neighbours.
      Axis::Horizontal if labeled => this.w_full(),
      Axis::Horizontal => this.h(px(1.0)).w_full(),
    })
  }

  /// Creates a new vertical divider with default (solid) style.
  pub fn vertical() -> Self {
    Self {
      base: Self::render_base(Axis::Vertical, false),
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
      base: Self::render_base(Axis::Horizontal, false),
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

  fn render_solid(color: Hsla) -> impl IntoElement {
    div().size_full().bg(color)
  }

  fn render_dashed(axis: Axis, color: Hsla) -> impl IntoElement {
    div().size_full().child(
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
  }
}

impl Styled for Divider {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Divider {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let color = self.color.unwrap_or(cx.theme().border);
    let labeled = self.label.clone().filter(|_| self.axis == Axis::Horizontal);

    let mut base = match labeled {
      Some(label) => {
        // the line runs behind the label chip; the chip's background masks it
        div()
          .w_full()
          .flex()
          .items_center()
          .justify_center()
          .child(
            div()
              .absolute()
              .left_0()
              .right_0()
              .top(relative(0.5))
              .h(px(1.0))
              .bg(color),
          )
          .child(
            div()
              .px_2()
              .bg(cx.theme().background)
              .text_color(cx.theme().muted_foreground)
              .child(label),
          )
      }
      None => Self::render_base(self.axis, false)
        .flex_shrink_0()
        .child(match self.line_style {
          DividerStyle::Solid => Self::render_solid(color).into_any_element(),
          DividerStyle::Dashed => Self::render_dashed(self.axis, color).into_any_element(),
        }),
    };

    base.refine_style(&self.style)
  }
}
