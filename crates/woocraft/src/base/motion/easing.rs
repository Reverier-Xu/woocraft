//! CSS-compatible easing policies.

use std::{fmt, num::NonZeroU32, rc::Rc, sync::Arc};

/// A cubic bezier function like CSS `cubic-bezier`.
///
/// Builder:
///
/// https://cubic-bezier.com
pub(crate) fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> impl Fn(f32) -> f32 {
  // Polynomial form of the unit bezier, where p0 = (0, 0) and p3 = (1, 1).
  let (cx, cy) = (3.0 * x1, 3.0 * y1);
  let (bx, by) = (3.0 * (x2 - x1) - cx, 3.0 * (y2 - y1) - cy);
  let (ax, ay) = (1.0 - cx - bx, 1.0 - cy - by);
  let sample_x = move |t: f32| ((ax * t + bx) * t + cx) * t;
  let sample_y = move |t: f32| ((ay * t + by) * t + cy) * t;
  let slope_x = move |t: f32| (3.0 * ax * t + 2.0 * bx) * t + cx;

  // Solve `x(s) = t` for the curve parameter `s`.
  let solve_s = move |t: f32| {
    let mut s = t;
    for _ in 0..8 {
      let error = sample_x(s) - t;
      if error.abs() < 1e-6 {
        return s;
      }
      let slope = slope_x(s);
      if slope.abs() < 1e-6 {
        break;
      }
      s = (s - error / slope).clamp(0.0, 1.0);
    }

    let (mut low, mut high) = (0.0, 1.0);
    let mut s = t;
    for _ in 0..32 {
      let x = sample_x(s);
      if (x - t).abs() < 1e-6 {
        break;
      }
      if x < t {
        low = s;
      } else {
        high = s;
      }
      s = (low + high) / 2.0;
    }
    s
  };

  move |t: f32| {
    let t = t.clamp(0.0, 1.0);
    // `t` is elapsed progress along x, not the curve parameter: solve
    // `x(s) = t` before sampling y, otherwise the curve reads much slower
    // than the same control points do in CSS. GPUI asserts easing deltas
    // stay within [0, 1], so clamp away solver and rounding error.
    sample_y(solve_s(t)).clamp(0.0, 1.0)
  }
}

/// Cubic ease-out — fast start, slow end.
///
/// The default curve for [`super::Transition`]: it settles into place without
/// overshooting, which reads as an entrance rather than a slide.
pub(crate) fn ease_out_cubic(t: f32) -> f32 {
  let t = t.clamp(0.0, 1.0);
  1.0 - (1.0 - t).powi(3)
}

/// The point at which a stepped easing jumps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StepPosition {
  JumpStart,
  JumpEnd,
  JumpNone,
  JumpBoth,
}

/// One output and its optional input position in a CSS-like `linear()` curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearStop {
  pub output: f32,
  pub input: Option<f32>,
}

impl LinearStop {
  pub const fn new(output: f32) -> Self {
    Self {
      output,
      input: None,
    }
  }

  pub const fn at(output: f32, input: f32) -> Self {
    Self {
      output,
      input: Some(input),
    }
  }
}

/// Invalid easing configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EasingError {
  InvalidBezierControlPoint,
  InvalidStepCount,
  InvalidLinearStops,
}

impl fmt::Display for EasingError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::InvalidBezierControlPoint => {
        f.write_str("cubic Bézier control points must be finite and x must be within 0..=1")
      }
      Self::InvalidStepCount => f.write_str("step easing requires a valid step count"),
      Self::InvalidLinearStops => f.write_str("linear easing stops are invalid"),
    }
  }
}

impl std::error::Error for EasingError {}

/// A cheap, cloneable CSS-compatible easing policy.
#[derive(Clone, Default)]
pub enum Easing {
  Linear,
  Ease,
  EaseIn,
  #[default]
  EaseOut,
  EaseInOut,
  CubicBezier {
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
  },
  Steps {
    count: NonZeroU32,
    position: StepPosition,
  },
  LinearStops(Arc<[(f32, f32)]>),
  Custom(Rc<dyn Fn(f32) -> f32>),
}

impl fmt::Debug for Easing {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Linear => f.write_str("Linear"),
      Self::Ease => f.write_str("Ease"),
      Self::EaseIn => f.write_str("EaseIn"),
      Self::EaseOut => f.write_str("EaseOut"),
      Self::EaseInOut => f.write_str("EaseInOut"),
      Self::CubicBezier { x1, y1, x2, y2 } => f
        .debug_struct("CubicBezier")
        .field("x1", x1)
        .field("y1", y1)
        .field("x2", x2)
        .field("y2", y2)
        .finish(),
      Self::Steps { count, position } => f
        .debug_struct("Steps")
        .field("count", count)
        .field("position", position)
        .finish(),
      Self::LinearStops(stops) => f.debug_tuple("LinearStops").field(stops).finish(),
      Self::Custom(_) => f.write_str("Custom(..)"),
    }
  }
}

impl Easing {
  pub fn cubic_bezier(x1: f32, y1: f32, x2: f32, y2: f32) -> Result<Self, EasingError> {
    if !x1.is_finite()
      || !x2.is_finite()
      || !(0.0..=1.0).contains(&x1)
      || !(0.0..=1.0).contains(&x2)
      || !y1.is_finite()
      || !y2.is_finite()
    {
      return Err(EasingError::InvalidBezierControlPoint);
    }
    Ok(Self::CubicBezier { x1, y1, x2, y2 })
  }

  pub fn steps(count: u32, position: StepPosition) -> Result<Self, EasingError> {
    let count = NonZeroU32::new(count).ok_or(EasingError::InvalidStepCount)?;
    if position == StepPosition::JumpNone && count.get() == 1 {
      return Err(EasingError::InvalidStepCount);
    }
    Ok(Self::Steps { count, position })
  }

  pub fn linear_stops(stops: impl IntoIterator<Item = LinearStop>) -> Result<Self, EasingError> {
    let mut stops: Vec<_> = stops.into_iter().collect();
    if stops.len() < 2
      || stops
        .iter()
        .any(|stop| !stop.output.is_finite() || stop.input.is_some_and(|v| !v.is_finite()))
    {
      return Err(EasingError::InvalidLinearStops);
    }

    if stops[0].input.is_none() {
      stops[0].input = Some(0.0);
    }
    let last = stops.len() - 1;
    if stops[last].input.is_none() {
      stops[last].input = Some(1.0);
    }

    let mut anchor = 0;
    while anchor < last {
      let Some(next) = ((anchor + 1)..=last).find(|&ix| stops[ix].input.is_some()) else {
        return Err(EasingError::InvalidLinearStops);
      };
      let from = stops[anchor].input.unwrap();
      let to = stops[next].input.unwrap();
      if !(0.0..=1.0).contains(&from) || !(0.0..=1.0).contains(&to) || to < from {
        return Err(EasingError::InvalidLinearStops);
      }
      let span = (next - anchor) as f32;
      for (offset, stop) in stops[(anchor + 1)..next].iter_mut().enumerate() {
        stop.input = Some(from + (to - from) * (offset + 1) as f32 / span);
      }
      anchor = next;
    }

    Ok(Self::LinearStops(
      stops
        .into_iter()
        .map(|stop| (stop.input.unwrap(), stop.output))
        .collect(),
    ))
  }

  #[inline]
  pub fn sample(&self, progress: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    match self {
      Self::Linear => progress,
      Self::Ease => cubic_bezier(0.25, 0.1, 0.25, 1.0)(progress),
      Self::EaseIn => cubic_bezier(0.42, 0.0, 1.0, 1.0)(progress),
      Self::EaseOut => cubic_bezier(0.0, 0.0, 0.58, 1.0)(progress),
      Self::EaseInOut => cubic_bezier(0.42, 0.0, 0.58, 1.0)(progress),
      Self::CubicBezier { x1, y1, x2, y2 } => cubic_bezier(*x1, *y1, *x2, *y2)(progress),
      Self::Steps { count, position } => {
        let count = count.get() as f32;
        let (jumps, offset) = match position {
          StepPosition::JumpStart => (count, 1.0),
          StepPosition::JumpEnd => (count, 0.0),
          StepPosition::JumpNone => (count - 1.0, 0.0),
          StepPosition::JumpBoth => (count + 1.0, 1.0),
        };
        ((progress * count).floor() + offset).clamp(0.0, jumps) / jumps
      }
      Self::LinearStops(stops) => {
        let upper = stops.partition_point(|(input, _)| *input <= progress);
        if upper == 0 {
          return stops[0].1;
        }
        if upper == stops.len() {
          return stops[stops.len() - 1].1;
        }
        let (x0, y0) = stops[upper - 1];
        let (x1, y1) = stops[upper];
        if x0 == x1 {
          y1
        } else {
          y0 + (y1 - y0) * ((progress - x0) / (x1 - x0))
        }
      }
      Self::Custom(easing) => easing(progress),
    }
  }
}

#[cfg(test)]
mod error_tests {
  use super::*;

  #[test]
  fn bezier_errors_name_the_invalid_control_point() {
    let error = Easing::cubic_bezier(0.2, f32::NAN, 0.8, 1.0).unwrap_err();

    assert_eq!(error, EasingError::InvalidBezierControlPoint);
    assert_eq!(
      error.to_string(),
      "cubic Bézier control points must be finite and x must be within 0..=1"
    );
    let _: &dyn std::error::Error = &error;
  }

  use super::cubic_bezier;

  #[test]
  fn cubic_bezier_matches_engine_published_values() {
    // Reference values sampled from the CSS `ease` curve.
    let ease = cubic_bezier(0.25, 0.1, 0.25, 1.);
    for (t, expected) in [
      (0.0, 0.0),
      (0.2, 0.295),
      (0.5, 0.802),
      (0.8, 0.976),
      (1.0, 1.0),
    ] {
      assert!(
        (ease(t) - expected).abs() < 1e-3,
        "ease({t}) = {}, expected {expected}",
        ease(t)
      );
    }

    // Chromium's CubicBezier(0.25, 0, 0.75, 1) expectations, from
    // ui/gfx/geometry/cubic_bezier_unittest.cc (epsilon 0.00015 there;
    // widened for our f32 arithmetic).
    let curve = cubic_bezier(0.25, 0., 0.75, 1.);
    for (t, expected) in [
      (0.05, 0.01136),
      (0.1, 0.03978),
      (0.15, 0.07978),
      (0.2, 0.12803),
      (0.25, 0.18235),
      (0.3, 0.24115),
      (0.35, 0.30323),
      (0.4, 0.36761),
      (0.45, 0.43345),
      (0.5, 0.5),
      (0.6, 0.63238),
      (0.65, 0.69676),
      (0.7, 0.75884),
      (0.75, 0.81764),
      (0.8, 0.87196),
      (0.85, 0.92021),
      (0.9, 0.96021),
      (0.95, 0.98863),
    ] {
      assert!(
        (curve(t) - expected).abs() < 3e-4,
        "curve({t}) = {}, Chromium says {expected}",
        curve(t)
      );
    }
  }

  #[test]
  fn cubic_bezier_with_thirds_x_maps_time_identically() {
    // x1 = 1/3, x2 = 2/3 collapse the x solve to the identity, making the
    // output the plain y polynomial.
    let ease = cubic_bezier(1. / 3., 0.72, 2. / 3., 1.);
    for step in 0..=100 {
      let t = step as f32 / 100.;
      let one_t = 1. - t;
      let expected = 3. * 0.72 * one_t * one_t * t + 3. * one_t * t * t + t * t * t;
      assert!(
        (ease(t) - expected).abs() < 1e-4,
        "ease({t}) = {}, expected {expected}",
        ease(t)
      );
    }
  }

  #[test]
  fn cubic_bezier_matches_the_css_definition() {
    // Reference solver written straight off the CSS Easing Functions
    // definition — solve `x(s) = t` by bisection in f64, then sample
    // `y(s)` — independent of the production Newton solver.
    fn css_reference(x1: f64, y1: f64, x2: f64, y2: f64, t: f64) -> f64 {
      let sample = |p1: f64, p2: f64, s: f64| {
        3. * p1 * (1. - s) * (1. - s) * s + 3. * p2 * (1. - s) * s * s + s * s * s
      };
      let (mut low, mut high) = (0f64, 1f64);
      for _ in 0..64 {
        let mid = (low + high) / 2.;
        if sample(x1, x2, mid) < t {
          low = mid;
        } else {
          high = mid;
        }
      }
      sample(y1, y2, (low + high) / 2.)
    }

    let curves = [
      // The runtime curves in this repo.
      (0.25, 0.1, 0.25, 1.),
      (1. / 3., 0.72, 2. / 3., 1.),
      // CSS keyword curves.
      (0.42, 0., 1., 1.),
      (0., 0., 0.58, 1.),
      (0.42, 0., 0.58, 1.),
      (0., 0., 1., 1.),
      // Degenerate x slopes: zero at s = 0.5, 0, and 1, forcing the
      // Newton solve to fall back to bisection.
      (1., 0., 0., 1.),
      (0., 0., 0., 1.),
      (1., 0.5, 1., 0.5),
    ];
    for (x1, y1, x2, y2) in curves {
      let ease = cubic_bezier(x1, y1, x2, y2);
      for step in 0..=1000 {
        let t = step as f32 / 1000.;
        let y = ease(t);
        let expected = css_reference(x1 as f64, y1 as f64, x2 as f64, y2 as f64, t as f64) as f32;
        assert!(
          (y - expected).abs() < 5e-4,
          "cubic_bezier({x1}, {y1}, {x2}, {y2})({t}) = {y}, CSS = {expected}"
        );
        // GPUI panics when an easing delta leaves [0, 1].
        assert!((0.0..=1.0).contains(&y), "ease({t}) = {y} out of range");
      }
    }
  }

  #[test]
  fn cubic_bezier_is_monotonic_and_clamped() {
    let ease = cubic_bezier(0.32, 0.72, 0., 1.);
    assert_eq!(ease(-1.), 0.);
    assert_eq!(ease(2.), 1.);

    let mut previous = 0.;
    for step in 0..=100 {
      let current = ease(step as f32 / 100.);
      assert!(current >= previous - 1e-4, "not monotonic at {step}");
      previous = current;
    }
  }
}
