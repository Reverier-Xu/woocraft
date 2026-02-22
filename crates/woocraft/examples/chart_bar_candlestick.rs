use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, BarChart, CandlestickChart, Selectable, StyledExt, Theme, ThemeMode, h_flex, init,
  v_flex,
};

#[derive(Clone)]
struct SalesRow {
  channel: &'static str,
  total: f64,
}

#[derive(Clone)]
struct CandleRow {
  session: &'static str,
  open: f64,
  high: f64,
  low: f64,
  close: f64,
}

fn sales_data() -> Vec<SalesRow> {
  vec![
    SalesRow {
      channel: "Web",
      total: 128.0,
    },
    SalesRow {
      channel: "Mobile",
      total: 176.0,
    },
    SalesRow {
      channel: "Partner",
      total: 94.0,
    },
    SalesRow {
      channel: "Retail",
      total: 156.0,
    },
    SalesRow {
      channel: "API",
      total: 112.0,
    },
  ]
}

fn candle_data() -> Vec<CandleRow> {
  vec![
    CandleRow {
      session: "Mon",
      open: 102.0,
      high: 109.0,
      low: 98.0,
      close: 106.0,
    },
    CandleRow {
      session: "Tue",
      open: 106.0,
      high: 113.0,
      low: 104.0,
      close: 111.0,
    },
    CandleRow {
      session: "Wed",
      open: 111.0,
      high: 114.0,
      low: 102.0,
      close: 104.0,
    },
    CandleRow {
      session: "Thu",
      open: 104.0,
      high: 108.0,
      low: 99.0,
      close: 101.0,
    },
    CandleRow {
      session: "Fri",
      open: 101.0,
      high: 118.0,
      low: 100.0,
      close: 116.0,
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
    let bars = BarChart::new(sales_data())
      .x(|d| d.channel)
      .y(|d| d.total)
      .label(|d| format!("{:.0}", d.total));

    let candles = CandlestickChart::new(candle_data())
      .x(|d| d.session)
      .open(|d| d.open)
      .high(|d| d.high)
      .low(|d| d.low)
      .close(|d| d.close)
      .body_width_ratio(0.66);

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
              .child("Chart Example · Bar + Candlestick"),
          )
          .child(
            h_flex()
              .gap_2()
              .child(
                woocraft::Button::new("bar-candle-theme-light")
                  .label("Light")
                  .selected(!cx.theme().mode.is_dark())
                  .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
              )
              .child(
                woocraft::Button::new("bar-candle-theme-dark")
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
          .child(div().font_medium().child("Sales by Channel"))
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child("Default bar style + value labels"),
          )
          .child(div().mt_3().w_full().h(px(250.)).child(bars)),
      )
      .child(
        div()
          .rounded(cx.theme().radius_lg)
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().card)
          .p_4()
          .child(div().font_medium().child("OHLC Candlestick"))
          .child(
            div()
              .text_sm()
              .text_color(cx.theme().muted_foreground)
              .child("Bullish candles use success color; bearish candles use danger color"),
          )
          .child(div().mt_3().w_full().h(px(250.)).child(candles)),
      )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1060.), px(840.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| ChartWindow::view(cx),
      )
      .expect("open bar+candlestick chart demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Chart Example · Bar + Candlestick");
      })
      .expect("update bar+candlestick chart demo window failed");
  });
}
