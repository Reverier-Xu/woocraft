//! CAM16, the color appearance model HCT takes its hue and chroma from.
//!
//! A color appearance model answers a different question than a color space:
//! not "what is this signal" but "what does this look like, here". That is why
//! it needs [`ViewingConditions`], and why the same sRGB value can carry a
//! slightly different hue under a different surround.
//!
//! The implementation follows the CAM16 equations as published by the Colour
//! Appearance Model working group, with the same constants the reference
//! implementation (Material Color Utilities) uses, so results are comparable
//! to the rest of the M3 ecosystem.

use crate::{
  srgb::Srgb,
  tone::{tone_from_y, y_from_tone},
};

/// The D65 white point, in XYZ scaled so that `Y = 100`.
const WHITE_POINT_D65: [f64; 3] = [95.047, 100.0, 108.883];

/// XYZ to CAM16 cone responses.
const XYZ_TO_CAM16RGB: [[f64; 3]; 3] = [
  [0.401_288, 0.650_173, -0.051_461],
  [-0.250_268, 1.204_414, 0.045_854],
  [-0.002_079, 0.048_952, 0.953_127],
];

/// CAM16 cone responses back to XYZ.
const CAM16RGB_TO_XYZ: [[f64; 3]; 3] = [
  [1.862_067_8, -1.011_254_7, 0.149_186_78],
  [0.387_526_54, 0.621_447_44, -0.008_973_98],
  [-0.015_841_50, -0.034_122_94, 1.049_964_4],
];

/// Intermediate values of the CAM16 conversion that depend only on the
/// viewing conditions.
///
/// Computing these once per set of conditions is what makes a palette cheap:
/// every color in it shares the same surround.
#[derive(Clone, Debug)]
pub struct ViewingConditions {
  n: f64,
  aw: f64,
  nbb: f64,
  ncb: f64,
  c: f64,
  nc: f64,
  rgb_d: [f64; 3],
  fl: f64,
  fl_root: f64,
  z: f64,
  /// `(1.64 - 0.29^n)^0.73`, the chroma-to-`t` denominator.
  ///
  /// Depends only on the conditions, but appears in every forward and inverse
  /// conversion; a plane sweeps tens of thousands of those, so it is computed
  /// once here instead of twice per sample.
  t_denominator: f64,
}

impl Default for ViewingConditions {
  /// The conditions Material and this crate's palettes assume: sRGB display,
  /// 200 lux adapting field, mid-grey background, average surround.
  fn default() -> Self {
    Self::with_background_tone(50.0)
  }
}

impl ViewingConditions {
  /// sRGB-like conditions with a custom background tone.
  ///
  /// The background tone is the lightness of the area surrounding the color.
  /// The default of 50 is mid-grey, which is the reference for the tones this
  /// crate produces.
  pub fn with_background_tone(background_tone: f64) -> Self {
    Self::new(
      WHITE_POINT_D65,
      200.0 / std::f64::consts::PI * y_from_tone(50.0) / 100.0,
      background_tone,
      2.0,
      false,
    )
  }

  /// Conditions from the physical parameters of the environment.
  ///
  /// - `white_point`: the illuminant, in XYZ scaled so `Y = 100`.
  /// - `adapting_luminance`: brightness of the room; lux times `0.0586`.
  /// - `background_tone`: L\* of the area around the color.
  /// - `surround`: `0.0` for a dark cinema, `1.0` for a dim room, `2.0` when
  ///   the color and its surround are lit alike.
  /// - `discounting_illuminant`: whether the observer compensates for the tint
  ///   of the light. Displays are self-luminous, so this is `false` for UI.
  pub fn new(
    white_point: [f64; 3], adapting_luminance: f64, background_tone: f64, surround: f64,
    discounting_illuminant: bool,
  ) -> Self {
    // A pure black background is non-physical and makes `n` vanish, which
    // would turn every derived value into an infinity.
    let background_tone = background_tone.max(0.1);

    let [x, y, z] = white_point;
    let r_w = x * XYZ_TO_CAM16RGB[0][0] + y * XYZ_TO_CAM16RGB[0][1] + z * XYZ_TO_CAM16RGB[0][2];
    let g_w = x * XYZ_TO_CAM16RGB[1][0] + y * XYZ_TO_CAM16RGB[1][1] + z * XYZ_TO_CAM16RGB[1][2];
    let b_w = x * XYZ_TO_CAM16RGB[2][0] + y * XYZ_TO_CAM16RGB[2][1] + z * XYZ_TO_CAM16RGB[2][2];

    let f = 0.8 + surround / 10.0;
    let c = if f >= 0.9 {
      lerp(0.59, 0.69, (f - 0.9) * 10.0)
    } else {
      lerp(0.525, 0.59, (f - 0.8) * 10.0)
    };
    let d = if discounting_illuminant {
      1.0
    } else {
      (f * (1.0 - (1.0 / 3.6) * ((-adapting_luminance - 42.0) / 92.0).exp())).clamp(0.0, 1.0)
    };
    let nc = f;

    let rgb_d = [
      d * (100.0 / r_w) + 1.0 - d,
      d * (100.0 / g_w) + 1.0 - d,
      d * (100.0 / b_w) + 1.0 - d,
    ];

    let k = 1.0 / (5.0 * adapting_luminance + 1.0);
    let k4 = k * k * k * k;
    let k4_f = 1.0 - k4;
    let fl = k4 * adapting_luminance + 0.1 * k4_f * k4_f * (5.0 * adapting_luminance).cbrt();
    let n = y_from_tone(background_tone) / y;
    let z = 1.48 + n.sqrt();
    let nbb = 0.725 / n.powf(0.2);
    let ncb = nbb;

    let rgb_a_factors = [
      (fl * rgb_d[0] * r_w / 100.0).powf(0.42),
      (fl * rgb_d[1] * g_w / 100.0).powf(0.42),
      (fl * rgb_d[2] * b_w / 100.0).powf(0.42),
    ];
    let rgb_a = [
      400.0 * rgb_a_factors[0] / (rgb_a_factors[0] + 27.13),
      400.0 * rgb_a_factors[1] / (rgb_a_factors[1] + 27.13),
      400.0 * rgb_a_factors[2] / (rgb_a_factors[2] + 27.13),
    ];
    let aw = (2.0 * rgb_a[0] + rgb_a[1] + 0.05 * rgb_a[2]) * nbb;

    Self {
      n,
      aw,
      nbb,
      ncb,
      c,
      nc,
      rgb_d,
      fl,
      fl_root: fl.powf(0.25),
      z,
      t_denominator: (1.64 - 0.29f64.powf(n)).powf(0.73),
    }
  }

  /// Adaptation exponent for chroma.
  pub fn n(&self) -> f64 {
    self.n
  }

  /// Achromatic response of the white point.
  pub fn aw(&self) -> f64 {
    self.aw
  }

  /// Luminance-level adaptation factor.
  pub fn nbb(&self) -> f64 {
    self.nbb
  }

  /// Chromatic surround induction factor.
  pub fn ncb(&self) -> f64 {
    self.ncb
  }

  /// Surround impact on contrast.
  pub fn c(&self) -> f64 {
    self.c
  }

  /// Surround impact on chromatic induction.
  pub fn nc(&self) -> f64 {
    self.nc
  }

  /// Cone response adaptation per channel.
  pub fn rgb_d(&self) -> [f64; 3] {
    self.rgb_d
  }

  /// Luminance adaptation factor.
  pub fn fl(&self) -> f64 {
    self.fl
  }

  /// Fourth root of [`Self::fl`].
  pub fn fl_root(&self) -> f64 {
    self.fl_root
  }

  /// Base for the lightness exponent.
  pub fn z(&self) -> f64 {
    self.z
  }

  /// `(1.64 - 0.29^n)^0.73`, the denominator that maps chroma to `t`.
  pub fn t_denominator(&self) -> f64 {
    self.t_denominator
  }
}

fn lerp(from: f64, to: f64, amount: f64) -> f64 {
  (1.0 - amount) * from + amount * to
}

/// Sign-preserving signum: `-1`, `0` or `1`.
fn signum(value: f64) -> f64 {
  if value < 0.0 {
    -1.0
  } else if value == 0.0 {
    0.0
  } else {
    1.0
  }
}

/// A color in CAM16.
///
/// HCT uses [`Cam16::hue`] and [`Cam16::chroma`]; [`Cam16::j`] and the UCS
/// coordinates exist for color difference, which a design system needs when it
/// interpolates or compares colors.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cam16 {
  hue: f64,
  chroma: f64,
  j: f64,
  q: f64,
  m: f64,
  s: f64,
  jstar: f64,
  astar: f64,
  bstar: f64,
  /// Relative luminance `Y` of the color, `0..=100`.
  ///
  /// Stored rather than recomputed because it is the one quantity that does
  /// not depend on the viewing conditions, and tone is derived from it.
  y: f64,
}

impl Cam16 {
  /// CAM16 coordinates of an sRGB color under the given viewing conditions.
  pub fn from_srgb(color: Srgb, conditions: &ViewingConditions) -> Self {
    let linear = color.to_linear();
    let [r, g, b] = linear;
    // sRGB to XYZ, in the 0..100 scale CAM16 uses.
    let x = (0.412_338_95 * r + 0.357_620_64 * g + 0.180_510_42 * b) * 100.0;
    let y = (0.2126 * r + 0.7152 * g + 0.0722 * b) * 100.0;
    let z = (0.019_321_41 * r + 0.119_163_82 * g + 0.950_344_78 * b) * 100.0;

    Self::from_xyz(x, y, z, conditions)
  }

  /// CAM16 coordinates of an XYZ color, scaled so `Y = 100`.
  pub fn from_xyz(x: f64, y: f64, z: f64, conditions: &ViewingConditions) -> Self {
    let r_t = x * XYZ_TO_CAM16RGB[0][0] + y * XYZ_TO_CAM16RGB[0][1] + z * XYZ_TO_CAM16RGB[0][2];
    let g_t = x * XYZ_TO_CAM16RGB[1][0] + y * XYZ_TO_CAM16RGB[1][1] + z * XYZ_TO_CAM16RGB[1][2];
    let b_t = x * XYZ_TO_CAM16RGB[2][0] + y * XYZ_TO_CAM16RGB[2][1] + z * XYZ_TO_CAM16RGB[2][2];

    let rgb_d = conditions.rgb_d();
    let r_d = rgb_d[0] * r_t;
    let g_d = rgb_d[1] * g_t;
    let b_d = rgb_d[2] * b_t;

    let fl = conditions.fl();
    let adapt = |component: f64| {
      let factor = (fl * component.abs() / 100.0).powf(0.42);
      signum(component) * 400.0 * factor / (factor + 27.13)
    };
    let r_a = adapt(r_d);
    let g_a = adapt(g_d);
    let b_a = adapt(b_d);

    let a = (11.0 * r_a - 12.0 * g_a + b_a) / 11.0;
    let b = (r_a + g_a - 2.0 * b_a) / 9.0;
    let u = (20.0 * r_a + 20.0 * g_a + 21.0 * b_a) / 20.0;
    let p2 = (40.0 * r_a + 20.0 * g_a + b_a) / 20.0;

    let hue = sanitize_degrees(b.atan2(a).to_degrees());
    let hue_radians = hue.to_radians();
    let ac = p2 * conditions.nbb();
    let j = 100.0 * (ac / conditions.aw()).powf(conditions.c() * conditions.z());
    let q =
      4.0 / conditions.c() * (j / 100.0).sqrt() * (conditions.aw() + 4.0) * conditions.fl_root();

    let hue_prime = if hue < 20.14 { hue + 360.0 } else { hue };
    let e_hue = 0.25 * ((hue_prime.to_radians() + 2.0).cos() + 3.8);
    let p1 = 50_000.0 / 13.0 * e_hue * conditions.nc() * conditions.ncb();
    let t = p1 * a.hypot(b) / (u + 0.305);
    let alpha = conditions.t_denominator() * t.powf(0.9);
    let chroma = alpha * (j / 100.0).sqrt();
    let m = chroma * conditions.fl_root();
    let s = 50.0 * ((alpha * conditions.c()) / (conditions.aw() + 4.0)).sqrt();

    let jstar = (1.0 + 100.0 * 0.007) * j / (1.0 + 0.007 * j);
    let mstar = 1.0 / 0.0228 * (0.0228 * m).ln_1p();
    let astar = mstar * hue_radians.cos();
    let bstar = mstar * hue_radians.sin();

    Self {
      hue,
      chroma,
      j,
      q,
      m,
      s,
      jstar,
      astar,
      bstar,
      y,
    }
  }

  /// The XYZ colour for CAM16 coordinates, without building a [`Cam16`] first.
  ///
  /// [`Cam16::from_jch`] computes the UCS coordinates and its own `Y`, then
  /// [`Cam16::to_xyz`] inverts the model again; a caller that only wants the
  /// colour pays for both. This is the same inverse without the round trip,
  /// which matters when a plane sweeps tens of thousands of coordinates.
  ///
  /// `j` is CAM16 lightness; use [`Cam16::xyz_from_hct`] for tone.
  pub(crate) fn xyz_from_jch(
    j: f64, chroma: f64, hue: f64, conditions: &ViewingConditions,
  ) -> [f64; 3] {
    let alpha = if chroma == 0.0 || j == 0.0 {
      0.0
    } else {
      chroma / (j / 100.0).sqrt()
    };

    let t = (alpha / conditions.t_denominator()).powf(1.0 / 0.9);
    let hue_radians = hue.to_radians();
    let e_hue = 0.25 * ((hue_radians + 2.0).cos() + 3.8);
    let ac = conditions.aw() * (j / 100.0).powf(1.0 / conditions.c() / conditions.z());
    let p1 = e_hue * (50_000.0 / 13.0) * conditions.nc() * conditions.ncb();
    let p2 = ac / conditions.nbb();

    let h_sin = hue_radians.sin();
    let h_cos = hue_radians.cos();
    let gamma = 23.0 * (p2 + 0.305) * t / (23.0 * p1 + 11.0 * t * h_cos + 108.0 * t * h_sin);
    let a = gamma * h_cos;
    let b = gamma * h_sin;

    let r_a = (460.0 * p2 + 451.0 * a + 288.0 * b) / 1403.0;
    let g_a = (460.0 * p2 - 891.0 * a - 261.0 * b) / 1403.0;
    let b_a = (460.0 * p2 - 220.0 * a - 6300.0 * b) / 1403.0;

    let inverse_adapt = |component: f64| {
      let base = (27.13 * component.abs() / (400.0 - component.abs())).max(0.0);
      signum(component) * (100.0 / conditions.fl()) * base.powf(1.0 / 0.42)
    };
    let rgb_d = conditions.rgb_d();
    let r_f = inverse_adapt(r_a) / rgb_d[0];
    let g_f = inverse_adapt(g_a) / rgb_d[1];
    let b_f = inverse_adapt(b_a) / rgb_d[2];

    [
      r_f * CAM16RGB_TO_XYZ[0][0] + g_f * CAM16RGB_TO_XYZ[0][1] + b_f * CAM16RGB_TO_XYZ[0][2],
      r_f * CAM16RGB_TO_XYZ[1][0] + g_f * CAM16RGB_TO_XYZ[1][1] + b_f * CAM16RGB_TO_XYZ[1][2],
      r_f * CAM16RGB_TO_XYZ[2][0] + g_f * CAM16RGB_TO_XYZ[2][1] + b_f * CAM16RGB_TO_XYZ[2][2],
    ]
  }

  /// The XYZ colour for HCT coordinates.
  ///
  /// This is the conversion callers outside this crate should use: the first
  /// argument is **tone** (CIE L\*), the quantity HCT is defined on, and the
  /// CAM16 lightness that produces it is solved for internally. Working in tone
  /// directly is what keeps a sampled coordinate and the colour it denotes
  /// inside the same gamut — passing a tone where a `J` was expected makes a
  /// legal colour look like it lies outside sRGB.
  pub fn xyz_from_hct(
    tone: f64, chroma: f64, hue: f64, conditions: &ViewingConditions,
  ) -> [f64; 3] {
    // `Y` is what tone pins down; `J` is what the model inverts. Bisect on `J`
    // until the produced colour's luminance matches the requested tone, which
    // keeps this consistent with the forward conversion used to read a colour
    // back into HCT.
    let target_y = crate::tone::y_from_tone(tone);
    let mut low = 0.0;
    let mut high = 100.0;
    let mut best = [0.0; 3];

    for _ in 0..24 {
      let mid = (low + high) / 2.0;
      let xyz = Self::xyz_from_jch(mid, chroma, hue, conditions);
      best = xyz;
      if xyz[1] < target_y {
        low = mid;
      } else {
        high = mid;
      }
    }

    best
  }

  /// Hue angle in degrees, `0.0..360.0`.
  pub fn hue(&self) -> f64 {
    self.hue
  }

  /// Chroma: colorfulness relative to the white point. Unbounded, and limited
  /// in practice by the sRGB gamut to roughly `0..130`.
  pub fn chroma(&self) -> f64 {
    self.chroma
  }

  /// Lightness `J`, `0.0..=100.0`.
  pub fn j(&self) -> f64 {
    self.j
  }

  /// Brightness `Q`.
  pub fn q(&self) -> f64 {
    self.q
  }

  /// Colorfulness `M`.
  pub fn m(&self) -> f64 {
    self.m
  }

  /// Saturation `S`.
  pub fn s(&self) -> f64 {
    self.s
  }

  /// Lightness coordinate in CAM16-UCS.
  pub fn jstar(&self) -> f64 {
    self.jstar
  }

  /// `a*` coordinate in CAM16-UCS.
  pub fn astar(&self) -> f64 {
    self.astar
  }

  /// `b*` coordinate in CAM16-UCS.
  pub fn bstar(&self) -> f64 {
    self.bstar
  }

  /// The perceptual distance to `other`, in CAM16-UCS.
  ///
  /// This is the CAM16 analogue of CIEDE2000: a distance that matches how
  /// different two colors look, not how far apart their numbers are.
  pub fn distance(&self, other: &Self) -> f64 {
    let dj = self.jstar - other.jstar;
    let da = self.astar - other.astar;
    let db = self.bstar - other.bstar;
    let de_prime = (dj * dj + da * da + db * db).sqrt();
    1.41 * de_prime.powf(0.63)
  }

  /// The sRGB color for these coordinates.
  ///
  /// Channels are clamped into the gamut; the caller decides what to do about
  /// that. [`crate::Hct`] does not clamp, it solves for a color that is inside
  /// the gamut to begin with.
  pub fn to_srgb(&self, conditions: &ViewingConditions) -> Srgb {
    let [x, y, z] = self.to_xyz(conditions);
    let linear = [
      (3.241_377_479_238_868_5 * x - 1.537_665_240_285_185_1 * y - 0.498_853_668_462_680_53 * z)
        / 100.0,
      (-0.969_145_251_300_532_1 * x + 1.875_885_345_106_787_2 * y + 0.041_565_856_169_120_61 * z)
        / 100.0,
      (0.055_620_936_896_913_05 * x - 0.203_955_245_647_421_23 * y + 1.057_179_911_122_033_5 * z)
        / 100.0,
    ];
    Srgb::from_linear(linear).clamp()
  }

  /// The XYZ color for these coordinates, scaled so `Y = 100`.
  pub fn to_xyz(&self, conditions: &ViewingConditions) -> [f64; 3] {
    let alpha = if self.chroma == 0.0 || self.j == 0.0 {
      0.0
    } else {
      self.chroma / (self.j / 100.0).sqrt()
    };

    let t = (alpha / conditions.t_denominator()).powf(1.0 / 0.9);
    let hue_radians = self.hue.to_radians();
    let e_hue = 0.25 * ((hue_radians + 2.0).cos() + 3.8);
    let ac = conditions.aw() * (self.j / 100.0).powf(1.0 / conditions.c() / conditions.z());
    let p1 = e_hue * (50_000.0 / 13.0) * conditions.nc() * conditions.ncb();
    let p2 = ac / conditions.nbb();

    let h_sin = hue_radians.sin();
    let h_cos = hue_radians.cos();
    let gamma = 23.0 * (p2 + 0.305) * t / (23.0 * p1 + 11.0 * t * h_cos + 108.0 * t * h_sin);
    let a = gamma * h_cos;
    let b = gamma * h_sin;

    let r_a = (460.0 * p2 + 451.0 * a + 288.0 * b) / 1403.0;
    let g_a = (460.0 * p2 - 891.0 * a - 261.0 * b) / 1403.0;
    let b_a = (460.0 * p2 - 220.0 * a - 6300.0 * b) / 1403.0;

    let inverse_adapt = |component: f64| {
      let base = (27.13 * component.abs() / (400.0 - component.abs())).max(0.0);
      signum(component) * (100.0 / conditions.fl()) * base.powf(1.0 / 0.42)
    };
    let rgb_d = conditions.rgb_d();
    let r_f = inverse_adapt(r_a) / rgb_d[0];
    let g_f = inverse_adapt(g_a) / rgb_d[1];
    let b_f = inverse_adapt(b_a) / rgb_d[2];

    [
      r_f * CAM16RGB_TO_XYZ[0][0] + g_f * CAM16RGB_TO_XYZ[0][1] + b_f * CAM16RGB_TO_XYZ[0][2],
      r_f * CAM16RGB_TO_XYZ[1][0] + g_f * CAM16RGB_TO_XYZ[1][1] + b_f * CAM16RGB_TO_XYZ[1][2],
      r_f * CAM16RGB_TO_XYZ[2][0] + g_f * CAM16RGB_TO_XYZ[2][1] + b_f * CAM16RGB_TO_XYZ[2][2],
    ]
  }

  /// The tone of this color, `0.0..=100.0`.
  ///
  /// This is L\*, derived from relative luminance, not CAM16's own `J`. The
  /// distinction is the whole point of HCT: `J` depends on the surround, `L*`
  /// does not, so only the latter can promise a contrast ratio.
  pub fn tone(&self) -> f64 {
    tone_from_y(self.y)
  }

  /// Relative luminance `Y`, `0.0..=100.0`.
  pub fn luminance(&self) -> f64 {
    self.y
  }
}

/// XYZ (scaled so `Y = 100`) to linear Display-P3.
///
/// The D65 matrix from the Display-P3 specification. Used only to answer "is
/// this colour inside P3", which is what a picker needs in order to draw the
/// wider gamut's boundary — the crate itself still renders in sRGB.
const XYZ_TO_LINEAR_P3: [[f64; 3]; 3] = [
  [
    2.493_496_940_635_514,
    -0.931_383_633_151_705_9,
    -0.402_710_794_935_793_6,
  ],
  [
    -0.829_488_964_866_711_9,
    1.762_664_102_462_009_5,
    0.023_624_662_772_673_4,
  ],
  [
    0.035_845_831_035_678_5,
    -0.076_172_365_500_204_5,
    0.956_884_562_340_483_1,
  ],
];

/// XYZ (scaled so `Y = 100`) to linear sRGB.
///
/// The inverse of the matrix CAM16 uses on the way in, so a colour can be
/// tested against sRGB without going through the HCT solver — which always
/// returns an in-gamut result and therefore cannot answer "was this clamped".
const XYZ_TO_LINEAR_SRGB: [[f64; 3]; 3] = [
  [
    3.240_969_941_904_864,
    -1.537_383_177_570_094,
    -0.498_610_760_293_003,
  ],
  [
    -0.969_243_636_280_879,
    1.875_967_501_507_72,
    0.041_555_057_230_707,
  ],
  [
    0.055_630_079_696_994,
    -0.203_976_958_158_877,
    1.056_971_514_213_459,
  ],
];

/// Linear sRGB channels for an XYZ colour, on the 0..1 scale.
///
/// Values outside `0..=1` are kept as-is: the caller decides whether that means
/// "out of gamut" or "clamp for display", and clamping here would hide the
/// difference.
pub fn linear_srgb(xyz: [f64; 3]) -> [f64; 3] {
  [0, 1, 2].map(|row| {
    (xyz[0] * XYZ_TO_LINEAR_SRGB[row][0]
      + xyz[1] * XYZ_TO_LINEAR_SRGB[row][1]
      + xyz[2] * XYZ_TO_LINEAR_SRGB[row][2])
      / 100.0
  })
}

/// Whether a colour lies inside the sRGB gamut.
///
/// Takes XYZ on the same 0..100 scale [`Cam16::to_xyz`] produces.
pub fn in_srgb(xyz: [f64; 3]) -> bool {
  const SLACK: f64 = 1e-3;
  (0..3).all(|row| {
    let linear = (xyz[0] * XYZ_TO_LINEAR_SRGB[row][0]
      + xyz[1] * XYZ_TO_LINEAR_SRGB[row][1]
      + xyz[2] * XYZ_TO_LINEAR_SRGB[row][2])
      / 100.0;
    (-SLACK..=1.0 + SLACK).contains(&linear)
  })
}

/// Whether a colour lies inside the Display-P3 gamut.
///
/// P3 is a strict superset of sRGB, so anything in sRGB is in P3. A colour
/// outside P3 cannot be shown on a wide-gamut display either, which is the
/// interesting boundary for a picker: it separates "reduced to sRGB" from
/// "not displayable at all".
pub fn in_p3(xyz: [f64; 3]) -> bool {
  // `to_xyz` yields the 0..100 scale while the matrix expects 0..1, so the
  // product is scaled back down here. The slack absorbs floating-point noise in
  // the matrix product, well below what a display can show.
  const SLACK: f64 = 1e-3;
  (0..3).all(|row| {
    let linear = (xyz[0] * XYZ_TO_LINEAR_P3[row][0]
      + xyz[1] * XYZ_TO_LINEAR_P3[row][1]
      + xyz[2] * XYZ_TO_LINEAR_P3[row][2])
      / 100.0;
    (-SLACK..=1.0 + SLACK).contains(&linear)
  })
}

/// Wraps an angle in degrees into `0.0..360.0`.
pub(crate) fn sanitize_degrees(degrees: f64) -> f64 {
  let degrees = degrees % 360.0;
  if degrees < 0.0 {
    degrees + 360.0
  } else {
    degrees
  }
}

/// Wraps an angle in radians into `0.0..2pi`.
pub(crate) fn sanitize_radians(angle: f64) -> f64 {
  (angle + std::f64::consts::PI * 8.0) % (std::f64::consts::PI * 2.0)
}

/// Whether `b` lies in the cyclic interval from `a` to `c`, going in the
/// positive direction.
pub(crate) fn in_cyclic_order(a: f64, b: f64, c: f64) -> bool {
  sanitize_radians(b - a) < sanitize_radians(c - a)
}

/// Linear `0..100` to gamma-encoded `0..255`.
pub(crate) fn delinearized_channel(linear: f64) -> u8 {
  let encoded = crate::srgb::delinearize(linear / 100.0);
  (encoded * 255.0).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::reference::CAM16_FROM_HEX;

  fn almost_eq(a: f64, b: f64, tolerance: f64) -> bool {
    (a - b).abs() <= tolerance
  }

  #[test]
  fn matches_reference_implementation() {
    let conditions = ViewingConditions::default();
    for &(hex, hue, chroma, j, jstar, astar, bstar) in CAM16_FROM_HEX {
      let color = Srgb::from_hex(hex).expect("fixture hex parses");
      let cam = Cam16::from_srgb(color, &conditions);

      // Hue is meaningless for neutrals, where the reference reports the
      // hue of the white point rather than a real angle.
      if chroma > 3.0 {
        assert!(
          almost_eq(cam.hue(), hue, 1e-6),
          "{hex}: hue {} != {hue}",
          cam.hue()
        );
      }
      assert!(
        almost_eq(cam.chroma(), chroma, 1e-6),
        "{hex}: chroma {} != {chroma}",
        cam.chroma()
      );
      assert!(almost_eq(cam.j(), j, 1e-6), "{hex}: J {} != {j}", cam.j());
      assert!(
        almost_eq(cam.jstar(), jstar, 1e-6),
        "{hex}: J* {} != {jstar}",
        cam.jstar()
      );
      assert!(
        almost_eq(cam.astar(), astar, 1e-6),
        "{hex}: a* {} != {astar}",
        cam.astar()
      );
      assert!(
        almost_eq(cam.bstar(), bstar, 1e-6),
        "{hex}: b* {} != {bstar}",
        cam.bstar()
      );
    }
  }

  #[test]
  fn xyz_round_trips() {
    let conditions = ViewingConditions::default();
    for hex in ["#000000", "#ffffff", "#ff0000", "#123456", "#6750a4"] {
      let color = Srgb::from_hex(hex).expect("hex parses");
      let cam = Cam16::from_srgb(color, &conditions);
      let back = cam.to_srgb(&conditions);
      let (r1, g1, b1) = color.to_rgb8();
      let (r2, g2, b2) = back.to_rgb8();
      assert_eq!(
        (r1, g1, b1),
        (r2, g2, b2),
        "{hex} round trip via CAM16 changed the color"
      );
    }
  }

  #[test]
  fn ucs_distance_is_zero_for_identical_colors() {
    let conditions = ViewingConditions::default();
    let cam = Cam16::from_srgb(Srgb::from_hex("#6750a4").unwrap(), &conditions);
    assert_eq!(cam.distance(&cam), 0.0);
  }

  #[test]
  fn tone_is_independent_of_viewing_conditions() {
    // Two different surrounds give different CAM16 lightness but the same
    // tone, which is exactly why HCT states contrast in tone.
    let color = Srgb::from_hex("#6750a4").unwrap();
    let dim = ViewingConditions::with_background_tone(20.0);
    let bright = ViewingConditions::with_background_tone(80.0);

    let a = Cam16::from_srgb(color, &dim);
    let b = Cam16::from_srgb(color, &bright);
    assert!(
      (a.j() - b.j()).abs() > 1.0,
      "J should move with the surround"
    );
    assert!(
      (a.tone() - b.tone()).abs() < 1e-9,
      "tone must not move with the surround"
    );
  }
}

#[cfg(test)]
mod p3_tests {
  use super::*;
  use crate::srgb::Srgb;

  #[test]
  fn srgb_is_always_inside_p3() {
    let conditions = ViewingConditions::default();
    for hex in [
      "#000000", "#ffffff", "#ff0000", "#00ff00", "#0000ff", "#6750a4", "#123456",
    ] {
      let cam = Cam16::from_srgb(Srgb::from_hex(hex).unwrap(), &conditions);
      let xyz = cam.to_xyz(&conditions);
      assert!(in_p3(xyz), "{hex} should be inside P3");
    }
  }

  #[test]
  fn a_very_saturated_color_can_leave_p3() {
    // Beyond P3 the XYZ test must fail; the solver clamps into sRGB, so build
    // the XYZ directly from an impossible HCT coordinate instead.
    let conditions = ViewingConditions::default();
    let xyz = Cam16::xyz_from_hct(60.0, 180.0, 140.0, &conditions);
    assert!(!in_p3(xyz), "an out-of-gamut coordinate should leave P3");
  }
}

#[cfg(test)]
mod srgb_gamut_tests {
  use super::*;
  use crate::{Hct, Srgb, Tone};

  #[test]
  fn in_srgb_agrees_with_the_produced_color() {
    // The plane draws its sRGB boundary from `in_srgb`, so it must agree with
    // the color the solver actually produced: a produced color is by definition
    // inside sRGB, and its own XYZ must test as such.
    let conditions = ViewingConditions::default();
    for tone in [30.0, 50.0, 70.0] {
      for hue in (0..360).step_by(30) {
        for chroma in [10.0, 30.0, 50.0, 70.0, 100.0] {
          let solved = Hct::new(f64::from(hue), chroma, Tone::new(tone));
          let cam = Cam16::from_srgb(solved.srgb(), &conditions);
          assert!(
            in_srgb(cam.to_xyz(&conditions)),
            "hue {hue} chroma {chroma} tone {tone}: produced {} tests as out of sRGB",
            solved.srgb()
          );
        }
      }
    }
  }

  #[test]
  fn an_impossible_request_tests_as_out_of_srgb() {
    // Something the gamut cannot hold at all must be reported as outside.
    let conditions = ViewingConditions::default();
    let xyz = Cam16::xyz_from_hct(60.0, 120.0, 140.0, &conditions);
    assert!(!in_srgb(xyz));
  }

  #[test]
  fn p3_is_a_superset_of_srgb() {
    let conditions = ViewingConditions::default();
    for hex in ["#ff0000", "#00ff00", "#0000ff", "#6750a4"] {
      let cam = Cam16::from_srgb(Srgb::from_hex(hex).unwrap(), &conditions);
      let xyz = cam.to_xyz(&conditions);
      assert!(in_srgb(xyz) && in_p3(xyz), "{hex} should be inside both");
    }
  }
}
