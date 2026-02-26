use std::cmp::Ordering;

use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Subscription, Window, WindowBounds, WindowOptions, div,
  prelude::FluentBuilder as _, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Column, ColumnFixed, ColumnSort, Input, InputState,
  PopupMenu, PopupMenuItem, Selectable, Sizable as _, StyledExt as _, Table, TableDelegate,
  TableEvent, TableState, Tag, Theme, ThemeMode, h_flex, init, v_flex, window_border,
};

#[derive(Clone)]
struct Employee {
  id: usize,
  name: String,
  team: &'static str,
  title: &'static str,
  score: u32,
  active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableCol {
  Id,
  Name,
  Team,
  Title,
  Score,
  Status,
  Actions,
}

impl TableCol {
  fn key(self) -> &'static str {
    match self {
      TableCol::Id => "id",
      TableCol::Name => "name",
      TableCol::Team => "team",
      TableCol::Title => "title",
      TableCol::Score => "score",
      TableCol::Status => "status",
      TableCol::Actions => "actions",
    }
  }

  fn label(self) -> &'static str {
    match self {
      TableCol::Id => "ID",
      TableCol::Name => "Name",
      TableCol::Team => "Team",
      TableCol::Title => "Title",
      TableCol::Score => "Score",
      TableCol::Status => "Status",
      TableCol::Actions => "Actions",
    }
  }

  fn default_width(self) -> gpui::Pixels {
    match self {
      TableCol::Id => px(72.),
      TableCol::Name => px(180.),
      TableCol::Team => px(140.),
      TableCol::Title => px(180.),
      TableCol::Score => px(100.),
      TableCol::Status => px(120.),
      TableCol::Actions => px(120.),
    }
  }

  fn sortable(self) -> bool {
    !matches!(self, TableCol::Actions)
  }

  fn selectable(self) -> bool {
    !matches!(self, TableCol::Actions)
  }

  fn fixed(self) -> Option<ColumnFixed> {
    match self {
      TableCol::Id | TableCol::Name => Some(ColumnFixed::Left),
      _ => None,
    }
  }
}

struct TableDemoDelegate {
  all_rows: Vec<Employee>,
  rows: Vec<Employee>,
  columns: Vec<TableCol>,
  query: String,
}

impl TableDemoDelegate {
  fn new() -> Self {
    let teams = ["Core", "Infra", "Design", "Data", "QA", "Growth"];
    let titles = [
      "Engineer",
      "Staff Engineer",
      "Designer",
      "Product Manager",
      "SRE",
      "Analyst",
    ];

    let all_rows = (1..=180)
      .map(|id| Employee {
        id,
        name: format!("Member {:03}", id),
        team: teams[id % teams.len()],
        title: titles[(id * 3) % titles.len()],
        score: ((id * 37) % 101) as u32,
        active: id % 4 != 0,
      })
      .collect::<Vec<_>>();

    Self {
      rows: all_rows.clone(),
      all_rows,
      columns: vec![
        TableCol::Id,
        TableCol::Name,
        TableCol::Team,
        TableCol::Title,
        TableCol::Score,
        TableCol::Status,
        TableCol::Actions,
      ],
      query: String::new(),
    }
  }

  fn set_query(&mut self, query: String) {
    self.query = query.trim().to_string();
    if self.query.is_empty() {
      self.rows = self.all_rows.clone();
      return;
    }

    let q = self.query.to_ascii_lowercase();
    self.rows = self
      .all_rows
      .iter()
      .filter(|row| {
        row.name.to_ascii_lowercase().contains(&q)
          || row.team.to_ascii_lowercase().contains(&q)
          || row.title.to_ascii_lowercase().contains(&q)
      })
      .cloned()
      .collect();
  }

  fn sort_rows(&mut self, col: TableCol, sort: ColumnSort) {
    let cmp = |a: &Employee, b: &Employee| -> Ordering {
      match col {
        TableCol::Id => a.id.cmp(&b.id),
        TableCol::Name => a.name.cmp(&b.name),
        TableCol::Team => a.team.cmp(b.team),
        TableCol::Title => a.title.cmp(b.title),
        TableCol::Score => a.score.cmp(&b.score),
        TableCol::Status => a.active.cmp(&b.active),
        TableCol::Actions => a.id.cmp(&b.id),
      }
    };

    self.rows.sort_by(|a, b| match sort {
      ColumnSort::Ascending => cmp(a, b),
      ColumnSort::Descending => cmp(b, a),
      ColumnSort::Default => a.id.cmp(&b.id),
    });
  }
}

impl TableDelegate for TableDemoDelegate {
  fn columns_count(&self, _cx: &App) -> usize {
    self.columns.len()
  }

  fn rows_count(&self, _cx: &App) -> usize {
    self.rows.len()
  }

  fn column(&self, col_ix: usize, _cx: &App) -> Column {
    let col = self.columns[col_ix];
    let mut column = Column::new(col.key(), col.label()).width(col.default_width());

    if col.sortable() {
      column = column.sortable();
    }
    if !col.selectable() {
      column = column.selectable(false);
    }
    if let Some(fixed) = col.fixed() {
      column = column.fixed(fixed);
    }
    if matches!(col, TableCol::Score | TableCol::Id) {
      column = column.text_right();
    }
    if matches!(col, TableCol::Actions) {
      column = column.text_center().resizable(false).movable(false).p_0();
    }

    column
  }

  fn perform_sort(
    &mut self, col_ix: usize, sort: ColumnSort, _window: &mut Window,
    _cx: &mut Context<TableState<Self>>,
  ) {
    if let Some(col) = self.columns.get(col_ix).copied() {
      self.sort_rows(col, sort);
    }
  }

  fn move_column(
    &mut self, col_ix: usize, to_ix: usize, _window: &mut Window,
    _cx: &mut Context<TableState<Self>>,
  ) {
    if col_ix >= self.columns.len() || to_ix >= self.columns.len() {
      return;
    }

    let col = self.columns.remove(col_ix);
    self.columns.insert(to_ix, col);
  }

  fn context_menu(
    &mut self, row_ix: usize, menu: PopupMenu, _window: &mut Window,
    cx: &mut Context<TableState<Self>>,
  ) -> PopupMenu {
    let Some(row) = self.rows.get(row_ix) else {
      return menu;
    };

    let row_label = format!("{} ({})", row.name, row.team);
    let row_id = row.id;
    let table = cx.entity().clone();

    menu
      .item(PopupMenuItem::label(format!("Row {}", row_id)))
      .separator()
      .item(
        PopupMenuItem::new(format!("Select {}", row_label)).on_click(move |_, _, cx| {
          table.update(cx, |table, cx| {
            table.set_selected_row(row_ix, cx);
          });
        }),
      )
      .item(PopupMenuItem::new("Print Row").on_click(move |_, _, _| {
        println!("table example: row {} selected from context menu", row_id);
      }))
  }

  fn render_td(
    &mut self, row_ix: usize, col_ix: usize, _window: &mut Window,
    cx: &mut Context<TableState<Self>>,
  ) -> impl IntoElement {
    let Some(row) = self.rows.get(row_ix) else {
      return div().into_any_element();
    };

    let col = self.columns[col_ix];
    match col {
      TableCol::Id => h_flex()
        .w_full()
        .justify_end()
        .child(format!("{:03}", row.id))
        .into_any_element(),
      TableCol::Name => h_flex()
        .w_full()
        .items_center()
        .gap_2()
        .child(div().size_2().rounded_full().bg(if row.active {
          cx.theme().success
        } else {
          cx.theme().muted_foreground
        }))
        .child(row.name.clone())
        .into_any_element(),
      TableCol::Team => Tag::new().small().child(row.team).into_any_element(),
      TableCol::Title => div().child(row.title).into_any_element(),
      TableCol::Score => h_flex()
        .w_full()
        .justify_end()
        .child(format!("{}", row.score))
        .into_any_element(),
      TableCol::Status => {
        if row.active {
          Tag::success().small().child("Active").into_any_element()
        } else {
          Tag::warning().small().child("Paused").into_any_element()
        }
      }
      TableCol::Actions => h_flex()
        .w_full()
        .justify_end()
        .child(
          Button::new(("row-action", row.id))
            .small()
            .flat()
            .label("Inspect"),
        )
        .into_any_element(),
    }
  }

  fn cell_text(&self, row_ix: usize, col_ix: usize, _cx: &App) -> String {
    let Some(row) = self.rows.get(row_ix) else {
      return String::new();
    };

    match self.columns[col_ix] {
      TableCol::Id => row.id.to_string(),
      TableCol::Name => row.name.clone(),
      TableCol::Team => row.team.to_string(),
      TableCol::Title => row.title.to_string(),
      TableCol::Score => row.score.to_string(),
      TableCol::Status => {
        if row.active {
          "Active".to_string()
        } else {
          "Paused".to_string()
        }
      }
      TableCol::Actions => "Inspect".to_string(),
    }
  }
}

struct TableWindow {
  table_state: Entity<TableState<TableDemoDelegate>>,
  query_state: Entity<InputState>,
  last_event: String,
  dump_preview: String,
  cell_mode: bool,
  _subscriptions: Vec<Subscription>,
}

impl TableWindow {
  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let query_state = cx.new(|cx| InputState::new(cx).placeholder("Filter by name/team/title"));

    let table_state = cx.new(|cx| {
      TableState::new(TableDemoDelegate::new(), window, cx)
        .cell_selectable(false)
        .row_selectable(true)
        .col_selectable(true)
        .col_movable(true)
        .col_resizable(true)
        .sortable(true)
    });

    cx.new(|cx| {
      let subscriptions = vec![cx.subscribe(
        &table_state,
        |this: &mut Self, _, event: &TableEvent, cx| {
          this.last_event = match event {
            TableEvent::SelectRow(row) => format!("SelectRow({row})"),
            TableEvent::DoubleClickedRow(row) => format!("DoubleClickedRow({row})"),
            TableEvent::SelectColumn(col) => format!("SelectColumn({col})"),
            TableEvent::SelectCell(row, col) => format!("SelectCell({row}, {col})"),
            TableEvent::DoubleClickedCell(row, col) => {
              format!("DoubleClickedCell({row}, {col})")
            }
            TableEvent::RightClickedRow(row) => format!("RightClickedRow({row:?})"),
            TableEvent::RightClickedCell(row, col) => {
              format!("RightClickedCell({row}, {col})")
            }
            TableEvent::MoveColumn(from, to) => format!("MoveColumn({from} -> {to})"),
            TableEvent::ColumnWidthsChanged(widths) => {
              format!("ColumnWidthsChanged(len={})", widths.len())
            }
            TableEvent::ClearSelection => "ClearSelection".to_string(),
          };
          cx.notify();
        },
      )];

      Self {
        table_state: table_state.clone(),
        query_state,
        last_event: "Ready".to_string(),
        dump_preview: "".to_string(),
        cell_mode: false,
        _subscriptions: subscriptions,
      }
    })
  }
}

impl Render for TableWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();
    let selected_info = {
      let table = self.table_state.read(cx);
      if let Some((row, col)) = table.selected_cell() {
        format!("cell ({row}, {col})")
      } else if let Some(row) = table.selected_row() {
        format!("row {row}")
      } else if let Some(col) = table.selected_col() {
        format!("column {col}")
      } else {
        "none".to_string()
      }
    };

    window_border().child(
      v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(
          div()
            .text_xl()
            .font_semibold()
            .child("Woocraft Table Example"),
        )
        .child(
          h_flex()
            .gap_3()
            .child(
              Button::new("table-theme-light")
                .label("Light")
                .selected(!is_dark)
                .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
            )
            .child(
              Button::new("table-theme-dark")
                .label("Dark")
                .selected(is_dark)
                .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
            )
            .child(
              Button::new("table-toggle-cell-mode")
                .label(if self.cell_mode {
                  "Cell Selection: ON"
                } else {
                  "Cell Selection: OFF"
                })
                .when(self.cell_mode, |this| this.primary())
                .on_click(cx.listener(|this, _, _, cx| {
                  this.cell_mode = !this.cell_mode;
                  let cell_mode = this.cell_mode;
                  this.table_state.update(cx, |table, cx| {
                    table.cell_selectable = cell_mode;
                    table.row_selectable = true;
                    table.col_selectable = true;
                    table.clear_selection(cx);
                    cx.notify();
                  });
                  cx.notify();
                })),
            )
            .child(
              Button::new("table-dump")
                .label("Dump Preview")
                .on_click(cx.listener(|this, _, _, cx| {
                  let (headers, rows) = this.table_state.read(cx).dump(cx);
                  let mut lines = vec![headers.join(", ")];
                  lines.extend(
                    rows
                      .into_iter()
                      .take(6)
                      .map(|row| row.into_iter().collect::<Vec<_>>().join(", ")),
                  );
                  this.dump_preview = lines.join("\n");
                  cx.notify();
                })),
            ),
        )
        .child(
          h_flex()
            .gap_3()
            .child(Input::new(&self.query_state).cleanable(true).w(px(360.)))
            .child(
              Button::new("table-apply-filter")
                .label("Apply Filter")
                .on_click(cx.listener(|this, _, _, cx| {
                  let query = this.query_state.read(cx).value().trim().to_string();
                  this.table_state.update(cx, |table, cx| {
                    table.delegate_mut().set_query(query);
                    table.clear_selection(cx);
                    cx.notify();
                  });
                })),
            )
            .child(
              Button::new("table-clear-filter")
                .default()
                .label("Clear")
                .on_click(cx.listener(|this, _, window, cx| {
                  this
                    .query_state
                    .update(cx, |input, cx| input.set_value("", window, cx));
                  this.table_state.update(cx, |table, cx| {
                    table.delegate_mut().set_query(String::new());
                    table.clear_selection(cx);
                    cx.notify();
                  });
                })),
            ),
        )
        .child(
          div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
              "Selected: {} | Last Event: {}",
              selected_info, self.last_event
            )),
        )
        .child(
          div().flex_1().min_h(px(420.0)).child(
            Table::new(&self.table_state)
              .stripe(true)
              .bordered(true)
              .scrollbar_visible(true, true),
          ),
        )
        .when(!self.dump_preview.is_empty(), |this| {
          this
            .child(
              div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .font_medium()
                .child("Dump Preview (first 6 rows):"),
            )
            .child(
              div()
                .text_xs()
                .font_family("Reverier Mono")
                .bg(cx.theme().card)
                .rounded(cx.theme().radius_container)
                .border_1()
                .border_color(cx.theme().border)
                .p_3()
                .child(self.dump_preview.clone()),
            )
        }),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1200.), px(860.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |window, cx| TableWindow::view(window, cx),
      )
      .expect("open table demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Table Example");
      })
      .expect("update table demo window failed");
  });
}
