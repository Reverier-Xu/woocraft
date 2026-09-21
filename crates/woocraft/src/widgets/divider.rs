//! Visual separator line between content sections.
//!
//! Divider renders a horizontal or vertical line, typically used to visually
//! separate sections of content. Supports solid and dashed line styles,
//! optional center labels (for horizontal dividers), and customizable colors.

use gpui::{
  App, Axis, Div, Hsla, IntoElement, ParentElement, PathBuilder, Pixels, RenderOnce, SharedString,
  StyleRefinement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _, rems,
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
  style: StyleRefinement,
  label: Option<SharedString>,
  axis: Axis,
  color: Option<Hsla>,
  line_style: DividerStyle,
}

impl Divider {
  fn render_base(axis: Axis, labeled: bool, thickness: Pixels) -> Div {
    div().map(|this| match axis {
      Axis::Vertical => this.w(thickness).h_full(),
      // labeled horizontal dividers size themselves to the label chip so the
      // absolutely-positioned line behind it never overlaps neighbours.
      Axis::Horizontal if labeled => this.w_full(),
      Axis::Horizontal => this.h(thickness).w_full(),
    })
  }

  /// Creates a new vertical divider with default (solid) style.
  pub fn vertical() -> Self {
    Self {
      style: StyleRefinement::default(),
      label: None,
      axis: Axis::Vertical,
      color: None,
      line_style: DividerStyle::Solid,
    }
  }

  /// Creates a new horizontal divider with default (solid) style.
  pub fn horizontal() -> Self {
    Self {
      style: StyleRefinement::default(),
      label: None,
      axis: Axis::Horizontal,
      color: None,
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

  fn render_dashed(axis: Axis, color: Hsla, thickness: Pixels) -> impl IntoElement {
    div().size_full().child(
      canvas(
        move |_, _, _| {},
        move |bounds, _, window, _| {
          // the dash rhythm is rem-driven like every other size: a quarter
          // rem on, an eighth off.
          let rem = window.rem_size();
          let dash = rems(0.25).to_pixels(rem);
          let gap = rems(0.125).to_pixels(rem);
          let mut builder = PathBuilder::stroke(thickness).dash_array(&[dash, gap]);
          let half = thickness / 2.;
          let (start, end) = match axis {
            Axis::Horizontal => {
              let x = bounds.origin.x;
              let y = bounds.origin.y + half;
              (point(x, y), point(x + bounds.size.width, y))
            }
            Axis::Vertical => {
              let x = bounds.origin.x + half;
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
    let thickness = cx.theme().border_width;
    let labeled = self.label.clone().filter(|_| self.axis == Axis::Horizontal);

    let base = match labeled {
      Some(label) => {
        // classic divider-with-text: two line segments with the label between
        // them — no background chip, the container sizes to the label.
        div()
          .w_full()
          .flex()
          .items_center()
          .gap(rems(0.5))
          .child(div().flex_1().h(thickness).bg(color))
          .child(div().text_color(cx.theme().muted_foreground).child(label))
          .child(div().flex_1().h(thickness).bg(color))
      }
      None => Self::render_base(self.axis, false, thickness)
        .flex_shrink_0()
        .child(match self.line_style {
          DividerStyle::Solid => Self::render_solid(color).into_any_element(),
          DividerStyle::Dashed => {
            Self::render_dashed(self.axis, color, thickness).into_any_element()
          }
        }),
    };

    base.refine_style(&self.style)
  }
}
