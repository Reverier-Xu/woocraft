use gpui::{App, Hsla};

use crate::ActiveTheme as _;

mod area_chart;
mod axes;
mod bar_chart;
mod candlestick_chart;
mod line_chart;
mod pie_chart;

pub use area_chart::AreaChart;
pub use bar_chart::BarChart;
pub use candlestick_chart::CandlestickChart;
pub use line_chart::LineChart;
pub use pie_chart::PieChart;

/// Default series colour, cycling through the theme's accent ramp.
///
/// Shared by every chart so a multi-series line/area chart and a pie chart
/// paint the same colours for the same series index.
#[inline]
pub(super) fn palette(index: usize, cx: &App) -> Hsla {
  let colors = [
    cx.theme().primary,
    cx.theme().accent,
    cx.theme().success,
    cx.theme().warning,
    cx.theme().danger,
    cx.theme().blue,
    cx.theme().cyan,
    cx.theme().green,
    cx.theme().yellow,
    cx.theme().red,
  ];
  colors[index % colors.len()]
}
