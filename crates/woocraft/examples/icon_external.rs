use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use rust_embed::RustEmbed;
use woocraft::{
  ActiveTheme, CombinedSource, EmbeddedSource, Icon, IconName, StyledExt, h_flex, init,
  register_icon, v_flex,
};

#[derive(RustEmbed)]
#[folder = "examples/assets"]
#[include = "external/**/*.svg"]
struct ExternalAssets;

#[derive(Default)]
struct ExternalIconWindow;

impl ExternalIconWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self)
  }
}

impl Render for ExternalIconWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
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
          .child("Woocraft External Icon Source"),
      )
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child(
            "Built-in assets use tech.woooo.woocraft/assets/* and external assets use external/*.",
          ),
      )
      .child(
        h_flex()
          .gap_6()
          .items_center()
          .child(
            v_flex()
              .gap_2()
              .items_center()
              .child(Icon::new(IconName::Apps).size_10())
              .child(div().text_sm().child("Built-in icon")),
          )
          .child(
            v_flex()
              .gap_2()
              .items_center()
              .child(Icon::new("external/star.svg").size_10())
              .child(div().text_sm().child("External path icon")),
          )
          .child(
            v_flex()
              .gap_2()
              .items_center()
              .child(Icon::new("external-star").size_10())
              .child(div().text_sm().child("External alias icon")),
          ),
      )
  }
}

fn main() {
  let assets = CombinedSource::new()
    .with(woocraft::Assets)
    .with(EmbeddedSource::<ExternalAssets>::new());

  gpui_platform::application()
    .with_assets(assets)
    .run(|cx: &mut App| {
      init(cx);
      cx.activate(true);

      register_icon("external-star", "external/star.svg");

      let bounds = Bounds::centered(None, GpuiSize::new(px(760.), px(420.)), cx);
      let window = cx
        .open_window(
          WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            ..Default::default()
          },
          |_window, cx| ExternalIconWindow::view(cx),
        )
        .expect("open external icon demo window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft External Icon Example");
        })
        .expect("update external icon demo window failed");
    });
}
