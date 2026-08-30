// Re-export git dependencies so downstream crates resolve the exact same
// revisions as `woocraft` itself. Git dependencies are unified by Cargo across
// a dependency graph only when they point to the same source; importing these
// crates directly in a consumer can silently pick a different rev (or a
// semver-incompatible copy), so prefer the re-exports below.
pub use gpui;
pub use gpui_macros;
pub use gpui_sum_tree;

pub mod actions;
#[macro_use]
pub mod base;
pub mod i18n;
mod widgets;

rust_i18n::i18n!("locales", fallback = "en-us");

#[cfg(feature = "resources")]
mod assets;

#[cfg(feature = "resources")]
pub use assets::*;
pub use base::*;
pub use i18n::{
  SUPPORTED_LOCALES, WOOCRAFT_I18N_DOMAIN, available_locales, extend_locale, load_locale, locale,
  locale_display_name, set_locale, translate, translate_in_locale, translate_woocraft,
  translate_woocraft_in_locale, try_translate, try_translate_in_locale, try_translate_woocraft,
  try_translate_woocraft_in_locale, woocraft_key,
};
pub use rust_i18n::{available_locales as available_locales_macro, t, tkv};
pub use widgets::*;
#[cfg(feature = "terminal")]
pub use woocraft_terminal::alacritty_terminal;

pub const DEFAULT_FONT_FAMILY: &str = "Maple Mono";

pub fn init(cx: &mut gpui::App) {
  #[cfg(feature = "resources")]
  assets::register_fonts(cx.text_system())
    .expect("failed to register embedded fonts from src/assets/fonts");

  i18n::init();
  actions::init(cx);
  base::init(cx);
  widgets::init(cx);
}
