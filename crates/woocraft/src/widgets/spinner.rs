//! Animated rotating icon for indicating loading or processing state.
//!
//! Spinner renders an animated icon that continuously rotates, commonly
//! displayed while content is loading, processing, or waiting. The rotation
//! speed follows the theme spinner duration token.

use std::time::Duration;

use gpui::{
  Animation, AnimationExt as _, App, Hsla, IntoElement, ParentElement, RenderOnce, StyleRefinement,
  Styled, Transformation, Window, div, linear, percentage, prelude::FluentBuilder as _,
};

use crate::{Icon, IconName, base::StyledExt, theme::duration};

#[derive(IntoElement)]
/// Animated rotating icon indicating loading or processing state.
///
/// The icon renders at the surrounding text size (`1rem`). The default icon
/// is [`IconName::SpinnerIos`] and one rotation takes the theme spinner
/// duration. every spinner shares one animation clock, so concurrent
/// spinners rotate in lockstep.
pub struct Spinner {
  icon: Icon,
  speed: Duration,
  color: Option<Hsla>,
  style: StyleRefinement,
}

impl Default for Spinner {
  fn default() -> Self {
    Self::new()
  }
}

impl Spinner {
  /// Creates a new spinner with the default icon and speed.
  pub fn new() -> Self {
    Self {
      speed: duration::SPINNER,
      icon: Icon::new(IconName::SpinnerIos),
      color: None,
      style: StyleRefinement::default(),
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

impl Styled for Spinner {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Spinner {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let icon = self
      .icon
      .when_some(self.color, |this, color| this.text_color(color));

    div()
      .child(icon.with_animation(
        "spinner-rotate",
        Animation::new(self.speed).repeat().with_easing(linear),
        |this, delta| this.transform(Transformation::rotate(percentage(delta))),
      ))
      .refine_style(&self.style)
  }
}
