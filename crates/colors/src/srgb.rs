//! sRGB colors, in both gamma-encoded and linear form.

use std::fmt;

/// A color in the sRGB color space.
///
/// Channels are stored gamma-encoded, as they appear in CSS and hex notation,
/// in `0.0..=1.0`. Use [`Srgb::to_linear`] to get the linear-light form the
/// color appearance model works in.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Srgb {
  /// Red channel, gamma-encoded, `0.0..=1.0`.
  pub r: f64,
  /// Green channel, gamma-encoded, `0.0..=1.0`.
  pub g: f64,
  /// Blue channel, gamma-encoded, `0.0..=1.0`.
  pub b: f64,
  /// Alpha channel, `0.0..=1.0`. Ignored by color conversions.
  pub a: f64,
}

impl Srgb {
  /// A color from gamma-encoded channels.
  pub const fn new(r: f64, g: f64, b: f64) -> Self {
    Self { r, g, b, a: 1.0 }
  }

  /// A color from gamma-encoded channels and an alpha channel.
  pub const fn with_alpha(r: f64, g: f64, b: f64, a: f64) -> Self {
    Self { r, g, b, a }
  }

  /// A color from 8-bit channels.
  pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
    Self::new(
      f64::from(r) / 255.0,
      f64::from(g) / 255.0,
      f64::from(b) / 255.0,
    )
  }

  /// Parses `#RGB`, `#RGBA`, `#RRGGBB` or `#RRGGBBAA`, with or without the
  /// leading `#`.
  ///
  /// Returns `None` for any other length or for non-hex digits.
  pub fn from_hex(hex: impl AsRef<str>) -> Option<Self> {
    let hex = hex.as_ref();
    let hex = hex.strip_prefix('#').unwrap_or(hex);
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
      return None;
    }

    let byte = |s: &str| u8::from_str_radix(s, 16).ok();
    let nibble = |s: &str| byte(&format!("{s}{s}"));

    let (r, g, b, a) = match hex.len() {
      3 => (
        nibble(&hex[0..1])?,
        nibble(&hex[1..2])?,
        nibble(&hex[2..3])?,
        255,
      ),
      4 => (
        nibble(&hex[0..1])?,
        nibble(&hex[1..2])?,
        nibble(&hex[2..3])?,
        nibble(&hex[3..4])?,
      ),
      6 => (byte(&hex[0..2])?, byte(&hex[2..4])?, byte(&hex[4..6])?, 255),
      8 => (
        byte(&hex[0..2])?,
        byte(&hex[2..4])?,
        byte(&hex[4..6])?,
        byte(&hex[6..8])?,
      ),
      _ => return None,
    };

    Some(Self::with_alpha(
      f64::from(r) / 255.0,
      f64::from(g) / 255.0,
      f64::from(b) / 255.0,
      f64::from(a) / 255.0,
    ))
  }

  /// The 8-bit channels, rounded and clamped.
  pub fn to_rgb8(&self) -> (u8, u8, u8) {
    let channel = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    (channel(self.r), channel(self.g), channel(self.b))
  }

  /// `#RRGGBB`, upper case. The alpha channel is not included; see
  pub fn to_hex(&self) -> String {
    let (r, g, b) = self.to_rgb8();
    format!("#{r:02X}{g:02X}{b:02X}")
  }

  /// The linear-light channels, `0.0..=1.0`.
  ///
  /// This is the form relative luminance and every color appearance model are
  /// defined on. It applies the sRGB transfer function, not a plain
  /// exponentiation, so it matches what a display actually does.
  pub fn to_linear(&self) -> [f64; 3] {
    [linearize(self.r), linearize(self.g), linearize(self.b)]
  }

  /// A color from linear-light channels.
  pub fn from_linear(linear: [f64; 3]) -> Self {
    Self::new(
      delinearize(linear[0]),
      delinearize(linear[1]),
      delinearize(linear[2]),
    )
  }

  /// This color as HSL, the form GPUI's `Hsla` is built from.
  ///
  /// HSL is a lossless re-encoding of gamma-encoded sRGB — the same color, a
  /// different parameterisation — so this conversion is exact and loses only
  /// the sub-`f64` digits. It exists because that is the representation the
  /// view layer consumes; the design system's own reasoning stays in HCT.
  pub fn to_hsl(&self) -> Hsl {
    let max = self.r.max(self.g).max(self.b);
    let min = self.r.min(self.g).min(self.b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;

    if delta.abs() < f64::EPSILON {
      return Hsl {
        h: 0.0,
        s: 0.0,
        l: lightness,
        a: self.a,
      };
    }

    let saturation = if lightness > 0.5 {
      delta / (2.0 - max - min)
    } else {
      delta / (max + min)
    };

    let hue = if max == self.r {
      (self.g - self.b) / delta + if self.g < self.b { 6.0 } else { 0.0 }
    } else if max == self.g {
      (self.b - self.r) / delta + 2.0
    } else {
      (self.r - self.g) / delta + 4.0
    };

    Hsl {
      h: hue * 60.0,
      s: saturation,
      l: lightness,
      a: self.a,
    }
  }

  /// Clamps every channel into `0.0..=1.0`.
  pub fn clamp(&self) -> Self {
    Self::with_alpha(
      self.r.clamp(0.0, 1.0),
      self.g.clamp(0.0, 1.0),
      self.b.clamp(0.0, 1.0),
      self.a,
    )
  }
}

impl Default for Srgb {
  fn default() -> Self {
    Self::new(0.0, 0.0, 0.0)
  }
}

impl fmt::Display for Srgb {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.to_hex())
  }
}

/// A color in HSL, the parameterisation GPUI's `Hsla` uses.
///
/// Channels are `h: 0..360`, `s: 0..1`, `l: 0..1`, `a: 0..1`. HSL is not
/// perceptually uniform — a lightness step in HSL is not a step in perceived
/// lightness, and its saturation axis moves lightness — which is exactly why
/// the design system computes in HCT and only converts here, at the boundary.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hsl {
  /// Hue angle in degrees, `0.0..360.0`.
  pub h: f64,
  /// Saturation, `0.0..=1.0`.
  pub s: f64,
  /// Lightness, `0.0..=1.0`.
  pub l: f64,
  /// Alpha, `0.0..=1.0`.
  pub a: f64,
}

impl Hsl {
  /// The sRGB color for these HSL coordinates.
  pub fn to_srgb(&self) -> Srgb {
    let h = sanitize_degrees(self.h) / 360.0;
    let s = self.s.clamp(0.0, 1.0);
    let l = self.l.clamp(0.0, 1.0);

    if s == 0.0 {
      return Srgb::with_alpha(l, l, l, self.a);
    }

    let q = if l < 0.5 {
      l * (1.0 + s)
    } else {
      l + s - l * s
    };
    let p = 2.0 * l - q;
    let channel = |t: f64| {
      let t = if t < 0.0 {
        t + 1.0
      } else if t > 1.0 {
        t - 1.0
      } else {
        t
      };
      if t < 1.0 / 6.0 {
        (q - p).mul_add(6.0 * t, p)
      } else if t < 0.5 {
        q
      } else if t < 2.0 / 3.0 {
        (q - p).mul_add((2.0 / 3.0 - t) * 6.0, p)
      } else {
        p
      }
    };

    Srgb::with_alpha(
      channel(h + 1.0 / 3.0),
      channel(h),
      channel(h - 1.0 / 3.0),
      self.a,
    )
  }
}

fn sanitize_degrees(degrees: f64) -> f64 {
  let degrees = degrees % 360.0;
  if degrees < 0.0 {
    degrees + 360.0
  } else {
    degrees
  }
}

/// The sRGB electro-optical transfer function: gamma-encoded to linear.
pub(crate) fn linearize(value: f64) -> f64 {
  let value = value.clamp(0.0, 1.0);
  if value <= 0.040_449_936 {
    value / 12.92
  } else {
    ((value + 0.055) / 1.055).powf(2.4)
  }
}

/// The inverse of [`linearize`]: linear to gamma-encoded.
pub(crate) fn delinearize(value: f64) -> f64 {
  let value = value.clamp(0.0, 1.0);
  if value <= 0.003_130_8 {
    value * 12.92
  } else {
    1.055 * value.powf(1.0 / 2.4) - 0.055
  }
}
