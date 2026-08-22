use std::ops::Range;

use gpui::{Half, Pixels, px};

pub(crate) const VERTICAL_SCROLLBAR_WIDTH: Pixels = px(32.0);
pub(crate) const DEFAULT_VIEWPORT_ROWS: usize = 24;
const MIN_THUMB_HEIGHT: Pixels = px(8.0);

pub(crate) fn viewport_rows(viewport_height: Pixels, line_height: Pixels) -> usize {
  if viewport_height <= px(0.0) || line_height <= px(0.0) {
    return DEFAULT_VIEWPORT_ROWS;
  }

  ((viewport_height / line_height) as usize).max(1)
}

pub(crate) fn max_top_row(total_rows: usize, viewport_rows: usize) -> usize {
  total_rows.saturating_sub(viewport_rows.min(total_rows))
}

pub(crate) fn clamp_top_row(top_row: usize, total_rows: usize, viewport_rows: usize) -> usize {
  top_row.min(max_top_row(total_rows, viewport_rows))
}

pub(crate) fn compute_scrollbar_thumb(
  top_row: usize, viewport_rows: usize, total_rows: usize, track_height: Pixels,
) -> (Pixels, Pixels) {
  if track_height <= px(0.0) {
    return (px(0.0), track_height);
  }

  if total_rows == 0 {
    return (px(0.0), track_height);
  }

  let total_rows = total_rows.max(1) as f32;
  let viewport_rows = viewport_rows.max(1) as f32;
  let clamped_top_row = top_row.min(total_rows as usize) as f32;
  let track_height_f32 = f32::from(track_height);

  let thumb_height = (track_height_f32 * (viewport_rows / total_rows))
    .max(f32::from(MIN_THUMB_HEIGHT))
    .min(track_height_f32);
  let max_top = (total_rows - viewport_rows).max(0.0);
  let y = if max_top <= 0.0 {
    0.0
  } else {
    let frac = (clamped_top_row / max_top).clamp(0.0, 1.0);
    frac * (track_height_f32 - thumb_height)
  };

  (px(y), px(thumb_height))
}

pub(crate) fn scrollbar_y_to_top_row(
  local_y: Pixels, total_rows: usize, viewport_rows: usize, track_height: Pixels,
) -> usize {
  if total_rows == 0 || track_height <= px(0.0) {
    return 0;
  }

  let (_thumb_y, thumb_height) =
    compute_scrollbar_thumb(0, viewport_rows, total_rows, track_height);
  let travel = (track_height - thumb_height).max(px(0.0));
  if travel <= px(0.0) {
    return 0;
  }

  let thumb_center = (local_y - thumb_height.half()).clamp(px(0.0), travel);
  let frac = (f32::from(thumb_center) / f32::from(travel)).clamp(0.0, 1.0);
  let max_top = max_top_row(total_rows, viewport_rows);
  (frac * max_top as f32).round() as usize
}

/// Selects the minimap preview window for the given scroll state.
///
/// Fit regime (`total <= capacity`): the whole document is requested and the
/// editor scales the returned rows up to fill the track. Windowed regime: a
/// `capacity`-row window anchored on the thumb's top edge, so `thumb_y` rows
/// of already-scrolled context appear above it and `capacity - thumb_y` rows
/// below — the preview content then scrolls up in real time while the thumb
/// stays the global scrollbar.
///
/// `anchor` is the row at the thumb's top edge (the viewport top, in the
/// backend's row space); the returned range may extend past the document, in
/// which case the backend clamps it to its own content.
pub(crate) fn preview_window(
  total: usize, capacity: usize, anchor: usize, thumb_y: f32,
) -> Range<usize> {
  if total <= capacity {
    return 0..capacity;
  }
  let start = (anchor as f32 - thumb_y).round().max(0.0) as usize;
  start..(start + capacity)
}

/// Tiles `count` preview rows over a `scale`-per-row track with cumulative
/// rounding, so consecutive rows share no pixels and the total is exactly
/// `round(count * scale)`.
pub(crate) fn tile_preview_rows(count: usize, scale: f32) -> Vec<usize> {
  (0..count)
    .map(|i| (scale * (i + 1) as f32).round() as usize - (scale * i as f32).round() as usize)
    .collect()
}

#[cfg(test)]
mod tests {
  use gpui::px;

  use super::*;

  #[test]
  fn preview_window_fit_regime_requests_whole_document() {
    // Few/medium logs: total <= capacity → the whole document, scaled up.
    assert_eq!(preview_window(13, 600, 0, 0.0), 0..600);
    assert_eq!(preview_window(300, 600, 130, 260.0), 0..600);
    assert_eq!(preview_window(600, 600, 560, 590.0), 0..600);
    assert_eq!(preview_window(0, 600, 0, 0.0), 0..600);
  }

  #[test]
  fn preview_window_windowed_regime_anchors_on_thumb() {
    // Many logs: capacity-row window anchored so the thumb's top edge shows
    // the viewport top (anchor), with thumb_y rows of context above.
    assert_eq!(preview_window(10000, 600, 0, 0.0), 0..600);
    assert_eq!(preview_window(10000, 600, 500, 29.7), 470..1070);
    assert_eq!(preview_window(10000, 600, 5000, 297.2), 4703..5303);
    // Bottom of the document: window clamps via the backend, still H rows.
    assert_eq!(preview_window(10000, 600, 9960, 592.0), 9368..9968);
    // thumb_y beyond the anchor (near the top) must not go negative.
    assert_eq!(preview_window(10000, 600, 10, 12.0), 0..600);
  }

  #[test]
  fn tile_preview_rows_fills_the_track_exactly() {
    // Windowed: 1px per row.
    assert_eq!(tile_preview_rows(600, 1.0), vec![1; 600]);
    // Fit: 300 rows over 600px → 2px each.
    assert_eq!(tile_preview_rows(300, 2.0), vec![2; 300]);
    // Fit: 13 rows over 600px → cumulative rounding tiles exactly 600px.
    let heights = tile_preview_rows(13, 600.0 / 13.0);
    assert_eq!(heights.len(), 13);
    assert_eq!(heights.iter().sum::<usize>(), 600);
    // Odd scales still tile with no gaps or overlaps.
    let heights = tile_preview_rows(7, 120.0 / 7.0);
    assert_eq!(heights.iter().sum::<usize>(), 120);
    let heights = tile_preview_rows(1000, 597.6 / 1000.0);
    assert_eq!(heights.iter().sum::<usize>(), 598); // round(597.6)
  }

  #[test]
  fn viewport_row_count_has_floor_and_default() {
    assert_eq!(viewport_rows(px(0.0), px(20.0)), DEFAULT_VIEWPORT_ROWS);
    assert_eq!(viewport_rows(px(100.0), px(20.0)), 5);
    assert_eq!(viewport_rows(px(5.0), px(20.0)), 1);
  }

  #[test]
  fn top_row_is_clamped_to_valid_range() {
    assert_eq!(max_top_row(100, 10), 90);
    assert_eq!(clamp_top_row(95, 100, 10), 90);
    assert_eq!(clamp_top_row(3, 2, 10), 0);
  }

  #[test]
  fn scrollbar_mapping_round_trips_reasonably() {
    let track_height = px(200.0);
    let (thumb_y, thumb_height) = compute_scrollbar_thumb(40, 10, 100, track_height);
    assert!(thumb_height >= MIN_THUMB_HEIGHT);
    let mapped = scrollbar_y_to_top_row(thumb_y + thumb_height.half(), 100, 10, track_height);
    assert!(mapped.abs_diff(40) <= 1);
  }
}
