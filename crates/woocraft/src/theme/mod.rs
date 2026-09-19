//! Theme system for managing colors, tokens, and appearance settings.
//!
//! Provides global Theme configuration including light/dark mode, the oklch
//! color system, sizing tokens (radius, fonts, icons), and scrollbar
//! behavior. The theme system auto-syncs with system appearance (light/dark
//! mode) and can be programmatically updated. All UI components automatically
//! inherit theme colors via the ActiveTheme trait.
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

use gpui::{App, Global, Pixels, Rems, SharedString, WindowAppearance, px, rems};
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
/// visibility). Available globally via Theme::global(cx) and accessible
/// through ActiveTheme trait. Colors update automatically when mode changes or
/// tokens are reloaded.
///
/// All sizing is stored in rems and therefore follows the window root
/// font-size (`1rem` defaults to `16px`), so a single font-size change scales
/// the entire system. Outside rich-text rendering, components render at the
/// medium/default size and `1rem` text.
///
/// `Theme` is deliberately not `Copy` (it is far beyond the 64-bit copy
/// threshold): borrow it through [`ActiveTheme`] — `cx.theme()` — or clone
/// explicitly at the few places a private snapshot is worth it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
  pub mode: ThemeMode,
  pub tokens: ThemeTokens,
  pub colors: ThemeColors,
  /// body font size; `1rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub font_size: Rems,
  /// icon size; `1rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub icon_size: Rems,
  /// control corner radius; `0.25rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub radius: Rems,
  /// large surface corner radius; `0.5rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub radius_lg: Rems,
  /// container corner radius; `0.375rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub radius_container: Rems,
  /// tile grid pitch; `0.625rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub tile_grid_size: Rems,
  /// tile corner radius; `0.375rem` by default.
  #[serde(
    serialize_with = "serialize_rems",
    deserialize_with = "deserialize_rems"
  )]
  pub tile_radius: Rems,
  /// border and outline width — the design system's single pixel exception.
  /// components inset it from their padding so geometry stays on the rem
  /// grid; `1px` by default, overridable per theme and inherited everywhere.
  pub border_width: Pixels,
  /// default font family applied to component text; the embedded
  /// [`crate::DEFAULT_FONT_FAMILY`] by default. components inject it at
  /// their roots, and applications override it either here or through
  /// per-element styles.
  pub font_family: SharedString,
  pub scrollbar_show: ScrollbarShow,
}

impl Default for Theme {
  fn default() -> Self {
    let tokens = ThemeTokens::default();
    Self {
      mode: ThemeMode::Light,
      colors: ThemeColors::from_tokens(&tokens, false),
      tokens,
      font_size: rems(1.),
      icon_size: rems(1.),
      radius: rems(0.25),
      radius_lg: rems(0.5),
      radius_container: rems(0.375),
      tile_grid_size: rems(0.625),
      tile_radius: rems(0.375),
      border_width: px(1.),
      font_family: SharedString::from(crate::DEFAULT_FONT_FAMILY),
      scrollbar_show: ScrollbarShow::default(),
    }
  }
}

impl Global for Theme {}

fn serialize_rems<S: serde::Serializer>(rems: &Rems, serializer: S) -> Result<S::Ok, S::Error> {
  serializer.serialize_f32(rems.0)
}

fn deserialize_rems<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Rems, D::Error> {
  Ok(Rems(f32::deserialize(deserializer)?))
}

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
    theme.colors = ThemeColors::from_tokens(&theme.tokens, mode.is_dark());
    tracing::debug!(?mode, "theme mode changed");
    cx.refresh_windows();
  }

  /// Loads new theme tokens and recalculates all derived colors.
  pub fn load_tokens(tokens: ThemeTokens, mode: ThemeMode, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.tokens = tokens;
    theme.mode = mode;
    theme.colors = ThemeColors::from_tokens(&theme.tokens, mode.is_dark());
    tracing::debug!(?mode, "theme tokens loaded");
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

  /// Returns a fully opaque color for `hue` using the active theme primary
  /// OKLCH lightness and chroma.
  ///
  /// # Example
  /// ```rust,ignore
  /// let accent = cx.theme().color_for_hue(130.0);
  /// ```
  #[inline]
  pub fn color_for_hue(&self, hue: f32) -> gpui::Hsla {
    self.tokens.syntax_color(hue)
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

  let mode = Theme::global(cx).mode;
  tracing::debug!(?mode, "theme initialized");
}

#[cfg(test)]
mod tests {
  use super::Theme;

  fn almost_eq(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-4
  }

  fn same_color(a: gpui::Hsla, b: gpui::Hsla) -> bool {
    almost_eq(a.h, b.h) && almost_eq(a.s, b.s) && almost_eq(a.l, b.l) && almost_eq(a.a, b.a)
  }

  #[test]
  fn color_for_hue_uses_theme_primary_lightness_and_chroma() {
    let theme = Theme::default();
    let hue = 130.0;

    assert!(same_color(
      theme.color_for_hue(hue),
      theme.tokens.syntax_color(hue)
    ));
  }
}
