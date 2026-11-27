//! HCT tone: CIE L\*, the axis that makes contrast predictable.

use std::{
  fmt,
  ops::{Add, Sub},
};

/// A CIE L\* value, in `0.0..=100.0`, named "tone" as in HCT.
///
/// Tone is a fixed function of relative luminance `Y`:
///
/// ```text
/// Y = 100 * f((T + 16) / 116),  f(t) = t^3 if t^3 > 216/24389 else (116t - 16) / (24389/27)
/// ```
///
/// That is the whole reason HCT can state contrast in its coordinates: because
/// `Y` and tone determine each other, a difference in tone determines a
/// contrast ratio regardless of hue or chroma. A tone difference of 40
/// guarantees at least 3:1, and 50 guarantees at least 4.5:1.
///
/// The type exists so a tone cannot be confused with a chroma or an angle. It
/// is `Copy`, ordered, and clamps on construction.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Tone(f64);

impl Tone {
  /// Smallest representable tone.
  pub const MIN: Self = Self(0.0);
  /// Largest representable tone.
  pub const MAX: Self = Self(100.0);

  /// A tone clamped into `0.0..=100.0`.
  ///
  /// Non-finite input becomes [`Tone::MIN`], so a division by zero upstream
  /// cannot smuggle a `NaN` into a palette.
  pub fn new(value: f64) -> Self {
    if value.is_finite() {
      Self(value.clamp(0.0, 100.0))
    } else {
      Self::MIN
    }
  }

  /// The tone as a plain number.
  pub const fn value(self) -> f64 {
    self.0
  }

  /// Relative luminance `Y`, in `0.0..=100.0`, for this tone.
  pub fn luminance(self) -> f64 {
    y_from_tone(self.0)
  }

  /// The tone of a relative luminance `Y`, in `0.0..=100.0`.
  pub fn from_luminance(y: f64) -> Self {
    Self::new(tone_from_y(y))
  }

  /// The signed distance to `other`, in tone units.
  pub fn distance_to(self, other: Self) -> f64 {
    other.0 - self.0
  }
}

impl Default for Tone {
  fn default() -> Self {
    Self::new(50.0)
  }
}

impl fmt::Display for Tone {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

impl From<f64> for Tone {
  fn from(value: f64) -> Self {
    Self::new(value)
  }
}

impl From<Tone> for f64 {
  fn from(tone: Tone) -> Self {
    tone.0
  }
}

impl Add<f64> for Tone {
  type Output = Self;

  fn add(self, rhs: f64) -> Self {
    Self::new(self.0 + rhs)
  }
}

impl Sub<f64> for Tone {
  type Output = Self;

  fn sub(self, rhs: f64) -> Self {
    Self::new(self.0 - rhs)
  }
}

/// The CIE L\* cube-root helper, `f(t)` in the L\*a\*b\* definition.
fn lab_f(t: f64) -> f64 {
  const EPSILON: f64 = 216.0 / 24389.0;
  const KAPPA: f64 = 24389.0 / 27.0;
  if t > EPSILON {
    t.powf(1.0 / 3.0)
  } else {
    (KAPPA * t + 16.0) / 116.0
  }
}

/// The inverse of [`lab_f`].
fn lab_f_inv(t: f64) -> f64 {
  const EPSILON: f64 = 216.0 / 24389.0;
  const KAPPA: f64 = 24389.0 / 27.0;
  let t3 = t * t * t;
  if t3 > EPSILON {
    t3
  } else {
    (116.0 * t - 16.0) / KAPPA
  }
}

/// Relative luminance `Y` in `0.0..=100.0` for an L\* value.
pub(crate) fn y_from_tone(tone: f64) -> f64 {
  100.0 * lab_f_inv((tone + 16.0) / 116.0)
}

/// L\* for a relative luminance `Y` in `0.0..=100.0`.
pub(crate) fn tone_from_y(y: f64) -> f64 {
  lab_f(y / 100.0) * 116.0 - 16.0
}
