use std::time::Duration;

use gpui::{
  Animation, AnimationExt as _, App, Hsla, IntoElement, RenderOnce, Transformation, Window, div,
  linear, percentage, ParentElement, Styled as _, prelude::FluentBuilder as _,
};

use crate::{Icon, IconName, Sizable, Size};

#[derive(IntoElement)]
pub struct Spinner {
  size: Size,
  icon: Icon,
  speed: Duration,
  color: Option<Hsla>,
}

impl Spinner {
  pub fn new() -> Self {
    Self {
      size: Size::Medium,
      speed: Duration::from_secs_f64(0.8),
      icon: Icon::new(IconName::SpinnerIos),
      color: None,
    }
  }

  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = icon.into();
    self
  }

  pub fn color(mut self, color: Hsla) -> Self {
    self.color = Some(color);
    self
  }

  pub fn speed(mut self, speed: Duration) -> Self {
    self.speed = speed;
    self
  }
}

impl Sizable for Spinner {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl RenderOnce for Spinner {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let icon = self
      .icon
      .with_size(self.size)
      .when_some(self.color, |this, color| this.text_color(color));

    div().child(icon.with_animation(
      "spinner-rotate",
      Animation::new(self.speed).repeat().with_easing(linear),
      |this, delta| this.transform(Transformation::rotate(percentage(delta))),
    ))
  }
}
