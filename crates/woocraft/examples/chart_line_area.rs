use gpuim::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, AreaChart, LineChart, Selectable, StyledExt, Theme, ThemeMode, h_flex, init, v_flex,
};

#[derive(Clone)]
struct TrendRow {
  month: &'static str,
  revenue: f64,
  cost: f64,
}

fn trend_data() -> Vec<TrendRow> {
  vec![
    TrendRow {
      month: "Jan",
      revenue: 34.0,
      cost: 22.0,
    },
    TrendRow {
      month: "Feb",
      revenue: 41.0,
      cost: 25.0,
    },
    TrendRow {
      month: "Mar",
      revenue: 39.0,
      cost: 27.0,
    },
    TrendRow {
      month: "Apr",
      revenue: 46.0,
      cost: 28.0,
    },
    TrendRow {
      month: "May",
      revenue: 52.0,
      cost: 31.0,
    },
    TrendRow {
      month: "Jun",
      revenue: 57.0,
      cost: 33.0,
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
    let data = trend_data();
    let line = LineChart::new(data.clone())
      .x(|d| d.month)
      .y(|d| d.revenue)
      .dot()
      .natural();

    let area = AreaChart::new(data)
      .x(|d| d.month)
      .y(|d| d.revenue)
      .stroke(cx.theme().primary)
      .fill(cx.theme().primary.opacity(0.22))
      .linear()
      .y(|d| d.cost)
      .stroke(cx.theme().accent)
      .fill(cx.theme().accent.opacity(0.16))
      .step_after();

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
              .child("Chart Example · Line + Area"),
          )
          .child(
            h_flex()
              .gap_2()
              .child(
                woocraft::Button::new("chart-theme-light")
                  .label("Light")
                  .selected(!cx.theme().mode.is_dark())
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
              )
              .child(
                woocraft::Button::new("chart-theme-dark")
                  .label("Dark")
                  .selected(cx.theme().mode.is_dark())
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
              ),
          ),
      )
      .child(
        div()
          .rounded(cx.theme().radius_lg)
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().card)
          .p_4()
          .child(div().font_medium().child("Revenue Trend"))
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child("Natural curve + dot markers"),
          )
          .child(div().mt_3().w_full().h(px(260.)).child(line)),
      )
      .child(
        div()
          .rounded(cx.theme().radius_lg)
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().card)
          .p_4()
          .child(div().font_medium().child("Mixed Area Series"))
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child("Revenue (linear) + active users (step-after)"),
          )
          .child(div().mt_3().w_full().h(px(260.)).child(area)),
      )
  }
}

fn main() {
  let app = gpuim_platform::application().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1080.), px(860.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| ChartWindow::view(cx),
      )
      .expect("open line+area chart demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Chart Example · Line + Area");
      })
      .expect("update line+area chart demo window failed");
  });
}
