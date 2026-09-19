//! woocraft — gpu-accelerated graphical components for the woocraft design
//! system.
//!
//! the foundation layer is [`gpui-base`]: style-free behavior, interaction,
//! and infrastructure primitives, with the `gpui-pre` runtime snapshot
//! re-exported as [`gpui`]. on top of it this crate ships its infrastructure:
//! [`error`] for fallible operations, [`logging`] for telemetry bootstrap,
//! [`theme`] for the oklch-defined design system, [`i18n`] for the
//! domain-scoped translations, and — behind the default `resources` feature —
//! [`assets`] embedding the built-in icons and fonts. versioning restarts at
//! `0.6.0`.
//!
//! [`gpui-base`]: https://docs.rs/gpui-base

pub mod error;
pub mod i18n;
pub mod logging;
pub mod theme;

#[cfg(feature = "resources")]
pub mod assets;

#[cfg(feature = "resources")]
pub use assets::{
  Assets, BUILTIN_ASSET_PREFIX, CombinedSource, EmbeddedSource, has_asset, list_assets,
  register_fonts,
};
pub use error::{Error, Result};
pub use gpui;
pub use i18n::{
  SUPPORTED_LOCALES, WOOCRAFT_I18N_DOMAIN, available_locales, extend_locale, load_locale, locale,
  locale_display_name, set_locale, translate, translate_in_locale, translate_woocraft,
  translate_woocraft_in_locale, try_translate, try_translate_in_locale, try_translate_woocraft,
  try_translate_woocraft_in_locale, woocraft_key,
};
pub use rust_i18n::{available_locales as available_locales_macro, t, tkv};
pub use theme::{
  ActiveTheme, ScrollbarShow, SyntaxTokenHues, Theme, ThemeColors, ThemeMode, ThemeTokens,
};

pub const DEFAULT_FONT_FAMILY: &str = "Maple Mono";

/// The embedded terminal-optimized monospace family: Maple Mono NF (the
/// Nerd Font build — Nerd Font icons, powerline, box drawing, braille and
/// math symbols at uniform cell metrics). Regular/Bold/Italic/BoldItalic
/// are embedded; CJK still resolves through the platform fallback chain.
pub const TERMINAL_FONT_FAMILY: &str = "Maple Mono NF";

rust_i18n::i18n!("locales", fallback = "en-us");

/// Initializes built-in infrastructure: registers every embedded font face
/// with the gpui text system, installs the global [`Theme`] synced with the
/// system appearance, and activates the environment locale.
///
/// Safe to call once per [`gpui::App`]; font registration failures surface
/// through the returned [`Result`] instead of aborting the application.
pub fn init(cx: &mut gpui::App) -> Result<()> {
  #[cfg(feature = "resources")]
  register_fonts(cx.text_system())?;

  theme::init(cx);
  i18n::init();

  Ok(())
}
