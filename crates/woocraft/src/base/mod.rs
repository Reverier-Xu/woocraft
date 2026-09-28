#[macro_use]
mod macros;

mod anchor;
mod animation;
mod caret;
mod element_ext;
mod fonts;
mod icon;
mod index_path;
mod interaction;
mod layout;
mod motion;
mod plot;
mod selection;
mod style;
mod theme;
mod time;
mod tree;

pub use anchor::*;
pub use animation::*;
pub use caret::*;
pub use element_ext::*;
pub use fonts::*;
use gpui::App;
pub use icon::*;
pub use index_path::*;
pub use interaction::*;
pub use layout::*;
pub use motion::*;
pub use plot::*;
pub use selection::*;
pub use style::*;
pub use theme::*;
pub use time::*;
pub use tree::*;

/// Initializes the base layer: theme, system appearance sync, and the
/// operating system's reduced-motion preference.
///
/// This explicit definition shadows the `init` re-exported from [`theme`]
/// so that every base subsystem initializes in one call from
/// [`crate::init`].
pub fn init(cx: &mut App) {
  theme::init(cx);
  motion::init(cx);
}
