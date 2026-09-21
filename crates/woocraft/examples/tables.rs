//! tables gallery — the styled data table with a sortable column model and
//! row selection over a one-thousand-row virtualized body, with light/dark
//! switching and rem scaling.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example tables
//! ```

use std::rc::Rc;

use gpui::{
  App, AppContext, Bounds, Context, Entity, Global, InteractiveElement as _, IntoElement,
  ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement as _, Styled,
  Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Button, Column, ColumnSort, Table, TableEvent, TableState, Theme, ThemeMode,
  application, init, logging,
};

/// the current ui font size, adjustable from the gallery header.
struct UiScale(Pixels);

impl Global for UiScale {}

const MIN_REM: f32 = 12.0;
const MAX_REM: f32 = 28.0;
const REM_STEP: f32 = 2.0;

const ROWS: usize = 1_000;

struct TablesGallery {
  state: Entity<TableState>,
  files: Rc<Vec<(SharedString, u64)>>,
  selection: Option<usize>,
  sort: Option<(usize, ColumnSort)>,
  _subscription: gpui::Subscription,
}

impl TablesGallery {
  fn new(cx: &mut Context<Self>) -> Self {
    let mut files: Vec<_> = (0..ROWS)
      .map(|ix| {
        (
          SharedString::from(format!("artifact-{ix:04}.crate")),
          ((ix as u64 * 37) % 900) + 4,
        )
      })
      .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));

    let state = cx.new(|cx| {
      let mut state = TableState::new(cx);
      state.set_columns(
        vec![
          Column::new("name", "name").sortable(true),
          Column::new("size", "size").sortable(true).fixed(rems(8.)),
        ],
        cx,
      );
      state
    });
    let subscription = cx.subscribe(&state, Self::on_table_event);

    Self {
      state,
      files: Rc::new(files),
      selection: None,
      sort: None,
      _subscription: subscription,
    }
  }

  fn on_table_event(&mut self, _: Entity<TableState>, event: &TableEvent, cx: &mut Context<Self>) {
    match event {
      TableEvent::Select(row) => self.selection = *row,
      TableEvent::Sort(column, sort) => {
        self.sort = Some((*column, *sort));
        let mut files = self.files.as_ref().clone();
        files.sort_by(|a, b| {
          let ordering = match column {
            0 => a.0.cmp(&b.0),
            _ => a.1.cmp(&b.1),
          };
          if *sort == ColumnSort::Descending {
            ordering.reverse()
          } else {
            ordering
          }
        });
        self.files = Rc::new(files);
        self.state.update(cx, |state, _| {
          state.scroll_to_row(0, gpui::ScrollStrategy::Top);
        });
      }
    }
    cx.notify();
  }

  fn reset_sort(&mut self, cx: &mut Context<Self>) {
    self.sort = None;
    self.state.update(cx, |state, cx| state.set_sort(None, cx));
    cx.notify();
  }
}

impl Render for TablesGallery {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let scale = cx.global::<UiScale>().0;
    let files = self.files.clone();
    let muted_foreground = theme.muted_foreground;
    let sort_label = match self.sort {
      Some((column, sort)) => format!(
        "sorted by {} {}",
        if column == 0 { "name" } else { "size" },
        match sort {
          ColumnSort::Ascending => "ascending",
          ColumnSort::Descending => "descending",
        }
      ),
      None => "unsorted".to_string(),
    };
    let selection_label = match self.selection {
      Some(row) => format!("selected row {row}"),
      None => "nothing selected".to_string(),
    };
    let reset = cx.entity();

    div()
      .id("tables-gallery")
      .size_full()
      .overflow_y_scroll()
      .font_family(theme.font_family.clone())
      .bg(theme.background)
      .text_color(theme.foreground)
      .flex()
      .flex_col()
      .gap_6()
      .p_6()
      .child(header(theme, scale))
      .child(
        div()
          .flex()
          .flex_col()
          .gap_2()
          .child(
            div()
              .text_color(theme.muted_foreground)
              .child("files · 1,000 rows"),
          )
          .child(
            div()
              .flex()
              .items_center()
              .gap_3()
              .child(
                div()
                  .text_color(muted_foreground)
                  .child(SharedString::from(selection_label)),
              )
              .child(
                div()
                  .text_color(muted_foreground)
                  .child(SharedString::from(sort_label)),
              )
              .child(div().flex_1())
              .child(
                Button::new("reset-sort")
                  .label("reset sort")
                  .on_click(move |_, _, cx| {
                    reset.update(cx, TablesGallery::reset_sort);
                  }),
              ),
          )
          .child(
            div()
              .h(rems(20.))
              .rounded_lg()
              .border_1()
              .border_color(theme.border)
              .bg(theme.card)
              .p_2()
              .child(
                Table::new("files-table", &self.state)
                  .rows(self.files.len())
                  .render_cell(move |row, column, _, cx| {
                    let (name, kilobytes) = &files[row];
                    match column {
                      0 => div().child(name.clone()).into_any_element(),
                      _ => div()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(format!("{kilobytes} kb")))
                        .into_any_element(),
                    }
                  }),
              ),
          ),
      )
  }
}

fn header(theme: &Theme, scale: Pixels) -> impl IntoElement {
  let dark = theme.mode.is_dark();

  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_3()
    .child(div().child("woocraft tables"))
    .child(
      div()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(
          format!("{:?}", theme.mode).to_lowercase(),
        )),
    )
    .child(
      div()
        .flex()
        .items_center()
        .gap_2()
        .child(
          div()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(format!("rem {scale}"))),
        )
        .child(scale_button("scale-down", "\u{2212}", -REM_STEP))
        .child(scale_button("scale-up", "+", REM_STEP)),
    )
    .child(div().flex_1())
    .child(mode_button(theme, ThemeMode::Light, !dark))
    .child(mode_button(theme, ThemeMode::Dark, dark))
}

fn scale_button(id: &'static str, label: &'static str, delta: f32) -> Button {
  Button::new(id).label(label).on_click(move |_, window, cx| {
    let next = (cx.global::<UiScale>().0 + px(delta))
      .max(px(MIN_REM))
      .min(px(MAX_REM));
    cx.set_global(UiScale(next));
    window.set_rem_size(next);
    cx.refresh_windows();
  })
}

fn mode_button(theme: &Theme, mode: ThemeMode, active: bool) -> impl IntoElement {
  let label = format!("{:?}", mode).to_lowercase();

  div()
    .id(SharedString::from(format!("mode-{label}")))
    .cursor_pointer()
    .rounded_md()
    .border_1()
    .border_color(if active { theme.primary } else { theme.border })
    .when(active, |this| {
      this.bg(theme.primary).text_color(theme.primary_foreground)
    })
    .px_3()
    .py_1()
    .on_click(move |_: &gpui::ClickEvent, _, cx| Theme::set_mode(mode, cx))
    .child(SharedString::from(label))
}

fn main() {
  let _ = logging::init();

  application().with_assets(Assets).run(|cx: &mut App| {
    if let Err(err) = init(cx) {
      eprintln!("woocraft init failed: {err}");
      return;
    }
    cx.set_global(UiScale(px(16.)));

    let bounds = Bounds {
      origin: Point {
        x: px(120.),
        y: px(120.),
      },
      size: size(px(920.), px(720.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_, cx| cx.new(TablesGallery::new))
        .expect("failed to open the tables gallery window");
    })
    .detach();
  });
}
