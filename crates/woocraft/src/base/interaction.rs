//! Color system for interactive component states.
//!
//! Provides InteractionColors for consistent styling of interactive elements
//! across different states (hover, active, disabled). Includes ColorExt trait
//! for color manipulation operations (darken, lighten, adjust opacity, etc.).

use gpuim::Hsla;

use crate::base::theme::opacity;

/// Color palette for interactive components across all states.
///
/// Defines base, foreground, border, hover, and active colors for consistent
/// interactive element styling. Supports solid, transparent, and outline color
/// modes.
#[derive(Debug, Clone, Copy)]
pub struct InteractionColors {
  /// Base/background color for the component.
  pub base: Hsla,
  /// Foreground/text color.
  pub foreground: Hsla,
  /// Border color.
  pub border: Hsla,
  /// Color on hover (typically lighter/more opaque version of base).
  pub hover: Hsla,
  /// Color when active/pressed (typically darker/more opaque version of base).
  pub active: Hsla,
}

impl InteractionColors {
  /// Creates interactive colors for a solid (filled) button style.
  ///
  /// Base is the filled color, hover/active use opacity adjustments (0.8 and
  /// 0.6).
  pub fn solid(color: Hsla, foreground: Hsla) -> Self {
    Self {
      base: color,
      foreground,
      border: color,
      hover: Hsla {
        a: opacity::solid::HOVER,
        ..color
      },
      active: Hsla {
        a: opacity::solid::ACTIVE,
        ..color
      },
    }
  }

  /// Creates interactive colors for a transparent (ghost) button style.
  ///
  /// Base is transparent, foreground is visible. Hover/active use opacity to
  /// show interaction.
  pub fn transparent(foreground: Hsla) -> Self {
    Self {
      base: Hsla::transparent_black(),
      foreground,
      border: Hsla::transparent_black(),
      hover: Hsla {
        a: opacity::transparent::HOVER,
        ..foreground
      },
      active: Hsla {
        a: opacity::transparent::ACTIVE,
        ..foreground
      },
    }
  }

  /// Creates interactive colors for an outline (border-only) button style.
  ///
  /// Base is transparent with visible border. Foreground and border use the
  /// provided color.
  pub fn outline(color: Hsla, foreground: Hsla) -> Self {
    Self {
      base: Hsla::transparent_black(),
      foreground: color,
      border: color,
      hover: Hsla {
        a: opacity::transparent::HOVER,
        ..foreground
      },
      active: Hsla {
        a: opacity::transparent::ACTIVE,
        ..foreground
      },
    }
  }

  /// Overrides the border color while keeping other colors unchanged.
  pub fn with_border(mut self, border: Hsla) -> Self {
    self.border = border;
    self
  }
}

/// Extension trait for color manipulation on Hsla colors.
///
/// Provides convenient methods for common color adjustments: opacity/alpha
/// changes, saturation adjustment, blending, and lightness changes
/// (darken/lighten).
pub trait ColorExt: Sized {
  /// Sets the opacity/alpha value (0.0 = transparent, 1.0 = opaque).
  fn opacity(self, alpha: f32) -> Self;

  /// Alias for opacity(). Sets the alpha/transparency value.
  fn alpha(self, alpha: f32) -> Self;

  /// Sets the saturation (0.0 = grayscale, 1.0 = fully saturated).
  fn saturation(self, saturation: f32) -> Self;

  /// Blends this color with an overlay color using alpha compositing.
  fn blend(self, overlay: Self) -> Self;

  /// Darkens the color by reducing lightness (amount 0.0..1.0).
  fn darken(self, amount: f32) -> Self;

  /// Lightens the color by increasing lightness (amount 0.0..1.0).
  fn lighten(self, amount: f32) -> Self;
}

impl ColorExt for Hsla {
  fn opacity(mut self, alpha: f32) -> Self {
    self.a = alpha.clamp(0.0, 1.0);
    self
  }

  fn alpha(self, alpha: f32) -> Self {
    self.opacity(alpha)
  }

  fn saturation(mut self, saturation: f32) -> Self {
    self.s = saturation.clamp(0.0, 1.0);
    self
  }

  fn blend(self, overlay: Self) -> Self {
    let bg = self.to_rgb();
    let fg = overlay.to_rgb();

    let out_a = fg.a + bg.a * (1.0 - fg.a);
    if out_a <= 0.0 {
      return Hsla::transparent_black();
    }

    let out_r = (fg.r * fg.a + bg.r * bg.a * (1.0 - fg.a)) / out_a;
    let out_g = (fg.g * fg.a + bg.g * bg.a * (1.0 - fg.a)) / out_a;
    let out_b = (fg.b * fg.a + bg.b * bg.a * (1.0 - fg.a)) / out_a;

    gpuim::Rgba {
      r: out_r.clamp(0.0, 1.0),
      g: out_g.clamp(0.0, 1.0),
      b: out_b.clamp(0.0, 1.0),
      a: out_a.clamp(0.0, 1.0),
    }
    .into()
  }

  fn darken(mut self, amount: f32) -> Self {
    self.l = (self.l - amount).clamp(0.0, 1.0);
    self
  }

  fn lighten(mut self, amount: f32) -> Self {
    self.l = (self.l + amount).clamp(0.0, 1.0);
    self
  }
}
