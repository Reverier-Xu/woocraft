//! The HCT solver: from hue, chroma and tone to an sRGB color.
//!
//! Going *from* a color to HCT is a closed-form conversion: sRGB to XYZ to
//! CAM16 gives hue and chroma, and L\* of `Y` gives tone. Going back is the
//! hard direction, because the three coordinates are not orthogonal in sRGB:
//! an arbitrary `(hue, chroma, tone)` triple usually has no exact sRGB answer,
//! and the honest answer is the color with that hue and tone whose chroma is as
//! large as the gamut allows.
//!
//! That is what this solver computes, in two stages:
//!
//! 1. **Newton on lightness.** Assume the request is in gamut and iterate the
//!    CAM16 inverse until `Y` matches the tone. If every channel stays inside
//!    `0..100` and the result is close enough, it is the answer.
//! 2. **Bisection in hue.** Otherwise the answer lies on the surface of the
//!    gamut cube, on the plane of constant `Y`. The solver finds the segment of
//!    that plane's polygonal intersection with the cube that contains the
//!    requested hue, then bisects within it. This keeps hue and tone exact and
//!    gives up chroma, which is the mapping a design system wants: hue is
//!    intent, tone is accessibility, chroma is the flexible axis.
//!
//! Stage 2 is the expensive one, so it is only reached when stage 1 fails,
//! which for a real palette is the minority of tones.

use crate::{
  cam16::{Cam16, ViewingConditions, delinearized_channel, in_cyclic_order, sanitize_degrees},
  srgb::Srgb,
  tone::y_from_tone,
};

/// Linear RGB to scaled cone responses, as the solver's hue function needs.
const SCALED_DISCOUNT_FROM_LINRGB: [[f64; 3]; 3] = [
  [
    0.001_200_833_568_784_504,
    0.002_389_694_492_170_889,
    0.000_279_574_288_586_112_4,
  ],
  [
    0.000_589_108_665_137_599_9,
    0.002_978_550_257_343_875_8,
    0.000_327_066_610_400_839_8,
  ],
  [
    0.000_101_466_924_916_405_72,
    0.000_536_421_435_918_669_4,
    0.003_297_940_177_071_207_6,
  ],
];

/// The inverse of [`SCALED_DISCOUNT_FROM_LINRGB`].
const LINRGB_FROM_SCALED_DISCOUNT: [[f64; 3]; 3] = [
  [
    1_373.219_870_959_423_1,
    -1_100.425_119_075_482_1,
    -7.278_681_089_101_213,
  ],
  [
    -271.815_969_077_903,
    559.658_046_594_073_3,
    -32.460_474_827_911_94,
  ],
  [
    1.962_289_959_966_566_6,
    -57.173_814_538_844_006,
    308.723_319_781_238_5,
  ],
];

/// Relative luminance contribution of each linear RGB channel.
const Y_FROM_LINRGB: [f64; 3] = [0.2126, 0.7152, 0.0722];

/// Hue boundaries at every half 8-bit step.
///
/// The reference implementation ships these as a 255-entry table of magic
/// numbers. They are exactly the sRGB linearization of the half-steps, so this
/// crate derives them instead: one fewer generated table to keep in sync with
/// the algorithm around it.
fn critical_plane(index: usize) -> f64 {
  let encoded = (index.min(255) as f64 + 0.5) / 255.0;
  let linear = if encoded <= 0.040_449_936 {
    encoded / 12.92
  } else {
    ((encoded + 0.055) / 1.055).powf(2.4)
  };
  linear * 100.0
}

/// Solves the HCT equation.
///
/// This is the machinery behind [`crate::Hct::new`]; it is public so callers
/// that want to know whether a request was gamut mapped can drive it directly
/// and compare the requested chroma with the produced color's.
pub struct HctSolver;

impl HctSolver {
  /// The sRGB color closest to the requested HCT coordinates.
  ///
  /// Hue and tone are honoured exactly where possible; chroma is reduced when
  /// the request falls outside sRGB.
  pub fn solve(hue: f64, chroma: f64, tone: f64) -> Srgb {
    // Achromatic requests and the ends of the tone scale need no solving: the
    // answer is a grey, and the solver's Newton stage would divide by zero.
    if chroma < 0.0001 || tone < 0.0001 || tone > 99.9999 {
      let grey = delinearized_channel(y_from_tone(tone));
      return Srgb::from_rgb8(grey, grey, grey);
    }

    let hue = sanitize_degrees(hue);
    let hue_radians = hue.to_radians();
    let y = y_from_tone(tone);

    if let Some(color) = Self::solve_by_newton(hue_radians, chroma, y) {
      return color;
    }

    let linear = Self::bisect_to_limit(y, hue_radians);
    Srgb::from_rgb8(
      delinearized_channel(linear[0]),
      delinearized_channel(linear[1]),
      delinearized_channel(linear[2]),
    )
  }

  /// The CAM16 coordinates a solved HCT color actually has.
  ///
  /// Useful for reporting how much chroma a gamut mapping gave up.
  pub fn solve_to_cam(hue: f64, chroma: f64, tone: f64) -> Cam16 {
    Cam16::from_srgb(
      Self::solve(hue, chroma, tone),
      &ViewingConditions::default(),
    )
  }

  /// Stage 1: Newton's method on lightness, assuming the request is in gamut.
  ///
  /// Returns `None` when the iteration leaves the gamut cube, which is the
  /// signal to fall back to bisection. The `2 * fn(j) / j` term approximates
  /// the derivative, which is why the iteration converges in a handful of
  /// rounds.
  fn solve_by_newton(hue_radians: f64, chroma: f64, y: f64) -> Option<Srgb> {
    let conditions = ViewingConditions::default();
    let t_inner = 1.0 / (1.64 - 0.29f64.powf(conditions.n())).powf(0.73);
    let e_hue = 0.25 * ((hue_radians + 2.0).cos() + 3.8);
    let p1 = e_hue * (50_000.0 / 13.0) * conditions.nc() * conditions.ncb();
    let h_sin = hue_radians.sin();
    let h_cos = hue_radians.cos();

    let mut j = y.sqrt() * 11.0;

    for round in 0..5 {
      let j_normalized = j / 100.0;
      let alpha = if chroma == 0.0 || j == 0.0 {
        0.0
      } else {
        chroma / j_normalized.sqrt()
      };
      let t = (alpha * t_inner).powf(1.0 / 0.9);
      let ac = conditions.aw() * j_normalized.powf(1.0 / conditions.c() / conditions.z());
      let p2 = ac / conditions.nbb();
      let gamma = 23.0 * (p2 + 0.305) * t / (23.0 * p1 + 11.0 * t * h_cos + 108.0 * t * h_sin);
      let a = gamma * h_cos;
      let b = gamma * h_sin;
      let r_a = (460.0 * p2 + 451.0 * a + 288.0 * b) / 1403.0;
      let g_a = (460.0 * p2 - 891.0 * a - 261.0 * b) / 1403.0;
      let b_a = (460.0 * p2 - 220.0 * a - 6300.0 * b) / 1403.0;

      // `inverse_adaptation` yields the scaled cone response, i.e. the value
      // already divided by the adapting factor, which is exactly the space
      // `LINRGB_FROM_SCALED_DISCOUNT` maps from. Dividing by `rgb_d` again
      // here would undo the adaptation a second time.
      let linrgb = multiply(
        [
          inverse_adaptation(r_a),
          inverse_adaptation(g_a),
          inverse_adaptation(b_a),
        ],
        &LINRGB_FROM_SCALED_DISCOUNT,
      );

      if linrgb[0] < 0.0 || linrgb[1] < 0.0 || linrgb[2] < 0.0 {
        return None;
      }

      let fnj =
        Y_FROM_LINRGB[0] * linrgb[0] + Y_FROM_LINRGB[1] * linrgb[1] + Y_FROM_LINRGB[2] * linrgb[2];
      if fnj <= 0.0 {
        return None;
      }

      if round == 4 || (fnj - y).abs() < 0.002 {
        if linrgb[0] > 100.01 || linrgb[1] > 100.01 || linrgb[2] > 100.01 {
          return None;
        }
        return Some(Srgb::from_rgb8(
          delinearized_channel(linrgb[0]),
          delinearized_channel(linrgb[1]),
          delinearized_channel(linrgb[2]),
        ));
      }

      j -= (fnj - y) * j / (2.0 * fnj);
    }

    None
  }

  /// Stage 2: bisect along the gamut surface at constant `Y` for the hue.
  fn bisect_to_limit(y: f64, target_hue: f64) -> [f64; 3] {
    let [mut left, mut right] = Self::bisect_to_segment(y, target_hue);
    let mut left_hue = Self::hue_of(left);

    for axis in 0..3 {
      if left[axis] == right[axis] {
        continue;
      }

      let (mut l_plane, mut r_plane) = if left[axis] < right[axis] {
        (
          Self::critical_plane_below(delinearize_linear(left[axis])),
          Self::critical_plane_above(delinearize_linear(right[axis])),
        )
      } else {
        (
          Self::critical_plane_above(delinearize_linear(left[axis])),
          Self::critical_plane_below(delinearize_linear(right[axis])),
        )
      };

      for _ in 0..8 {
        if (r_plane - l_plane).abs() <= 1 {
          break;
        }

        let m_plane = (l_plane + r_plane).div_euclid(2);
        let mid_coordinate = critical_plane(m_plane.max(0) as usize);
        let mid = set_coordinate(left, mid_coordinate, right, axis);
        let mid_hue = Self::hue_of(mid);

        if in_cyclic_order(left_hue, target_hue, mid_hue) {
          right = mid;
          r_plane = m_plane;
        } else {
          left = mid;
          left_hue = mid_hue;
          l_plane = m_plane;
        }
      }
    }

    midpoint(left, right)
  }

  /// Finds the segment of the `Y` plane's intersection with the gamut cube
  /// that contains `target_hue`.
  ///
  /// The twelve candidate vertices are visited in the order the reference
  /// implementation uses, which walks the polygon edge by edge. Until the walk
  /// has passed the starting point the segment is not yet meaningful, so the
  /// first candidates only establish the ends; after that each vertex either
  /// replaces the near or the far end depending on where the target hue falls.
  fn bisect_to_segment(y: f64, target_hue: f64) -> [[f64; 3]; 2] {
    let mut left = [-1.0, -1.0, -1.0];
    let mut right = left;
    let mut left_hue = 0.0;
    let mut right_hue = 0.0;
    let mut initialized = false;
    let mut uncut = true;

    for n in 0..12 {
      let mid = Self::nth_vertex(y, n);
      if mid[0] < 0.0 {
        continue;
      }

      let mid_hue = Self::hue_of(mid);
      if !initialized {
        left = mid;
        right = mid;
        left_hue = mid_hue;
        right_hue = mid_hue;
        initialized = true;
        continue;
      }

      if uncut || in_cyclic_order(left_hue, mid_hue, right_hue) {
        uncut = false;
        if in_cyclic_order(left_hue, target_hue, mid_hue) {
          right = mid;
          right_hue = mid_hue;
        } else {
          left = mid;
          left_hue = mid_hue;
        }
      }
    }

    let _ = right_hue;
    [left, right]
  }

  /// The `n`th possible vertex of the `Y` plane's intersection with the cube.
  ///
  /// Twelve candidate points come from clamping two channels to the cube's
  /// corners and solving the third for `Y`. Points outside the cube are
  /// reported as `-1` in every channel.
  fn nth_vertex(y: f64, n: usize) -> [f64; 3] {
    let k_r = Y_FROM_LINRGB[0];
    let k_g = Y_FROM_LINRGB[1];
    let k_b = Y_FROM_LINRGB[2];

    let coord_a = if n % 4 <= 1 { 0.0 } else { 100.0 };
    let coord_b = if n.is_multiple_of(2) { 0.0 } else { 100.0 };

    let (r, g, b, solved) = if n < 4 {
      let (g, b) = (coord_a, coord_b);
      let r = (y - g * k_g - b * k_b) / k_r;
      (r, g, b, r)
    } else if n < 8 {
      let (b, r) = (coord_a, coord_b);
      let g = (y - r * k_r - b * k_b) / k_g;
      (r, g, b, g)
    } else {
      let (r, g) = (coord_a, coord_b);
      let b = (y - r * k_r - g * k_g) / k_b;
      (r, g, b, b)
    };

    if (0.0..=100.0).contains(&solved) {
      [r, g, b]
    } else {
      [-1.0, -1.0, -1.0]
    }
  }

  /// The CAM16 hue of a linear RGB color, in radians.
  fn hue_of(linrgb: [f64; 3]) -> f64 {
    let scaled = multiply(linrgb, &SCALED_DISCOUNT_FROM_LINRGB);
    let r_a = chromatic_adaptation(scaled[0]);
    let g_a = chromatic_adaptation(scaled[1]);
    let b_a = chromatic_adaptation(scaled[2]);
    let a = (11.0 * r_a - 12.0 * g_a + b_a) / 11.0;
    let b = (r_a + g_a - 2.0 * b_a) / 9.0;
    b.atan2(a)
  }

  /// The largest half-step plane at or below `x`.
  fn critical_plane_below(x: f64) -> i32 {
    (x - 0.5).floor() as i32
  }

  /// The smallest half-step plane at or above `x`.
  fn critical_plane_above(x: f64) -> i32 {
    (x - 0.5).ceil() as i32
  }
}

/// Linear `0..100` to gamma-encoded `0..255` as a float.
fn delinearize_linear(linear: f64) -> f64 {
  let normalized = linear / 100.0;
  let encoded = if normalized <= 0.003_130_8 {
    normalized * 12.92
  } else {
    1.055 * normalized.powf(1.0 / 2.4) - 0.055
  };
  encoded * 255.0
}

/// CAM16 chromatic adaptation for one channel.
fn chromatic_adaptation(component: f64) -> f64 {
  let af = component.abs().powf(0.42);
  component.signum() * 400.0 * af / (af + 27.13)
}

/// The inverse of [`chromatic_adaptation`].
fn inverse_adaptation(adapted: f64) -> f64 {
  let adapted_abs = adapted.abs();
  let base = (27.13 * adapted_abs / (400.0 - adapted_abs)).max(0.0);
  adapted.signum() * base.powf(1.0 / 0.42)
}

/// Multiplies a 1x3 row vector by a 3x3 matrix.
///
/// Each output channel is a dot product of the input with one *row* of the
/// matrix, matching the convention the reference implementation uses.
fn multiply(vector: [f64; 3], matrix: &[[f64; 3]; 3]) -> [f64; 3] {
  [
    vector[0] * matrix[0][0] + vector[1] * matrix[0][1] + vector[2] * matrix[0][2],
    vector[0] * matrix[1][0] + vector[1] * matrix[1][1] + vector[2] * matrix[1][2],
    vector[0] * matrix[2][0] + vector[1] * matrix[2][1] + vector[2] * matrix[2][2],
  ]
}

/// The point `t` where `lerp(source, target, t) == mid`.
fn intercept(source: f64, mid: f64, target: f64) -> f64 {
  (mid - source) / (target - source)
}

fn lerp_point(source: [f64; 3], t: f64, target: [f64; 3]) -> [f64; 3] {
  [
    source[0] + (target[0] - source[0]) * t,
    source[1] + (target[1] - source[1]) * t,
    source[2] + (target[2] - source[2]) * t,
  ]
}

fn set_coordinate(source: [f64; 3], coordinate: f64, target: [f64; 3], axis: usize) -> [f64; 3] {
  let t = intercept(source[axis], coordinate, target[axis]);
  lerp_point(source, t, target)
}

fn midpoint(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
  [
    (a[0] + b[0]) / 2.0,
    (a[1] + b[1]) / 2.0,
    (a[2] + b[2]) / 2.0,
  ]
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reference::HCT_SOLVES;

  fn cyclic_delta(a: f64, b: f64) -> f64 {
    let delta = (a - b).abs() % 360.0;
    delta.min(360.0 - delta)
  }

  #[test]
  fn critical_planes_match_the_reference_table() {
    // The reference ships 255 magic numbers; they are half-step linearized
    // 8-bit values. Spot-check the ends and a few interior entries against
    // values taken from that table.
    assert!((critical_plane(0) - 0.015_176_349_177_441_876).abs() < 1e-15);
    assert!((critical_plane(1) - 0.045_529_047_532_325_624).abs() < 1e-15);
    assert!((critical_plane(100) - 12.878_295_467_455_942).abs() < 1e-12);
    assert!((critical_plane(200) - 58.082_412_840_126_21).abs() < 1e-12);
    assert!((critical_plane(254) - 99.554_524_972_107_76).abs() < 1e-12);
  }

  #[test]
  fn solves_match_the_reference_implementation() {
    let mut worst = 0.0f64;
    for &(hue, chroma, tone, hex, out_hue, out_chroma, out_tone) in HCT_SOLVES {
      let solved = HctSolver::solve(hue, chroma, tone);
      let expected = Srgb::from_hex(hex).expect("fixture hex parses");

      let (r1, g1, b1) = solved.to_rgb8();
      let (r2, g2, b2) = expected.to_rgb8();
      let channel_delta = [
        (i16::from(r1) - i16::from(r2)).abs(),
        (i16::from(g1) - i16::from(g2)).abs(),
        (i16::from(b1) - i16::from(b2)).abs(),
      ]
      .into_iter()
      .max()
      .unwrap_or(0) as f64;
      worst = worst.max(channel_delta);

      assert!(
        channel_delta <= 1.0,
        "HCT({hue}, {chroma}, {tone}) gave {solved}, reference {expected} \
         (max channel delta {channel_delta})"
      );

      // The produced color must reproduce the requested hue and tone; chroma
      // is the axis allowed to shrink. Neutrals are exempt from both checks
      // on chroma and hue: the D65 white point is slightly chromatic in CAM16,
      // so even a grey reports a nonzero chroma at the white point's hue.
      let cam = Cam16::from_srgb(solved, &ViewingConditions::default());
      if chroma > 3.0 {
        assert!(
          cyclic_delta(cam.hue(), out_hue) < 2.0,
          "HCT({hue}, {chroma}, {tone}): hue {} != reference {out_hue}",
          cam.hue()
        );
      }
      assert!(
        (cam.tone() - out_tone).abs() < 1.5,
        "HCT({hue}, {chroma}, {tone}): tone {} != reference {out_tone}",
        cam.tone()
      );
      assert!(
        (cam.chroma() - out_chroma).abs() < 2.0,
        "HCT({hue}, {chroma}, {tone}): chroma {} != reference {out_chroma}",
        cam.chroma()
      );
    }
    assert!(
      worst <= 1.0,
      "worst channel delta against the reference was {worst}"
    );
  }

  #[test]
  fn achromatic_requests_are_grey() {
    for tone in [0.0, 10.0, 50.0, 99.0, 100.0] {
      let color = HctSolver::solve(123.0, 0.0, tone);
      let (r, g, b) = color.to_rgb8();
      assert!(
        r.abs_diff(g) <= 1 && g.abs_diff(b) <= 1,
        "tone {tone} produced {color}, which is not grey"
      );
    }
  }

  #[test]
  fn tone_extremes_are_black_and_white() {
    assert_eq!(HctSolver::solve(0.0, 40.0, 0.0).to_hex(), "#000000");
    assert_eq!(HctSolver::solve(0.0, 40.0, 100.0).to_hex(), "#FFFFFF");
  }

  #[test]
  fn out_of_gamut_requests_keep_hue_and_tone() {
    // An impossible request: maximum chroma at a mid tone.
    let color = HctSolver::solve(250.0, 130.0, 50.0);
    let cam = Cam16::from_srgb(color, &ViewingConditions::default());
    assert!(
      cyclic_delta(cam.hue(), 250.0) < 2.0,
      "hue drifted to {}",
      cam.hue()
    );
    assert!(
      (cam.tone() - 50.0).abs() < 1.0,
      "tone drifted to {}",
      cam.tone()
    );
    assert!(
      cam.chroma() < 130.0,
      "chroma should have been reduced, got {}",
      cam.chroma()
    );
  }
}
