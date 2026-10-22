use std::path::Path;

use gpui::{Hsla, Rgba};
use palette::{FromColor, OklabHue, Oklch, Srgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeColors {
  pub background: Hsla,
  pub foreground: Hsla,
  pub accent: Hsla,
  pub accent_foreground: Hsla,
  pub card: Hsla,
  pub card_foreground: Hsla,
  pub popover: Hsla,
  pub popover_foreground: Hsla,
  pub primary: Hsla,
  pub primary_foreground: Hsla,
  pub muted: Hsla,
  pub muted_foreground: Hsla,
  pub border: Hsla,
  pub input: Hsla,
  pub success: Hsla,
  pub warning: Hsla,
  pub danger: Hsla,
  pub info: Hsla,
  pub red: Hsla,
  pub green: Hsla,
  pub blue: Hsla,
  pub yellow: Hsla,
  pub cyan: Hsla,
  pub scrollbar: Hsla,
  pub scrollbar_thumb: Hsla,
  pub scrollbar_thumb_hover: Hsla,
  pub transparent: Hsla,
  pub selection: Hsla,
  pub caret: Hsla,
  pub editor_background: Hsla,
  pub editor_foreground: Hsla,
  pub editor_active_line: Hsla,
  pub editor_line_number: Hsla,
  pub editor_active_line_number: Hsla,
  pub editor_invisible: Hsla,
  pub secondary: Hsla,
  pub secondary_hover: Hsla,
  pub secondary_foreground: Hsla,
  pub tab_bar: Hsla,
  pub tab_bar_segmented: Hsla,
  pub tab_foreground: Hsla,
  pub tab_active: Hsla,
  pub tab_active_foreground: Hsla,
  pub title_bar: Hsla,
  pub drag_border: Hsla,
  pub drop_target: Hsla,
  pub tiles: Hsla,
}

/// Syntax token hues used by the editor highlighter.
///
/// Chroma and lightness come from [`ThemeTokens::chroma`] and
/// [`ThemeTokens::lightness`] to keep a coherent palette.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SyntaxTokenHues {
  pub attribute: f32,
  pub boolean: f32,
  pub comment: f32,
  pub comment_doc: f32,
  pub constant: f32,
  pub constructor: f32,
  pub embedded: f32,
  pub emphasis: f32,
  pub emphasis_strong: f32,
  pub enum_: f32,
  pub function: f32,
  pub hint: f32,
  pub keyword: f32,
  pub label: f32,
  pub link_text: f32,
  pub link_uri: f32,
  pub number: f32,
  pub operator: f32,
  pub predictive: f32,
  pub preproc: f32,
  pub primary: f32,
  pub property: f32,
  pub punctuation: f32,
  pub punctuation_bracket: f32,
  pub punctuation_delimiter: f32,
  pub punctuation_list_marker: f32,
  pub punctuation_special: f32,
  pub string: f32,
  pub string_escape: f32,
  pub string_regex: f32,
  pub string_special: f32,
  pub string_special_symbol: f32,
  pub tag: f32,
  pub tag_doctype: f32,
  pub text_literal: f32,
  pub title: f32,
  pub type_: f32,
  pub variable: f32,
  pub variable_special: f32,
  pub variant: f32,
}

impl Default for SyntaxTokenHues {
  fn default() -> Self {
    // Derived from the Catppuccin Mocha syntax palette. Neutral source-theme
    // tokens are remapped to nearby accent hues because Woocraft stores syntax
    // colors as hue-only accents.
    Self {
      attribute: 86.5,
      boolean: 52.6,
      comment: 277.3,
      comment_doc: 228.7,
      constant: 52.6,
      constructor: 86.5,
      embedded: 18.0,
      emphasis: 2.8,
      emphasis_strong: 2.8,
      enum_: 86.5,
      function: 259.9,
      hint: 210.3,
      keyword: 304.8,
      label: 277.3,
      link_text: 277.3,
      link_uri: 259.9,
      number: 52.6,
      operator: 182.7,
      predictive: 228.7,
      preproc: 86.5,
      primary: 304.8,
      property: 182.7,
      punctuation: 277.3,
      punctuation_bracket: 277.3,
      punctuation_delimiter: 182.7,
      punctuation_list_marker: 182.7,
      punctuation_special: 30.5,
      string: 142.7,
      string_escape: 336.3,
      string_regex: 336.3,
      string_special: 2.8,
      string_special_symbol: 18.0,
      tag: 259.9,
      tag_doctype: 304.8,
      text_literal: 142.7,
      title: 2.8,
      type_: 86.5,
      variable: 8.8,
      variable_special: 2.8,
      variant: 182.7,
    }
  }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeTokens {
  pub primary: f32,
  pub error: f32,
  pub warning: f32,
  pub success: f32,
  pub info: f32,
  #[serde(default = "default_red_hue")]
  pub red: f32,
  #[serde(default = "default_green_hue")]
  pub green: f32,
  #[serde(default = "default_blue_hue")]
  pub blue: f32,
  #[serde(default = "default_yellow_hue")]
  pub yellow: f32,
  #[serde(default = "default_cyan_hue")]
  pub cyan: f32,
  pub accent: f32,
  #[serde(default = "default_secondary_hue")]
  pub secondary: f32,
  pub lightness: f32,
  pub chroma: f32,
  pub light_bg_lightness: f32,
  pub dark_bg_lightness: f32,
  pub light_bg_chroma: f32,
  pub dark_bg_chroma: f32,
  #[serde(default)]
  pub syntax: SyntaxTokenHues,
}

fn default_secondary_hue() -> f32 {
  290.0
}

fn default_red_hue() -> f32 {
  24.0
}

fn default_green_hue() -> f32 {
  150.0
}

fn default_blue_hue() -> f32 {
  248.0
}

fn default_yellow_hue() -> f32 {
  48.0
}

fn default_cyan_hue() -> f32 {
  190.0
}

impl Default for ThemeTokens {
  fn default() -> Self {
    Self {
      primary: 248.0,
      error: 24.0,
      warning: 48.0,
      success: 150.0,
      info: 248.0,
      red: 24.0,
      green: 150.0,
      blue: 248.0,
      yellow: 48.0,
      cyan: 190.0,
      accent: 200.0,
      secondary: 290.0,
      lightness: 0.64,
      chroma: 0.17,
      light_bg_lightness: 0.96,
      dark_bg_lightness: 0.18,
      light_bg_chroma: 0.015,
      dark_bg_chroma: 0.0,
      syntax: SyntaxTokenHues::default(),
    }
  }
}

impl ThemeTokens {
  #[inline]
  pub fn syntax_color(&self, hue: f32) -> Hsla {
    to_hsla_from_oklch(self.lightness, self.chroma, hue, 1.0)
  }

  pub fn from_json_str(input: &str) -> anyhow::Result<Self> {
    Ok(serde_json::from_str(input)?)
  }

  pub fn from_toml_str(input: &str) -> anyhow::Result<Self> {
    Ok(toml::from_str(input)?)
  }

  pub fn from_json_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
    let content = std::fs::read_to_string(path)?;
    Self::from_json_str(&content)
  }

  pub fn from_toml_file(path: impl AsRef<Path>) -> anyhow::Result<Self> {
    let content = std::fs::read_to_string(path)?;
    Self::from_toml_str(&content)
  }
}

fn to_hsla_from_oklch(lightness: f32, chroma: f32, hue_degrees: f32, alpha: f32) -> Hsla {
  let oklch = Oklch::new(lightness, chroma, OklabHue::from_degrees(hue_degrees));
  let rgb: Srgb = Srgb::from_color(oklch);
  let rgba = Rgba {
    r: rgb.red.clamp(0.0, 1.0),
    g: rgb.green.clamp(0.0, 1.0),
    b: rgb.blue.clamp(0.0, 1.0),
    a: alpha.clamp(0.0, 1.0),
  };
  rgba.into()
}

fn with_alpha(color: Hsla, alpha: f32) -> Hsla {
  Hsla {
    a: alpha.clamp(0.0, 1.0),
    ..color
  }
}

/// Multiplies a colour's alpha by `factor` instead of replacing it.
fn scale_alpha(color: Hsla, factor: f32) -> Hsla {
  Hsla {
    a: (color.a * factor).clamp(0.0, 1.0),
    ..color
  }
}

/// OKLCH lightness of the near-white text candidate.
const LIGHT_TEXT_LEVEL: f32 = 0.97;
/// OKLCH lightness of the near-black text candidate.
const DARK_TEXT_LEVEL: f32 = 0.03;

/// sRGB relative luminance, using the APCA main tone response curve (2.4).
fn srgb_to_y(color: Hsla) -> f32 {
  let rgb = color.to_rgb();
  let linearize = |c: f32| c.clamp(0.0, 1.0).powf(2.4);

  const R: f32 = 0.2126729;
  const G: f32 = 0.7151522;
  const B: f32 = 0.0721750;

  R * linearize(rgb.r) + G * linearize(rgb.g) + B * linearize(rgb.b)
}

/// Perceptual lightness contrast (`Lc`) of `text` on `background`, following
/// SAPC-8 / APCA W3 0.1.9 with the G-4g constants.
///
/// Positive values are dark text on a lighter background, negative values are
/// light text on a darker background. Unlike the symmetric WCAG 2.x contrast
/// ratio, the two polarities use different exponents, which is what makes
/// mid-tone surfaces resolve to light text (crossover near a background
/// luminance of 0.32, i.e. OKLCH lightness ~0.68, for a near-white candidate)
/// instead of dark text (crossover near 0.18 for the WCAG ratio).
fn apca_lc(text: Hsla, background: Hsla) -> f32 {
  const BLACK_THRESHOLD: f32 = 0.022;
  const BLACK_CLAMP: f32 = 1.414;
  const DELTA_Y_MIN: f32 = 0.0005;
  const LOW_CLIP: f32 = 0.1;
  const SCALE: f32 = 1.14;
  const OFFSET: f32 = 0.027;

  let soft_clamp = |y: f32| {
    if y > BLACK_THRESHOLD {
      y
    } else {
      y + (BLACK_THRESHOLD - y).powf(BLACK_CLAMP)
    }
  };

  let text_y = soft_clamp(srgb_to_y(text));
  let background_y = soft_clamp(srgb_to_y(background));

  if (background_y - text_y).abs() < DELTA_Y_MIN {
    return 0.0;
  }

  if background_y > text_y {
    let sapc = (background_y.powf(0.56) - text_y.powf(0.57)) * SCALE;

    if sapc < LOW_CLIP {
      0.0
    } else {
      (sapc - OFFSET) * 100.0
    }
  } else {
    let sapc = (background_y.powf(0.65) - text_y.powf(0.62)) * SCALE;

    if sapc > -LOW_CLIP {
      0.0
    } else {
      (sapc + OFFSET) * 100.0
    }
  }
}

fn pick_readable_text(background: Hsla, light_text: Hsla, dark_text: Hsla) -> Hsla {
  let light_contrast = apca_lc(light_text, background).abs();
  let dark_contrast = apca_lc(dark_text, background).abs();

  if light_contrast >= dark_contrast {
    light_text
  } else {
    dark_text
  }
}

impl ThemeColors {
  pub fn from_tokens(tokens: ThemeTokens, is_dark: bool) -> Self {
    let bg_lightness = if is_dark {
      tokens.dark_bg_lightness
    } else {
      tokens.light_bg_lightness
    };
    let bg_chroma = if is_dark {
      tokens.dark_bg_chroma
    } else {
      tokens.light_bg_chroma
    };

    let card_lightness = if is_dark {
      (bg_lightness + 0.04).clamp(0.0, 1.0)
    } else {
      (bg_lightness + 0.02).clamp(0.0, 1.0)
    };
    let muted_lightness = ((bg_lightness + tokens.lightness) * 0.5).clamp(0.0, 1.0);

    let background = to_hsla_from_oklch(bg_lightness, bg_chroma, tokens.primary, 1.0);
    // Candidates are pinned near the ends of the lightness scale: a mid-tone
    // fill can only reach a usable `Lc` against a near-white or near-black
    // foreground, so deriving them from the background lightness would leave
    // every saturated fill with a low-contrast winner.
    let light_text = to_hsla_from_oklch(LIGHT_TEXT_LEVEL, 0.01, tokens.primary, 1.0);
    let dark_text = to_hsla_from_oklch(DARK_TEXT_LEVEL, 0.01, tokens.primary, 1.0);

    let card = to_hsla_from_oklch(card_lightness, bg_chroma, tokens.primary, 1.0);
    let accent = to_hsla_from_oklch(muted_lightness, bg_chroma + 0.03, tokens.accent, 1.0);
    // Muted is a neutral surface: background-derived lightness with the
    // primary hue at very low chroma, like a solid version of the
    // border/input overlay.
    let muted = to_hsla_from_oklch(muted_lightness, bg_chroma + 0.01, tokens.primary, 1.0);
    // Secondary is its own purple-leaning surface so it no longer collapses
    // into muted.
    let secondary = to_hsla_from_oklch(muted_lightness, bg_chroma + 0.03, tokens.secondary, 1.0);
    let popover = background;
    let primary = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.primary, 1.0);

    // Every surface picks its own foreground. The `muted_lightness` derived
    // surfaces are mid-tones whenever the brand lightness is dark, so they
    // cross the polarity boundary long before the page background does.
    let foreground = pick_readable_text(background, light_text, dark_text);
    let card_foreground = pick_readable_text(card, light_text, dark_text);
    let accent_foreground = pick_readable_text(accent, light_text, dark_text);
    let popover_foreground = foreground;
    let primary_foreground = pick_readable_text(primary, light_text, dark_text);
    let muted_foreground = with_alpha(pick_readable_text(muted, light_text, dark_text), 0.75);
    let secondary_foreground = pick_readable_text(secondary, light_text, dark_text);

    let border = with_alpha(foreground, 0.1);
    let input = border;
    // Each semantic colour is derived from its own hue token, so changing one
    // hue never moves another colour.
    let info = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.info, 1.0);
    let success = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.success, 1.0);
    let warning = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.warning, 1.0);
    let danger = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.error, 1.0);
    let red = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.red, 1.0);
    let green = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.green, 1.0);
    let blue = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.blue, 1.0);
    let yellow = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.yellow, 1.0);
    let cyan = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.cyan, 1.0);
    let scrollbar = with_alpha(background, 0.4);
    let scrollbar_thumb = with_alpha(foreground, 0.4);
    let scrollbar_thumb_hover = with_alpha(foreground, 0.6);
    let transparent = gpui::transparent_white();
    let selection = with_alpha(primary, 0.3);
    let caret = foreground;
    let editor_background = background;
    let editor_foreground = foreground;
    let editor_active_line = with_alpha(foreground, if is_dark { 0.08 } else { 0.05 });
    let editor_line_number = with_alpha(foreground, if is_dark { 0.45 } else { 0.4 });
    let editor_active_line_number = foreground;
    let editor_invisible = with_alpha(foreground, 0.4);
    let secondary_hover_lightness = if is_dark {
      muted_lightness + 0.04
    } else {
      muted_lightness - 0.04
    }
    .clamp(0.0, 1.0);
    let secondary_hover = to_hsla_from_oklch(
      secondary_hover_lightness,
      bg_chroma + 0.035,
      tokens.secondary,
      1.0,
    );
    let tab_bar = card;
    let tab_bar_segmented = muted;
    let tab_foreground = muted_foreground;
    let tab_active = background;
    let tab_active_foreground = foreground;
    let title_bar = card;
    let drag_border = primary;
    let drop_target = with_alpha(primary, 0.2);
    let tiles = card;

    Self {
      background,
      foreground,
      accent,
      accent_foreground,
      card,
      card_foreground,
      popover,
      popover_foreground,
      primary,
      primary_foreground,
      muted,
      muted_foreground,
      border,
      input,
      success,
      warning,
      danger,
      info,
      red,
      green,
      blue,
      yellow,
      cyan,
      scrollbar,
      scrollbar_thumb,
      scrollbar_thumb_hover,
      transparent,
      selection,
      caret,
      editor_background,
      editor_foreground,
      editor_active_line,
      editor_line_number,
      editor_active_line_number,
      editor_invisible,
      secondary,
      secondary_hover,
      secondary_foreground,
      tab_bar,
      tab_bar_segmented,
      tab_foreground,
      tab_active,
      tab_active_foreground,
      title_bar,
      drag_border,
      drop_target,
      tiles,
    }
  }

  pub fn disabled(&self) -> Self {
    self.with_alpha(0.6)
  }

  /// Multiplies the alpha of every token, so already-translucent tokens
  /// (borders, overlays, muted text) keep their relative transparency.
  pub fn with_alpha(&self, alpha: f32) -> Self {
    Self {
      background: scale_alpha(self.background, alpha),
      foreground: scale_alpha(self.foreground, alpha),
      accent: scale_alpha(self.accent, alpha),
      accent_foreground: scale_alpha(self.accent_foreground, alpha),
      card: scale_alpha(self.card, alpha),
      card_foreground: scale_alpha(self.card_foreground, alpha),
      popover: scale_alpha(self.popover, alpha),
      popover_foreground: scale_alpha(self.popover_foreground, alpha),
      primary: scale_alpha(self.primary, alpha),
      primary_foreground: scale_alpha(self.primary_foreground, alpha),
      muted: scale_alpha(self.muted, alpha),
      muted_foreground: scale_alpha(self.muted_foreground, alpha),
      border: scale_alpha(self.border, alpha),
      input: scale_alpha(self.input, alpha),
      success: scale_alpha(self.success, alpha),
      warning: scale_alpha(self.warning, alpha),
      danger: scale_alpha(self.danger, alpha),
      info: scale_alpha(self.info, alpha),
      red: scale_alpha(self.red, alpha),
      green: scale_alpha(self.green, alpha),
      blue: scale_alpha(self.blue, alpha),
      yellow: scale_alpha(self.yellow, alpha),
      cyan: scale_alpha(self.cyan, alpha),
      scrollbar: scale_alpha(self.scrollbar, alpha),
      scrollbar_thumb: scale_alpha(self.scrollbar_thumb, alpha),
      scrollbar_thumb_hover: scale_alpha(self.scrollbar_thumb_hover, alpha),
      transparent: scale_alpha(self.transparent, alpha),
      selection: scale_alpha(self.selection, alpha),
      caret: scale_alpha(self.caret, alpha),
      editor_background: scale_alpha(self.editor_background, alpha),
      editor_foreground: scale_alpha(self.editor_foreground, alpha),
      editor_active_line: scale_alpha(self.editor_active_line, alpha),
      editor_line_number: scale_alpha(self.editor_line_number, alpha),
      editor_active_line_number: scale_alpha(self.editor_active_line_number, alpha),
      editor_invisible: scale_alpha(self.editor_invisible, alpha),
      secondary: scale_alpha(self.secondary, alpha),
      secondary_hover: scale_alpha(self.secondary_hover, alpha),
      secondary_foreground: scale_alpha(self.secondary_foreground, alpha),
      tab_bar: scale_alpha(self.tab_bar, alpha),
      tab_bar_segmented: scale_alpha(self.tab_bar_segmented, alpha),
      tab_foreground: scale_alpha(self.tab_foreground, alpha),
      tab_active: scale_alpha(self.tab_active, alpha),
      tab_active_foreground: scale_alpha(self.tab_active_foreground, alpha),
      title_bar: scale_alpha(self.title_bar, alpha),
      drag_border: scale_alpha(self.drag_border, alpha),
      drop_target: scale_alpha(self.drop_target, alpha),
      tiles: scale_alpha(self.tiles, alpha),
    }
  }

  pub fn light() -> Self {
    Self::from_tokens(ThemeTokens::default(), false)
  }

  pub fn dark() -> Self {
    Self::from_tokens(ThemeTokens::default(), true)
  }
}

#[cfg(test)]
mod tests {
  use std::io::Write;

  use super::{ThemeColors, ThemeTokens};

  #[test]
  fn border_uses_text_alpha_rule() {
    let colors = ThemeColors::from_tokens(ThemeTokens::default(), false);
    assert!((colors.border.a - 0.1).abs() < 1e-6);
    assert!((colors.border.h - colors.foreground.h).abs() < 1e-6);
    assert!((colors.border.s - colors.foreground.s).abs() < 1e-6);
    assert!((colors.border.l - colors.foreground.l).abs() < 1e-6);
  }

  #[test]
  fn disabled_multiplies_alpha() {
    let colors = ThemeColors::light().disabled();
    assert!((colors.background.a - 0.6).abs() < 1e-6);
    assert!((colors.foreground.a - 0.6).abs() < 1e-6);
    assert!((colors.success.a - 0.6).abs() < 1e-6);
    assert!((colors.primary.a - 0.6).abs() < 1e-6);
    assert!((colors.border.a - 0.06).abs() < 1e-6);
    assert!((colors.muted_foreground.a - 0.45).abs() < 1e-6);
    assert!((colors.selection.a - 0.18).abs() < 1e-6);
    assert!((colors.scrollbar.a - 0.24).abs() < 1e-6);
    assert!((colors.scrollbar_thumb_hover.a - 0.36).abs() < 1e-6);
    assert!((colors.drop_target.a - 0.12).abs() < 1e-6);
  }

  #[test]
  fn with_alpha_multiplies_existing_alpha() {
    let colors = ThemeColors::light().with_alpha(0.5);
    assert!((colors.border.a - 0.05).abs() < 1e-6);
    assert!((colors.selection.a - 0.15).abs() < 1e-6);
  }

  #[test]
  fn surface_foregrounds_follow_their_own_surface() {
    let tokens = ThemeTokens {
      lightness: 0.24,
      ..ThemeTokens::default()
    };
    let colors = ThemeColors::from_tokens(tokens, false);

    // The page background is still light, so page text is dark.
    assert!(super::apca_lc(colors.foreground, colors.background) > 0.0);

    // But surfaces derived from `muted_lightness` are now mid-tones: reusing
    // the page foreground on them would have picked dark text.
    for (name, surface, foreground) in [
      ("card", colors.card, colors.card_foreground),
      ("accent", colors.accent, colors.accent_foreground),
      ("muted", colors.muted, colors.muted_foreground),
      ("secondary", colors.secondary, colors.secondary_foreground),
    ] {
      let light = super::apca_lc(colors.foreground, surface).abs();
      let chosen = super::apca_lc(foreground, surface).abs();

      assert!(chosen >= light, "{name}: {chosen} should beat {light}");
    }

    assert!(super::apca_lc(colors.accent_foreground, colors.accent) < 0.0);
    assert!(super::apca_lc(colors.secondary_foreground, colors.secondary) < 0.0);
  }

  #[test]
  fn disabled_keeps_transparent_transparent() {
    let colors = ThemeColors::light().disabled();

    assert!(colors.transparent.a <= f32::EPSILON);
  }

  #[test]
  fn apca_matches_reference_values() {
    let white = gpui::white();
    let black = gpui::black();

    assert!((super::apca_lc(black, white) - 106.04).abs() < 0.05);
    assert!((super::apca_lc(white, black) + 107.88).abs() < 0.05);
  }

  #[test]
  fn foreground_polarity_matches_background() {
    let tokens = ThemeTokens::default();
    let light_colors = ThemeColors::from_tokens(tokens, false);
    let dark_colors = ThemeColors::from_tokens(tokens, true);

    // Dark text on the light background, light text on the dark background.
    assert!(super::apca_lc(light_colors.foreground, light_colors.background) > 0.0);
    assert!(super::apca_lc(dark_colors.foreground, dark_colors.background) < 0.0);
  }

  #[test]
  fn mid_tone_fills_prefer_light_text() {
    let tokens = ThemeTokens::default();
    let colors = ThemeColors::from_tokens(tokens, false);
    let light_text = super::to_hsla_from_oklch(super::LIGHT_TEXT_LEVEL, 0.01, tokens.primary, 1.0);
    let dark_text = super::to_hsla_from_oklch(super::DARK_TEXT_LEVEL, 0.01, tokens.primary, 1.0);

    // Every accent hue at the default lightness sits in the band where a
    // polarisation-aware metric prefers light text, even though the symmetric
    // WCAG 2.x ratio prefers dark text here.
    let fills = [
      ("primary", colors.primary),
      ("info", colors.info),
      ("success", colors.success),
      ("warning", colors.warning),
      ("danger", colors.danger),
      ("cyan", colors.cyan),
    ];

    for (name, fill) in fills {
      let light = super::apca_lc(light_text, fill).abs();
      let dark = super::apca_lc(dark_text, fill).abs();

      assert!(
        light > dark,
        "{name}: light {light} should beat dark {dark}"
      );
      // At the default lightness the light candidate lands between 53 and 62
      // `Lc` across the token hues; the darkest fills are the near-black
      // winners, so anything below this means the candidate got dimmed again.
      assert!(
        light >= 50.0,
        "{name}: light text contrast {light} is too low"
      );
    }

    assert!((colors.primary_foreground.l - light_text.l).abs() < 1e-6);
  }

  #[test]
  fn semantic_colors_are_derived_from_their_own_hue() {
    let tokens = ThemeTokens {
      primary: 0.0,
      error: 60.0,
      warning: 120.0,
      success: 180.0,
      info: 240.0,
      red: 270.0,
      green: 300.0,
      blue: 330.0,
      yellow: 15.0,
      cyan: 345.0,
      ..ThemeTokens::default()
    };
    let colors = ThemeColors::from_tokens(tokens, false);

    fn assert_channel_eq(name: &str, channel: &str, actual: f32, expected: f32) {
      assert!(
        (actual - expected).abs() < 1e-6,
        "{name}.{channel}: {actual} != {expected}"
      );
    }

    for (name, actual, hue) in [
      ("primary", colors.primary, tokens.primary),
      ("info", colors.info, tokens.info),
      ("success", colors.success, tokens.success),
      ("warning", colors.warning, tokens.warning),
      ("danger", colors.danger, tokens.error),
      ("red", colors.red, tokens.red),
      ("green", colors.green, tokens.green),
      ("blue", colors.blue, tokens.blue),
      ("yellow", colors.yellow, tokens.yellow),
      ("cyan", colors.cyan, tokens.cyan),
    ] {
      let expected = super::to_hsla_from_oklch(tokens.lightness, tokens.chroma, hue, 1.0);
      assert_channel_eq(name, "h", actual.h, expected.h);
      assert_channel_eq(name, "s", actual.s, expected.s);
      assert_channel_eq(name, "l", actual.l, expected.l);
    }
  }

  #[test]
  fn parse_tokens_from_json_and_toml() {
    let json = r#"{
            "primary": 248,
            "error": 24,
            "warning": 48,
            "success": 150,
            "info": 248,
            "accent": 130,
            "lightness": 0.64,
            "chroma": 0.17,
            "light_bg_lightness": 0.96,
            "dark_bg_lightness": 0.18,
            "light_bg_chroma": 0.015,
            "dark_bg_chroma": 0.015
        }"#;
    let toml = r#"
            primary = 248
            error = 24
            warning = 48
            success = 150
            info = 248
            accent = 130
            lightness = 0.64
            chroma = 0.17
            light_bg_lightness = 0.96
            dark_bg_lightness = 0.18
            light_bg_chroma = 0.015
            dark_bg_chroma = 0.015
        "#;

    let from_json = ThemeTokens::from_json_str(json).expect("json tokens should parse");
    let from_toml = ThemeTokens::from_toml_str(toml).expect("toml tokens should parse");

    assert_eq!(from_json.primary, 248.0);
    assert_eq!(from_toml.primary, 248.0);
    assert_eq!(from_json.dark_bg_chroma, 0.015);
    assert_eq!(from_toml.dark_bg_chroma, 0.015);

    // Theme files written before the spectrum hue tokens existed still parse
    // and fall back to their default hues.
    assert_eq!(from_json.red, 24.0);
    assert_eq!(from_json.green, 150.0);
    assert_eq!(from_json.blue, 248.0);
    assert_eq!(from_json.yellow, 48.0);
    assert_eq!(from_json.cyan, 190.0);
    assert_eq!(from_toml.red, 24.0);
    assert_eq!(from_toml.green, 150.0);
    assert_eq!(from_toml.blue, 248.0);
    assert_eq!(from_toml.yellow, 48.0);
    assert_eq!(from_toml.cyan, 190.0);
  }

  #[test]
  fn parse_tokens_from_files() {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let json_path = dir.join(format!("woocraft-theme-tokens-{pid}.json"));
    let toml_path = dir.join(format!("woocraft-theme-tokens-{pid}.toml"));

    let mut json_file = std::fs::File::create(&json_path).expect("create json file");
    let mut toml_file = std::fs::File::create(&toml_path).expect("create toml file");

    write!(
            json_file,
            "{{\"primary\":248,\"error\":24,\"warning\":48,\"success\":150,\"info\":248,\"accent\":130,\"lightness\":0.64,\"chroma\":0.17,\"light_bg_lightness\":0.96,\"dark_bg_lightness\":0.18,\"light_bg_chroma\":0.015,\"dark_bg_chroma\":0.015}}"
        )
        .expect("write json file");

    write!(
            toml_file,
            "primary = 248\nerror = 24\nwarning = 48\nsuccess = 150\ninfo = 248\naccent = 130\nlightness = 0.64\nchroma = 0.17\nlight_bg_lightness = 0.96\ndark_bg_lightness = 0.18\nlight_bg_chroma = 0.015\ndark_bg_chroma = 0.015\n"
        )
        .expect("write toml file");

    let from_json = ThemeTokens::from_json_file(&json_path).expect("read json tokens");
    let from_toml = ThemeTokens::from_toml_file(&toml_path).expect("read toml tokens");

    assert_eq!(from_json.info, 248.0);
    assert_eq!(from_toml.info, 248.0);

    let _ = std::fs::remove_file(&json_path);
    let _ = std::fs::remove_file(&toml_path);
  }
}
