use gpui::Hsla;

use crate::base::theme::opacity;

#[derive(Debug, Clone, Copy)]
pub struct InteractionColors {
  pub base: Hsla,
  pub foreground: Hsla,
  pub border: Hsla,
  pub hover: Hsla,
  pub active: Hsla,
}

impl InteractionColors {
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

  pub fn with_border(mut self, border: Hsla) -> Self {
    self.border = border;
    self
  }
}

pub trait ColorExt: Sized {
  fn opacity(self, alpha: f32) -> Self;
  fn darken(self, amount: f32) -> Self;
  fn lighten(self, amount: f32) -> Self;
}

impl ColorExt for Hsla {
  fn opacity(mut self, alpha: f32) -> Self {
    self.a = alpha.clamp(0.0, 1.0);
    self
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
