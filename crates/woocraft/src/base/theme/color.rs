use std::path::Path;

use gpui::{Hsla, Rgba};
use palette::{FromColor, OklabHue, Oklch, Srgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeColors {
  pub background: Hsla,
  pub foreground: Hsla,
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
  pub ring: Hsla,
  pub success: Hsla,
  pub warning: Hsla,
  pub danger: Hsla,
  pub scrollbar: Hsla,
  pub scrollbar_thumb: Hsla,
  pub scrollbar_thumb_hover: Hsla,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ThemeTokens {
  pub primary: f32,
  pub error: f32,
  pub warning: f32,
  pub success: f32,
  pub info: f32,
  pub accent: f32,
  pub lightness: f32,
  pub chroma: f32,
  pub light_bg_lightness: f32,
  pub dark_bg_lightness: f32,
  pub light_bg_chroma: f32,
  pub dark_bg_chroma: f32,
}

impl Default for ThemeTokens {
  fn default() -> Self {
    Self {
      primary: 248.0,
      error: 24.0,
      warning: 48.0,
      success: 150.0,
      info: 248.0,
      accent: 130.0,
      lightness: 0.64,
      chroma: 0.17,
      light_bg_lightness: 0.96,
      dark_bg_lightness: 0.18,
      light_bg_chroma: 0.015,
      dark_bg_chroma: 0.0,
    }
  }
}

impl ThemeTokens {
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

fn with_lightness(color: Hsla, lightness: f32) -> Hsla {
  Hsla {
    l: lightness.clamp(0.0, 1.0),
    ..color
  }
}

fn relative_luminance(color: Hsla) -> f32 {
  let rgb = color.to_rgb();

  let linearize = |c: f32| {
    if c <= 0.03928 {
      c / 12.92
    } else {
      ((c + 0.055) / 1.055).powf(2.4)
    }
  };

  let r = linearize(rgb.r);
  let g = linearize(rgb.g);
  let b = linearize(rgb.b);

  0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast_ratio(foreground: Hsla, background: Hsla) -> f32 {
  let l1 = relative_luminance(foreground);
  let l2 = relative_luminance(background);
  let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
  (lighter + 0.05) / (darker + 0.05)
}

fn pick_readable_text(background: Hsla, light_text: Hsla, dark_text: Hsla) -> Hsla {
  let light_contrast = contrast_ratio(light_text, background);
  let dark_contrast = contrast_ratio(dark_text, background);

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

    let light_theme_text_lightness = (1.0 - tokens.light_bg_lightness).clamp(0.0, 1.0);
    let dark_theme_text_lightness = (1.0 - tokens.dark_bg_lightness).clamp(0.0, 1.0);
    let card_lightness = if is_dark {
      (bg_lightness + 0.04).clamp(0.0, 1.0)
    } else {
      (bg_lightness + 0.02).clamp(0.0, 1.0)
    };
    let muted_lightness = ((bg_lightness + tokens.lightness) * 0.5).clamp(0.0, 1.0);

    let background = to_hsla_from_oklch(bg_lightness, bg_chroma, tokens.primary, 1.0);
    let light_theme_text = with_lightness(
      to_hsla_from_oklch(light_theme_text_lightness, 0.01, tokens.primary, 1.0),
      light_theme_text_lightness,
    );
    let dark_theme_text = with_lightness(
      to_hsla_from_oklch(dark_theme_text_lightness, 0.01, tokens.primary, 1.0),
      dark_theme_text_lightness,
    );
    let foreground = pick_readable_text(background, light_theme_text, dark_theme_text);
    let card = to_hsla_from_oklch(card_lightness, bg_chroma, tokens.primary, 1.0);
    let card_foreground = foreground;
    let popover = background;
    let popover_foreground = foreground;

    let primary = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.primary, 1.0);
    let primary_foreground = pick_readable_text(primary, light_theme_text, dark_theme_text);
    let muted = to_hsla_from_oklch(muted_lightness, bg_chroma + 0.01, tokens.accent, 1.0);
    let muted_foreground = with_alpha(foreground, 0.75);

    let border = with_alpha(foreground, 0.1);
    let input = border;
    let ring = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.info, 1.0);
    let success = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.success, 1.0);
    let warning = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.warning, 1.0);
    let danger = to_hsla_from_oklch(tokens.lightness, tokens.chroma, tokens.error, 1.0);
    let scrollbar = with_alpha(background, 0.4);
    let scrollbar_thumb = with_alpha(foreground, 0.4);
    let scrollbar_thumb_hover = with_alpha(foreground, 0.6);

    Self {
      background,
      foreground,
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
      ring,
      success,
      warning,
      danger,
      scrollbar,
      scrollbar_thumb,
      scrollbar_thumb_hover,
    }
  }

  pub fn disabled(&self) -> Self {
    self.with_alpha(0.6)
  }

  pub fn with_alpha(&self, alpha: f32) -> Self {
    Self {
      background: with_alpha(self.background, alpha),
      foreground: with_alpha(self.foreground, alpha),
      card: with_alpha(self.card, alpha),
      card_foreground: with_alpha(self.card_foreground, alpha),
      popover: with_alpha(self.popover, alpha),
      popover_foreground: with_alpha(self.popover_foreground, alpha),
      primary: with_alpha(self.primary, alpha),
      primary_foreground: with_alpha(self.primary_foreground, alpha),
      muted: with_alpha(self.muted, alpha),
      muted_foreground: with_alpha(self.muted_foreground, alpha),
      border: with_alpha(self.border, alpha),
      input: with_alpha(self.input, alpha),
      ring: with_alpha(self.ring, alpha),
      success: with_alpha(self.success, alpha),
      warning: with_alpha(self.warning, alpha),
      danger: with_alpha(self.danger, alpha),
      scrollbar: with_alpha(self.scrollbar, alpha),
      scrollbar_thumb: with_alpha(self.scrollbar_thumb, alpha),
      scrollbar_thumb_hover: with_alpha(self.scrollbar_thumb_hover, alpha),
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

  fn almost_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
  }

  fn same_color(a: gpui::Hsla, b: gpui::Hsla) -> bool {
    almost_eq(a.h, b.h) && almost_eq(a.s, b.s) && almost_eq(a.l, b.l) && almost_eq(a.a, b.a)
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
  fn disabled_applies_uniform_alpha() {
    let colors = ThemeColors::from_tokens(ThemeTokens::default(), true).disabled();
    assert!((colors.background.a - 0.6).abs() < 1e-6);
    assert!((colors.foreground.a - 0.6).abs() < 1e-6);
    assert!((colors.success.a - 0.6).abs() < 1e-6);
  }

  #[test]
  fn text_lightness_is_inverse_of_bg_lightness() {
    let tokens = ThemeTokens::default();
    let light_colors = ThemeColors::from_tokens(tokens, false);
    let dark_colors = ThemeColors::from_tokens(tokens, true);

    assert!((light_colors.foreground.l - (1.0 - tokens.light_bg_lightness)).abs() < 0.05);
    assert!((dark_colors.foreground.l - (1.0 - tokens.dark_bg_lightness)).abs() < 0.05);
  }

  #[test]
  fn foreground_uses_light_or_dark_text_candidate() {
    let tokens = ThemeTokens::default();
    let light_colors = ThemeColors::from_tokens(tokens, false);
    let dark_colors = ThemeColors::from_tokens(tokens, true);

    let light_candidate = super::with_lightness(
      super::to_hsla_from_oklch(1.0 - tokens.light_bg_lightness, 0.01, tokens.primary, 1.0),
      1.0 - tokens.light_bg_lightness,
    );
    let dark_candidate = super::with_lightness(
      super::to_hsla_from_oklch(1.0 - tokens.dark_bg_lightness, 0.01, tokens.primary, 1.0),
      1.0 - tokens.dark_bg_lightness,
    );

    assert!(
      same_color(light_colors.foreground, light_candidate)
        || same_color(light_colors.foreground, dark_candidate)
    );
    assert!(
      same_color(dark_colors.foreground, light_candidate)
        || same_color(dark_colors.foreground, dark_candidate)
    );
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
