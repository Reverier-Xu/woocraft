use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Disableable, Icon, IconName, Selectable, Sizable as _,
  StyledExt, Theme, ThemeMode, h_flex, init, v_flex,
};

#[derive(Default)]
struct ButtonWindow {
  clicks: usize,
}

impl ButtonWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self::default())
  }
}

impl Render for ButtonWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();

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
          .child("Woocraft Button Preview"),
      )
      .child(div().child("Theme"))
      .child(
        h_flex()
          .gap_3()
          .child(
            Button::new("btn-theme-light")
              .label("Light")
              .selected(!is_dark)
              .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
          )
          .child(
            Button::new("btn-theme-dark")
              .label("Dark")
              .selected(is_dark)
              .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
          ),
      )
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child(format!("Last click count: {}", self.clicks)),
      )
      .child(div().text_sm().child("Variants"))
      .child(
        h_flex()
          .gap_3()
          .child(Button::new("btn-default").label("Default").default())
          .child(Button::new("btn-down").label("Down").default())
          .child(Button::new("btn-link").label("Link").link())
          .child(Button::new("btn-flat").label("Flat").flat())
          .child(
            Button::new("btn-primary")
              .label("Primary")
              .primary()
              .on_click(cx.listener(|this, _, _, cx| {
                this.clicks += 1;
                cx.notify();
              })),
          )
          .child(Button::new("btn-success").label("Success").success())
          .child(Button::new("btn-warning").label("Warning").warning())
          .child(Button::new("btn-danger").label("Danger").danger())
          .child(
            Button::new("btn-disabled")
              .label("Disabled")
              .primary()
              .disabled(true),
          ),
      )
      .child(div().text_sm().child("Sizes"))
      .child(
        h_flex()
          .items_end()
          .gap_3()
          .child(Button::new("btn-sm").label("Small").small())
          .child(Button::new("btn-md").label("Medium"))
          .child(Button::new("btn-lg").label("Large").large()),
      )
      .child(div().text_sm().child("States"))
      .child(
        h_flex()
          .gap_3()
          .child(
            Button::new("btn-outline")
              .label("Outline")
              .default()
              .outline(true),
          )
          .child(
            Button::new("btn-selected")
              .label("Selected")
              .default()
              .selected(true),
          )
          .child(
            Button::new("btn-loading")
              .label("Loading")
              .primary()
              .loading(true),
          )
          .child(
            Button::new("btn-loading-custom")
              .label("Loading Custom")
              .default()
              .loading(true)
              .icon(Icon::new(IconName::Search)),
          ),
      )
      .child(div().text_sm().child("Icon"))
      .child(
        h_flex()
          .gap_3()
          .child(Button::new("btn-icon").icon(Icon::new(IconName::Settings)))
          .child(
            Button::new("btn-label-icon")
              .icon(Icon::new(IconName::SpinnerIos))
              .label("Search"),
          ),
      )
  }
}

fn main() {
  let app = gpui::Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(900.), px(620.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| ButtonWindow::view(cx),
      )
      .expect("open button demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Button Example");
      })
      .expect("update button demo window failed");
  });
}
