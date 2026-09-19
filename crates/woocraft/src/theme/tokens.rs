//! sizing, opacity, and motion constants shared across the design system.
//!
//! these are the fixed numeric conventions of the woocraft design language;
//! colors live in [`ThemeTokens`](crate::ThemeTokens) and are derived at
//! runtime instead.

use gpui::Hsla;

/// returns `color` with its alpha channel replaced by `alpha` (clamped to
/// `0.0..=1.0`).
pub(crate) fn with_alpha(color: Hsla, alpha: f32) -> Hsla {
  Hsla {
    a: alpha.clamp(0.0, 1.0),
    ..color
  }
}

pub mod opacity {
  pub const DISABLED: f32 = 0.6;

  pub mod transparent {
    pub const HOVER: f32 = 0.05;
    pub const ACTIVE: f32 = 0.1;
  }

  pub mod solid {
    pub const HOVER: f32 = 0.9;
    pub const ACTIVE: f32 = 0.8;
  }

  pub const MUTED_FOREGROUND: f32 = 0.75;
  pub const BORDER: f32 = 0.1;
  pub const MUTED: f32 = 0.2;
}

pub mod duration {
  use std::time::Duration;

  pub const SPINNER: Duration = Duration::from_millis(800);
  pub const CARET_BLINK_ON: Duration = Duration::from_millis(700);
  pub const CARET_BLINK_OFF: Duration = Duration::from_millis(1200);
  pub const SWITCH_TOGGLE: Duration = Duration::from_millis(150);
  pub const INDETERMINATE: Duration = Duration::from_millis(1200);
  pub const NOTIFICATION_DEFAULT: Duration = Duration::from_secs(5);
  pub const ANIMATION_FRAME: Duration = Duration::from_millis(33);
}
