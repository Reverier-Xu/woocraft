use gpui::{Half, Pixels, px};

pub(crate) const VERTICAL_SCROLLBAR_WIDTH: Pixels = px(14.0);
pub(crate) const DEFAULT_VIEWPORT_ROWS: usize = 24;
const MIN_THUMB_HEIGHT: Pixels = px(16.0);

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

#[cfg(test)]
mod tests {
  use gpui::px;

  use super::*;

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
