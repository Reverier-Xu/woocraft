//! Shared axis + grid painting for the cartesian charts (line, bar, area,
//! candlestick).
//!
//! The four cartesian charts draw an identical x axis with thinned tick
//! labels plus a dashed horizontal grid; only the tick-label alignment and
//! (for band scales) the label offset differ.

use gpui::{App, Bounds, Pixels, SharedString, TextAlign, Window, px};

use crate::{AXIS_GAP, ActiveTheme, AxisText, Grid, PlotAxis};

/// Alignment policy for x tick labels.
pub(super) enum XTickAlign {
  /// Point scales: the first label aligns to the left edge, the last to the
  /// right edge, everything else centers.
  Edges,
  /// Band scales: every label centers over its band, offset by half the
  /// band width.
  Band { band_width: f32 },
}

/// Paints the x axis, tick labels, and the dashed horizontal grid shared by
/// the cartesian charts.
///
/// `tick_label` resolves the `i`-th datum to its label text and pixel
/// position (`None` skips the datum); `tick_margin` thins the labels to
/// every `tick_margin`-th datum. The axis sits `AXIS_GAP` above the bottom
/// edge of `bounds`.
pub(super) fn paint_axes(
  data_len: usize, tick_label: impl Fn(usize) -> Option<(SharedString, f32)>, tick_margin: usize,
  align: XTickAlign, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App,
) {
  let height = bounds.size.height.as_f32() - AXIS_GAP;

  let x_label = (0..data_len).filter_map(|i| {
    if (i + 1) % tick_margin != 0 {
      return None;
    }

    let (text, tick) = tick_label(i)?;
    let (tick, align) = match align {
      XTickAlign::Edges => (tick, edge_align(i, data_len)),
      XTickAlign::Band { band_width } => (tick + band_width / 2., TextAlign::Center),
    };

    Some(AxisText::new(text, tick, cx.theme().muted_foreground).align(align))
  });

  PlotAxis::new()
    .x(height)
    .x_label(x_label)
    .stroke(cx.theme().border)
    .paint(&bounds, window, cx);

  Grid::new()
    .y((0..=3).map(|i| height * i as f32 / 4.0).collect())
    .stroke(cx.theme().border)
    .dash_array(&[px(4.), px(2.)])
    .paint(&bounds, window);
}

/// Alignment of the `index`-th label out of `len` along the x axis.
fn edge_align(index: usize, len: usize) -> TextAlign {
  match index {
    0 if len == 1 => TextAlign::Center,
    0 => TextAlign::Left,
    i if i == len - 1 => TextAlign::Right,
    _ => TextAlign::Center,
  }
}
