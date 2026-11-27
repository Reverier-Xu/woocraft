//! HCT: the color system this crate exposes to the design system.

use std::fmt;

use crate::{
  cam16::{Cam16, ViewingConditions, sanitize_degrees},
  contrast::Contrast,
  solver::HctSolver,
  srgb::{Hsl, Srgb},
  tone::Tone,
};

/// A color in HCT: CAM16 hue and chroma, CIE L\* tone.
///
/// This is the type a design system should carry. It states intent (a hue), an
/// intensity (a chroma) and an accessibility contract (a tone) separately, so
/// a palette can be described in numbers that mean something before any hex
/// value is chosen:
///
/// ```rust
/// use woocraft_colors::{Hct, Tone};
///
/// let brand = Hct::new(250.0, 48.0, Tone::new(40.0));
/// let on_brand = brand.with_tone(Tone::new(100.0));
/// // Tone 40 against tone 100 clears the body-text ratio by construction.
/// assert!(brand.contrast_ratio(on_brand) >= 4.5);
/// ```
///
/// Chroma is bounded by the sRGB gamut, differently at every hue and tone.
/// [`Hct::new`] therefore produces the requested hue and tone with the largest
/// chroma the gamut allows, which may be less than requested. Read the result
/// back to find out what was actually achieved.
#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Hct {
  hue: f64,
  chroma: f64,
  tone: Tone,
  srgb: Srgb,
}

impl Hct {
  /// The color for the given hue, chroma and tone.
  ///
  /// Hue and tone are honoured where the sRGB gamut allows; chroma is reduced
  /// as needed. For a request that fits in gamut the result reproduces all
  /// three coordinates to within 8-bit rounding.
  pub fn new(hue: f64, chroma: f64, tone: Tone) -> Self {
    let srgb = HctSolver::solve(hue, chroma, tone.value());
    Self::from_srgb(srgb)
  }

  /// The HCT coordinates of an sRGB color.
  pub fn from_srgb(srgb: Srgb) -> Self {
    let cam = Cam16::from_srgb(srgb, &ViewingConditions::default());
    Self {
      hue: cam.hue(),
      chroma: cam.chroma(),
      tone: Tone::new(cam.tone()),
      srgb,
    }
  }

  /// Parses a hex color and reads it as HCT.
  ///
  /// Returns `None` if the string is not a valid 3, 4, 6 or 8 digit hex color.
  pub fn from_hex(hex: impl AsRef<str>) -> Option<Self> {
    Srgb::from_hex(hex).map(Self::from_srgb)
  }

  /// The hue angle in degrees, `0.0..360.0`.
  pub fn hue(&self) -> f64 {
    self.hue
  }

  /// The chroma actually achieved, which may be below what was requested.
  pub fn chroma(&self) -> f64 {
    self.chroma
  }

  /// The tone.
  pub fn tone(&self) -> Tone {
    self.tone
  }

  /// The sRGB color.
  pub fn srgb(&self) -> Srgb {
    self.srgb
  }

  /// The color as HSL, ready for a view layer that speaks HSL.
  pub fn hsl(&self) -> Hsl {
    self.srgb.to_hsl()
  }

  /// This color at another tone, keeping hue and chroma intent.
  ///
  /// Chroma may shrink if the new tone cannot hold it — a saturated blue at
  /// tone 95 is not representable. Hue is preserved, so the result still reads
  /// as the same color, only lighter or darker.
  pub fn with_tone(&self, tone: Tone) -> Self {
    Self::new(self.hue, self.chroma, tone)
  }

  /// This color with another hue, keeping chroma and tone.
  pub fn with_hue(&self, hue: f64) -> Self {
    Self::new(hue, self.chroma, self.tone)
  }

  /// This color with another chroma, keeping hue and tone.
  ///
  /// Chroma is clamped by the gamut at this hue and tone, so the result may be
  /// less saturated than requested.
  pub fn with_chroma(&self, chroma: f64) -> Self {
    Self::new(self.hue, chroma, self.tone)
  }

  /// The color with its alpha replaced.
  pub fn with_alpha(&self, alpha: f64) -> Self {
    Self {
      srgb: Srgb::with_alpha(self.srgb.r, self.srgb.g, self.srgb.b, alpha),
      ..*self
    }
  }

  /// The WCAG contrast ratio between this color and `other`, `1.0..=21.0`.
  pub fn contrast_ratio(&self, other: Self) -> f64 {
    Contrast::ratio_of_tones(self.tone, other.tone)
  }

  /// Whether this color reaches `ratio` against `other`.
  pub fn has_contrast(&self, other: Self, ratio: f64) -> bool {
    self.contrast_ratio(other) >= ratio
  }

  /// The tone of the same hue and chroma that reaches `ratio` against this
  /// color, preferring the lighter option.
  ///
  /// This is how a foreground gets chosen without a search: the answer follows
  /// from the surface's tone alone.
  pub fn contrasting_tone(&self, ratio: f64) -> Option<Tone> {
    Contrast::contrasting_tone(self.tone, ratio)
  }

  /// A foreground color for this one: same hue, chroma limited by the gamut,
  /// tone chosen to reach `ratio`.
  ///
  /// Falls back to the extreme of the tone scale when no tone can reach the
  /// ratio, so a caller always gets a color — check
  /// [`Hct::has_contrast`] if the exact ratio matters.
  pub fn foreground(&self, ratio: f64) -> Self {
    let tone = self.contrasting_tone(ratio).unwrap_or_else(|| {
      if self.tone.value() >= 50.0 {
        Tone::MIN
      } else {
        Tone::MAX
      }
    });
    self.with_tone(tone)
  }

  /// The perceptual distance to `other`, in CAM16-UCS.
  pub fn distance(&self, other: Self) -> f64 {
    let conditions = ViewingConditions::default();
    let a = Cam16::from_srgb(self.srgb, &conditions);
    let b = Cam16::from_srgb(other.srgb, &conditions);
    a.distance(&b)
  }

  /// Whether this color is a neutral, i.e. has no meaningful hue.
  pub fn is_neutral(&self) -> bool {
    self.chroma < 0.5
  }

  /// Interpolates toward `other` in HCT.
  ///
  /// Hue takes the shorter way around the circle and chroma and tone
  /// interpolate linearly, which keeps a gradient between two brand colors on
  /// the hue path a viewer expects rather than through grey.
  pub fn lerp(&self, other: Self, t: f64) -> Self {
    let t = t.clamp(0.0, 1.0);

    // A neutral has no meaningful hue, so it takes the other color's hue
    // instead of dragging the interpolation toward an arbitrary angle.
    let hue = match (self.is_neutral(), other.is_neutral()) {
      (true, true) => self.hue,
      (true, false) => other.hue,
      (false, true) => self.hue,
      (false, false) => {
        let delta = shortest_hue_delta(self.hue, other.hue);
        sanitize_degrees(self.hue + delta * t)
      }
    };

    Self::new(
      hue,
      lerp(self.chroma, other.chroma, t),
      Tone::new(lerp(self.tone.value(), other.tone.value(), t)),
    )
  }
}

fn lerp(from: f64, to: f64, t: f64) -> f64 {
  (1.0 - t) * from + t * to
}

/// The signed shortest angular distance from `from` to `to`, in `-180..180`.
fn shortest_hue_delta(from: f64, to: f64) -> f64 {
  let delta = sanitize_degrees(to - from);
  if delta > 180.0 { delta - 360.0 } else { delta }
}

impl Default for Hct {
  fn default() -> Self {
    Self::new(0.0, 0.0, Tone::default())
  }
}

impl fmt::Display for Hct {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.srgb)
  }
}

impl From<Srgb> for Hct {
  fn from(srgb: Srgb) -> Self {
    Self::from_srgb(srgb)
  }
}

impl From<Hct> for Srgb {
  fn from(hct: Hct) -> Self {
    hct.srgb
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reference::HCT_FROM_HEX;

  fn almost_eq(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
  }

  fn cyclic_delta(a: f64, b: f64) -> f64 {
    let delta = (a - b).abs() % 360.0;
    delta.min(360.0 - delta)
  }

  #[test]
  fn reads_reference_colors() {
    for &(hex, hue, chroma, tone) in HCT_FROM_HEX {
      let hct = Hct::from_hex(hex).expect("fixture hex parses");

      // Hue is undefined for neutrals; the reference reports the hue of the
      // white point there, which is not a property of the color.
      if chroma > 3.0 {
        assert!(
          cyclic_delta(hct.hue(), hue) < 1e-6,
          "{hex}: hue {} != {hue}",
          hct.hue()
        );
      }
      assert!(
        almost_eq(hct.chroma(), chroma, 1e-6),
        "{hex}: chroma {} != {chroma}",
        hct.chroma()
      );
      assert!(
        almost_eq(hct.tone().value(), tone, 1e-6),
        "{hex}: tone {} != {tone}",
        hct.tone()
      );
    }
  }

  #[test]
  fn round_trips_through_srgb() {
    for hex in [
      "#000000", "#ffffff", "#ff0000", "#00ff00", "#0000ff", "#808080", "#6750a4", "#123456",
    ] {
      let hct = Hct::from_hex(hex).expect("hex parses");
      let back = Hct::new(hct.hue(), hct.chroma(), hct.tone());
      assert_eq!(
        hct.srgb().to_hex(),
        back.srgb().to_hex(),
        "{hex} did not survive an HCT round trip"
      );
    }
  }

  #[test]
  fn tone_changes_keep_hue() {
    let base = Hct::new(250.0, 40.0, Tone::new(40.0));
    for tone in [20.0, 30.0, 60.0, 80.0] {
      let lighter = base.with_tone(Tone::new(tone));
      assert!(
        cyclic_delta(lighter.hue(), base.hue()) < 2.0,
        "tone {tone}: hue drifted from {} to {}",
        base.hue(),
        lighter.hue()
      );
      assert!(
        almost_eq(lighter.tone().value(), tone, 1.0),
        "tone {tone}: produced {}",
        lighter.tone()
      );
    }
  }

  #[test]
  fn foreground_meets_the_requested_ratio() {
    for (surface_tone, ratio) in [(20.0, 4.5), (40.0, 4.5), (80.0, 4.5), (95.0, 3.0)] {
      let surface = Hct::new(250.0, 20.0, Tone::new(surface_tone));
      let foreground = surface.foreground(ratio);
      assert!(
        foreground.has_contrast(surface, ratio),
        "surface tone {surface_tone}: foreground {} only reaches {:.2}:1",
        foreground.tone(),
        foreground.contrast_ratio(surface)
      );
    }
  }

  #[test]
  fn contrast_is_independent_of_hue_and_chroma() {
    // The property the whole system exists for: tone decides contrast, nothing
    // else. Two colors at the same tone, whatever their hue and chroma, have
    // the same ratio against a background.
    let background = Hct::new(0.0, 0.0, Tone::new(100.0));
    let mut ratios = Vec::new();
    for hue in [0.0, 60.0, 120.0, 200.0, 300.0] {
      for chroma in [0.0, 20.0, 60.0] {
        let color = Hct::new(hue, chroma, Tone::new(40.0));
        ratios.push(color.contrast_ratio(background));
      }
    }
    let min = ratios.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = ratios.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    // The spread comes from 8-bit quantisation of the produced colors, not
    // from the tone axis: the same request at different hues rounds to a
    // slightly different Y. It must stay well under a tenth of a ratio.
    assert!(max - min < 0.1, "same tone gave ratios from {min} to {max}");
  }

  #[test]
  fn lerp_takes_the_short_hue_path() {
    let red = Hct::new(10.0, 50.0, Tone::new(50.0));
    let magenta = Hct::new(340.0, 50.0, Tone::new(50.0));
    let middle = red.lerp(magenta, 0.5);
    // The short way from 10 to 340 goes through 355, not through 175.
    assert!(
      cyclic_delta(middle.hue(), 355.0) < 3.0,
      "hue went to {}, expected about 355",
      middle.hue()
    );
  }

  #[test]
  fn lerp_with_a_neutral_keeps_the_chromatic_hue() {
    let grey = Hct::new(0.0, 0.0, Tone::new(50.0));
    let brand = Hct::new(250.0, 40.0, Tone::new(50.0));
    let middle = grey.lerp(brand, 0.5);
    // Halfway between a neutral and a brand color, the chroma is half, so the
    // produced color's hue is measured with less precision — the solver can
    // only place a hue within the gamut at that chroma. What matters is that
    // the interpolation did not rotate toward an arbitrary angle.
    assert!(
      cyclic_delta(middle.hue(), 250.0) < 25.0,
      "grey dragged the hue to {}",
      middle.hue()
    );
    assert!(
      middle.chroma() > 5.0,
      "the interpolated color should still carry chroma"
    );
  }

  #[test]
  fn hsl_matches_the_srgb_color() {
    for hex in ["#000000", "#ffffff", "#ff0000", "#6750a4", "#123456"] {
      let hct = Hct::from_hex(hex).expect("hex parses");
      let round_tripped = hct.hsl().to_srgb();
      let (r1, g1, b1) = hct.srgb().to_rgb8();
      let (r2, g2, b2) = round_tripped.to_rgb8();
      assert_eq!(
        (r1, g1, b1),
        (r2, g2, b2),
        "{hex} did not survive an HSL round trip"
      );
    }
  }

  #[test]
  fn alpha_is_carried_through_hsl() {
    let hct = Hct::from_hex("#6750a480").expect("hex parses");
    assert!(almost_eq(hct.hsl().a, 128.0 / 255.0, 1e-9));
  }
}
