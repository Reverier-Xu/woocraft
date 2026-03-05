//! Linear progress indicator showing completion status as a percentage.
//!
//! Progress displays a filled bar representing progress from 0% to 100%,
//! typically used during file uploads, downloads, or long-running operations.
//! Includes an optional label and percentage text, with customizable colors.

use std::f32::consts::TAU;

use gpui::{
  AnyElement, App, Hsla, IntoElement, ParentElement, PathBuilder, RenderOnce, SharedString,
  StyleRefinement, Styled, Window, canvas, div, point, px, relative,
};

use crate::{ActiveTheme, Size, StyledExt, h_flex, translate_woocraft};

#[derive(IntoElement)]
/// Linear progress bar showing completion percentage.
///
/// Renders a horizontal bar where the filled portion represents progress
/// (0-100%). Displays an optional label and percentage text above the bar. The
/// track color defaults to a semi-transparent version of the fill color.
pub struct Progress {
  style: StyleRefinement,
  color: Option<Hsla>,
  track_color: Option<Hsla>,
  text_color: Option<Hsla>,
  label: SharedString,
  value: f32,
  size: Size,
}

impl Default for Progress {
  fn default() -> Self {
    Self::new()
  }
}

impl Progress {
  /// Creates a new progress bar with 0% completion and default "Loading..."
  /// label.
  pub fn new() -> Self {
    Self {
      value: 0.0,
      color: None,
      track_color: None,
      text_color: None,
      label: translate_woocraft("common.loading").into(),
      style: StyleRefinement::default(),
      size: Size::default(),
    }
  }

  /// Sets the fill color (defaults to theme primary).
  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }

  /// Sets the current progress as a percentage (0-100, clamped automatically).
  pub fn value(mut self, value: f32) -> Self {
    self.value = value.clamp(0.0, 100.0);
    self
  }

  /// Sets the label text displayed above the progress bar.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = label.into();
    self
  }

  /// Sets the background/track color (defaults to fill color with 20% opacity).
  pub fn track_color(mut self, color: impl Into<Hsla>) -> Self {
    self.track_color = Some(color.into());
    self
  }

  /// Sets the text color for label and percentage (defaults to theme muted
  /// foreground).
  pub fn text_color(mut self, color: impl Into<Hsla>) -> Self {
    self.text_color = Some(color.into());
    self
  }
}

impl_styled!(Progress);
impl_sizable!(Progress);

impl RenderOnce for Progress {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let color = self.color.unwrap_or(cx.theme().primary);
    let track_color = self.track_color.unwrap_or(color.opacity(0.2));
    let text_color = self.text_color.unwrap_or(cx.theme().muted_foreground);
    let text_size = self.size.text_size();
    let progress = format!("{:.0}%", self.value);
    let line_h = self.size.track_thickness();
    let label_gap = self.size.component_gap();

    div()
      .w_full()
      .relative()
      .h(text_size + label_gap + line_h)
      .refine_style(&self.style)
      .child(
        h_flex()
          .absolute()
          .top_0()
          .left_0()
          .right_0()
          .h(text_size)
          .items_center()
          .justify_between()
          .text_size(text_size)
          .text_color(text_color)
          .child(self.label)
          .child(progress),
      )
      .child(
        div()
          .absolute()
          .bottom_0()
          .left_0()
          .right_0()
          .h(line_h)
          .rounded(line_h / 2.0)
          .bg(track_color),
      )
      .child(
        div()
          .absolute()
          .bottom_0()
          .left_0()
          .h(line_h)
          .rounded(line_h / 2.0)
          .bg(color)
          .w(relative(self.value / 100.0)),
      )
  }
}

#[derive(IntoElement)]
pub struct ProgressCircle {
  style: StyleRefinement,
  color: Option<Hsla>,
  track_color: Option<Hsla>,
  text_color: Option<Hsla>,
  value: f32,
  size: Size,
  children: Vec<AnyElement>,
}

impl Default for ProgressCircle {
  fn default() -> Self {
    Self::new()
  }
}

impl ProgressCircle {
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      color: None,
      track_color: None,
      text_color: None,
      value: 0.0,
      size: Size::default(),
      children: Vec::new(),
    }
  }

  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }

  pub fn track_color(mut self, color: impl Into<Hsla>) -> Self {
    self.track_color = Some(color.into());
    self
  }

  pub fn text_color(mut self, color: impl Into<Hsla>) -> Self {
    self.text_color = Some(color.into());
    self
  }

  pub fn value(mut self, value: f32) -> Self {
    self.value = value.clamp(0.0, 100.0);
    self
  }
}

impl_parent_element!(ProgressCircle);
impl_styled!(ProgressCircle);
impl_sizable!(ProgressCircle);

impl RenderOnce for ProgressCircle {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let color = self.color.unwrap_or(cx.theme().primary);
    let track_color = self.track_color.unwrap_or(color.opacity(0.2));
    let text_color = self.text_color.unwrap_or(cx.theme().muted_foreground);
    let percentage_text = format!("{:.0}%", self.value);

    let diameter = self.size.circle_diameter();
    let stroke = self.size.stroke_width();

    let progress = (self.value / 100.0).clamp(0.0, 1.0);

    div()
      .relative()
      .size(diameter)
      .items_center()
      .justify_center()
      .refine_style(&self.style)
      .child(
        canvas(
          move |_, _, _| {},
          move |bounds, _, window, _| {
            let center = point(
              bounds.origin.x + bounds.size.width / 2.0,
              bounds.origin.y + bounds.size.height / 2.0,
            );
            let radius = (bounds.size.width.min(bounds.size.height) / 2.0) - stroke / 2.0;

            let mut bg = PathBuilder::stroke(stroke);
            let segments = 64;
            for idx in 0..=segments {
              let t = idx as f32 / segments as f32;
              let angle = -TAU / 4.0 + TAU * t;
              let p = point(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
              );
              if idx == 0 {
                bg.move_to(p);
              } else {
                bg.line_to(p);
              }
            }
            if let Ok(path) = bg.build() {
              window.paint_path(path, track_color);
            }

            if progress > 0.0 {
              let mut fg = PathBuilder::stroke(stroke);
              let steps = ((segments as f32) * progress).ceil() as usize;
              for idx in 0..=steps {
                let t = idx as f32 / segments as f32;
                let angle = -TAU / 4.0 + TAU * t;
                let p = point(
                  center.x + radius * angle.cos(),
                  center.y + radius * angle.sin(),
                );
                if idx == 0 {
                  fg.move_to(p);
                } else {
                  fg.line_to(p);
                }
              }

              if let Ok(path) = fg.build() {
                window.paint_path(path, color);
              }
            }
          },
        )
        .absolute()
        .size_full(),
      )
      .child(
        h_flex()
          .absolute()
          .size_full()
          .items_center()
          .justify_center()
          .text_size(self.size.text_size().min(px(14.0)))
          .text_color(text_color)
          .child(percentage_text),
      )
      .children(self.children)
  }
}
