//! Value interpolation and paint transforms.
//!
//! [`Interpolate`] is what every motion primitive animates: a type that can
//! blend between two application-owned values. [`MotionTransform`] bundles the
//! transforms a component applies while animating so a single sampled value
//! drives translation, scale and opacity together.

use gpui::{Bounds, Hsla, Pixels, Point, Size, point};

/// Blends between two application-owned values.
///
/// This is what every motion primitive animates. The scalar, pixel, point and
/// colour impls cover the values components animate; anything else implements
/// the trait for itself.
pub trait Interpolate: Clone {
  fn interpolate(&self, target: &Self, progress: f32) -> Self;
}

impl Interpolate for f32 {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    self + (target - self) * progress
  }
}

impl Interpolate for Pixels {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    let from: f32 = (*self).into();
    let to: f32 = (*target).into();
    gpui::px(from.interpolate(&to, progress))
  }
}

impl Interpolate for Point<Pixels> {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    point(
      self.x.interpolate(&target.x, progress),
      self.y.interpolate(&target.y, progress),
    )
  }
}

impl Interpolate for Hsla {
  /// Interpolates each channel linearly. Intended for transitions between
  /// near-grayscale UI colours, where hue interpolation is irrelevant.
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    Hsla {
      h: self.h.interpolate(&target.h, progress),
      s: self.s.interpolate(&target.s, progress),
      l: self.l.interpolate(&target.l, progress),
      a: self.a.interpolate(&target.a, progress),
    }
  }
}

impl Interpolate for Size<Pixels> {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    Size::new(
      self.width.interpolate(&target.width, progress),
      self.height.interpolate(&target.height, progress),
    )
  }
}

impl Interpolate for Bounds<Pixels> {
  fn interpolate(&self, target: &Self, progress: f32) -> Self {
    Bounds::new(
      self.origin.interpolate(&target.origin, progress),
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
      translation: self.translation.interpolate(&target.translation, progress),
      scale: gpui::point(
        self.scale.x.interpolate(&target.scale.x, progress),
        self.scale.y.interpolate(&target.scale.y, progress),
      ),
      rotation_radians: self
        .rotation_radians
        .interpolate(&target.rotation_radians, progress),
      opacity: self.opacity.interpolate(&target.opacity, progress),
    }
  }
}
