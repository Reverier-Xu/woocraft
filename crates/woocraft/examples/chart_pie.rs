use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, PieChart, Selectable, StyledExt, Theme, ThemeMode, h_flex, init, v_flex,
};

#[derive(Clone)]
struct Segment {
  label: &'static str,
  value: f32,
}

fn segment_data() -> Vec<Segment> {
  vec![
    Segment {
      label: "Search",
      value: 35.0,
    },
    Segment {
      label: "Direct",
      value: 21.0,
    },
    Segment {
      label: "Referral",
      value: 15.0,
    },
    Segment {
      label: "Social",
      value: 11.0,
    },
    Segment {
      label: "Ads",
      value: 18.0,
    },
  ]
}

#[derive(Default)]
struct ChartWindow;

impl ChartWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self)
  }
}

impl Render for ChartWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let data = segment_data();

    let pie = PieChart::new(data.clone())
      .value(|d| d.value)
      .pad_angle(0.02);

    let donut = PieChart::new(data)
      .value(|d| d.value)
      .inner_radius(64.0)
      .outer_radius(98.0)
      .pad_angle(0.03)
      .outer_radius_fn(|arc| if arc.value > 20.0 { 110.0 } else { 94.0 });

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
              .child("Chart Example · Pie + Donut"),
          )
          .child(
            h_flex()
              .gap_2()
              .child(
                woocraft::Button::new("pie-theme-light")
                  .label("Light")
                  .selected(!cx.theme().mode.is_dark())
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
              )
              .child(
                woocraft::Button::new("pie-theme-dark")
                  .label("Dark")
                  .selected(cx.theme().mode.is_dark())
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
              ),
          ),
      )
      .child(
        h_flex()
          .w_full()
          .gap_4()
          .child(
            v_flex()
              .flex_1()
              .rounded(cx.theme().radius_lg)
              .border_1()
              .border_color(cx.theme().border)
              .bg(cx.theme().card)
              .p_4()
              .child(div().font_medium().child("Traffic Share Pie"))
              .child(
                div()
                  .text_sm()
                  .text_color(cx.theme().muted_foreground)
                  .child("Auto palette + proportional slices"),
              )
              .child(div().mt_3().w_full().h(px(320.)).child(pie)),
          )
          .child(
            v_flex()
              .flex_1()
              .rounded(cx.theme().radius_lg)
              .border_1()
              .border_color(cx.theme().border)
              .bg(cx.theme().card)
              .p_4()
              .child(div().font_medium().child("Adaptive Donut"))
              .child(
                div()
                  .text_sm()
                  .text_color(cx.theme().muted_foreground)
                  .child("Large values have longer outer radius"),
              )
              .child(div().mt_3().w_full().h(px(320.)).child(donut)),
          ),
      )
      .child(
        v_flex()
          .rounded(cx.theme().radius_lg)
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().card)
          .p_4()
          .gap_1()
          .child(div().font_medium().child("Segments"))
          .children(segment_data().into_iter().map(|item| {
            h_flex()
              .justify_between()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child(item.label)
              .child(format!("{:.1}%", item.value))
          })),
      )
  }
}

fn main() {
  let app = gpui::Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1080.), px(800.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| ChartWindow::view(cx),
      )
      .expect("open pie chart demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Chart Example · Pie + Donut");
      })
      .expect("update pie chart demo window failed");
  });
}
