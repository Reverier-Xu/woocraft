//! Markdown component: parser and gpui renderer.
//!
//! See [`Markdown`] for the element and [`ParsedMarkdown`] for the cacheable
//! parse result.
//!
//! The renderer is split by responsibility rather than by size:
//!
//! - [`parser`] turns CommonMark/GFM source into [`ParsedMarkdown`], the
//!   cacheable block tree.
//! - [`palette`] resolves the active theme into the colors and inline styles
//!   the renderer uses.
//! - [`selection`] owns text-selection offsets over the rendered runs.
//! - [`table`] measures and renders tables.
//! - [`element`] is the component itself: block rendering plus the [`Markdown`]
//!   element and its builder.

mod element;
mod palette;
mod parser;
mod selection;
mod table;

pub use element::*;
pub use parser::*;
