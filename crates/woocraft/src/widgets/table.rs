//! styled data table over a column model with sorting and row selection.
//!
//! base ships only the unstyled accessibility table roles; the column model,
//! the sort descriptor, and the row selection live here in [`TableState`],
//! while the data itself stays with the caller — a [`Table::render_cell`]
//! callback materializes the visible cells inside a virtualized row list.
//!
//! ```rust,ignore
//! let state = cx.new(|cx| TableState::new(cx));
//! state.update(cx, |state, _| {
//!   state.set_columns(vec![
//!     Column::new("name", "name").sortable(true),
//!     Column::new("size", "size").fixed(rems(6.)),
//!   ]);
//! });
//! let table = Table::new(&state)
//!   .rows(files.len())
//!   .render_cell(|row, column, _, _| div().child(format!("{row}:{column}")));
//! ```

use std::{ops::Range, rc::Rc};

use gpui::{
  AnyElement, App, ElementId, EventEmitter, InteractiveElement, IntoElement, ParentElement as _,
  Refineable as _, Rems, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, rems,
};

use crate::{
  ActiveTheme, Icon, IconName,
  theme::{opacity, with_alpha},
};

/// sort direction of one column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColumnSort {
  #[default]
  Ascending,
  Descending,
}

impl ColumnSort {
  /// flips the direction.
  pub fn toggled(self) -> Self {
    match self {
      ColumnSort::Ascending => ColumnSort::Descending,
      ColumnSort::Descending => ColumnSort::Ascending,
    }
  }
}

/// width of one column: an absolute track or a share of the leftover space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnWidth {
  /// absolute width in rems.
  Fixed(Rems),
  /// grow weight over the space the fixed columns leave free.
  Fraction(f32),
}

/// one column of the table: identity, header label, width, and whether the
/// header cycles a sort descriptor.
#[derive(Debug, Clone, PartialEq)]
pub struct Column {
  /// stable semantic identity, owned by the application.
  pub id: SharedString,
  /// header label text.
  pub name: SharedString,
  /// width of the column track.
  pub width: ColumnWidth,
  /// whether the header click cycles [`TableState::sort`].
  pub sortable: bool,
}

impl Column {
  /// creates a column that shares the leftover space by default.
  pub fn new(id: impl Into<SharedString>, name: impl Into<SharedString>) -> Self {
    Self {
      id: id.into(),
      name: name.into(),
      width: ColumnWidth::Fraction(1.),
      sortable: false,
    }
  }

  /// pins the column to an absolute width in rems.
  pub fn fixed(mut self, width: impl Into<gpui::Rems>) -> Self {
    self.width = ColumnWidth::Fixed(width.into());
    self
  }

  /// sets the grow weight over the leftover space.
  pub fn fraction(mut self, weight: f32) -> Self {
    self.width = ColumnWidth::Fraction(weight);
    self
  }

  /// marks the column header as sort-cycling.
  pub fn sortable(mut self, sortable: bool) -> Self {
    self.sortable = sortable;
    self
  }
}

/// user intents surfaced by the table state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableEvent {
  /// the selected row changed; `usize` is the row index, `None` clears.
  Select(Option<usize>),
  /// the sort descriptor changed; `usize` is the column index.
  Sort(usize, ColumnSort),
}

/// the table model: columns, the sort descriptor, and the selected row.
///
/// the caller owns the rows themselves; the model only indexes them.
pub struct TableState {
  columns: Vec<Column>,
  sort: Option<(usize, ColumnSort)>,
  selected: Option<usize>,
  pub(crate) scroll_handle: gpui::UniformListScrollHandle,
}

impl EventEmitter<TableEvent> for TableState {}

impl TableState {
  /// creates an empty model without columns, sort, or selection.
  pub fn new(_: &mut gpui::App) -> Self {
    Self {
      columns: Vec::new(),
      sort: None,
      selected: None,
      scroll_handle: gpui::UniformListScrollHandle::new(),
    }
  }

  /// the configured columns.
  pub fn columns(&self) -> &[Column] {
    &self.columns
  }

  /// replaces the columns; the sort descriptor that points past the new
  /// column count is dropped.
  pub fn set_columns(&mut self, columns: Vec<Column>, cx: &mut gpui::Context<Self>) {
    self.columns = columns;
    if self.sort.is_some_and(|(ix, _)| ix >= self.columns.len()) {
      self.sort = None;
    }
    cx.notify();
  }

  /// the current sort descriptor: column index and direction.
  pub fn sort(&self) -> Option<(usize, ColumnSort)> {
    self.sort
  }

  /// sets the sort descriptor directly.
  pub fn set_sort(&mut self, sort: Option<(usize, ColumnSort)>, cx: &mut gpui::Context<Self>) {
    self.sort = sort;
    cx.notify();
  }

  /// cycles the sort of one column: none, then ascending, then descending,
  /// then none again. sorts of other columns are replaced.
  pub fn sort_by(&mut self, column: usize, cx: &mut gpui::Context<Self>) {
    let next = match self.sort {
      Some((ix, sort)) if ix == column => {
        if sort == ColumnSort::Descending {
          None
        } else {
          Some((column, sort.toggled()))
        }
      }
      _ => Some((column, ColumnSort::Ascending)),
    };
    if let Some((ix, sort)) = next {
      cx.emit(TableEvent::Sort(ix, sort));
    }
    self.sort = next;
    cx.notify();
  }

  /// the selected row index.
  pub fn selected(&self) -> Option<usize> {
    self.selected
  }

  /// selects one row; `None` clears the selection.
  pub fn select(&mut self, row: Option<usize>, cx: &mut gpui::Context<Self>) {
    if self.selected == row {
      return;
    }
    self.selected = row;
    cx.emit(TableEvent::Select(row));
    cx.notify();
  }

  /// scrolls the virtualized rows so the given row is visible.
  pub fn scroll_to_row(&self, row: usize, strategy: gpui::ScrollStrategy) {
    self.scroll_handle.scroll_to_item(row, strategy);
  }
}

/// cell content builder for one visible row/column pair.
type RenderCell = Rc<dyn Fn(usize, usize, &mut Window, &mut App) -> AnyElement + 'static>;

/// the styled table: a bordered card surface with a sticky themed header row
/// and a virtualized body whose cells come from the caller's render callback.
#[derive(IntoElement)]
pub struct Table {
  id: ElementId,
  state: gpui::Entity<TableState>,
  rows: usize,
  row_height: gpui::Rems,
  render_cell: Option<RenderCell>,
  style: StyleRefinement,
}

impl Table {
  /// creates a table bound to its model; the element id doubles as the
  /// debug selector for headless geometry tests.
  pub fn new(id: impl Into<ElementId>, state: &gpui::Entity<TableState>) -> Self {
    Self {
      id: id.into(),
      state: state.clone(),
      rows: 0,
      row_height: rems(2.25),
      render_cell: None,
      style: StyleRefinement::default(),
    }
  }

  /// sets the number of rows; only the visible window is materialized.
  pub fn rows(mut self, rows: usize) -> Self {
    self.rows = rows;
    self
  }

  /// overrides the row and header height; the default is `2.25rem`.
  pub fn row_height(mut self, height: impl Into<gpui::Rems>) -> Self {
    self.row_height = height.into();
    self
  }

  /// sets the cell content builder invoked for every visible row/column.
  pub fn render_cell(
    mut self, render: impl Fn(usize, usize, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.render_cell = Some(Rc::new(render));
    self
  }
}

impl Styled for Table {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

fn apply_cell_width<E: Styled>(cell: E, width: ColumnWidth, rem: gpui::Pixels) -> E {
  match width {
    ColumnWidth::Fixed(rems) => cell.w(rems.to_pixels(rem)).flex_none(),
    ColumnWidth::Fraction(weight) => cell.flex_grow(weight).flex_basis(px(0.)),
  }
}

impl RenderOnce for Table {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let rem = window.rem_size();
    let row_height = self.row_height.to_pixels(rem);
    let (fg, muted_foreground, primary, border) = (
      theme.foreground,
      theme.muted_foreground,
      theme.primary,
      theme.border,
    );
    let border_width = theme.border_width;
    let hover_bg = with_alpha(fg, opacity::transparent::HOVER);
    let selected_bg = with_alpha(primary, opacity::transparent::ACTIVE);
    let (columns, sort, scroll_handle) = {
      let state = self.state.read(cx);
      (
        state.columns.clone(),
        state.sort(),
        state.scroll_handle.clone(),
      )
    };
    let render_cell = self
      .render_cell
      .clone()
      .unwrap_or_else(|| Rc::new(|_, _, _, _| div().into_any_element()));
    let id = self.id.clone();

    let header = div()
      .flex()
      .flex_row()
      .items_center()
      .h(row_height)
      .flex_none()
      .text_color(muted_foreground)
      .children(columns.iter().enumerate().map(|(ix, column)| {
        let cell = div()
          .id(ix)
          .role(gpui::Role::ColumnHeader)
          .flex()
          .flex_row()
          .items_center()
          .gap(rems(0.5))
          .px(rems(0.75))
          .overflow_hidden()
          .whitespace_nowrap()
          .child(div().truncate().child(column.name.clone()));
        let cell = apply_cell_width(cell, column.width, rem);
        let state = self.state.clone();
        cell.when(column.sortable, |this| {
          this
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg))
            .on_click(move |_, _, cx| {
              state.update(cx, |state, cx| state.sort_by(ix, cx));
            })
            .children(
              sort
                .filter(|(sort_ix, _)| *sort_ix == ix)
                .map(|(_, direction)| {
                  let icon = match direction {
                    ColumnSort::Ascending => IconName::ArrowUp,
                    ColumnSort::Descending => IconName::ArrowDown,
                  };
                  Icon::new(icon)
                }),
            )
        })
      }));

    let body_state = self.state.clone();
    let body = gpui::uniform_list(
      "table-rows",
      self.rows,
      move |visible_range: Range<usize>, window: &mut Window, cx: &mut App| {
        let selected = body_state.read(cx).selected();
        visible_range
          .map(|row| {
            let select = body_state.clone();
            let render_cell = render_cell.clone();
            let mut cells: Vec<AnyElement> = Vec::with_capacity(columns.len());
            for (column_ix, column) in columns.iter().enumerate() {
              let cell = div()
                .flex()
                .flex_row()
                .items_center()
                .px(rems(0.75))
                .overflow_hidden()
                .whitespace_nowrap()
                .child((render_cell)(row, column_ix, window, cx));
              cells.push(apply_cell_width(cell, column.width, rem).into_any_element());
            }
            let selected = Some(row) == selected;
            div()
              .id(row)
              .role(gpui::Role::Row)
              .flex()
              .flex_row()
              .items_center()
              .h(row_height)
              .when(selected, |this| this.bg(selected_bg))
              .hover(move |s| s.bg(hover_bg))
              .on_click(move |_, _, cx| {
                select.update(cx, |state, cx| {
                  state.select(
                    if state.selected() == Some(row) {
                      None
                    } else {
                      Some(row)
                    },
                    cx,
                  );
                });
              })
              .children(cells)
          })
          .collect()
      },
    )
    .track_scroll(&scroll_handle)
    .flex_1()
    .min_h_0();

    let mut container = div()
      .id(id.clone())
      .debug_selector(move || id.to_string())
      .role(gpui::Role::Table)
      .flex()
      .flex_col()
      .min_h_0()
      .overflow_hidden()
      .rounded(theme.radius_container)
      .border(border_width)
      .border_color(border)
      .bg(theme.card)
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(fg)
      .child(header)
      .child(div().h(border_width).w_full().flex_none().bg(border))
      .child(body);

    // caller refinements win over the card defaults.
    container.style().refine(&self.style);
    container
  }
}

#[cfg(test)]
mod tests {
  use gpui::{
    AppContext as _, InteractiveElement as _, IntoElement, ParentElement as _, Render, Styled, div,
  };

  use super::{Column, ColumnSort, Table, TableEvent, TableState};

  /// a host view that owns the state entity and collects its events, since
  /// subscriptions need an entity context to attach to.
  struct EventLogHost {
    state: gpui::Entity<TableState>,
    events: std::rc::Rc<std::cell::RefCell<Vec<TableEvent>>>,
    _subscription: gpui::Subscription,
  }

  impl EventLogHost {
    fn new(cx: &mut gpui::Context<Self>) -> Self {
      let state = cx.new(|cx| TableState::new(cx));
      let events = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
      let sink = events.clone();
      let subscription = cx.subscribe(&state, move |_, _, event: &TableEvent, _| {
        sink.borrow_mut().push(event.clone());
      });
      Self {
        state,
        events,
        _subscription: subscription,
      }
    }
  }

  impl Render for EventLogHost {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
      div()
    }
  }

  #[gpui::test]
  fn sorting_cycles_through_three_states(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (view, cx) = cx.add_window_view(|_, cx| EventLogHost::new(cx));
    cx.update(|_, cx| {
      let state = view.read(cx).state.clone();
      state.update(cx, |state, cx| {
        state.set_columns(vec![Column::new("name", "name").sortable(true)], cx);
        state.sort_by(0, cx);
        state.sort_by(0, cx);
        state.sort_by(0, cx);
      });
      assert_eq!(state.read(cx).sort(), None);
    });

    let events = cx.update(|_, cx| view.read(cx).events.borrow().clone());
    assert_eq!(
      events.as_slice(),
      [
        TableEvent::Sort(0, ColumnSort::Ascending),
        TableEvent::Sort(0, ColumnSort::Descending),
      ]
    );
  }

  #[gpui::test]
  fn selection_emits_and_toggles(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (view, cx) = cx.add_window_view(|_, cx| EventLogHost::new(cx));
    cx.update(|_, cx| {
      let state = view.read(cx).state.clone();
      state.update(cx, |state, cx| {
        state.select(Some(3), cx);
        state.select(Some(3), cx);
      });
      assert_eq!(state.read(cx).selected(), Some(3));
      state.update(cx, |state, cx| state.select(None, cx));
      assert_eq!(state.read(cx).selected(), None);
    });
    let events = cx.update(|_, cx| view.read(cx).events.borrow().clone());
    assert_eq!(
      events.as_slice(),
      [TableEvent::Select(Some(3)), TableEvent::Select(None)]
    );
  }

  #[gpui::test]
  fn replacing_columns_drops_an_out_of_range_sort(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (view, cx) = cx.add_window_view(|_, cx| EventLogHost::new(cx));
    cx.update(|_, cx| {
      let state = view.read(cx).state.clone();
      state.update(cx, |state, cx| {
        state.set_columns(vec![Column::new("a", "a"), Column::new("b", "b")], cx);
        state.set_sort(Some((1, ColumnSort::Ascending)), cx);
        state.set_columns(vec![Column::new("a", "a")], cx);
      });
      assert_eq!(state.read(cx).sort(), None);
    });
  }

  struct Host {
    state: gpui::Entity<TableState>,
  }

  impl Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
      div()
        .debug_selector(|| "table-host".into())
        .size_full()
        .child(
          Table::new("table", &self.state)
            .rows(3)
            .render_cell(|row, column, _, _| {
              div().child(format!("r{row}c{column}")).into_any_element()
            }),
        )
    }
  }

  #[gpui::test]
  fn the_table_paints_its_surface(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, cx| {
      let state = cx.new(|cx| {
        let mut state = TableState::new(cx);
        state.columns = vec![Column::new("a", "a"), Column::new("b", "b")];
        state
      });
      Host { state }
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    assert!(cx.debug_bounds("table").is_some(), "the table paints");
  }
}
