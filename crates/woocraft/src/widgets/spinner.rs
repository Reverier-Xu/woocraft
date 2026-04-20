//! Animated rotating icon for indicating loading or processing state.
//!
//! Spinner renders an animated icon that continuously rotates, commonly
//! displayed while content is loading, processing, or waiting. Fully
//! customizable: change the icon, rotation speed, and color.

use std::time::Duration;

use gpuim::{
  Animation, AnimationExt as _, App, Hsla, IntoElement, ParentElement, RenderOnce, Styled as _,
  Transformation, Window, div, linear, percentage, prelude::FluentBuilder as _,
};

use crate::{Icon, IconName, Sizable, Size, duration};

#[derive(IntoElement)]
/// Animated rotating icon indicating loading or processing state.
///
/// Spinner displays a fullscreen rotation animation. Default icon is
/// `SpinnerIos`. Rotation speed defaults to theme duration (typically 1-2
/// seconds).
pub struct Spinner {
  size: Size,
  icon: Icon,
  speed: Duration,
  color: Option<Hsla>,
}

impl Default for Spinner {
  fn default() -> Self {
    Self::new()
  }
}

impl Spinner {
  /// Creates a new spinner with default icon (SpinnerIos) and size.
  pub fn new() -> Self {
    Self {
      size: Size::Medium,
      speed: duration::SPINNER,
      icon: Icon::new(IconName::SpinnerIos),
      color: None,
    }
  }

  /// Sets the rotating icon (e.g., SpinnerIos, Loader, LoaderCircle).
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = icon.into();
    self
  }

  /// Sets the icon color.
  pub fn color(mut self, color: Hsla) -> Self {
    self.color = Some(color);
    self
  }

  /// Sets the rotation speed (one full rotation per duration).
  pub fn speed(mut self, speed: Duration) -> Self {
    self.speed = speed;
    self
  }
}

impl_sizable!(Spinner);

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
