//! High-performance data table with virtual scrolling, column customization,
//! and cell selection.
//!
//! The table module provides a flexible, feature-rich table component designed
//! to handle large datasets efficiently through virtual rendering. Rows are
//! only rendered when visible, making performance independent of data size.
//! Supports multiple selection modes (row, column, cell), keyboard navigation,
//! context menus, and dynamic column configuration.
//!
//! # Architecture
//! The table system is split into several components:
//! - [`Table`]: The main rendering element (thin wrapper around TableState)
//! - [`TableState`]: Manages table state and keyboard handling
//! - [`TableDelegate`]: Trait for custom column definitions and cell rendering
//! - [`TableColumn`]: Column metadata (width, sortability, custom renderers)
//!
//! # Features
//! - **Virtual Scrolling**: Only visible rows are rendered, linear O(1) memory
//!   cost
//! - **Selection Modes**: Row, column, or cell selection with multi-select
//!   support
//! - **Keyboard Navigation**: Arrow keys, Tab, Home/End, PageUp/Down with
//!   intelligent selection
//! - **Column Customization**: Resizable, reorderable, fixed (pinned), and
//!   sortable columns
//! - **Custom Renderers**: Per-column cell rendering via
//!   `TableColumn::builder_for_cell()`
//! - **Striping & Borders**: Optional alternate row colors and grid borders
//! - **Context Menus**: Right-click support on rows and cells
//! - **Scrollbar Control**: Independent control of vertical/horizontal
//!   scrollbar visibility
//!
//! # Example
//! ```rust,ignore
//! use woocraft::{Table, TableState, TableDelegate};
//!
//! // Simple table with standard delegate
//! let table_state = cx.new(|cx| {
//!   let delegate = MyTableDelegate { rows: vec![...] };
//!   TableState::new(delegate, cx)
//! });
//!
//! Table::new(&table_state)
//!   .stripe(true)
//!   .bordered(true)
//! ```
//!
//! # Performance Notes
//! - Table uses virtual rendering via GPUI's layout system; O(1) memory for
//!   10k+ rows
//! - Column rendering is memoized; changing one column doesn't re-render
//!   unchanged columns
//! - For very large tables (100k+ rows), ensure `TableDelegate::row_count()` is
//!   O(1)
//! - Custom cell renderers should avoid blocking operations; consider async
//!   cells via `Entity<View>`

use gpuim::{Edges, Pixels};

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

/// Configuration options for table styling and behavior.
///
/// Used internally by [`Table`] to pass configuration to [`TableState`].
pub(crate) struct TableOptions {
  /// Visibility of scrollbars (top, bottom, left, right edges).
  pub(crate) scrollbar_visible: Edges<bool>,
  /// Whether to use alternating row colors (striping) for improved readability.
  pub(crate) stripe: bool,
  /// Whether to render grid borders between cells.
  pub(crate) bordered: bool,
  /// Cell content size (affects padding, text size, and gaps).
  pub(crate) size: Size,
  /// Optional bottom gap (in pixels) to allow scrolling past the last element.
  pub(crate) bottom_gap: Option<Pixels>,
  /// Whether to auto-detect column widths from header and sample rows.
  pub(crate) auto_detect_col_width: bool,
}

impl Default for TableOptions {
  fn default() -> Self {
    Self {
      scrollbar_visible: Edges::all(true),
      stripe: false,
      bordered: true,
      size: Size::default(),
      bottom_gap: None,
      auto_detect_col_width: false,
    }
  }
}

pub trait TableThemeExt {
  fn table_bg(&self) -> gpuim::Hsla;
  fn table_head(&self) -> gpuim::Hsla;
  fn table_head_foreground(&self) -> gpuim::Hsla;
  fn table_even(&self) -> gpuim::Hsla;
  fn table_hover(&self) -> gpuim::Hsla;
  fn table_active(&self) -> gpuim::Hsla;
}

impl TableThemeExt for crate::Theme {
  fn table_bg(&self) -> gpuim::Hsla {
    self.background
  }

  fn table_head(&self) -> gpuim::Hsla {
    self.title_bar
  }

  fn table_head_foreground(&self) -> gpuim::Hsla {
    self.foreground
  }

  fn table_even(&self) -> gpuim::Hsla {
    self.foreground.opacity(0.015)
  }

  fn table_hover(&self) -> gpuim::Hsla {
    self.foreground.opacity(0.04)
  }

  fn table_active(&self) -> gpuim::Hsla {
    self.primary.opacity(0.12)
  }
}
