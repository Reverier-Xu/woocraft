use gpui::{App, Entity, IntoElement, KeyBinding, Pixels, RenderOnce, Window};

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

/// A table element with support for row, column, and cell selection.
///
/// # Features
///
/// - **Multiple Selection Modes**: Support for row, column, and cell selection
/// - **Cell Selection**: Click to select individual cells, with keyboard
///   navigation
/// - **Virtual Scrolling**: Efficient rendering of large datasets
/// - **Resizable Columns**: Drag column borders to resize
/// - **Movable Columns**: Drag column headers to reorder
/// - **Fixed Columns**: Pin columns to the left side
/// - **Sortable Columns**: Click column headers to sort
/// - **Context Menus**: Right-click support for rows and cells
///
/// # Cell Selection Mode
///
/// When cell selection is enabled via [`TableState::cell_selectable()`]:
/// - Click on cells to select them
/// - A row selector column appears on the left for selecting entire rows
/// - Keyboard navigation (arrow keys, Tab, Home, End, PageUp, PageDown) works
///   at cell level
/// - Right-click and double-click events are supported
///
/// See [`TableState`] for more details on cell selection.
///
/// # Example
///
/// ```rust,ignore
/// let table_state = cx.new(|cx| {
///     TableState::new(delegate, cx)
///         .cell_selectable(true)
///         .row_selectable(true)
/// });
///
/// Table::new(&table_state)
///     .stripe(true)
///     .bordered(true)
/// ```
#[derive(IntoElement)]
pub struct Table<D: TableDelegate> {
  state: Entity<TableState<D>>,
  stripe: bool,
  bordered: bool,
  size: Size,
  scrollbar_visible_vertical: bool,
  scrollbar_visible_horizontal: bool,
  bottom_gap: Option<Pixels>,
}

impl<D> Table<D>
where
  D: TableDelegate,
{
  /// Create a new Table element with the given [`TableState`].
  pub fn new(state: &Entity<TableState<D>>) -> Self {
    Self {
      state: state.clone(),
      stripe: false,
      bordered: true,
      size: Size::default(),
      scrollbar_visible_vertical: true,
      scrollbar_visible_horizontal: true,
      bottom_gap: None,
    }
  }

  /// Set to use stripe style of the table, default to false.
  pub fn stripe(mut self, stripe: bool) -> Self {
    self.stripe = stripe;
    self
  }

  /// Set to use border style of the table, default to true.
  pub fn bordered(mut self, bordered: bool) -> Self {
    self.bordered = bordered;
    self
  }

  /// Set scrollbar visibility.
  pub fn scrollbar_visible(mut self, vertical: bool, horizontal: bool) -> Self {
    self.scrollbar_visible_vertical = vertical;
    self.scrollbar_visible_horizontal = horizontal;
    self
  }

  /// Set a bottom gap (in pixels) so the user can scroll past the last
  /// element.
  pub fn bottom_gap(mut self, gap: impl Into<Pixels>) -> Self {
    self.bottom_gap = Some(gap.into());
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
    self.state.update(cx, |state, _| {
      state.options.bordered = self.bordered;
      state.options.stripe = self.stripe;
      state.options.size = self.size;
      state.options.scrollbar_visible = gpui::Edges {
        right: self.scrollbar_visible_vertical,
        bottom: self.scrollbar_visible_horizontal,
        ..Default::default()
      };
      state.options.bottom_gap = self.bottom_gap;
    });

    self.state
  }
}
