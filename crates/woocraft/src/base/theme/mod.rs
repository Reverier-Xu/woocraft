use gpui::{px, App, Global, Pixels, WindowAppearance};
use serde::{Deserialize, Serialize};

mod color;
mod tokens;

pub use color::*;
pub use tokens::*;

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

pub trait ActiveTheme {
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
  pub fn global(cx: &App) -> &Theme {
    cx.global::<Theme>()
  }

  pub fn global_mut(cx: &mut App) -> &mut Theme {
    cx.global_mut::<Theme>()
  }

  pub fn set_mode(mode: ThemeMode, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.mode = mode;
    theme.colors = ThemeColors::from_tokens(theme.tokens, mode.is_dark());
    cx.refresh_windows();
  }

  pub fn load_tokens(tokens: ThemeTokens, mode: ThemeMode, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.tokens = tokens;
    theme.mode = mode;
    theme.colors = ThemeColors::from_tokens(tokens, mode.is_dark());
    cx.refresh_windows();
  }

  pub fn sync_system_appearance(cx: &mut App) {
    Self::set_mode(cx.window_appearance().into(), cx);
  }

  pub fn sync_scrollbar_appearance(cx: &mut App) {
    Theme::global_mut(cx).scrollbar_show = if cx.should_auto_hide_scrollbars() {
      ScrollbarShow::Scrolling
    } else {
      ScrollbarShow::Hover
    };
  }
}

pub fn init(cx: &mut App) {
  if !cx.has_global::<Theme>() {
    cx.set_global(Theme::default());
  }
  Theme::sync_scrollbar_appearance(cx);
  Theme::sync_system_appearance(cx);
}
