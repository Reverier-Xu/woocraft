//! window chrome + system tray demo.
//!
//! runs a frameless window on linux (client-side decorations: shadow, resize
//! edges, draggable title bar with minimize/maximize/close), plus a system
//! tray icon with a context menu wired to the demo actions.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example window_tray --features tray
//! ```

use std::time::Duration;

use gpui::{
  App, AppContext, Bounds, Context, Global, InteractiveElement, IntoElement, ParentElement, Point,
  Render, SharedString, Styled, Window, WindowBounds, WindowOptions, div, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Button, ButtonVariants, Theme, ThemeMode, TitleBar, Tray, TrayAppContext,
  TrayEvent, TrayMenuItem, WindowBorder, application, init, logging, tray_events, window_paddings,
};

/// the last tray event, rendered inside the window for feedback.
#[derive(Default)]
struct LastTrayEvent(Option<String>);

impl Global for LastTrayEvent {}

fn main() {
  let _ = logging::init();

  application().with_assets(Assets).run(|cx: &mut App| {
    if let Err(err) = init(cx) {
      eprintln!("woocraft init failed: {err}");
      return;
    }
    cx.set_global(LastTrayEvent::default());

    let _ = cx.set_tray(
      Tray::new()
        .tooltip("woocraft tray demo")
        .icon_bytes(include_bytes!("assets/tray.png").as_slice())
        .menu(vec![
          TrayMenuItem::action("toggle-theme", "toggle theme"),
          TrayMenuItem::separator(),
          TrayMenuItem::action("quit", "quit"),
        ]),
    );

    if let Some(events) = tray_events(cx) {
      cx.spawn(async move |cx| {
        loop {
          while let Ok(event) = events.try_recv() {
            cx.update(|cx| handle_tray_event(event, cx));
          }
          cx.background_executor()
            .timer(Duration::from_millis(150))
            .await;
        }
      })
      .detach();
    }

    let bounds = Bounds {
      origin: Point {
        x: px(140.),
        y: px(140.),
      },
      size: size(px(900.), px(640.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      titlebar: Some(TitleBar::title_bar_options()),
      window_decorations: Some(gpui::WindowDecorations::Client),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_window, cx| cx.new(|_| WindowTrayDemo))
        .expect("failed to open the window tray window");
    })
    .detach();
  });
}

fn handle_tray_event(event: TrayEvent, cx: &mut App) {
  let description = match &event {
    TrayEvent::Click { button, position } => {
      format!("click {button:?} at {position:?}")
    }
    TrayEvent::DoubleClick => "double click".to_string(),
    TrayEvent::MenuClicked { id } => {
      match id.as_str() {
        "quit" => cx.quit(),
        "toggle-theme" => {
          let next = if cx.theme().mode.is_dark() {
            ThemeMode::Light
          } else {
            ThemeMode::Dark
          };
          Theme::set_mode(next, cx);
        }
        _ => {}
      }
      format!("menu: {id}")
    }
  };

  if cx.has_global::<LastTrayEvent>() {
    cx.set_global(LastTrayEvent(Some(description)));
  }
  cx.refresh_windows();
}

struct WindowTrayDemo;

impl Render for WindowTrayDemo {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let paddings = window_paddings(window);
    let last_event = cx.global::<LastTrayEvent>().0.clone().unwrap_or_default();

    // the outer padding reserves room for the client-side drop shadow; the
    // bordered surface lives inside it.
    div()
      .id("window-root")
      .size_full()
      .pt(paddings.top)
      .pb(paddings.bottom)
      .pl(paddings.left)
      .pr(paddings.right)
      .child(
        WindowBorder::new().child(
          div()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .child(
              TitleBar::new()
                .title("woocraft window + tray")
                .theme_button(true),
            )
            .child(
              div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .p(rems(2.))
                .child(
                  div()
                    .text_color(theme.muted_foreground)
                    .child("this window is drawn by woocraft: shadow, resize edges, title bar drag, and window controls."),
                )
                .child(
                  div()
                    .text_color(theme.foreground)
                    .child(SharedString::from(format!(
                      "last tray event: {last_event}"
                    ))),
                )
                .child(
                  div().flex().gap_2().child(
                    Button::new("demo-quit")
                      .label("quit")
                      .flat()
                      .on_click(|_, _, cx| {
                        cx.quit();
                      }),
                  ),
                ),
            ),
        ),
      )
  }
}
