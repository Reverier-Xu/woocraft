use gpui::{
  AnyElement, App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Pixels, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Selectable, StyledExt, Theme, ThemeMode, h_flex, h_resizable, init, resizable_panel,
  v_flex, v_resizable,
};

fn panel_box(content: impl Into<String>, _cx: &App) -> AnyElement {
  div()
    .p_4()
    .size_full()
    .child(content.into())
    .into_any_element()
}

struct ResizableWindow;

impl ResizableWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self)
  }
}

impl Render for ResizableWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();

    v_flex()
      .size_full()
      .p_6()
      .gap_4()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(
        h_flex()
          .items_center()
          .justify_between()
          .child(
            div()
              .text_xl()
              .font_semibold()
              .child("Woocraft Resizable Preview"),
          )
          .child(
            h_flex()
              .gap_2()
              .child(
                woocraft::Button::new("resizable-theme-light")
                  .label("Light")
                  .selected(!is_dark)
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
              )
              .child(
                woocraft::Button::new("resizable-theme-dark")
                  .label("Dark")
                  .selected(is_dark)
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
              ),
          ),
      )
      .child(
        div()
          .h(px(420.))
          .border_1()
          .border_color(cx.theme().border)
          .rounded_md()
          .overflow_hidden()
          .child(
            v_resizable("resizable-root")
              .on_resize(|state, _, cx| {
                println!("resized: {:?}", state.read(cx).sizes());
              })
              .child(
                h_resizable("resizable-top")
                  .child(
                    resizable_panel()
                      .size(px(180.))
                      .size_range(px(120.)..px(360.))
                      .child(panel_box("Left (120..360)", cx)),
                  )
                  .child(panel_box("Center (Grow)", cx))
                  .child(
                    resizable_panel()
                      .size(px(260.))
                      .size_range(px(180.)..Pixels::MAX)
                      .child(panel_box("Right (>=180)", cx)),
                  ),
              )
              .child(panel_box("Middle", cx))
              .child(
                resizable_panel()
                  .size(px(90.))
                  .size_range(px(90.)..Pixels::MAX)
                  .child(panel_box("Bottom (>=90)", cx)),
              ),
          ),
      )
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child("Drag separators between panels to resize."),
      )
  }
}

fn main() {
  let app = gpui::Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1080.), px(760.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| ResizableWindow::view(cx),
      )
      .expect("open resizable demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Resizable Example");
      })
      .expect("update resizable demo window failed");
  });
}
