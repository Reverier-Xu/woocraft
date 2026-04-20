use std::rc::Rc;

use gpuim::{App, Context, Entity, IntoElement, KeyBinding, Pixels, RenderOnce, Window};

use crate::{
  Sizable, Size, TableDelegate, TableState,
  actions::{
    Cancel, SelectDown, SelectFirst, SelectLast, SelectNextColumn, SelectPageDown, SelectPageUp,
    SelectPrevColumn, SelectUp,
  },
};

const CONTEXT: &str = "Table";

pub(crate) fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("escape", Cancel, Some(CONTEXT)),
    KeyBinding::new("up", SelectUp, Some(CONTEXT)),
    KeyBinding::new("down", SelectDown, Some(CONTEXT)),
    KeyBinding::new("left", SelectPrevColumn, Some(CONTEXT)),
    KeyBinding::new("right", SelectNextColumn, Some(CONTEXT)),
    KeyBinding::new("home", SelectFirst, Some(CONTEXT)),
    KeyBinding::new("end", SelectLast, Some(CONTEXT)),
    KeyBinding::new("pageup", SelectPageUp, Some(CONTEXT)),
    KeyBinding::new("pagedown", SelectPageDown, Some(CONTEXT)),
    KeyBinding::new("tab", SelectNextColumn, Some(CONTEXT)),
    KeyBinding::new("shift-tab", SelectPrevColumn, Some(CONTEXT)),
  ]);
}

type BlankContextMenuBuilder<D> =
  dyn Fn(crate::PopupMenu, &mut Window, &mut Context<TableState<D>>) -> crate::PopupMenu;

/// A high-performance table element with virtual scrolling and cell selection.
///
/// `Table` is the primary user-facing component for displaying tabular data. It
/// wraps a [`TableState`] and delegates rendering to it. Configuration happens
/// via builder methods on `Table` itself; the underlying state manages
/// interaction, selection, and keyboard handling.
///
/// # Features
/// - **Virtual Scrolling**: Efficient rendering of large datasets (tested with
///   100k+ rows)
/// - **Multiple Selection Modes**: Row selection, column selection, or
///   individual cell selection
/// - **Keyboard Navigation**: Full keyboard support (arrow keys, Tab, Home/End,
///   PageUp/Down)
/// - **Customizable Rendering**: Per-cell, per-column, and per-row custom
///   rendering via delegate
/// - **Visual Customization**: Striping, borders, scrollbar visibility
///
/// # Configuration
/// Configuration happens at the [`TableState`] level (via the delegate) and at
/// the `Table` rendering level (via builder methods). Most configuration should
/// happen before adding the table to the UI tree to avoid unnecessary
/// re-renders.
///
/// # Selection Modes
/// The delegate determines selection behavior:
/// - **Row Selection**: Click rows to select entire rows; keyboard selects
///   adjacent rows
/// - **Cell Selection**: Click individual cells; keyboard navigates
///   cell-by-cell
/// - **Multi-Select**: Hold Shift to extend selection; Ctrl/Cmd to toggle cells
///
/// # Example
/// ```rust,ignore
/// // Create table state with custom delegate
/// let table_state = cx.new(|cx| {
///   TableState::new(my_delegate, cx)
///     .cell_selectable(true)
/// });
///
/// // Render the table with visual options
/// Table::new(&table_state)
///   .stripe(true)        // Alternate row colors
///   .bordered(true)      // Show grid lines
///   .scrollbar_visible(true, true)  // Show both scrollbars
/// ```
///
/// # Performance Characteristics
/// The table uses virtual scrolling, so rendering time and memory are
/// **O(viewport_height)**, not O(row_count). This means:
/// - 100 rows with the same performance as 10,000 rows (if viewport fits ~20
///   rows)
/// - Smooth scrolling and fast initial render regardless of data size
/// - Horizontal scrolling within cells also uses virtualization
///
/// # Limitations
/// - Currently single-line cells only (text wrapping not supported)
/// - Column resizing is not persisted (resets on re-render)
/// - Sorting is handled by the delegate, not the table itself
#[derive(IntoElement)]
pub struct Table<D: TableDelegate> {
  state: Entity<TableState<D>>,
  stripe: bool,
  bordered: bool,
  size: Size,
  auto_detect_col_width: bool,
  scrollbar_visible_vertical: bool,
  scrollbar_visible_horizontal: bool,
  bottom_gap: Option<Pixels>,
  blank_context_menu_builder: Option<Rc<BlankContextMenuBuilder<D>>>,
}

impl<D> Table<D>
where
  D: TableDelegate,
{
  /// Create a new Table element with the given [`TableState`].
  ///
  /// The [`TableState`] typically comes from `cx.new(|cx|
  /// TableState::new(delegate, cx))`. Configuration such as selection mode
  /// happens on the state, not on the Table itself.
  pub fn new(state: &Entity<TableState<D>>) -> Self {
    Self {
      state: state.clone(),
      stripe: false,
      bordered: true,
      size: Size::default(),
      auto_detect_col_width: false,
      scrollbar_visible_vertical: true,
      scrollbar_visible_horizontal: true,
      bottom_gap: None,
      blank_context_menu_builder: None,
    }
  }

  /// Enable alternating row background colors for improved readability.
  ///
  /// When enabled, even-numbered rows use a slightly darker background.
  /// Default: `false`.
  pub fn stripe(mut self, stripe: bool) -> Self {
    self.stripe = stripe;
    self
  }

  /// Show or hide grid borders between cells and rows.
  ///
  /// When enabled, cells are separated by subtle borders. Default: `true`.
  pub fn bordered(mut self, bordered: bool) -> Self {
    self.bordered = bordered;
    self
  }

  /// Control scrollbar visibility for vertical and horizontal directions.
  ///
  /// Scrollbars appear automatically only when content exceeds viewport size,
  /// but can be hidden entirely with `false`. Default: both `true`.
  ///
  /// # Arguments
  /// * `vertical` - Show vertical (up/down) scrollbar
  /// * `horizontal` - Show horizontal (left/right) scrollbar
  pub fn scrollbar_visible(mut self, vertical: bool, horizontal: bool) -> Self {
    self.scrollbar_visible_vertical = vertical;
    self.scrollbar_visible_horizontal = horizontal;
    self
  }

  /// Add bottom padding (in pixels) so users can scroll past the last row.
  ///
  /// Useful for ensuring the last row isn't hidden behind fixed UI elements or
  /// to improve visual balance. Optional.
  pub fn bottom_gap(mut self, gap: impl Into<Pixels>) -> Self {
    self.bottom_gap = Some(gap.into());
    self
  }

  /// Enable/disable auto-detection for column widths.
  ///
  /// When enabled, table computes each column width from shaped text metrics
  /// (same strategy used by Input caret/selection measurement), using the
  /// widest item among header and the first 3 rows of
  /// `TableDelegate::cell_text`, then clamps with column `min_width` /
  /// `max_width`.
  ///
  /// Default: `false`.
  pub fn auto_detect_col_width(mut self, auto_detect_col_width: bool) -> Self {
    self.auto_detect_col_width = auto_detect_col_width;
    self
  }

  /// Inject a context menu builder for right-clicks on table blank area.
  ///
  /// This does not affect row context menus provided by [`TableDelegate`].
  pub fn blank_context_menu<F>(mut self, builder: F) -> Self
  where
    F: Fn(crate::PopupMenu, &mut Window, &mut Context<TableState<D>>) -> crate::PopupMenu + 'static,
  {
    self.blank_context_menu_builder = Some(Rc::new(builder));
    self
  }
}

impl<D> Sizable for Table<D>
where
  D: TableDelegate,
{
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl<D> RenderOnce for Table<D>
where
  D: TableDelegate,
{
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    self.state.update(cx, |state, cx| {
      let should_refresh_col_groups = state.options.auto_detect_col_width
        != self.auto_detect_col_width
        || (self.auto_detect_col_width && state.options.size != self.size);

      state.options.bordered = self.bordered;
      state.options.stripe = self.stripe;
      state.options.size = self.size;
      state.options.auto_detect_col_width = self.auto_detect_col_width;
      state.options.scrollbar_visible = gpuim::Edges {
        right: self.scrollbar_visible_vertical,
        bottom: self.scrollbar_visible_horizontal,
        ..Default::default()
      };
      state.options.bottom_gap = self.bottom_gap;
      state.blank_context_menu_builder = self.blank_context_menu_builder;

      if should_refresh_col_groups {
        state.refresh(cx);
      }
    });

    self.state
  }
}
