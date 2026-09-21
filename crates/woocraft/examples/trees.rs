//! trees gallery — the styled tree face over the base tree behavior model:
//! expand/collapse folders, keyboard navigation, selection, and reveal, with
//! light/dark switching and rem scaling.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example trees
//! ```

use gpui::{
  App, AppContext, Bounds, Context, Entity, Global, InteractiveElement as _, IntoElement,
  ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement as _, Styled,
  Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Button, Icon, IconName, Kbd, Theme, ThemeMode, Tree, TreeItem, TreeState,
  application, init, logging,
};

/// the current ui font size, adjustable from the gallery header.
struct UiScale(Pixels);

impl Global for UiScale {}

const MIN_REM: f32 = 12.0;
const MAX_REM: f32 = 28.0;
const REM_STEP: f32 = 2.0;

struct TreesGallery {
  state: Entity<TreeState>,
}

fn demo_tree() -> Vec<TreeItem> {
  vec![
    TreeItem::new("crates", "crates")
      .expanded(true)
      .child(
        TreeItem::new("woocraft", "woocraft")
          .expanded(true)
          .child(TreeItem::new("src", "src").child(TreeItem::new("lib", "lib.rs"))),
      )
      .child(
        TreeItem::new("tray", "woocraft-tray").child(TreeItem::new("platform", "platform.rs")),
      ),
    TreeItem::new("docs", "docs").child(TreeItem::new("plan", "plan.md")),
    TreeItem::new("license", "LICENSE"),
    TreeItem::new("readme", "README.md"),
  ]
}

impl TreesGallery {
  fn new(cx: &mut Context<Self>) -> Self {
    let state = cx.new(|cx| TreeState::new(cx).items(demo_tree()));
    Self { state }
  }
}

impl Render for TreesGallery {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let scale = cx.global::<UiScale>().0;
    let reveal_target = cx.entity();
    let selection = self
      .state
      .read(cx)
      .selected_entry()
      .map(|entry| entry.item().label.clone());

    div()
      .id("trees-gallery")
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
          .min_h_0()
          .flex_1()
          .child(
            div()
              .flex()
              .items_center()
              .gap_3()
              .child(
                div()
                  .text_color(theme.muted_foreground)
                  .child(SharedString::from(match &selection {
                    Some(label) => format!("selected {label}"),
                    None => "nothing selected".to_string(),
                  })),
              )
              .child(div().flex_1())
              .child(Button::new("reveal-plan").label("reveal plan.md").on_click(
                move |_, _, cx| {
                  reveal_target.update(cx, |gallery, cx| {
                    let id = SharedString::from("plan");
                    gallery.state.update(cx, |state, cx| {
                      state.reveal_item(&id, gpui::ScrollStrategy::Top, cx);
                    });
                  });
                },
              ))
              .child(Kbd::new(gpui::Keystroke::parse("up").unwrap()).into_any_element())
              .child(Kbd::new(gpui::Keystroke::parse("down").unwrap()).into_any_element())
              .child(Icon::new(IconName::ChevronRight)),
          )
          .child(
            div()
              .h(rems(20.))
              .rounded_lg()
              .border_1()
              .border_color(theme.border)
              .bg(theme.card)
              .p_2()
              .overflow_hidden()
              .child(Tree::new("files-tree", &self.state)),
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
    .child(div().child("woocraft trees"))
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
      size: size(px(720.), px(720.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_, cx| cx.new(TreesGallery::new))
        .expect("failed to open the trees gallery window");
    })
    .detach();
  });
}
