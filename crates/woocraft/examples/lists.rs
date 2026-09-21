//! lists gallery — the styled list faces plus both virtualization paths:
//! gpui's `uniform_list` for ten thousand equal-height rows and gpui-base's
//! `virtual_list` for variable-height content, all over the active theme
//! with light/dark switching and rem scaling.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example lists
//! ```

use gpui::{
  App, AppContext, Bounds, Context, Global, InteractiveElement as _, IntoElement, ParentElement,
  Pixels, Point, Render, SharedString, Size, StatefulInteractiveElement as _, Styled, Window,
  WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Button, Disableable, Icon, IconName, Kbd, List, ListItem, Selectable, Theme,
  ThemeMode, application, base::v_virtual_list, init, logging,
};

/// the current ui font size, adjustable from the gallery header.
struct UiScale(Pixels);

impl Global for UiScale {}

const MIN_REM: f32 = 12.0;
const MAX_REM: f32 = 28.0;
const REM_STEP: f32 = 2.0;

const LANGUAGES: [&str; 6] = ["rust", "go", "typescript", "haskell", "zig", "python"];
const VIRTUAL_ROWS: usize = 10_000;
const FEED_ITEMS: usize = 2_000;

/// one-line heights cycle 2rem / 3rem / 4rem to exercise variable measuring.
fn feed_row_height(ix: usize, rem: Pixels) -> Pixels {
  rems(2. + (ix % 3) as f32).to_pixels(rem)
}

struct ListsGallery {
  language: Option<usize>,
  row: Option<usize>,
  feed: Option<usize>,
  uniform_handle: gpui::UniformListScrollHandle,
  feed_sizes: std::rc::Rc<Vec<Size<Pixels>>>,
}

impl Default for ListsGallery {
  fn default() -> Self {
    let rem = px(16.);
    Self {
      language: Some(0),
      row: None,
      feed: None,
      uniform_handle: gpui::UniformListScrollHandle::new(),
      feed_sizes: std::rc::Rc::new(
        (0..FEED_ITEMS)
          .map(|ix| Size {
            width: px(0.),
            height: feed_row_height(ix, rem),
          })
          .collect(),
      ),
    }
  }
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
      size: size(px(920.), px(860.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_window, cx| cx.new(|_| ListsGallery::default()))
        .expect("failed to open the lists gallery window");
    })
    .detach();
  });
}

impl Render for ListsGallery {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    // the uniform section needs `&mut Context` for the processor, so build it
    // before the immutable theme borrow is taken for the remaining sections.
    let uniform = self.uniform_section(cx);
    let theme = cx.theme();
    let scale = cx.global::<UiScale>().0;

    div()
      .id("lists-gallery")
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
      .child(section(theme, "list", self.plain_list(cx)))
      .child(section(theme, "uniform list · 10,000 rows", uniform))
      .child(section(
        theme,
        "virtual list · variable heights",
        self.feed_section(cx, theme),
      ))
  }
}

impl ListsGallery {
  fn plain_list(&self, cx: &Context<Self>) -> impl IntoElement {
    let gallery = cx.entity();
    let mut list = List::new("languages");
    for (ix, language) in LANGUAGES.iter().enumerate() {
      let item = gallery.clone();
      list = list.child(
        ListItem::new(SharedString::from(format!("language-{ix}")))
          .icon(Icon::new(IconName::ChevronRight))
          .selected(self.language == Some(ix))
          .disabled(ix == LANGUAGES.len() - 1)
          .trailing(Kbd::new(gpui::Keystroke::parse("cmd-k").unwrap()))
          .on_click(move |_, _, cx| {
            item.update(cx, |gallery, cx| {
              gallery.language = (gallery.language != Some(ix)).then_some(ix);
              cx.notify();
            });
          })
          .child(SharedString::from(*language)),
      );
    }
    list
  }

  /// `use<>` keeps the returned element from capturing the `&mut Context`
  /// lifetime, so the caller can keep using the context afterwards.
  fn uniform_section(&self, cx: &mut Context<Self>) -> impl IntoElement + use<> {
    let muted_foreground = cx.theme().muted_foreground;
    let rows = cx.entity();
    let jumper = cx.entity();
    let selected = self.row;
    let uniform_handle = self.uniform_handle.clone();

    div()
      .flex()
      .flex_col()
      .gap_2()
      .h(rems(14.))
      .child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .child(
            div()
              .text_color(muted_foreground)
              .child(SharedString::from(match selected {
                Some(row) => format!("selected row {row}"),
                None => "nothing selected".to_string(),
              })),
          )
          .child(div().flex_1())
          .child(
            Button::new("jump-bottom")
              .label("jump to the last row")
              .on_click(move |_, _, cx| {
                jumper.update(cx, |gallery, _| {
                  gallery
                    .uniform_handle
                    .scroll_to_item(VIRTUAL_ROWS - 1, gpui::ScrollStrategy::Bottom);
                });
              }),
          ),
      )
      .child(
        gpui::uniform_list(
          "virtual-rows",
          VIRTUAL_ROWS,
          cx.processor(
            move |gallery, visible_range: std::ops::Range<usize>, _, _cx| {
              visible_range
                .map(|ix| {
                  let select = rows.clone();
                  ListItem::new(ix)
                    .selected(gallery.row == Some(ix))
                    .on_click(move |_, _, cx| {
                      select.update(cx, |gallery, cx| {
                        gallery.row = (gallery.row != Some(ix)).then_some(ix);
                        cx.notify();
                      });
                    })
                    .child(SharedString::from(format!("row {ix}")))
                })
                .collect()
            },
          ),
        )
        .track_scroll(&uniform_handle)
        .flex_1(),
      )
  }

  fn feed_section(&self, cx: &Context<Self>, theme: &Theme) -> impl IntoElement {
    let rows = cx.entity();
    let feed_sizes = self.feed_sizes.clone();
    let muted_foreground = theme.muted_foreground;

    div()
      .flex()
      .flex_col()
      .gap_2()
      .h(rems(14.))
      .child(
        div()
          .text_color(theme.muted_foreground)
          .child(SharedString::from(match self.feed {
            Some(feed) => format!("selected item {feed}"),
            None => "nothing selected".to_string(),
          })),
      )
      .child(v_virtual_list(
        cx.entity(),
        "feed",
        feed_sizes,
        move |gallery, visible_range, _, _| {
          visible_range
            .map(|ix| {
              let select = rows.clone();
              ListItem::new(ix)
                .selected(gallery.feed == Some(ix))
                .on_click(move |_, _, cx| {
                  select.update(cx, |gallery, cx| {
                    gallery.feed = (gallery.feed != Some(ix)).then_some(ix);
                    cx.notify();
                  });
                })
                .child(SharedString::from(format!("item {ix}")))
                .child(
                  div()
                    .text_color(muted_foreground)
                    .child("a variable-height entry measured by the virtual list"),
                )
                .h(feed_row_height(ix, px(16.)))
            })
            .collect()
        },
      ))
  }
}

fn header(theme: &Theme, scale: Pixels) -> impl IntoElement {
  let dark = theme.mode.is_dark();

  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_3()
    .child(div().child("woocraft lists"))
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

fn section(theme: &Theme, title: &str, content: impl IntoElement) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(
      div()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(title.to_uppercase())),
    )
    .child(
      div()
        .min_h_0()
        .flex()
        .flex_col()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.card)
        .p_2()
        .child(content),
    )
}
