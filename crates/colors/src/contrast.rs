//! Contrast, measured in tone.
//!
//! WCAG contrast is defined on relative luminance `Y`:
//!
//! ```text
//! ratio = (Y_lighter + 5) / (Y_darker + 5)
//! ```
//!
//! `Y` and tone determine each other, so the same ratio can be stated in tone
//! units — and, unlike `Y`, tone is linear to perception, which is what makes
//! "make it 40 tones lighter" a meaningful instruction. A tone difference of 40
//! guarantees at least 3:1 and 50 guarantees at least 4.5:1.

use crate::tone::Tone;

/// WCAG contrast ratios and the tone arithmetic behind them.
pub struct Contrast;

impl Contrast {
  /// Smallest possible ratio, two identical colors.
  pub const MIN: f64 = 1.0;
  /// Largest possible ratio, black against white.
  pub const MAX: f64 = 21.0;
  /// The ratio required for large text.
  pub const LARGE_TEXT: f64 = 3.0;
  /// The ratio required for body text.
  pub const BODY_TEXT: f64 = 4.5;
  /// The ratio required for the enhanced level.
  pub const ENHANCED: f64 = 7.0;
  /// The ratio a page's own text is solved for.
  ///
  /// Well past [`Self::ENHANCED`]: a ratio met exactly is a threshold passed,
  /// not an ink, and text solved there reads as grey no matter the hue. This is
  /// the strength at which the surface's own text looks like the surface's own
  /// text in both modes.
  pub const INK: f64 = 12.0;
  /// The ratio a hairline sits at against the surface it rules.
  ///
  /// Borders and dividers are not text: they mark an edge, so the ratio is
  /// small — but it is a ratio, not an opacity. Compositing a translucent line
  /// over a surface is linear in luminance, which makes the same alpha vanish
  /// on a light surface and stand out on a dark one; solving the tone instead
  /// gives the same edge in both modes.
  pub const HAIRLINE: f64 = 1.6;
  /// The ratio a recessed track sits at against the surface it is cut into.
  ///
  /// A track is chrome, not text: it marks the empty part of a control, so it
  /// is stronger than a [`Self::HAIRLINE`] but nowhere near text. Same rule as
  /// the hairline — the ratio is solved, never an opacity — for the same
  /// reason.
  pub const TRACK: f64 = 2.8;

  /// The WCAG contrast ratio between two tones, `1.0..=21.0`.
  pub fn ratio_of_tones(a: Tone, b: Tone) -> f64 {
    Self::ratio_of_luminance(a.luminance(), b.luminance())
  }

  /// The WCAG contrast ratio between two relative luminances, `0..=100` each.
  fn ratio_of_luminance(a: f64, b: f64) -> f64 {
    let lighter = a.max(b);
    let darker = a.min(b);
    (lighter + 5.0) / (darker + 5.0)
  }

  /// The lightest tone at or above `tone` that reaches `ratio` against it.
  ///
  /// Returns `None` when no tone on the scale can reach `ratio`, which is the
  /// honest answer for a request like "4.5:1 against white".
  pub fn lighter(tone: Tone, ratio: f64) -> Option<Tone> {
    Self::solve_contrast(tone, ratio, true)
  }

  /// The darkest tone at or below `tone` that reaches `ratio` against it.
  ///
  /// Returns `None` when no tone on the scale can reach `ratio`.
  pub fn darker(tone: Tone, ratio: f64) -> Option<Tone> {
    Self::solve_contrast(tone, ratio, false)
  }

  /// The tone of the opposite polarity that reaches `ratio` against `tone`.
  ///
  /// This is the "pick a foreground for this surface" question, answered
  /// without a search. A foreground is text, so it should sit on the other side
  /// of the surface rather than next to it: on a light surface it is dark, on a
  /// dark surface it is light. Both directions are tried, the opposite
  /// polarity is preferred, and the nearest tone in that direction wins — so
  /// the result is the closest readable tone, not the far end of the scale.
  pub fn contrasting_tone(tone: Tone, ratio: f64) -> Option<Tone> {
    let lighter = Self::lighter(tone, ratio);
    let darker = Self::darker(tone, ratio);

    // Polarity: a surface at or above mid-scale wants dark text and vice versa.
    let prefer_lighter = tone.value() < 50.0;
    match (lighter, darker) {
      (Some(light), Some(dark)) => {
        if prefer_lighter {
          Some(light)
        } else {
          Some(dark)
        }
      }
      (Some(light), None) => Some(light),
      (None, Some(dark)) => Some(dark),
      (None, None) => None,
    }
  }

  fn solve_contrast(tone: Tone, ratio: f64, lighter: bool) -> Option<Tone> {
    if !(Self::MIN..=Self::MAX).contains(&ratio) || tone.value() <= 0.0 || tone.value() >= 100.0 {
      return None;
    }

    let reference = tone.luminance();
    let target = if lighter {
      ratio * (reference + 5.0) - 5.0
    } else {
      (reference + 5.0) / ratio - 5.0
    };

    if !(0.0..=100.0).contains(&target) {
      return None;
    }

    // The ratio is only satisfied for a range of luminances because `Y` is
    // quantised by 8-bit output. Nudge past the boundary so rounding cannot
    // drop the result back under the requested ratio.
    let nudged = if lighter {
      (target + 0.4).min(100.0)
    } else {
      (target - 0.4).max(0.0)
    };
    let solved = Tone::from_luminance(nudged);

    let achieved = Self::ratio_of_tones(tone, solved);
    if achieved + 0.04 < ratio {
      return None;
    }

    if lighter && solved < tone {
      return None;
    }
    if !lighter && solved > tone {
      return None;
    }

    Some(solved)
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reference::TONE_CONTRAST;

  fn almost_eq(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
  }

  #[test]
  fn matches_reference_ratios() {
    for &(a, b, ratio) in TONE_CONTRAST {
      let ours = Contrast::ratio_of_tones(Tone::new(a), Tone::new(b));
      assert!(
        almost_eq(ours, ratio, 1e-9),
        "ratio of tones {a}/{b}: {ours} != {ratio}"
      );
    }
  }

  #[test]
  fn extremes_are_one_and_twenty_one() {
    assert!(almost_eq(
      Contrast::ratio_of_tones(Tone::new(0.0), Tone::new(0.0)),
      1.0,
      1e-9
    ));
    assert!(almost_eq(
      Contrast::ratio_of_tones(Tone::new(0.0), Tone::new(100.0)),
      21.0,
      1e-9
    ));
  }

  #[test]
  fn tone_difference_guarantees_a_ratio() {
    // The property that makes HCT usable as an accessibility contract: a
    // difference of 40 in tone guarantees at least 3:1, and 50 guarantees at
    // least 4.5:1. Both are lower bounds over the whole scale, so they hold
    // where there is room for them — the guarantee is stated for the range the
    // tone axis can actually span.
    for base in [0.0, 10.0, 20.0, 30.0, 40.0, 50.0, 60.0] {
      let darker = Tone::new(base);
      let plus_40 = Tone::new(base + 40.0);
      assert!(
        Contrast::ratio_of_tones(darker, plus_40) >= 3.0,
        "tone {base} + 40 is {}:1",
        Contrast::ratio_of_tones(darker, plus_40)
      );
    }

    for base in [0.0, 10.0, 20.0, 30.0, 40.0] {
      let darker = Tone::new(base);
      let plus_50 = Tone::new(base + 50.0);
      assert!(
        Contrast::ratio_of_tones(darker, plus_50) >= 4.5,
        "tone {base} + 50 is {}:1",
        Contrast::ratio_of_tones(darker, plus_50)
      );
    }
  }

  #[test]
  fn lighter_and_darker_reach_the_requested_ratio() {
    let surface = Tone::new(40.0);

    let lighter = Contrast::lighter(surface, Contrast::BODY_TEXT).expect("white is reachable");
    assert!(lighter >= surface);
    assert!(Contrast::ratio_of_tones(surface, lighter) >= Contrast::BODY_TEXT);

    let darker =
      Contrast::darker(Tone::new(90.0), Contrast::BODY_TEXT).expect("black is reachable");
    assert!(darker <= Tone::new(90.0));
    assert!(Contrast::ratio_of_tones(Tone::new(90.0), darker) >= Contrast::BODY_TEXT);
  }

  #[test]
  fn impossible_requests_return_none() {
    // Nothing reaches 7:1 against mid-grey.
    assert!(Contrast::lighter(Tone::new(50.0), Contrast::ENHANCED).is_none());
    assert!(Contrast::darker(Tone::new(50.0), Contrast::ENHANCED).is_none());
  }

  #[test]
  fn contrasting_tone_prefers_the_further_option() {
    let surface = Tone::new(20.0);
    let foreground =
      Contrast::contrasting_tone(surface, Contrast::BODY_TEXT).expect("a light foreground exists");
    assert!(
      foreground > surface,
      "on a dark surface the foreground should be lighter"
    );
    assert!(Contrast::ratio_of_tones(surface, foreground) >= Contrast::BODY_TEXT);
  }
}
