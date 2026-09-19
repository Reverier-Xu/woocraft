//! woocraft — gpu-accelerated graphical components for the woocraft design
//! system.
//!
//! the foundation layer is [`gpui-base`]: style-free behavior, interaction,
//! and infrastructure primitives. on top of it this crate ships its
//! infrastructure: [`error`] for fallible operations, [`logging`] for
//! telemetry bootstrap, [`theme`] for the oklch-defined design system,
//! [`i18n`] for the domain-scoped translations, and — behind the default
//! `resources` feature — [`assets`] embedding the built-in icons and fonts.
//! versioning restarts at `0.6.0`.
//!
//! the `gpui-pre` runtime family stays an internal detail: both layers are
//! re-exported so applications list `woocraft` alone, and a future migration
//! back to the official gpui release only touches this crate.
//!
//! | path | crate | role |
//! | --- | --- | --- |
//! | [`gpui`] | `gpui-pre` | zed's gpui runtime snapshot |
//! | [`platform`] | `gpui-pre-platform` | os platform backends |
//! | [`application`] | `gpui-pre-platform` | desktop bootstrap helper |
//!
//! [`gpui-base`]: https://docs.rs/gpui-base

pub mod error;
pub mod i18n;
pub mod icon;
pub mod logging;
pub mod theme;
pub mod widgets;

#[cfg(feature = "tray")]
pub mod tray;

#[cfg(feature = "resources")]
pub mod assets;

#[cfg(feature = "resources")]
pub use assets::{
  Assets, BUILTIN_ASSET_PREFIX, CombinedSource, EmbeddedSource, has_asset, has_icon, list_assets,
  list_icons, register_fonts,
};
pub use base::{
  Collapsible, Disableable, FocusableExt, RoleOverride, Selectable, StateStyle, StyledExt,
  box_shadow, h_flex, v_flex,
};
pub use error::{Error, Result};
pub use gpui;
pub use gpui_base as base;
pub use gpui_platform as platform;
pub use gpui_platform::application;
pub use i18n::{
  SUPPORTED_LOCALES, WOOCRAFT_I18N_DOMAIN, available_locales, extend_locale, load_locale, locale,
  locale_display_name, set_locale, translate, translate_in_locale, translate_woocraft,
  translate_woocraft_in_locale, try_translate, try_translate_in_locale, try_translate_woocraft,
  try_translate_woocraft_in_locale, woocraft_key,
};
pub use icon::{
  Icon, IconName, IconNamed, clear_custom_icons, custom_icon_path, register_icon, unregister_icon,
};
pub use rust_i18n::{available_locales as available_locales_macro, t, tkv};
pub use theme::{
  ActiveTheme, ScrollbarShow, SyntaxTokenHues, Theme, ThemeColors, ThemeMode, ThemeTokens,
};
#[cfg(feature = "tray")]
pub use tray::{
  Tray, TrayAppContext, TrayClickEvent, TrayEvent, TrayMenuItem, TrayMouseButton, tray_events,
};
pub use widgets::{
  badge::Badge,
  button::{Button, ButtonVariant, ButtonVariants},
  checkbox::{Checkbox, CheckboxState},
  divider::{Divider, DividerStyle},
  icon_label::IconLabel,
  kbd::Kbd,
  label::{HighlightsMatch, Label},
  radio::Radio,
  radio_group::RadioGroup,
  spinner::Spinner,
  switch::Switch,
  tag::{Tag, TagVariant},
  title_bar::TitleBar,
  toggle::Toggle,
  toggle_group::ToggleGroup,
  window_border::{WindowBorder, window_paddings},
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
  // foundation globals first: base state, reduce-motion, popover and focus
  // infrastructure that styled widgets build on.
  base::init(cx);

  #[cfg(feature = "resources")]
  register_fonts(cx.text_system())?;

  theme::init(cx);
  i18n::init();

  Ok(())
}

#[cfg(test)]
mod foundation_tests {
  use super::{Button, Selectable, h_flex, v_flex};

  #[test]
  fn button_satisfies_shared_control_traits() {
    let button = Button::new("foundation-test").selected(true);
    assert!(button.is_selected());
  }

  #[test]
  fn layout_helpers_are_available() {
    let _row = h_flex();
    let _column = v_flex();
  }
}
