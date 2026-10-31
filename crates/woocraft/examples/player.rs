//! Audio/video player on top of the optional `media` feature.
//!
//! ```text
//! cargo run -p woocraft --features media --example player -- <file-or-url>
//! cargo run -p woocraft --features media --example player -- --audio <file-or-url>
//! ```

use gpui::{
  App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render,
  Styled as _, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px,
};
use woocraft::{
  ActiveTheme as _, Player, PlayerOptions, PlayerView, Theme, ThemeMode, TitleBar, h_flex, init,
  v_flex,
};

struct PlayerExample {
  view: Entity<PlayerView>,
  source: String,
  error: Option<String>,
}

impl PlayerExample {
  fn new(source: String, video: bool, cx: &mut Context<Self>) -> Self {
    let options = if video {
      PlayerOptions::default().hardware_decoding(Default::default())
    } else {
      PlayerOptions::audio_only()
    };
    let player = cx.new(|cx| Player::new(cx, options));
    let view = cx.new(|cx| PlayerView::new(&player, cx));
    let error = player.read(cx).state().error.clone();
    let this = Self {
      view,
      source: source.clone(),
      error: error.clone(),
    };
    if error.is_none() {
      let _ = player.update(cx, |player, _| player.load(&source));
    }
    this
  }
}

impl Render for PlayerExample {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme().clone();
    v_flex()
      .size_full()
      .bg(theme.background)
      .child(
        h_flex()
          .w_full()
          .items_center()
          .justify_between()
          .px_3()
          .py_2()
          .child(woocraft::Label::new(self.source.clone()))
          .when_some(self.error.clone(), |this, error| {
            this.child(woocraft::Label::new(error))
          }),
      )
      .child(div().flex_1().w_full().child(self.view.clone()))
  }
}

fn main() {
  let mut args = std::env::args().skip(1);
  let mut video = true;
  let mut source = None;
  for arg in args.by_ref() {
    match arg.as_str() {
      "--audio" => video = false,
      other => source = Some(other.to_string()),
    }
  }
  let source = source.unwrap_or_else(|| {
    eprintln!("usage: player [--audio] <file-or-url>");
    std::process::exit(2);
  });

  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(move |cx: &mut App| {
      init(cx);
      cx.activate(true);
      Theme::set_mode(ThemeMode::Dark, cx);

      let bounds = Bounds::centered(None, gpui::Size::new(px(960.0), px(640.0)), cx);
      let source = source.clone();
      cx.open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          titlebar: Some(TitleBar::title_bar_options()),
          ..Default::default()
        },
        move |_window, cx| {
          let source = source.clone();
          cx.new(|cx| PlayerExample::new(source, video, cx))
        },
      )
      .expect("open player example window failed");
    });
}
