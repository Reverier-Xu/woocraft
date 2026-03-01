//! Theme system for managing colors, tokens, and appearance settings.
//!
//! Provides global Theme configuration including light/dark mode, color
//! palette, sizing tokens (radius, fonts, icons), and scrollbar behavior. The
//! theme system auto-syncs with system appearance (light/dark mode) and can be
//! programmatically updated. All UI components automatically inherit theme
//! colors via the ActiveTheme trait.
//!
//! # Example
//! ```rust,ignore
//! use woocraft::{Theme, ThemeMode, ActiveTheme};
//! use gpui::App;
//!
//! let theme = Theme::global(&cx);
//! Theme::set_mode(ThemeMode::Dark, &mut cx);
//! Theme::sync_system_appearance(&mut cx);
//! ```

use gpui::{App, Global, Pixels, WindowAppearance, px};
use serde::{Deserialize, Serialize};

mod color;
mod tokens;

pub use color::*;
pub use tokens::*;

/// Scrollbar visibility mode.
///
/// Scrollbars can be hidden until scrolling is needed, shown on hover, or
/// always visible.
/// - Scrolling: Hidden by default, appears only while scrolling
/// - Hover: Appears on mouse hover
/// - Always: Always visible
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash, Default)]
pub enum ScrollbarShow {
  #[default]
  Scrolling,
  Hover,
  Always,
}

impl ScrollbarShow {
  pub fn is_hover(self) -> bool {
    matches!(self, Self::Hover)
  }

  pub fn is_always(self) -> bool {
    matches!(self, Self::Always)
  }
}

/// Light or dark theme mode.
///
/// Affects the entire color palette of the application. Can sync automatically
/// with system settings or be set explicitly via Theme::set_mode().
/// - Light: Bright background, dark text (default)
/// - Dark: Dark background, bright text
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThemeMode {
  #[default]
  Light,
  Dark,
}

impl ThemeMode {
  pub fn is_dark(self) -> bool {
    matches!(self, Self::Dark)
  }
}

impl From<WindowAppearance> for ThemeMode {
  fn from(value: WindowAppearance) -> Self {
    match value {
      WindowAppearance::Light | WindowAppearance::VibrantLight => Self::Light,
      WindowAppearance::Dark | WindowAppearance::VibrantDark => Self::Dark,
    }
  }
}

/// Global application theme configuration.
///
/// Contains all theme settings: color palette for light/dark modes, sizing
/// tokens (fonts, icons, radii), and UI behavior settings (scrollbar
/// visibility). Available globally via Theme::global(cx) and accessible through
/// ActiveTheme trait. Colors update automatically when mode changes or tokens
/// are reloaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
  pub mode: ThemeMode,
  pub tokens: ThemeTokens,
  pub colors: ThemeColors,
  pub font_size: Pixels,
  pub icon_size: Pixels,
  pub radius: Pixels,
  pub radius_lg: Pixels,
  pub radius_container: Pixels,
  pub tile_grid_size: Pixels,
  pub tile_radius: Pixels,
  pub scrollbar_show: ScrollbarShow,
}

impl Default for Theme {
  fn default() -> Self {
    let tokens = ThemeTokens::default();
    Self {
      mode: ThemeMode::Light,
      colors: ThemeColors::from_tokens(tokens, false),
      tokens,
      font_size: px(16.),
      icon_size: px(16.),
      radius: px(4.),
      radius_lg: px(8.),
      radius_container: px(6.),
      tile_grid_size: px(10.),
      tile_radius: px(6.),
      scrollbar_show: ScrollbarShow::default(),
    }
  }
}

impl Global for Theme {}

/// Trait providing access to the global active theme.
///
/// Implemented for App context, allowing easy access to theme colors, tokens,
/// and settings. All components use this trait to fetch theme values for
/// rendering.
pub trait ActiveTheme {
  /// Returns a reference to the global active theme.
  fn theme(&self) -> &Theme;
}

impl ActiveTheme for App {
  fn theme(&self) -> &Theme {
    Theme::global(self)
  }
}

impl std::ops::Deref for Theme {
  type Target = ThemeColors;

  fn deref(&self) -> &Self::Target {
    &self.colors
  }
}

impl Theme {
  /// Gets a reference to the global theme.
  pub fn global(cx: &App) -> &Theme {
    cx.global::<Theme>()
  }

  /// Gets a mutable reference to the global theme.
  pub fn global_mut(cx: &mut App) -> &mut Theme {
    cx.global_mut::<Theme>()
  }

  /// Changes the theme mode (light/dark) and refreshes all windows.
  pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.mode = mode;
    theme.colors = ThemeColors::from_tokens(theme.tokens, mode.is_dark());
    cx.refresh_windows();
  }

  /// Loads new theme tokens and recalculates all derived colors.
  pub fn load_tokens(tokens: ThemeTokens, mode: ThemeMode, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.tokens = tokens;
    theme.mode = mode;
    theme.colors = ThemeColors::from_tokens(tokens, mode.is_dark());
    cx.refresh_windows();
  }

  /// Syncs the theme mode with the system appearance (light/dark).
  pub fn sync_system_appearance(cx: &mut App) {
    Self::set_mode(cx.window_appearance().into(), cx);
  }

  /// Updates scrollbar visibility based on system settings.
  pub fn sync_scrollbar_appearance(cx: &mut App) {
    Theme::global_mut(cx).scrollbar_show = if cx.should_auto_hide_scrollbars() {
      ScrollbarShow::Scrolling
    } else {
      ScrollbarShow::Hover
    };
  }

  /// Gets the editor background color.
  #[inline]
  pub fn editor_background(&self) -> gpui::Hsla {
    self.editor_background
  }
}

/// Initializes the global theme and syncs with system appearance.
pub fn init(cx: &mut App) {
  if !cx.has_global::<Theme>() {
    cx.set_global(Theme::default());
  }
  Theme::sync_scrollbar_appearance(cx);
  Theme::sync_system_appearance(cx);
}
