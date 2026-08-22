use std::ops::Range;

use gpui::{Hsla, Pixels, px};

/// The visual shape of a [`ScrollbarMarker`].
///
/// This enum is `#[non_exhaustive]`: new shapes may be added in future
/// releases without a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ScrollbarMarkerKind {
  /// A small filled dot centered on the marker row.
  Dot,
  /// A thin vertical pill centered on the marker row.
  Tick,
  /// A short horizontal pill centered on the marker row.
  Dash,
  /// A vertical bar spanning `row..=end_row`.
  Bar,
}

/// A marker rendered on the editor's vertical scrollbar track, e.g. a
/// minimap-style indicator for notable lines such as log entries at a given
/// severity or diagnostics.
///
/// Markers are anchored to document rows; the editor maps rows proportionally
/// onto the scrollbar track, so a marker at row `n` of `total` is drawn `n /
/// total` of the way down the track, independent of the current scroll
/// position.
///
/// # Overlap
///
/// When markers occupy the same track pixels, the one with the highest
/// [`ScrollbarMarker::priority`] wins. Ties are resolved in favor of the
/// marker that appears later in the vector returned by
/// [`super::EditorBackend::scrollbar_markers`]. The marker's alpha is
/// respected during blending, but it never affects which marker wins a pixel.
///
/// # Examples
///
/// ```no_run
/// use gpui::red;
/// use woocraft::ScrollbarMarker;
///
/// // A single dot on row 42.
/// let _ = ScrollbarMarker::dot(42, red());
///
/// // An error band spanning rows 10..=20, stacking above plain dots.
/// let _ = ScrollbarMarker::bar(10..21, red()).with_priority(10);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollbarMarker {
  /// First document row (0-based) the marker refers to.
  pub row: u64,
  /// Last document row the marker covers (inclusive). Equal to [`Self::row`]
  /// for point markers; only [`ScrollbarMarkerKind::Bar`] draws the span.
  pub end_row: u64,
  /// Visual shape of the marker.
  pub kind: ScrollbarMarkerKind,
  /// Marker color; the alpha channel is respected.
  pub color: Hsla,
  /// Stacking priority, higher wins pixel conflicts. Defaults to `0`.
  pub priority: u32,
}

impl ScrollbarMarker {
  /// A small filled dot centered on `row`.
  pub fn dot(row: u64, color: Hsla) -> Self {
    Self {
      row,
      end_row: row,
      kind: ScrollbarMarkerKind::Dot,
      color,
      priority: 0,
    }
  }

  /// A thin vertical pill centered on `row`.
  pub fn tick(row: u64, color: Hsla) -> Self {
    Self {
      row,
      end_row: row,
      kind: ScrollbarMarkerKind::Tick,
      color,
      priority: 0,
    }
  }

  /// A short horizontal pill centered on `row`.
  pub fn dash(row: u64, color: Hsla) -> Self {
    Self {
      row,
      end_row: row,
      kind: ScrollbarMarkerKind::Dash,
      color,
      priority: 0,
    }
  }

  /// A vertical bar spanning `range` (rows `range.start..range.end`,
  /// exclusive), e.g. for multi-line diagnostics.
  pub fn bar(range: Range<u64>, color: Hsla) -> Self {
    Self {
      row: range.start,
      end_row: range.end.saturating_sub(1).max(range.start),
      kind: ScrollbarMarkerKind::Bar,
      color,
      priority: 0,
    }
  }

  /// Extend the marker to also cover rows up to `end_row` (inclusive). Only
  /// meaningful for [`ScrollbarMarkerKind::Bar`].
  pub fn with_end_row(mut self, end_row: u64) -> Self {
    self.end_row = end_row.max(self.row);
    self
  }

  /// Set the stacking priority (higher wins pixel conflicts).
  pub fn with_priority(mut self, priority: u32) -> Self {
    self.priority = priority;
    self
  }
}

// Marker geometry in pixels. These are implementation details and may change
// between releases.
const DOT_SIZE: f32 = 5.0;
const TICK_WIDTH: f32 = 4.0;
const TICK_HEIGHT: f32 = 12.0;
const DASH_WIDTH: f32 = 8.0;
const DASH_HEIGHT: f32 = 3.0;
const BAR_WIDTH: f32 = 6.0;
/// Minimum height of a [`ScrollbarMarkerKind::Bar`], so single-row bars stay
/// visible.
const MIN_BAR_HEIGHT: f32 = 4.0;

/// The fixed geometry of a marker kind. `Bar` reports its width only; the
/// height is derived from the marker's row span.
fn fixed_geometry(kind: ScrollbarMarkerKind) -> (f32, f32) {
  match kind {
    ScrollbarMarkerKind::Dot => (DOT_SIZE, DOT_SIZE),
    ScrollbarMarkerKind::Tick => (TICK_WIDTH, TICK_HEIGHT),
    ScrollbarMarkerKind::Dash => (DASH_WIDTH, DASH_HEIGHT),
    ScrollbarMarkerKind::Bar => (BAR_WIDTH, 0.0),
  }
}

/// A marker fully laid out on the scrollbar track, ready to render.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedScrollbarMarker {
  pub left: Pixels,
  pub top: Pixels,
  pub width: Pixels,
  pub height: Pixels,
  pub kind: ScrollbarMarkerKind,
  pub color: Hsla,
}

/// Lays out `markers` onto the scrollbar track.
///
/// Rows are mapped proportionally: row `n` of `total_rows` is drawn `n /
/// total_rows` of the way down the track. Overlapping markers are resolved
/// per pixel by [`ScrollbarMarker::priority`] (ties go to the later marker),
/// and consecutive track pixels won by the same `(kind, color)` are merged
/// into a single segment so dense markers stay cheap to render.
pub(crate) fn resolve_scrollbar_markers(
  markers: &[ScrollbarMarker], total_rows: usize, track_width: Pixels, track_height: Pixels,
) -> Vec<ResolvedScrollbarMarker> {
  if markers.is_empty() || total_rows == 0 || track_height <= px(0.0) {
    return Vec::new();
  }

  let track_h = f32::from(track_height);
  let track_w = f32::from(track_width);
  let total = total_rows as f32;
  let height_px = track_h.ceil().max(1.0) as usize;

  let y_of = |row: f32| (row / total * track_h).clamp(0.0, track_h);

  // Per-pixel winners: (priority, marker index, kind, color).
  let mut winners: Vec<Option<(u32, usize, ScrollbarMarkerKind, Hsla)>> = vec![None; height_px];

  for (idx, marker) in markers.iter().enumerate() {
    let (_, height) = fixed_geometry(marker.kind);
    let (y_top, y_bottom) = match marker.kind {
      ScrollbarMarkerKind::Bar => {
        let start = y_of(marker.row as f32);
        let end = y_of(marker.end_row.saturating_add(1) as f32);
        (start, (end - start).max(MIN_BAR_HEIGHT) + start)
      }
      _ => {
        let center = y_of(marker.row as f32);
        (center - height / 2.0, center + height / 2.0)
      }
    };

    let top = y_top.clamp(0.0, track_h).floor() as usize;
    let bottom = (y_bottom.clamp(0.0, track_h).ceil() as usize).min(height_px);
    if bottom <= top {
      continue;
    }

    for slot in &mut winners[top..bottom] {
      let better = match slot {
        None => true,
        Some((priority, order, ..)) => {
          marker.priority > *priority || (marker.priority == *priority && idx > *order)
        }
      };
      if better {
        *slot = Some((marker.priority, idx, marker.kind, marker.color));
      }
    }
  }

  // Merge consecutive pixels won by the same (kind, color) into segments.
  let mut segments: Vec<ResolvedScrollbarMarker> = Vec::new();
  let mut pixel = 0;
  while pixel < height_px {
    let Some((_, _, kind, color)) = winners[pixel] else {
      pixel += 1;
      continue;
    };
    let start = pixel;
    while pixel < height_px && winners[pixel].is_some_and(|(_, _, k, c)| k == kind && c == color) {
      pixel += 1;
    }
    let (width, _) = fixed_geometry(kind);
    segments.push(ResolvedScrollbarMarker {
      left: px(((track_w - width) / 2.0).max(0.0).round()),
      top: px(start as f32),
      width: px(width),
      height: px((pixel - start) as f32),
      kind,
      color,
    });
  }
  segments
}

#[cfg(test)]
mod tests {
  use gpui::rgb;

  use super::*;

  const TRACK: (Pixels, Pixels) = (px(32.0), px(100.0));

  fn resolve(markers: &[ScrollbarMarker], total_rows: usize) -> Vec<ResolvedScrollbarMarker> {
    resolve_scrollbar_markers(markers, total_rows, TRACK.0, TRACK.1)
  }

  #[test]
  fn point_marker_is_centered_on_its_row() {
    let markers = [ScrollbarMarker::dot(50, rgb(0xFF0000).into())];
    let segments = resolve(&markers, 100);
    assert_eq!(segments.len(), 1);
    let segment = segments[0];
    assert_eq!(segment.top, px(47.0));
    assert_eq!(segment.height, px(6.0));
    assert_eq!(segment.width, px(5.0));
    // Centered within the 32px track: (32 - 5) / 2 = 13.5, rounded up.
    assert_eq!(segment.left, px(14.0));
  }

  #[test]
  fn markers_at_track_edges_are_clamped() {
    let markers = [ScrollbarMarker::dot(0, rgb(0xFF0000).into())];
    let segments = resolve(&markers, 100);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].top, px(0.0));
    assert!(segments[0].height >= px(2.0));
  }

  #[test]
  fn bar_spans_its_row_range() {
    // Rows 10..=20 of 100 → y(10) = 10px .. y(21) = 21px.
    let markers = [ScrollbarMarker::bar(10..21, rgb(0xFF0000).into())];
    let segments = resolve(&markers, 100);
    assert_eq!(segments.len(), 1);
    assert_eq!(segments[0].top, px(10.0));
    assert_eq!(segments[0].height, px(11.0));
  }

  #[test]
  fn single_row_bar_has_minimum_height() {
    let markers = [ScrollbarMarker::bar(50..51, rgb(0xFF0000).into())];
    let segments = resolve(&markers, 1000);
    assert_eq!(segments.len(), 1);
    assert!(segments[0].height >= px(4.0));
  }

  #[test]
  fn higher_priority_wins_pixel_conflicts() {
    // A full-height bar and a dot on row 50, both priority 0 → the later
    // marker (the dot) wins the shared pixels and splits the bar.
    let markers = [
      ScrollbarMarker::bar(0..101, rgb(0x00FF00).into()),
      ScrollbarMarker::dot(50, rgb(0xFF0000).into()),
    ];
    let segments = resolve(&markers, 100);
    let dot = segments
      .iter()
      .find(|segment| segment.color == rgb(0xFF0000).into())
      .expect("dot segment should win its pixels");
    assert_eq!(dot.top, px(47.0));
    assert_eq!(dot.height, px(6.0));
    // The bar is split around the dot.
    assert_eq!(segments.len(), 3);
  }

  #[test]
  fn priority_overrides_insertion_order() {
    // The dot has higher priority and wins even though it comes first.
    let markers = [
      ScrollbarMarker::dot(50, rgb(0xFF0000).into()).with_priority(10),
      ScrollbarMarker::bar(0..101, rgb(0x00FF00).into()),
    ];
    let segments = resolve(&markers, 100);
    let dot = segments
      .iter()
      .find(|segment| segment.color == rgb(0xFF0000).into())
      .expect("dot segment should win its pixels");
    assert_eq!(dot.height, px(6.0));
    assert!(segments.iter().any(|s| s.color == rgb(0x00FF00).into()));
  }

  #[test]
  fn empty_inputs_produce_no_segments() {
    assert!(resolve(&[], 100).is_empty());
    assert!(resolve(&[ScrollbarMarker::dot(0, rgb(0xFF0000).into())], 0).is_empty());
  }
}
