use std::path::Path;

use gpui::Hsla;
use serde::{Deserialize, Serialize};
use woocraft_colors::{Contrast, Hct, Tone};

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
/// Chroma and tone come from [`ThemeTokens::chroma`] and the fill tones, so a
/// theme's syntax colors sit at the same tone as its semantic fills.
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

/// Hue, chroma and tone tokens a theme is built from.
///
/// Every hue is a CAM16 hue angle. CAM16 does not distribute hues evenly: the
/// arc that reads as orange-to-amber is wide (roughly 50..100) while red sits
/// near 27 and yellow near 111, so hues chosen by eye in another space will
/// land closer together here than expected. The defaults below are spaced by
/// measured perceptual distance rather than by equal angles.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeTokens {
  /// Brand hue.
  pub primary: f32,
  /// Hue for errors and destructive actions.
  pub error: f32,
  /// Hue for warnings and caution states.
  pub warning: f32,
  /// Hue for success and confirmation states.
  pub success: f32,
  /// Hue for informational states.
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
  /// Chroma of the semantic fills: primary, success, warning, danger and the
  /// named accents. Shared, so a theme has one saturation level.
  ///
  /// This is HCT chroma, which is bounded by the sRGB gamut differently at
  /// every hue and tone. A fill whose hue cannot hold this chroma at its tone
  /// gets as much as the gamut allows, and the rest is dropped — hue and tone
  /// are never sacrificed for it.
  pub chroma: f32,
  /// Tone of the semantic fills in a light theme.
  pub light_fill_tone: f32,
  /// Tone of the semantic fills in a dark theme.
  pub dark_fill_tone: f32,
  /// Tone of the warning fill, as a step away from the theme's fill tone.
  ///
  /// Warning is the one role whose hue cannot hold chroma at the fill tone: the
  /// amber arc has no saturated sRGB color at tone 40, so a warning painted
  /// there comes out olive and reads as a dull error rather than a caution.
  /// Lifting it a few tones gives the hue room to keep its chroma, which is
  /// also why warning is conventionally brighter than danger.
  pub warning_tone_step: f32,
  /// Tone of the page background in a light theme.
  pub light_bg_tone: f32,
  /// Tone of the page background in a dark theme.
  pub dark_bg_tone: f32,
  /// Chroma of the page background. Surfaces are near-neutral, so this is
  /// small but non-zero: a trace of the primary hue is what keeps a surface
  /// from looking flat grey.
  pub surface_chroma: f32,
  #[serde(default)]
  pub syntax: SyntaxTokenHues,
}

fn default_secondary_hue() -> f32 {
  290.0
}

fn default_red_hue() -> f32 {
  20.0
}

fn default_green_hue() -> f32 {
  150.0
}

fn default_blue_hue() -> f32 {
  248.0
}

fn default_yellow_hue() -> f32 {
  85.0
}

fn default_cyan_hue() -> f32 {
  190.0
}

impl Default for ThemeTokens {
  fn default() -> Self {
    Self {
      primary: 248.0,
      error: 20.0,
      warning: 85.0,
      success: 150.0,
      info: 248.0,
      red: 20.0,
      green: 150.0,
      blue: 248.0,
      yellow: 85.0,
      cyan: 190.0,
      accent: 200.0,
      secondary: 290.0,
      chroma: 48.0,
      light_fill_tone: 40.0,
      dark_fill_tone: 80.0,
      warning_tone_step: 20.0,
      light_bg_tone: 98.0,
      dark_bg_tone: 12.0,
      surface_chroma: 0.6,
      syntax: SyntaxTokenHues::default(),
    }
  }
}

impl ThemeTokens {
  /// A syntax-highlighting color for `hue`.
  ///
  /// Syntax colors sit on text, so they use the fill tone of the active theme
  /// rather than the surface tone: they must read against the editor
  /// background, not blend into it.
  #[inline]
  pub fn syntax_color(&self, hue: f32) -> Hsla {
    self.syntax_color_in_mode(hue, false)
  }

  /// A syntax-highlighting color for `hue`, using the fill tone of `is_dark`.
  pub fn syntax_color_in_mode(&self, hue: f32, is_dark: bool) -> Hsla {
    let tone = self.fill_tone(is_dark);
    hct_to_hsla(Hct::new(f64::from(hue), f64::from(self.chroma), tone), 1.0)
  }

  /// The tone semantic fills use in the given mode.
  pub fn fill_tone(&self, is_dark: bool) -> Tone {
    Tone::new(if is_dark {
      self.dark_fill_tone
    } else {
      self.light_fill_tone
    } as f64)
  }

  /// The tone the page background uses in the given mode.
  pub fn bg_tone(&self, is_dark: bool) -> Tone {
    Tone::new(if is_dark {
      self.dark_bg_tone
    } else {
      self.light_bg_tone
    } as f64)
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

/// Converts an HCT color to the `Hsla` the view layer consumes.
///
/// This is the single boundary between the design system's color reasoning and
/// GPUI: everything above computes in HCT, everything below is HSL.
fn hct_to_hsla(color: Hct, alpha: f32) -> Hsla {
  let hsl = color.hsl();
  Hsla {
    h: (hsl.h as f32) / 360.0,
    s: hsl.s as f32,
    l: hsl.l as f32,
    a: alpha.clamp(0.0, 1.0),
  }
}

/// Replaces a color's alpha, keeping its hue, saturation and lightness.
fn with_alpha(color: Hsla, alpha: f32) -> Hsla {
  Hsla {
    a: alpha.clamp(0.0, 1.0),
    ..color
  }
}

/// Multiplies a color's alpha by `factor` instead of replacing it.
fn scale_alpha(color: Hsla, factor: f32) -> Hsla {
  Hsla {
    a: (color.a * factor).clamp(0.0, 1.0),
    ..color
  }
}

/// A neutral surface at `tone`, tinted with `hue` by the theme's surface
/// chroma.
fn neutral_surface(tokens: &ThemeTokens, tone: Tone, hue: f32) -> Hct {
  Hct::new(f64::from(hue), f64::from(tokens.surface_chroma), tone)
}

/// Picks the neutral grey that is most distinct from `background`.
///
/// Used where a thin line has to stay visible against an arbitrary color — the
/// color picker's indicator over its own gradient. Only greys are considered,
/// because the point is a ring that reads as neutral structure rather than a
/// second hue competing with the color it surrounds.
///
/// The choice is made on the tone axis, which is what makes it cheap and
/// predictable: the winner is simply the end of the scale further from the
/// background's tone, and the distance in tone is a contrast ratio. No
/// threshold, no candidate list.
pub fn contrasting_neutral(background: Hsla) -> Hsla {
  let tone = tone_of_hsla(background);
  let grey = if tone.value() < 50.0 {
    Tone::MAX
  } else {
    Tone::MIN
  };
  hct_to_hsla(Hct::new(0.0, 0.0, grey), 1.0)
}

/// The HCT color behind an `Hsla`.
fn hct_of_hsla(color: Hsla) -> Hct {
  let hsl = woocraft_colors::Hsl {
    h: f64::from(color.h) * 360.0,
    s: f64::from(color.s),
    l: f64::from(color.l),
    a: f64::from(color.a),
  };
  Hct::from_srgb(hsl.to_srgb())
}

/// The tone of an `Hsla`.
///
/// `Hsla` stores hue as a fraction of a turn while `Hsl` uses degrees, so the
/// conversion goes through the crate's own encoding rather than reinterpreting
/// channels.
fn tone_of_hsla(color: Hsla) -> Tone {
  hct_of_hsla(color).tone()
}

impl ThemeColors {
  /// Derives the full token set from the theme's hue, chroma and tone tokens.
  ///
  /// Every lightness in here is a tone, and every foreground is chosen by
  /// solving for a contrast ratio against its own surface rather than by
  /// picking the lighter of two candidates. That is what makes the accessible
  /// pairings hold by construction: a tone difference determines a contrast
  /// ratio, so `foreground` is guaranteed to reach
  /// [`Contrast::BODY_TEXT`] against `background` whatever the hue.
  pub fn from_tokens(tokens: ThemeTokens, is_dark: bool) -> Self {
    let bg_tone = tokens.bg_tone(is_dark);
    let fill_tone = tokens.fill_tone(is_dark);
    let hue = tokens.primary;

    // Surfaces step away from the background by a fixed number of tones, so the
    // layering reads the same in both modes: `card` is the raised surface and
    // `muted` the recessed one, both close to the page so they read as the same
    // material. Stepping toward the fills instead would land `muted` in the
    // mid-tones, where a foreground can only be chosen by throwing it to the
    // far end of the scale.
    let card_tone = bg_tone + if is_dark { 4.0 } else { -3.0 };
    let muted_tone = bg_tone + if is_dark { -4.0 } else { -3.0 };

    let background = neutral_surface(&tokens, bg_tone, hue);
    let card = neutral_surface(&tokens, card_tone, hue);
    let muted = neutral_surface(&tokens, muted_tone, hue);
    let accent = Hct::new(
      f64::from(tokens.accent),
      f64::from(tokens.chroma),
      muted_tone,
    );
    let secondary = Hct::new(
      f64::from(tokens.secondary),
      f64::from(tokens.chroma),
      muted_tone,
    );
    let popover = background;

    // Fills share one tone, so they are all equally prominent and all reach
    // the same contrast against the page. Their hues differ; their
    // accessibility does not.
    let primary = Hct::new(f64::from(hue), f64::from(tokens.chroma), fill_tone);
    let info = Hct::new(f64::from(tokens.info), f64::from(tokens.chroma), fill_tone);
    let success = Hct::new(
      f64::from(tokens.success),
      f64::from(tokens.chroma),
      fill_tone,
    );
    // Warning steps away from the fills *toward the background*, which is the
    // direction with headroom in both modes: lighter in a light theme, darker
    // in a dark one. Stepping up unconditionally would run a dark theme's
    // warning off the top of the scale into white.
    let warning_tone = if is_dark {
      fill_tone - f64::from(tokens.warning_tone_step)
    } else {
      fill_tone + f64::from(tokens.warning_tone_step)
    };
    let warning = Hct::new(
      f64::from(tokens.warning),
      f64::from(tokens.chroma),
      warning_tone,
    );
    let danger = Hct::new(f64::from(tokens.error), f64::from(tokens.chroma), fill_tone);
    let red = Hct::new(f64::from(tokens.red), f64::from(tokens.chroma), fill_tone);
    let green = Hct::new(f64::from(tokens.green), f64::from(tokens.chroma), fill_tone);
    let blue = Hct::new(f64::from(tokens.blue), f64::from(tokens.chroma), fill_tone);
    let yellow = Hct::new(
      f64::from(tokens.yellow),
      f64::from(tokens.chroma),
      fill_tone,
    );
    let cyan = Hct::new(f64::from(tokens.cyan), f64::from(tokens.chroma), fill_tone);

    // Every surface picks a foreground that reaches the enhanced ratio against
    // it. No candidate list, no audit: the tone axis answers this, and asking
    // for more than the minimum keeps body text solidly readable instead of
    // landing at the threshold.
    let foreground = background.foreground(Contrast::ENHANCED);
    let card_foreground = card.foreground(Contrast::ENHANCED);
    let popover_foreground = foreground;
    let accent_foreground = accent.foreground(Contrast::ENHANCED);
    let primary_foreground = primary.foreground(Contrast::BODY_TEXT);
    let muted_foreground = with_alpha(
      hct_to_hsla(muted.foreground(Contrast::BODY_TEXT), 1.0),
      0.75,
    );
    let secondary_foreground = secondary.foreground(Contrast::ENHANCED);

    let border = with_alpha(hct_to_hsla(foreground, 1.0), 0.1);
    let input = border;

    let scrollbar = with_alpha(hct_to_hsla(background, 1.0), 0.4);
    let scrollbar_thumb = with_alpha(hct_to_hsla(foreground, 1.0), 0.4);
    let scrollbar_thumb_hover = with_alpha(hct_to_hsla(foreground, 1.0), 0.6);
    let transparent = gpui::transparent_white();
    let selection = with_alpha(hct_to_hsla(primary, 1.0), 0.3);
    let caret = hct_to_hsla(foreground, 1.0);
    let editor_background = hct_to_hsla(background, 1.0);
    let editor_foreground = hct_to_hsla(foreground, 1.0);
    let editor_active_line = with_alpha(
      hct_to_hsla(foreground, 1.0),
      if is_dark { 0.08 } else { 0.05 },
    );
    let editor_line_number = with_alpha(
      hct_to_hsla(foreground, 1.0),
      if is_dark { 0.45 } else { 0.4 },
    );
    let editor_active_line_number = hct_to_hsla(foreground, 1.0);
    let editor_invisible = with_alpha(hct_to_hsla(foreground, 1.0), 0.4);
    // The hover step moves *toward* the foreground, so it reads as a lift in
    // both modes instead of depending on a hard-coded sign.
    let secondary_hover = secondary.with_tone(if is_dark {
      muted_tone + 4.0
    } else {
      muted_tone - 4.0
    });
    let tab_bar = hct_to_hsla(card, 1.0);
    let tab_bar_segmented = hct_to_hsla(muted, 1.0);
    let tab_foreground = muted_foreground;
    let tab_active = hct_to_hsla(background, 1.0);
    let tab_active_foreground = hct_to_hsla(foreground, 1.0);
    let title_bar = hct_to_hsla(card, 1.0);
    let drag_border = hct_to_hsla(primary, 1.0);
    let drop_target = with_alpha(hct_to_hsla(primary, 1.0), 0.2);
    let tiles = hct_to_hsla(card, 1.0);

    Self {
      background: hct_to_hsla(background, 1.0),
      foreground: hct_to_hsla(foreground, 1.0),
      accent: hct_to_hsla(accent, 1.0),
      accent_foreground: hct_to_hsla(accent_foreground, 1.0),
      card: hct_to_hsla(card, 1.0),
      card_foreground: hct_to_hsla(card_foreground, 1.0),
      popover: hct_to_hsla(popover, 1.0),
      popover_foreground: hct_to_hsla(popover_foreground, 1.0),
      primary: hct_to_hsla(primary, 1.0),
      primary_foreground: hct_to_hsla(primary_foreground, 1.0),
      muted: hct_to_hsla(muted, 1.0),
      muted_foreground,
      border,
      input,
      success: hct_to_hsla(success, 1.0),
      warning: hct_to_hsla(warning, 1.0),
      danger: hct_to_hsla(danger, 1.0),
      info: hct_to_hsla(info, 1.0),
      red: hct_to_hsla(red, 1.0),
      green: hct_to_hsla(green, 1.0),
      blue: hct_to_hsla(blue, 1.0),
      yellow: hct_to_hsla(yellow, 1.0),
      cyan: hct_to_hsla(cyan, 1.0),
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
      secondary: hct_to_hsla(secondary, 1.0),
      secondary_hover: hct_to_hsla(secondary_hover, 1.0),
      secondary_foreground: hct_to_hsla(secondary_foreground, 1.0),
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

  use woocraft_colors::Contrast;

  use super::{ThemeColors, ThemeTokens, tone_of_hsla};

  /// Contrast ratio between two theme colors, measured on the tone axis.
  fn ratio(a: gpui::Hsla, b: gpui::Hsla) -> f64 {
    Contrast::ratio_of_tones(tone_of_hsla(a), tone_of_hsla(b))
  }

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
  }

  #[test]
  fn with_alpha_multiplies_existing_alpha() {
    let colors = ThemeColors::light().with_alpha(0.5);
    assert!((colors.border.a - 0.05).abs() < 1e-6);
    assert!((colors.selection.a - 0.15).abs() < 1e-6);
  }

  #[test]
  fn disabled_keeps_transparent_transparent() {
    let colors = ThemeColors::light().disabled();
    assert!(colors.transparent.a <= f32::EPSILON);
  }

  #[test]
  fn every_foreground_reaches_body_text_contrast() {
    // The contract the tone axis exists for: each surface's foreground clears
    // the body-text ratio against that surface, in both modes, for any hue.
    for is_dark in [false, true] {
      for primary in [0.0, 60.0, 120.0, 200.0, 300.0] {
        let tokens = ThemeTokens {
          primary,
          ..ThemeTokens::default()
        };
        let colors = ThemeColors::from_tokens(tokens, is_dark);

        for (name, surface, foreground) in [
          ("background", colors.background, colors.foreground),
          ("card", colors.card, colors.card_foreground),
          ("popover", colors.popover, colors.popover_foreground),
          ("accent", colors.accent, colors.accent_foreground),
          ("primary", colors.primary, colors.primary_foreground),
          ("secondary", colors.secondary, colors.secondary_foreground),
        ] {
          let achieved = ratio(surface, foreground);
          assert!(
            achieved >= Contrast::BODY_TEXT - 0.05,
            "{name} foreground at primary hue {primary} (dark: {is_dark}) \
             only reaches {achieved:.2}:1"
          );
        }
      }
    }
  }

  #[test]
  fn muted_foreground_is_readable_despite_its_alpha() {
    // Muted text is deliberately translucent, so it cannot be checked by the
    // tone contract; what must hold is that the opaque version is readable and
    // that the translucency is the only thing reducing it.
    let colors = ThemeColors::light();
    assert!((colors.muted_foreground.a - 0.75).abs() < 1e-6);
    let opaque = super::with_alpha(colors.muted_foreground, 1.0);
    assert!(ratio(colors.muted, opaque) >= Contrast::BODY_TEXT - 0.05);
  }

  #[test]
  fn fills_share_one_tone_across_hues() {
    // Accessibility must not depend on which hue a semantic color happens to
    // use, so every fill sits at the same tone.
    for is_dark in [false, true] {
      let colors = ThemeColors::from_tokens(ThemeTokens::default(), is_dark);
      let reference = tone_of_hsla(colors.primary);

      for (name, fill) in [
        ("info", colors.info),
        ("success", colors.success),
        ("danger", colors.danger),
        ("red", colors.red),
        ("green", colors.green),
        ("blue", colors.blue),
        ("yellow", colors.yellow),
        ("cyan", colors.cyan),
      ] {
        let tone = tone_of_hsla(fill);
        assert!(
          (tone.value() - reference.value()).abs() < 0.5,
          "{name} tone {} != primary tone {} (dark: {is_dark})",
          tone.value(),
          reference.value()
        );
      }
    }
  }

  #[test]
  fn warning_and_danger_stay_distinguishable() {
    // Both are red-adjacent states, so the theme has to keep them apart by more
    // than a few degrees of hue. CAM16 distances are measured in UCS, where
    // ~5 is one just-noticeable step for large fields; 10 keeps the two states
    // clearly different at a glance rather than merely not identical.
    for is_dark in [false, true] {
      let colors = ThemeColors::from_tokens(ThemeTokens::default(), is_dark);
      let warning = super::hct_of_hsla(colors.warning);
      let danger = super::hct_of_hsla(colors.danger);

      let distance = warning.distance(danger);
      assert!(
        distance >= 10.0,
        "warning {} and danger {} are only {distance:.2} apart (dark: {is_dark})",
        warning.srgb(),
        danger.srgb()
      );

      // And they must not be told apart by tone alone: a warning and an error
      // at the same lightness would still read as the same colour.
      let hue_delta = (warning.hue() - danger.hue()).abs() % 360.0;
      let hue_delta = hue_delta.min(360.0 - hue_delta);
      assert!(
        hue_delta >= 40.0,
        "warning and danger are only {hue_delta:.1}° apart (dark: {is_dark})"
      );
    }
  }

  #[test]
  fn warning_keeps_its_chroma() {
    // A warning painted at the shared fill tone comes out olive because the
    // amber arc cannot hold chroma that low. The step exists to prevent that,
    // so assert the result is actually saturated.
    let colors = ThemeColors::light();
    let warning = super::hct_of_hsla(colors.warning);
    let danger = super::hct_of_hsla(colors.danger);

    assert!(
      warning.chroma() >= danger.chroma() * 0.9,
      "warning chroma {:.1} should be comparable to danger's {:.1}",
      warning.chroma(),
      danger.chroma()
    );
  }

  #[test]
  fn foreground_polarity_matches_background() {
    let tokens = ThemeTokens::default();
    let light_colors = ThemeColors::from_tokens(tokens, false);
    let dark_colors = ThemeColors::from_tokens(tokens, true);

    // Dark text on the light background, light text on the dark background.
    assert!(
      tone_of_hsla(light_colors.foreground).value() < tone_of_hsla(light_colors.background).value()
    );
    assert!(
      tone_of_hsla(dark_colors.foreground).value() > tone_of_hsla(dark_colors.background).value()
    );
  }

  #[test]
  fn card_steps_away_from_the_background() {
    for is_dark in [false, true] {
      let colors = ThemeColors::from_tokens(ThemeTokens::default(), is_dark);
      let bg = tone_of_hsla(colors.background).value();
      let card = tone_of_hsla(colors.card).value();

      if is_dark {
        assert!(card > bg, "a raised surface should be lighter in dark mode");
      } else {
        assert!(card < bg, "a raised surface should be darker in light mode");
      }
    }
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

    // Each semantic fill carries its own hue token. Hue is measured in HCT, so
    // it is read back through the color rather than from the HSL channels.
    for (name, fill, hue) in [
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
      let hct = super::hct_of_hsla(fill);
      let delta = (hct.hue() - f64::from(hue)).abs() % 360.0;
      let delta = delta.min(360.0 - delta);
      assert!(delta < 3.0, "{name} hue {} != token {hue}", hct.hue());
    }
  }

  #[test]
  fn tokens_parse_from_json_and_toml() {
    let json = r#"{
            "primary": 248,
            "error": 24,
            "warning": 48,
            "success": 150,
            "info": 248,
            "accent": 130,
            "chroma": 48,
            "light_fill_tone": 40,
            "dark_fill_tone": 80,
            "light_bg_tone": 98,
            "dark_bg_tone": 12,
            "warning_tone_step": 20,
            "surface_chroma": 0.6
        }"#;
    let toml = r#"
            primary = 248
            error = 24
            warning = 48
            success = 150
            info = 248
            accent = 130
            chroma = 48
            light_fill_tone = 40
            dark_fill_tone = 80
            light_bg_tone = 98
            dark_bg_tone = 12
            warning_tone_step = 20
            surface_chroma = 0.6
        "#;

    let from_json = ThemeTokens::from_json_str(json).expect("json tokens should parse");
    let from_toml = ThemeTokens::from_toml_str(toml).expect("toml tokens should parse");

    assert_eq!(from_json.primary, 248.0);
    assert_eq!(from_toml.primary, 248.0);
    assert_eq!(from_json.light_fill_tone, 40.0);
    assert_eq!(from_toml.dark_bg_tone, 12.0);

    // Hue tokens that a theme file omits still fall back to their defaults.
    assert_eq!(from_json.red, 20.0);
    assert_eq!(from_json.green, 150.0);
    assert_eq!(from_json.blue, 248.0);
    assert_eq!(from_json.yellow, 85.0);
    assert_eq!(from_json.cyan, 190.0);
    assert_eq!(from_toml.red, 20.0);
    assert_eq!(from_toml.cyan, 190.0);
  }

  #[test]
  fn tokens_parse_from_files() {
    let dir = std::env::temp_dir();
    let pid = std::process::id();
    let json_path = dir.join(format!("woocraft-theme-tokens-{pid}.json"));
    let toml_path = dir.join(format!("woocraft-theme-tokens-{pid}.toml"));

    let mut json_file = std::fs::File::create(&json_path).expect("create json file");
    let mut toml_file = std::fs::File::create(&toml_path).expect("create toml file");

    write!(
      json_file,
      "{{\"primary\":248,\"error\":24,\"warning\":48,\"success\":150,\"info\":248,\"accent\":130,\"chroma\":48,\"light_fill_tone\":40,\"dark_fill_tone\":80,\"light_bg_tone\":98,\"dark_bg_tone\":12,\"warning_tone_step\":20,\"surface_chroma\":0.6}}"
    )
    .expect("write json file");

    write!(
      toml_file,
      "primary = 248\nerror = 24\nwarning = 48\nsuccess = 150\ninfo = 248\naccent = 130\nchroma = 48\nlight_fill_tone = 40\ndark_fill_tone = 80\nlight_bg_tone = 98\ndark_bg_tone = 12\nwarning_tone_step = 20\nsurface_chroma = 0.6\n"
    )
    .expect("write toml file");

    let from_json = ThemeTokens::from_json_file(&json_path).expect("read json tokens");
    let from_toml = ThemeTokens::from_toml_file(&toml_path).expect("read toml tokens");

    assert_eq!(from_json.info, 248.0);
    assert_eq!(from_toml.info, 248.0);
    assert_eq!(from_json.light_bg_tone, 98.0);
    assert_eq!(from_toml.surface_chroma, 0.6);

    let _ = std::fs::remove_file(&json_path);
    let _ = std::fs::remove_file(&toml_path);
  }
}
