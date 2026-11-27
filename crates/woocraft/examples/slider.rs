//! Slider features: origin, ticks, immediate vs deferred dragging, and the
//! thumb/rail preview a media player's seek bar wants.
//!
//! ```text
//! cargo run -p woocraft --example slider
//! ```

use gpui::{
  AnyElement, App, AppContext as _, Bounds, Context, Entity, IntoElement, ParentElement as _,
  Render, Styled as _, Window, WindowBounds, WindowOptions, div,
};
use woocraft::{
  ActiveTheme as _, Label, Sizable as _, Size, Slider, SliderEvent, SliderOrigin, SliderPreview,
  SliderPreviewTrigger, SliderState, StyleSized, Theme, ThemeMode, TitleBar, init, v_flex,
  window_border,
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

/// A thumb-scoped preview: the slider only names a value, so the content can be
/// as small and as textual as this.
fn value_preview(preview: SliderPreview, cx: &App) -> AnyElement {
  let theme = cx.theme();
  div()
    .px(Size::Small.container_gap())
    .py(Size::Small.container_gap())
    .rounded(theme.radius)
    .border_1()
    .border_color(theme.border)
    .bg(theme.popover)
    .text_color(theme.popover_foreground)
    .text_size(Size::Small.text_size())
    .child(format!("{:.0}%", preview.value))
    .into_any_element()
}

/// A stand-in for a media player's frame preview. Note that it is not a text
/// tooltip: the card carries a picture-sized block whose colour follows the
/// value, and a drag marks itself as such.
fn frame_preview(preview: SliderPreview, cx: &App) -> AnyElement {
  let theme = cx.theme();
  let frame = (preview.value / 100.0).clamp(0.0, 1.0);
  v_flex()
    .w(gpui::px(112.))
    .p_2()
    .gap_1()
    .rounded(theme.radius_container)
    .border_1()
    .border_color(theme.border)
    .bg(theme.popover)
    .text_color(theme.popover_foreground)
    .child(
      div()
        .w_full()
        .h(gpui::px(56.))
        .rounded(theme.radius)
        .bg(theme.primary.opacity(0.15 + 0.6 * frame))
        .flex()
        .items_center()
        .justify_center()
        .text_size(Size::Small.text_size())
        .text_color(theme.primary_foreground)
        .child(if preview.dragging {
          "dragging"
        } else {
          "hovering"
        }),
    )
    .child(
      div()
        .text_size(Size::Small.text_size())
        .child(format!("{:.0}s", preview.value * 0.6)),
    )
    .into_any_element()
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

    window_border().child(
      v_flex()
        .size_full()
        .text_color(theme.foreground)
        .child(TitleBar::new().title("Slider"))
        .child(
          v_flex()
            .w_full()
            .container_padding(size)
            .container_gap_y(size)
            .child(Label::new(
              "Volume — hover the thumb for a text preview, drag for the live value",
            ))
            .child(
              div().w_full().child(
                Slider::new("demo-volume", &self.volume)
                  .with_size(size)
                  .preview(|preview, _, cx| value_preview(preview, cx))
                  .preview_trigger(SliderPreviewTrigger::Thumb),
              ),
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
              "Scrub — immediate off: hover the rail for a frame preview, drag previews what it will commit",
            ))
            .child(
              div().w_full().child(
                Slider::new("demo-scrub", &self.scrub)
                  .with_size(size)
                  .preview(|preview, _, cx| frame_preview(preview, cx)),
              ),
            )
            .child(Label::new(format!("last event: {}", self.seen))),
        ),
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

      let bounds = Bounds::centered(None, gpui::Size::new(gpui::px(720.0), gpui::px(520.0)), cx);
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
