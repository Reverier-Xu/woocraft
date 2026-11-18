//! Value interpolation and paint transforms.
//!
//! [`Interpolate`] is what every motion primitive animates: a type that can
//! blend between two application-owned values. [`MotionTransform`] bundles the
//! transforms a component applies while animating so a single sampled value
//! drives translation, scale and opacity together.

use gpui::{Bounds, Pixels, Size};

use crate::base::animation::Lerp;

pub trait Interpolate: Clone {
  fn interpolate(&self, target: &Self, progress: f32) -> Self;
}

impl<T: Lerp> Interpolate for T {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    self.lerp(target, progress)
  }
}

impl Interpolate for Size<Pixels> {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    Size::new(
      self.width.lerp(&target.width, progress),
      self.height.lerp(&target.height, progress),
    )
  }
}

impl Interpolate for Bounds<Pixels> {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    Bounds::new(
      self.origin.lerp(&target.origin, progress),
      self.size.interpolate(&target.size, progress),
    )
  }
}

/// A presentation-neutral bundle for coordinated paint transforms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionTransform {
  pub translation: gpui::Point<Pixels>,
  pub scale: gpui::Point<f32>,
  pub rotation_radians: f32,
  pub opacity: f32,
}

impl MotionTransform {
  pub fn identity() -> Self {
    Self {
      translation: gpui::point(gpui::px(0.0), gpui::px(0.0)),
      scale: gpui::point(1.0, 1.0),
      rotation_radians: 0.0,
      opacity: 1.0,
    }
  }
}

impl Default for MotionTransform {
  fn default() -> Self {
    Self::identity()
  }
}

impl Interpolate for MotionTransform {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    Self {
      translation: self.translation.lerp(&target.translation, progress),
      scale: gpui::point(
        self.scale.x.lerp(&target.scale.x, progress),
        self.scale.y.lerp(&target.scale.y, progress),
      ),
      rotation_radians: self
        .rotation_radians
        .lerp(&target.rotation_radians, progress),
      opacity: self.opacity.lerp(&target.opacity, progress),
    }
  }
}
