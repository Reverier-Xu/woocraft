//! Mapping between the Woocraft theme and the terminal ANSI palette.
//!
//! The palette resolution is intentionally pure (no GPUI context) so it can be
//! unit tested and reused both for rendering cells and for answering
//! [`TerminalEvent::ColorRequest`] queries from terminal applications.

use gpui::{Hsla, Rgba, black};
use woocraft_terminal::{CellColor, NamedColor, Rgb};

/// Number of entries answered for the classic 16 ANSI slots.
pub const ANSI_COLOR_COUNT: usize = 16;

/// Special palette indices, following the alacritty `Colors` interface.
const FOREGROUND_INDEX: usize = 256;
const BACKGROUND_INDEX: usize = 257;
const CURSOR_INDEX: usize = 258;

/// The resolved terminal palette for the active theme.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerminalPalette {
  /// The 16 base ANSI colors (normal + bright).
  pub ansi: [Hsla; ANSI_COLOR_COUNT],
  pub foreground: Hsla,
  pub background: Hsla,
  pub cursor: Hsla,
}

impl TerminalPalette {
  /// Builds the palette from the active theme.
  ///
  /// - Slot 0 (`black`) uses the editor background so that dark-on-dark apps
  ///   remain legible, and slot 7 (`white`) uses a muted foreground.
  /// - Magenta is synthesized from the theme's syntax token system because
  ///   [`crate::ThemeColors`] does not carry a dedicated magenta entry.
  /// - Bright variants lighten the base colors; dim variants darken them.
  pub fn from_theme(theme: &crate::Theme) -> Self {
    let colors = &theme.colors;
    let tokens = &theme.tokens;
    let magenta = tokens.syntax_color(300.0);

    let base = [
      colors.editor_background, // black
      colors.red,
      colors.green,
      colors.yellow,
      colors.blue,
      magenta,
      colors.cyan,
      colors.muted_foreground, // white
    ];

    let mut ansi = [black(); ANSI_COLOR_COUNT];
    for (index, color) in base.into_iter().enumerate() {
      ansi[index] = color;
      ansi[index + 8] = lighten(color, 0.12);
    }

    Self {
      ansi,
      foreground: colors.editor_foreground,
      background: colors.editor_background,
      cursor: colors.caret,
    }
  }

  /// The color for a palette index, covering the standard 256-color set plus
  /// the special alacritty-compatible indices (256 foreground, 257 background,
  /// 258 cursor).
  pub fn color_at_index(&self, index: usize) -> Option<Hsla> {
    match index {
      0..=15 => Some(self.ansi[index]),
      // 16-231 are a 6x6x6 RGB color cube, mapped to 0-255 using steps
      // defined by XTerm.
      16..=231 => {
        let (r, g, b) = rgb_channels_for_index(index as u8);
        Some(rgb_to_hsla(
          if r == 0 { 0 } else { r * 40 + 55 },
          if g == 0 { 0 } else { g * 40 + 55 },
          if b == 0 { 0 } else { b * 40 + 55 },
        ))
      }
      // 232-255 are a 24-step grayscale ramp from (8, 8, 8) to (238, 238, 238).
      232..=255 => {
        let value = (index as u8 - 232) * 10 + 8;
        Some(rgb_to_hsla(value, value, value))
      }
      FOREGROUND_INDEX => Some(self.foreground),
      BACKGROUND_INDEX => Some(self.background),
      CURSOR_INDEX => Some(self.cursor),
      _ => None,
    }
  }

  /// Resolves an alacritty cell color to a concrete GPUI color.
  pub fn convert(&self, color: &CellColor) -> Hsla {
    match color {
      CellColor::Named(named) => self.named_color(*named),
      CellColor::Spec(rgb) => rgb_to_hsla(rgb.r, rgb.g, rgb.b),
      CellColor::Indexed(index) => self
        .color_at_index(*index as usize)
        .unwrap_or(self.foreground),
    }
  }

  /// The palette color for a named alacritty color.
  pub fn named_color(&self, named: NamedColor) -> Hsla {
    match named {
      NamedColor::Black => self.ansi[0],
      NamedColor::Red => self.ansi[1],
      NamedColor::Green => self.ansi[2],
      NamedColor::Yellow => self.ansi[3],
      NamedColor::Blue => self.ansi[4],
      NamedColor::Magenta => self.ansi[5],
      NamedColor::Cyan => self.ansi[6],
      NamedColor::White => self.ansi[7],
      NamedColor::BrightBlack => self.ansi[8],
      NamedColor::BrightRed => self.ansi[9],
      NamedColor::BrightGreen => self.ansi[10],
      NamedColor::BrightYellow => self.ansi[11],
      NamedColor::BrightBlue => self.ansi[12],
      NamedColor::BrightMagenta => self.ansi[13],
      NamedColor::BrightCyan => self.ansi[14],
      NamedColor::BrightWhite => self.ansi[15],
      NamedColor::Foreground => self.foreground,
      NamedColor::Background => self.background,
      NamedColor::Cursor => self.cursor,
      NamedColor::DimBlack => darken(self.ansi[0], 0.2),
      NamedColor::DimRed => darken(self.ansi[1], 0.2),
      NamedColor::DimGreen => darken(self.ansi[2], 0.2),
      NamedColor::DimYellow => darken(self.ansi[3], 0.2),
      NamedColor::DimBlue => darken(self.ansi[4], 0.2),
      NamedColor::DimMagenta => darken(self.ansi[5], 0.2),
      NamedColor::DimCyan => darken(self.ansi[6], 0.2),
      NamedColor::DimWhite => darken(self.ansi[7], 0.2),
      NamedColor::BrightForeground => lighten(self.foreground, 0.12),
      NamedColor::DimForeground => darken(self.foreground, 0.2),
    }
  }

  /// Converts a palette entry back into 8-bit RGB, as required to answer
  /// terminal color requests.
  pub fn vte_rgb_at_index(&self, index: usize) -> Rgb {
    to_vte_rgb(self.color_at_index(index).unwrap_or_else(black))
  }
}

/// Converts a GPUI color into the 8-bit RGB representation terminals expect.
pub fn to_vte_rgb(color: Hsla) -> Rgb {
  let rgba = Rgba::from(color);
  Rgb {
    r: ((rgba.r * rgba.a) * 255.0) as u8,
    g: ((rgba.g * rgba.a) * 255.0) as u8,
    b: ((rgba.b * rgba.a) * 255.0) as u8,
  }
}

/// Converts 8-bit RGB into a GPUI color.
pub fn rgb_to_hsla(r: u8, g: u8, b: u8) -> Hsla {
  Hsla::from(Rgba {
    r: f32::from(r) / 255.0,
    g: f32::from(g) / 255.0,
    b: f32::from(b) / 255.0,
    a: 1.0,
  })
}

/// Generates the RGB channels in [0, 5] for a given index into the 6x6x6 ANSI
/// color cube.
///
/// See: [8 bit ANSI color](https://en.wikipedia.org/wiki/ANSI_escape_code#8-bit).
fn rgb_channels_for_index(index: u8) -> (u8, u8, u8) {
  debug_assert!((16..=231).contains(&index));
  let index = index - 16;
  let r = (index - (index % 36)) / 36;
  let g = ((index % 36) - (index % 6)) / 6;
  let b = (index % 36) % 6;
  (r, g, b)
}

fn lighten(color: Hsla, amount: f32) -> Hsla {
  Hsla {
    l: (color.l + amount).min(1.0),
    ..color
  }
}

fn darken(color: Hsla, amount: f32) -> Hsla {
  Hsla {
    l: (color.l - amount).max(0.0),
    ..color
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::Theme;

  #[test]
  fn base_palette_slots() {
    let palette = TerminalPalette::from_theme(&Theme::default());
    assert_eq!(palette.ansi.len(), 16);
    // Bright colors are lightened versions of the base colors.
    assert_eq!(palette.ansi[8], lighten(palette.ansi[0], 0.12));
  }

  #[test]
  fn cube_indices_use_xterm_steps() {
    let palette = TerminalPalette::from_theme(&Theme::default());
    // Index 16 is the first cube entry: (0, 0, 0).
    assert_eq!(palette.color_at_index(16), Some(rgb_to_hsla(0, 0, 0)));
    // Index 231 is the last cube entry: (5, 5, 5) -> 255, 255, 255.
    assert_eq!(
      palette.color_at_index(231),
      Some(rgb_to_hsla(255, 255, 255))
    );
    // Index 196 = 16 + 36*5 -> (5, 0, 0) -> 255, 0, 0.
    assert_eq!(palette.color_at_index(196), Some(rgb_to_hsla(255, 0, 0)));
  }

  #[test]
  fn grayscale_ramp() {
    let palette = TerminalPalette::from_theme(&Theme::default());
    assert_eq!(palette.color_at_index(232), Some(rgb_to_hsla(8, 8, 8)));
    assert_eq!(
      palette.color_at_index(255),
      Some(rgb_to_hsla(238, 238, 238))
    );
  }

  #[test]
  fn special_indices() {
    let palette = TerminalPalette::from_theme(&Theme::default());
    assert_eq!(palette.color_at_index(256), Some(palette.foreground));
    assert_eq!(palette.color_at_index(257), Some(palette.background));
    assert_eq!(palette.color_at_index(258), Some(palette.cursor));
    assert_eq!(palette.color_at_index(999), None);
  }

  #[test]
  fn vte_rgb_round_trip() {
    let rgb = to_vte_rgb(rgb_to_hsla(255, 0, 0));
    assert_eq!((rgb.r, rgb.g, rgb.b), (255, 0, 0));

    let palette = TerminalPalette::from_theme(&Theme::default());
    let request = palette.vte_rgb_at_index(196);
    assert_eq!((request.r, request.g, request.b), (255, 0, 0));
  }

  #[test]
  fn named_color_mapping() {
    let palette = TerminalPalette::from_theme(&Theme::default());
    assert_eq!(palette.named_color(NamedColor::Red), palette.ansi[1]);
    assert_eq!(
      palette.named_color(NamedColor::BrightCyan),
      palette.ansi[14]
    );
    assert_eq!(
      palette.named_color(NamedColor::Foreground),
      palette.foreground
    );
    assert_eq!(
      palette.named_color(NamedColor::Background),
      palette.background
    );
  }
}
