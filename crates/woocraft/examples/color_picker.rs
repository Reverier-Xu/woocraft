use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Subscription, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ColorPicker, ColorPickerEvent, ColorPickerState, Selectable, Sizable,
  StyledExt, Theme, ThemeMode, TitleBar, v_flex, window_border,
};

struct ColorPickerWindow {
  color_state: Entity<ColorPickerState>,
  _color_subscription: Subscription,
}

impl ColorPickerWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    let color_state = cx.new(|_| ColorPickerState::new());

    cx.new(|cx| {
      let _color_subscription =
        cx.subscribe(&color_state, |_: &mut Self, _, _: &ColorPickerEvent, cx| {
          cx.notify();
        });

      Self {
        color_state,
        _color_subscription,
      }
    })
  }
}

impl Render for ColorPickerWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let value = self.color_state.read(cx).value();
    let is_dark = cx.theme().mode.is_dark();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Woocraft Color Picker Example"))
        .child(
          div()
            .v_flex()
            .p_6()
            .gap_4()
            .child(div().text_xl().font_semibold().child("OKLCH Color Picker"))
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Supports drag and manual hex input."),
            )
            .child(
              div()
                .h_flex()
                .gap_3()
                .child(
                  Button::new("theme-light")
                    .label("Light")
                    .selected(!is_dark)
                    .small()
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                )
                .child(
                  Button::new("theme-dark")
                    .label("Dark")
                    .selected(is_dark)
                    .small()
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                ),
            )
            .child(
              ColorPicker::new("example-color-picker", &self.color_state)
                .medium()
                .w(px(320.0)),
            )
            .child(div().text_sm().child(format!("HEX: {}", value.rgba_hex)))
            .child(div().text_sm().child(format!(
              "RGBA: r={:.3}, g={:.3}, b={:.3}, a={:.3}",
              value.rgba.r, value.rgba.g, value.rgba.b, value.rgba.a
            )))
            .child(div().text_sm().child(format!(
              "OKLCH: l={:.3}, c={:.3}, h={:.1}, a={:.3}",
              value.oklch.lightness, value.oklch.chroma, value.oklch.hue, value.oklch.alpha
            )))
            .child(div().text_sm().child(format!(
              "HSLA: h={:.1}, s={:.3}, l={:.3}, a={:.3}",
              value.hsla.hue, value.hsla.saturation, value.hsla.lightness, value.hsla.alpha
            ))),
        ),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(980.), px(680.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          titlebar: Some(TitleBar::title_bar_options()),
          #[cfg(target_os = "linux")]
          window_background: gpui::WindowBackgroundAppearance::Transparent,
          #[cfg(target_os = "linux")]
          window_decorations: Some(gpui::WindowDecorations::Client),
          ..Default::default()
        },
        ColorPickerWindow::view,
      )
      .expect("open color picker demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Color Picker Example");
      })
      .expect("update color picker demo window failed");
  });
}
