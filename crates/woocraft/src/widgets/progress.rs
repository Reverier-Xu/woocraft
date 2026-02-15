use gpui::{
  App, Hsla, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement, Styled, Window,
  div, relative,
};

use crate::{ActiveTheme, Sizable, Size, StyledExt};

#[derive(IntoElement)]
pub struct Progress {
  style: StyleRefinement,
  color: Option<Hsla>,
  track_color: Option<Hsla>,
  text_color: Option<Hsla>,
  label: SharedString,
  value: f32,
  size: Size,
}

impl Progress {
  pub fn new() -> Self {
    Self {
      value: 0.0,
      color: None,
      track_color: None,
      text_color: None,
      label: rust_i18n::t!("common.loading").into(),
      style: StyleRefinement::default(),
      size: Size::default(),
    }
  }

  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }

  pub fn value(mut self, value: f32) -> Self {
    self.value = value.clamp(0.0, 100.0);
    self
  }

  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = label.into();
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
}

impl Styled for Progress {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl Sizable for Progress {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl RenderOnce for Progress {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let color = self.color.unwrap_or(cx.theme().primary);
    let track_color = self.track_color.unwrap_or(color.opacity(0.2));
    let text_color = self.text_color.unwrap_or(cx.theme().muted_foreground);
    let text_size = self.size.text_size();
    let progress = format!("{:.0}%", self.value);
    let line_h = gpui::px(2.0);
    let label_gap = gpui::px(6.0);

    div()
      .w_full()
      .relative()
      .h(text_size + label_gap + line_h)
      .refine_style(&self.style)
      .child(
        div()
          .absolute()
          .top_0()
          .left_0()
          .right_0()
          .h(text_size)
          .h_flex()
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
