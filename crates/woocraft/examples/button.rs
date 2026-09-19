//! button gallery — every variant, state, and icon treatment against the
//! active theme, with a light/dark/system switch to review both palettes.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example button
//! ```

use gpui::{
  App, AppContext, Bounds, ClickEvent, Context, Global, InteractiveElement, IntoElement,
  ParentElement, Pixels, Point, Render, SharedString, StatefulInteractiveElement, Styled, Window,
  WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Button, ButtonVariants, Icon, IconName, Theme, ThemeMode, application, init,
  logging,
};

/// the current ui font size, adjustable from the gallery header.
struct UiScale(Pixels);

impl Global for UiScale {}

const MIN_REM: f32 = 12.0;
const MAX_REM: f32 = 28.0;
const REM_STEP: f32 = 2.0;

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
      size: size(px(980.), px(760.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_window, cx| cx.new(|_| ButtonGallery))
        .expect("failed to open the button gallery window");
    })
    .detach();
  });
}

struct ButtonGallery;

impl Render for ButtonGallery {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let scale = cx.global::<UiScale>().0;

    div()
      .id("button-gallery")
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
      .child(section(
        theme,
        "solid variants",
        vec![
          labeled("primary", Button::new("primary").label("primary").primary()),
          labeled("success", Button::new("success").label("success").success()),
          labeled("warning", Button::new("warning").label("warning").warning()),
          labeled("info", Button::new("info").label("info").info()),
          labeled("danger", Button::new("danger").label("danger").danger()),
        ],
      ))
      .child(section(
        theme,
        "neutral variants",
        vec![
          labeled("default", Button::new("default").label("default").default()),
          labeled("flat", Button::new("flat").label("flat").flat()),
          labeled("link", Button::new("link").label("link").link()),
        ],
      ))
      .child(section(
        theme,
        "outline",
        vec![
          labeled(
            "primary",
            Button::new("outline-primary")
              .label("primary")
              .primary()
              .outline(true),
          ),
          labeled(
            "danger",
            Button::new("outline-danger")
              .label("danger")
              .danger()
              .outline(true),
          ),
          labeled(
            "info",
            Button::new("outline-info")
              .label("info")
              .info()
              .outline(true),
          ),
        ],
      ))
      .child(section(
        theme,
        "icons",
        vec![
          labeled(
            "icon only",
            Button::new("icon-only")
              .icon(Icon::new(IconName::Copy))
              .default(),
          ),
          labeled(
            "icon + label",
            Button::new("icon-label")
              .icon(Icon::new(IconName::AddCircle))
              .label("add")
              .primary(),
          ),
          labeled(
            "custom color icon",
            Button::new("icon-color")
              .icon(Icon::new(IconName::Delete).colorized(false))
              .label("delete")
              .danger(),
          ),
        ],
      ))
      .child(section(
        theme,
        "states",
        vec![
          labeled(
            "disabled",
            Button::new("disabled").label("disabled").disabled(true),
          ),
          labeled(
            "loading",
            Button::new("loading")
              .label("saving")
              .loading(true)
              .primary(),
          ),
          labeled(
            "selected",
            Button::new("selected")
              .label("selected")
              .selected(true)
              .default(),
          ),
        ],
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
    .child(div().child("woocraft button"))
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
    .child(
      div()
        .id("sync-system")
        .cursor_pointer()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .px_3()
        .py_1()
        .text_color(theme.foreground)
        .on_click(|_: &ClickEvent, _, cx| Theme::sync_system_appearance(cx))
        .child("sync system"),
    )
}

fn scale_button(id: &'static str, label: &'static str, delta: f32) -> Button {
  Button::new(id)
    .label(label)
    .default()
    .on_click(move |_, window, cx| {
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
    .on_click(move |_: &ClickEvent, _, cx| Theme::set_mode(mode, cx))
    .child(SharedString::from(label))
}

fn section(
  theme: &Theme, title: &'static str, buttons: Vec<(&'static str, Button)>,
) -> impl IntoElement {
  let cells = buttons
    .into_iter()
    .map(|(name, button)| {
      div()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .w(rems(9.))
        .child(button)
        .child(div().text_color(theme.muted_foreground).child(name))
    })
    .collect::<Vec<_>>();

  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(
      div()
        .text_color(theme.muted_foreground)
        .child(title.to_uppercase()),
    )
    .child(
      div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_4()
        .bg(theme.card)
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .p_4()
        .children(cells),
    )
}

fn labeled(name: &'static str, button: Button) -> (&'static str, Button) {
  (name, button)
}
