pub mod base;
mod examples;
mod i18n;
mod widgets;

#[cfg(feature = "resources")]
mod assets;

pub use base::*;
pub use widgets::*;
#[cfg(feature = "resources")]
pub use assets::*;

pub const DEFAULT_FONT_FAMILY: &str = "Reverier Mono";

pub fn init(cx: &mut gpui::App) {
	#[cfg(feature = "resources")]
	assets::register_fonts(cx.text_system())
		.expect("failed to register embedded fonts from src/assets/fonts");

	base::init(cx);
}
