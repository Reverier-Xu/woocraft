#![allow(clippy::module_inception)]

mod backend;
mod blink_cursor;
mod change;
mod cursor;
mod editor;
mod element;
pub mod highlighter;
mod indent;
pub mod lsp;
mod mask_pattern;
mod mode;
mod movement;
pub(crate) mod popovers;
mod rope_ext;
mod search;
mod selection;
mod state;
mod text_wrapper;
mod viewport;
mod viewport_element;

pub use backend::*;
pub use editor::Editor as CodeEditor;
#[allow(unused_imports)]
pub use highlighter::*;
pub use indent::TabSize;
pub use lsp::*;
pub use lsp_types::Position;
pub use mask_pattern::MaskPattern;
pub use rope_ext::*;
pub use ropey::Rope;
pub(crate) use state::*;
pub use state::{InputEvent as EditorEvent, InputState as EditorState};

pub(crate) fn init(cx: &mut gpui::App) {
  state::init(cx);
}
