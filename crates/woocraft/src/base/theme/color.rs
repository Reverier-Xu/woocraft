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
  pub muted_foreground: Hsla,
  pub border: Hsla,
  pub input: Hsla,
  pub success: Hsla,
  pub success_foreground: Hsla,
  pub warning: Hsla,
  pub warning_foreground: Hsla,
  pub danger: Hsla,
  pub danger_foreground: Hsla,
  pub info: Hsla,
  pub info_foreground: Hsla,
  pub red: Hsla,
  pub red_foreground: Hsla,
  pub green: Hsla,
  pub green_foreground: Hsla,
  pub blue: Hsla,
  pub blue_foreground: Hsla,
  pub yellow: Hsla,
  pub yellow_foreground: Hsla,
  pub cyan: Hsla,
  pub cyan_foreground: Hsla,
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
  /// Chroma shared by every semantic fill.
  ///
  /// One value for every hue, so the palette reads as a single family and only
  /// the hue axis varies between roles.
  ///
  /// It is not the value at which *every* hue fits: the sRGB gamut's ceiling
  /// varies by more than 2x across the hue circle, and the tightest arcs (the
  /// cyan-to-teal stretch around 180..230°, and amber around 85° in a light
  /// theme) top out near 35 at the fill tone. Holding chroma down to that would
  /// leave the reds and magentas, which can take 90, looking grey. The default
  /// sits high enough for those to stay vivid and lets the narrow arcs be
  /// gamut-mapped down, which is the trade the palette example visualises on
  /// its hue bar.
  pub chroma: f32,
  /// Tone of the semantic fills in a light theme.
  pub light_fill_tone: f32,
  /// Tone of the semantic fills in a dark theme.
  pub dark_fill_tone: f32,
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
  258.0
}

fn default_yellow_hue() -> f32 {
  70.0
}

fn default_cyan_hue() -> f32 {
  190.0
}

impl Default for ThemeTokens {
  fn default() -> Self {
    Self {
      primary: 258.0,
      error: 20.0,
      warning: 70.0,
      success: 150.0,
      info: 258.0,
      red: 20.0,
      green: 150.0,
      blue: 258.0,
      yellow: 70.0,
      cyan: 190.0,
      accent: 200.0,
      secondary: 290.0,
      chroma: 45.0,
      light_fill_tone: 62.0,
      dark_fill_tone: 66.0,
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

/// The color of an unfilled track, thumb or other recessed chrome.
///
/// Track surfaces are the page foreground at reduced alpha rather than a
/// separate opaque color. That keeps one expression for "structure, but not
/// content", lets the same value work on any surface the control sits on, and
/// means a themed foreground (say, a warm palette) carries through to the
/// controls instead of being mixed from a fixed black or white.
pub fn track_color(theme: &super::Theme) -> Hsla {
  theme.foreground.opacity(TRACK_ALPHA)
}

/// Alpha of [`track_color`].
///
/// Low enough that a track reads as a recess behind its content rather than as
/// a filled control of its own.
pub const TRACK_ALPHA: f32 = 0.3;

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

/// `color` moved `steps` down the HCT tone scale.
///
/// Interaction feedback is expressed in tone, not HSL lightness: tone is the
/// design system's lightness axis, so a pressed or hovered surface keeps the
/// hue and chroma the palette was built from instead of drifting toward grey.
pub fn darken_tone(color: Hsla, steps: f64) -> Hsla {
  let hct = hct_of_hsla(color);
  hct_to_hsla(
    hct.with_tone(Tone::new(hct.tone().value() - steps)),
    f64::from(color.a) as f32,
  )
}

/// The text color to use on `fill`.
///
/// The inverse of the palette's own pairing rule: solve for the tone that
/// reaches the body-text ratio against `fill`, keeping the fill's hue and as
/// much chroma as the gamut allows. Use this whenever a component accepts a
/// caller-supplied fill color, so its text is derived from that color instead
/// of borrowed from an unrelated role.
pub fn foreground_for(fill: Hsla) -> Hsla {
  hct_to_hsla(hct_of_hsla(fill).foreground(Contrast::BODY_TEXT), 1.0)
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

    let background = neutral_surface(&tokens, bg_tone, hue);
    let card = neutral_surface(&tokens, card_tone, hue);
    // One chroma and one tone for every fill, in both modes: the only axis a
    // role varies on is hue. Chroma is set high enough that the wide arcs (red,
    // magenta, purple) stay vivid, which means the narrow ones — cyan-to-teal,
    // and amber in a light theme — are gamut-mapped down. That is deliberate:
    // holding chroma low enough for every hue would grey out the whole palette.
    // The palette example draws the resulting wall on its hue bar.
    let chroma = f64::from(tokens.chroma);
    let fill = |hue: f32| Hct::new(f64::from(hue), chroma, fill_tone);

    let accent = Hct::new(f64::from(tokens.accent), chroma, fill_tone);
    let secondary = Hct::new(f64::from(tokens.secondary), chroma, fill_tone);
    let popover = background;

    // Fills share one tone, so they are all equally prominent and all reach the
    // same contrast against the page. Their hues differ; their accessibility
    // does not.
    let primary = fill(hue);
    let info = fill(tokens.info);
    let success = fill(tokens.success);
    let warning = fill(tokens.warning);
    let danger = fill(tokens.error);
    let red = fill(tokens.red);
    let green = fill(tokens.green);
    let blue = fill(tokens.blue);
    let yellow = fill(tokens.yellow);
    let cyan = fill(tokens.cyan);

    // Every fill carries its own foreground. They all resolve to the same tone
    // today because the fills share one tone, but deriving them from each fill
    // rather than reusing `primary_foreground` is what keeps the pair correct
    // once a theme gives a role its own tone — and it is the only way a caller
    // that supplies a custom fill color can ask for its matching text.
    let fill_foreground = |color: Hct| hct_to_hsla(color.foreground(Contrast::BODY_TEXT), 1.0);
    let success_foreground = fill_foreground(success);
    let warning_foreground = fill_foreground(warning);
    let danger_foreground = fill_foreground(danger);
    let info_foreground = fill_foreground(info);
    let red_foreground = fill_foreground(red);
    let green_foreground = fill_foreground(green);
    let blue_foreground = fill_foreground(blue);
    let yellow_foreground = fill_foreground(yellow);
    let cyan_foreground = fill_foreground(cyan);

    // Every surface picks a foreground that reaches the enhanced ratio against
    // it. No candidate list, no audit: the tone axis answers this, and asking
    // for more than the minimum keeps body text solidly readable instead of
    // landing at the threshold.
    let foreground = background.foreground(Contrast::ENHANCED);
    let card_foreground = card.foreground(Contrast::ENHANCED);
    let popover_foreground = foreground;
    let accent_foreground = fill_foreground(accent);
    let primary_foreground = primary.foreground(Contrast::BODY_TEXT);
    // Secondary text is the page's own foreground at reduced weight, not a
    // color derived from a separate surface.
    let muted_foreground = with_alpha(hct_to_hsla(foreground, 1.0), 0.6);
    let secondary_foreground = fill_foreground(secondary);

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
    // The hover step moves away from the page background, so it reads as a lift
    // in both modes instead of depending on a hard-coded sign.
    let secondary_hover = secondary.with_tone(if is_dark {
      fill_tone + 4.0
    } else {
      fill_tone - 4.0
    });
    let tab_bar_segmented = with_alpha(hct_to_hsla(foreground, 1.0), TRACK_ALPHA);
    let tab_bar = hct_to_hsla(card, 1.0);
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
      accent_foreground,
      card: hct_to_hsla(card, 1.0),
      card_foreground: hct_to_hsla(card_foreground, 1.0),
      popover: hct_to_hsla(popover, 1.0),
      popover_foreground: hct_to_hsla(popover_foreground, 1.0),
      primary: hct_to_hsla(primary, 1.0),
      primary_foreground: hct_to_hsla(primary_foreground, 1.0),
      muted_foreground,
      border,
      input,
      success: hct_to_hsla(success, 1.0),
      success_foreground,
      warning: hct_to_hsla(warning, 1.0),
      warning_foreground,
      danger: hct_to_hsla(danger, 1.0),
      danger_foreground,
      info: hct_to_hsla(info, 1.0),
      info_foreground,
      red: hct_to_hsla(red, 1.0),
      red_foreground,
      green: hct_to_hsla(green, 1.0),
      green_foreground,
      blue: hct_to_hsla(blue, 1.0),
      blue_foreground,
      yellow: hct_to_hsla(yellow, 1.0),
      yellow_foreground,
      cyan: hct_to_hsla(cyan, 1.0),
      cyan_foreground,
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
    self.with_alpha(super::tokens::opacity::DISABLED)
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
      muted_foreground: scale_alpha(self.muted_foreground, alpha),
      border: scale_alpha(self.border, alpha),
      input: scale_alpha(self.input, alpha),
      success: scale_alpha(self.success, alpha),
      success_foreground: scale_alpha(self.success_foreground, alpha),
      warning: scale_alpha(self.warning, alpha),
      warning_foreground: scale_alpha(self.warning_foreground, alpha),
      danger: scale_alpha(self.danger, alpha),
      danger_foreground: scale_alpha(self.danger_foreground, alpha),
      info: scale_alpha(self.info, alpha),
      info_foreground: scale_alpha(self.info_foreground, alpha),
      red: scale_alpha(self.red, alpha),
      red_foreground: scale_alpha(self.red_foreground, alpha),
      green: scale_alpha(self.green, alpha),
      green_foreground: scale_alpha(self.green_foreground, alpha),
      blue: scale_alpha(self.blue, alpha),
      blue_foreground: scale_alpha(self.blue_foreground, alpha),
      yellow: scale_alpha(self.yellow, alpha),
      yellow_foreground: scale_alpha(self.yellow_foreground, alpha),
      cyan: scale_alpha(self.cyan, alpha),
      cyan_foreground: scale_alpha(self.cyan_foreground, alpha),
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

  use woocraft_colors::Contrast;

  use super::{ThemeColors, ThemeTokens, darken_tone, hct_of_hsla, tone_of_hsla};

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
  fn muted_foreground_is_the_page_foreground_at_reduced_weight() {
    // Secondary text is the page foreground, dimmed — not a colour of its own.
    // So the opaque version must clear the body-text ratio against the page.
    let colors = ThemeColors::light();
    assert!((colors.muted_foreground.a - 0.6).abs() < 1e-6);

    let opaque = super::with_alpha(colors.muted_foreground, 1.0);
    assert!(ratio(colors.background, opaque) >= Contrast::BODY_TEXT - 0.05);
    assert!((opaque.h - colors.foreground.h).abs() < 1e-6);
    assert!((opaque.l - colors.foreground.l).abs() < 1e-6);
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
  fn every_fill_pairs_with_its_own_foreground() {
    // Each fill carries the text color derived from *that* fill, not borrowed
    // from another role. This is the invariant a component relies on when it
    // renders `theme().danger` behind `theme().danger_foreground`.
    for is_dark in [false, true] {
      let colors = ThemeColors::from_tokens(ThemeTokens::default(), is_dark);

      for (name, fill, foreground) in [
        ("primary", colors.primary, colors.primary_foreground),
        ("accent", colors.accent, colors.accent_foreground),
        ("secondary", colors.secondary, colors.secondary_foreground),
        ("success", colors.success, colors.success_foreground),
        ("warning", colors.warning, colors.warning_foreground),
        ("danger", colors.danger, colors.danger_foreground),
        ("info", colors.info, colors.info_foreground),
        ("red", colors.red, colors.red_foreground),
        ("green", colors.green, colors.green_foreground),
        ("blue", colors.blue, colors.blue_foreground),
        ("yellow", colors.yellow, colors.yellow_foreground),
        ("cyan", colors.cyan, colors.cyan_foreground),
      ] {
        let achieved = ratio(fill, foreground);
        assert!(
          achieved >= Contrast::BODY_TEXT - 0.05,
          "{name} foreground only reaches {achieved:.2}:1 (dark: {is_dark})"
        );

        // And it must be derived from its own fill: a fill's foreground should
        // agree with what `foreground_for` computes for it.
        let expected = super::foreground_for(fill);
        assert!(
          (expected.h - foreground.h).abs() < 1e-3
            && (expected.s - foreground.s).abs() < 1e-3
            && (expected.l - foreground.l).abs() < 1e-3,
          "{name} foreground was not derived from its own fill (dark: {is_dark})"
        );
      }
    }
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

  /// The token format is `serde` itself; the crate ships no wrapper around it,
  /// so this pins the field names a token file may use.
  /// Interaction feedback moves tone, so hue and chroma survive a hover or
  /// press instead of drifting toward grey the way an HSL lightness nudge does.
  #[test]
  fn darken_tone_moves_tone_only() {
    let base = gpui::Hsla {
      h: 0.55,
      s: 0.6,
      l: 0.5,
      a: 0.8,
    };
    let darker = darken_tone(base, 8.0);

    let before = hct_of_hsla(base);
    let after = hct_of_hsla(darker);
    assert!(after.tone().value() < before.tone().value() - 7.0);
    assert!((after.hue() - before.hue()).abs() < 1.0);
    assert!((f64::from(darker.a) - 0.8).abs() < 1e-6);
  }

  #[test]
  fn tokens_parse_from_serde() {
    let json = r#"{
      "primary": 248, "error": 24, "warning": 48, "success": 150, "info": 248,
      "accent": 130, "chroma": 34, "light_fill_tone": 40, "dark_fill_tone": 70,
      "light_bg_tone": 98, "dark_bg_tone": 12, "surface_chroma": 0.6
    }"#;
    let toml = "primary = 248\nerror = 24\nwarning = 48\nsuccess = 150\ninfo = 248\naccent = 130\nchroma = 34\nlight_fill_tone = 40\ndark_fill_tone = 70\nlight_bg_tone = 98\ndark_bg_tone = 12\nsurface_chroma = 0.6\n";

    let from_json: ThemeTokens = serde_json::from_str(json).expect("json tokens should parse");
    let from_toml: ThemeTokens = toml::from_str(toml).expect("toml tokens should parse");

    assert_eq!(from_json.primary, 248.0);
    assert_eq!(from_toml.primary, 248.0);
    assert_eq!(from_json.light_bg_tone, 98.0);
    assert_eq!(from_toml.chroma, 34.0);
  }
}
