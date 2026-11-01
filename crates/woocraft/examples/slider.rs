//! Slider features: origin, ticks and immediate vs deferred dragging.
//!
//! ```text
//! cargo run -p woocraft --example slider
//! ```

use gpui::{
  App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _, Render,
  Styled as _, Window, WindowBounds, WindowOptions, div,
};
use woocraft::{
  ActiveTheme as _, Label, Sizable as _, Size, Slider, SliderEvent, SliderOrigin, SliderState,
  StyleSized, Theme, ThemeMode, TitleBar, init, v_flex,
};

struct SliderDemo {
  volume: Entity<SliderState>,
  eq_bass: Entity<SliderState>,
  eq_mid: Entity<SliderState>,
  eq_treble: Entity<SliderState>,
  scrub: Entity<SliderState>,
  seen: String,
  size: Size,
  subscriptions: Vec<gpui::Subscription>,
}

impl SliderDemo {
  fn new(cx: &mut Context<Self>) -> Self {
    let volume = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(65.0)
    });
    let eq_band = |cx: &mut Context<SliderState>| {
      let _ = cx;
      SliderState::new()
        .min(-12.0)
        .max(12.0)
        .step(1.0)
        .default_value(-12.0)
        .with_origin(SliderOrigin::Middle)
        .with_ticks(true)
    };
    let eq_bass = cx.new(eq_band);
    let eq_mid = cx.new(|cx| eq_band(cx).with_major_tick_ratio(2).default_value(12.0));
    let eq_treble = cx.new(|cx| eq_band(cx).default_value(0.0));
    let scrub = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(30.0)
        .with_immediate(false)
    });

    let seen = String::new();
    let mut demo = Self {
      volume,
      eq_bass,
      eq_mid,
      eq_treble,
      scrub,
      seen,
      size: Size::Medium,
      subscriptions: Vec::new(),
    };
    for slider in [
      demo.volume.clone(),
      demo.eq_bass.clone(),
      demo.eq_mid.clone(),
      demo.eq_treble.clone(),
      demo.scrub.clone(),
    ] {
      let subscription = cx.subscribe(&slider, |this: &mut Self, _, event: &SliderEvent, cx| {
        this.note(event, cx)
      });
      demo.subscriptions.push(subscription);
    }
    demo
  }

  fn note(&mut self, event: &SliderEvent, cx: &mut Context<Self>) {
    self.seen = match event {
      SliderEvent::Change(value) => format!("Change {:?}", value.start()),
      SliderEvent::Preview(value) => format!("Preview {:?}", value.start()),
      SliderEvent::Commit(value) => format!("Commit {:?}", value.start()),
    };
    cx.notify();
  }
}

impl Render for SliderDemo {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let size = self.size;
    let value_of = |state: &Entity<SliderState>, cx: &App| state.read(cx).value().start();

    v_flex()
      .size_full()
      .bg(theme.background)
      .text_color(theme.foreground)
      .child(TitleBar::new().title("Slider"))
      .child(
        v_flex()
          .w_full()
          .container_padding(size)
          .container_gap_y(size)
          .child(Label::new("Volume — default origin, no ticks"))
          .child(
            div()
              .w_full()
              .child(Slider::new("demo-volume", &self.volume).with_size(size)),
          )
          .child(Label::new(format!(
            "Bass — origin middle, ticks, major every 5  ({:+.0} dB)",
            value_of(&self.eq_bass, cx)
          )))
          .child(
            div()
              .w_full()
              .child(Slider::new("demo-bass", &self.eq_bass).with_size(size)),
          )
          .child(Label::new(format!(
            "Mid — major every 2  ({:+.0} dB)",
            value_of(&self.eq_mid, cx)
          )))
          .child(
            div()
              .w_full()
              .child(Slider::new("demo-mid", &self.eq_mid).with_size(size)),
          )
          .child(Label::new(format!(
            "Treble — origin middle  ({:+.0} dB)",
            value_of(&self.eq_treble, cx)
          )))
          .child(
            div()
              .w_full()
              .child(Slider::new("demo-treble", &self.eq_treble).with_size(size)),
          )
          .child(Label::new(
            "Scrub — immediate off: drag previews, release commits",
          ))
          .child(
            div()
              .w_full()
              .child(Slider::new("demo-scrub", &self.scrub).with_size(size)),
          )
          .child(Label::new(format!("last event: {}", self.seen))),
      )
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      init(cx);
      cx.activate(true);
      Theme::set_mode(ThemeMode::Dark, cx);

      let bounds = Bounds::centered(None, gpui::Size::new(gpui::px(720.0), gpui::px(460.0)), cx);
      let window = cx
        .open_window(
          WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..TitleBar::window_options()
          },
          |_window, cx| cx.new(SliderDemo::new),
        )
        .expect("open slider example window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Slider Example");
        })
        .ok();
    });
}
