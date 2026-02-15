pub mod actions;
pub mod base;
mod examples;
mod i18n;
mod widgets;

rust_i18n::i18n!("locales", fallback = "en-us");

#[cfg(feature = "resources")]
mod assets;

#[cfg(feature = "resources")]
pub use assets::*;
pub use base::*;
pub use i18n::{SUPPORTED_LOCALES, locale, set_locale};
pub use widgets::*;

pub const DEFAULT_FONT_FAMILY: &str = "Reverier Mono";

pub fn init(cx: &mut gpui::App) {
  #[cfg(feature = "resources")]
  assets::register_fonts(cx.text_system())
    .expect("failed to register embedded fonts from src/assets/fonts");

  i18n::init();
  actions::init(cx);
  base::init(cx);
  widgets::init(cx);
}
