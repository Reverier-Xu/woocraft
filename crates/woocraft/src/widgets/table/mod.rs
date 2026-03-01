use gpui::{Edges, Pixels};

use crate::Size;

mod column;
mod delegate;
mod loading;
mod state;
#[allow(clippy::module_inception)]
mod table;

pub use column::*;
pub use delegate::*;
pub use state::*;
pub use table::*;

pub(crate) struct TableOptions {
  pub(crate) scrollbar_visible: Edges<bool>,
  /// Set stripe style of the table.
  pub(crate) stripe: bool,
  /// Set to use border style of the table.
  pub(crate) bordered: bool,
  /// The cell size of the table.
  pub(crate) size: Size,
  /// Optional bottom gap for scrolling past the last element.
  pub(crate) bottom_gap: Option<Pixels>,
}

impl Default for TableOptions {
  fn default() -> Self {
    Self {
      scrollbar_visible: Edges::all(true),
      stripe: false,
      bordered: true,
      size: Size::default(),
      bottom_gap: None,
    }
  }
}

pub trait TableThemeExt {
  fn table_bg(&self) -> gpui::Hsla;
  fn table_head(&self) -> gpui::Hsla;
  fn table_head_foreground(&self) -> gpui::Hsla;
  fn table_even(&self) -> gpui::Hsla;
  fn table_hover(&self) -> gpui::Hsla;
  fn table_active(&self) -> gpui::Hsla;
}

impl TableThemeExt for crate::Theme {
  fn table_bg(&self) -> gpui::Hsla {
    self.card
  }

  fn table_head(&self) -> gpui::Hsla {
    self.title_bar
  }

  fn table_head_foreground(&self) -> gpui::Hsla {
    self.foreground
  }

  fn table_even(&self) -> gpui::Hsla {
    self.foreground.opacity(0.015)
  }

  fn table_hover(&self) -> gpui::Hsla {
    self.foreground.opacity(0.04)
  }

  fn table_active(&self) -> gpui::Hsla {
    self.primary.opacity(0.12)
  }
}
