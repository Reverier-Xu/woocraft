//! Numeric range slider control with single or dual thumb support.
//!
//! Slider allows users to select numeric values by dragging a thumb along a
//! horizontal track. Supports single value selection (one thumb) or range
//! selection (two thumbs). Configurable min/max bounds, step intervals, and
//! scale modes (linear or logarithmic). Useful for volume controls, price
//! filters, date range selection, and any numeric input where visual feedback
//! and drag interaction improves UX over text input.
//!
//! # Features
//! - **Single or Range**: One thumb for a single value, two thumbs for a range
//! - **Numeric Bounds**: Set min, max, and step increment
//! - **Scale Modes**: Linear (uniform spacing) or Logarithmic (for
//!   audio/exponential data)
//! - **Keyboard Support**: Arrow keys adjust value; Alt/Shift modifiers for
//!   large/small steps
//! - **Visual Feedback**: Filled track, thumb indicator, and optional labels
//!   (via delegate)
//! - **Smooth Dragging**: Immediate visual feedback while dragging; the trigger
//!   area is a full-width band as tall as the thumb, and the thumb stays
//!   expanded for the whole drag gesture (`SliderState::dragging`)
//! - **Origin**: the primary track runs from the thumb to [`SliderOrigin`] —
//!   the minimum, the middle (equalizer bands, balance) or the maximum
//! - **Ticks**: a tick at every step, every `major_tick_ratio`-th one longer,
//!   skipped when the step would make them unreadable; ticks are marks rather
//!   than spans, so they are painted under the fill and read as a ruler behind
//!   the track
//! - **Deferred dragging**: `with_immediate(false)` previews the dragged value
//!   with its own thumb and a warning track, emits `Preview` while the pointer
//!   moves and `Commit` on release, and leaves the committed value alone until
//!   then — so a host that reports its own value back cannot cancel the drag
//!
//! Layout note: the rail is a single flat flex row, and everything on it is
//! placed by that flow. The two end stubs and the spans of the track are laid
//! end to end with pixel widths derived from the rail's measured width; a thumb
//! is a zero-width slot cut in between two spans, so it stands on its value
//! because that is where the flow puts it. Nothing on the rail computes a
//! position of its own, so a span's colouring and a thumb's position can never
//! disagree, two thumbs close enough to touch are two pills in the space
//! between them rather than two pills pushed apart, and a thumb takes no room
//! out of the travel range. The rail draws nothing until it has been measured,
//! because that is the only length its layout needs.
//!
//! Motion note: the thumb position is not sprung. A spring would have to
//! suspend travel while the pointer drags (`Spring::with_travel`), but
//! `SliderState` does not track the drag lifecycle, and the thumb position is
//! a layout-relative percentage — animating it would re-run layout every
//! frame instead of staying paint-only. Hover feedback (the thumb grows and
//! brightens) does animate, through a keyed target-value transition whose
//! target is `hovered || dragging`, so a pressed thumb stays expanded even
//! when the pointer leaves the hover area mid-drag. A thumb takes no width out
//! of the rail's flow, so the travel range stays exactly `width - cross` and
//! nothing but the pill itself moves while that animation runs.
//!
//! # Example
//! ```rust,ignore
//! // Volume slider (0-100)
//! let slider_state = cx.new(|cx| {
//!   SliderState::new()
//!     .min(0.0)
//!     .max(100.0)
//!     .step(1.0)
//! });
//!
//! // Price range filter ($10-$100)
//! let range_slider = cx.new(|cx| {
//!   SliderState::new()
//!     .min(10.0)
//!     .max(100.0)
//!     .value(SliderValue::Range(25.0, 75.0))
//! });
//! ```

use gpui::{
  AnyElement, App, AppContext as _, Axis, Bounds, Context, Div, DragMoveEvent, Empty, Entity,
  EntityId, EventEmitter, Hsla, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent,
  MouseMoveEvent, ParentElement, Pixels, Rems, Render, RenderOnce, SharedString, Stateful,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, deferred, div,
  prelude::FluentBuilder as _, px, relative, rems,
};

use crate::{
  ActiveTheme, Easing, ElementExt, Size, StyledExt, Transition, duration, h_flex, opacity,
  transition,
};

#[derive(Clone)]
struct DragThumb((EntityId, bool));

impl Render for DragThumb {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

/// Numeric value type for slider: single value or range.
///
/// `Single(f32)`: One numeric value (one thumb on the slider).
/// `Range(f32, f32)`: Two values representing a range (start, end) with two
/// thumbs. Range values are always kept in order (start ≤ end).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
  /// Single value (one thumb).
  Single(f32),
  /// Range: start and end values (two thumbs). Automatically kept in order.
  Range(f32, f32),
}

impl Default for SliderValue {
  fn default() -> Self {
    Self::Single(0.0)
  }
}

impl From<f32> for SliderValue {
  fn from(value: f32) -> Self {
    Self::Single(value)
  }
}

impl From<(f32, f32)> for SliderValue {
  fn from(value: (f32, f32)) -> Self {
    Self::Range(value.0, value.1)
  }
}

impl SliderValue {
  pub fn is_range(&self) -> bool {
    matches!(self, Self::Range(_, _))
  }

  pub fn start(&self) -> f32 {
    match self {
      Self::Single(value) => *value,
      Self::Range(start, _) => *start,
    }
  }

  pub fn end(&self) -> f32 {
    match self {
      Self::Single(value) => *value,
      Self::Range(_, end) => *end,
    }
  }

  fn set_start(&mut self, value: f32) {
    match self {
      Self::Single(current) => *current = value,
      Self::Range(_, end) => *self = Self::Range(value.min(*end), *end),
    }
  }

  fn set_end(&mut self, value: f32) {
    match self {
      Self::Single(current) => *current = value,
      Self::Range(start, _) => *self = Self::Range(*start, value.max(*start)),
    }
  }
}

/// Numeric scale type for slider value calculation.
///
/// Linear: Values increase uniformly across the track. For volume, brightness,
/// etc. Logarithmic: Values increase exponentially. For audio frequencies,
/// price ranges, etc. Requires min > 0 for logarithmic scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderScale {
  /// Uniform spacing. Default.
  #[default]
  Linear,
  /// Exponential spacing (min must be > 0).
  Logarithmic,
}

/// Every n-th tick is drawn longer unless the caller overrides it.
const DEFAULT_MAJOR_TICK_RATIO: u32 = 5;
/// Height of a minor tick, in ems of the slider's size tier.
const TICK_HEIGHT_EM: f32 = 0.3;
/// Height of a major tick; deliberately only a little taller, so the marks
/// read as a ruler rather than as a second track.
const MAJOR_TICK_HEIGHT_EM: f32 = 0.5;
/// Tick thickness in device pixels: a hairline, like a border.
const TICK_WIDTH_PX: f32 = 1.0;
/// Ticks are skipped above this many per track: a 0.01 step over 0..=100 would
/// otherwise paint ten thousand lines nobody can read.
const MAX_TICKS: u32 = 200;

#[derive(Clone)]
/// Events emitted by the slider when value changes.
pub enum SliderEvent {
  /// The value moved: a drag in immediate mode, a keyboard step, or an
  /// programmatic [`SliderState::set_value`]. Contains the new value.
  Change(SliderValue),
  /// A drag in non-immediate mode is in progress and has not been committed.
  /// Render the pending thumb with [`SliderState::drag_value`]; the slider's
  /// own value is unchanged until the pointer is released.
  Preview(SliderValue),
  /// A drag in non-immediate mode ended; the value is now the previewed one.
  Commit(SliderValue),
}

/// Where the primary track starts.
///
/// The primary track is drawn from the origin to the thumb, so the origin
/// decides what the fill means: growth from the minimum (a volume slider), from
/// the middle (an equalizer band, a balance control), or towards the maximum.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliderOrigin {
  /// The fill grows from the minimum.
  #[default]
  Start,
  /// The fill grows from the middle of the track.
  Middle,
  /// The fill grows from the maximum.
  End,
}

impl SliderOrigin {
  /// The origin as a percentage of the track.
  fn percentage(self) -> f32 {
    match self {
      Self::Start => 0.0,
      Self::Middle => 0.5,
      Self::End => 1.0,
    }
  }
}

/// Internal state management for slider control.
///
/// Handles numeric range calculation, value clamping, and scale conversion
/// (linear/logarithmic). Emits `SliderEvent::Change` when user drags or adjusts
/// value via keyboard. Use `SliderValue::Single()` or `SliderValue::Range()` to
/// start with different modes.
pub struct SliderState {
  min: f32,
  max: f32,
  step: f32,
  value: SliderValue,
  percentage: std::ops::Range<f32>,
  /// Bounds of the rail band, as laid out by the last frame.
  bounds: Bounds<Pixels>,
  /// Where the travel range starts inside [`SliderState::bounds`]: half the
  /// thumb's cross size, since a thumb's centre travels that far in from
  /// either end of the rail. It does not depend on how far the thumb has
  /// grown, so pointer positions keep mapping the same way while hovering.
  travel_inset: Pixels,
  scale: SliderScale,
  active_thumb_start: bool,
  /// Whether a pointer is currently pressed on the slider (click or drag).
  /// Keeps the thumb expanded for the whole gesture, even when the pointer
  /// leaves the hover area mid-drag.
  dragging: bool,
  /// Where the primary track starts.
  origin: SliderOrigin,
  /// Draw a tick at every step.
  ticks: bool,
  /// Every n-th tick is drawn longer; `0` is treated as `1`.
  major_tick_ratio: u32,
  /// Whether dragging moves the value immediately. When false the value only
  /// follows on release and the drag is previewed separately.
  immediate: bool,
  /// The value a non-immediate drag is previewing. Set while dragging, cleared
  /// when the gesture commits.
  drag_value: Option<SliderValue>,
}

impl Default for SliderState {
  fn default() -> Self {
    Self::new()
  }
}

impl SliderState {
  /// Create a new slider with default range (0-100), step 1, and linear scale.
  pub fn new() -> Self {
    let mut this = Self {
      min: 0.0,
      max: 100.0,
      step: 1.0,
      value: SliderValue::default(),
      percentage: 0.0..0.0,
      bounds: Bounds::default(),
      travel_inset: px(0.),
      scale: SliderScale::Linear,
      active_thumb_start: false,
      dragging: false,
      origin: SliderOrigin::default(),
      ticks: false,
      major_tick_ratio: DEFAULT_MAJOR_TICK_RATIO,
      immediate: true,
      drag_value: None,
    };
    this.sync_percentage();
    this
  }

  /// Set the minimum value for the slider.
  ///
  /// Default: 0.0. For logarithmic scale, min must be > 0.
  pub fn min(mut self, min: f32) -> Self {
    self.min = min;
    self.sync_percentage();
    self
  }

  /// Set the maximum value for the slider.
  ///
  /// Default: 100.0. Must be greater than min.
  pub fn max(mut self, max: f32) -> Self {
    self.max = max;
    self.sync_percentage();
    self
  }

  /// Set the step interval for keyboard adjustments.
  ///
  /// When user presses arrow keys, value changes by step amount. Default: 1.0.
  /// For fine-grain control, use small steps (e.g., 0.1).
  pub fn step(mut self, step: f32) -> Self {
    self.step = step.max(0.000_001);
    self
  }

  /// Set the numeric scale mode (Linear or Logarithmic).
  ///
  /// Linear: uniform spacing (default). Logarithmic: exponential spacing.
  /// For logarithmic scale, min must be > 0.
  pub fn scale(mut self, scale: SliderScale) -> Self {
    if matches!(scale, SliderScale::Logarithmic) {
      assert!(self.min > 0.0, "min must be > 0 for logarithmic slider");
      assert!(
        self.max > self.min,
        "max must be > min for logarithmic slider"
      );
    }
    self.scale = scale;
    self.sync_percentage();
    self
  }

  pub fn default_value(mut self, value: impl Into<SliderValue>) -> Self {
    self.value = value.into();
    self.sync_percentage();
    self
  }

  pub fn set_value(&mut self, value: impl Into<SliderValue>, cx: &mut Context<Self>) {
    self.value = self.snap_and_clamp(value.into());
    self.sync_percentage();
    cx.emit(SliderEvent::Change(self.value));
    cx.notify();
  }

  pub fn value(&self) -> SliderValue {
    self.value
  }

  /// The value a non-immediate drag is previewing, if one is in progress.
  pub fn drag_value(&self) -> Option<SliderValue> {
    self.drag_value
  }

  /// Where the primary track starts (default [`SliderOrigin::Start`]).
  pub fn with_origin(mut self, origin: SliderOrigin) -> Self {
    self.origin = origin;
    self
  }

  /// Changes the origin without a rebuild.
  pub fn set_origin(&mut self, origin: SliderOrigin, cx: &mut Context<Self>) {
    if self.origin != origin {
      self.origin = origin;
      cx.notify();
    }
  }

  /// Draws a tick at every step, with every `major_tick_ratio`-th one longer
  /// (`5` by default).
  pub fn with_ticks(mut self, ticks: bool) -> Self {
    self.ticks = ticks;
    self
  }

  /// Selects how many ticks apart the long ones are; `0` is treated as `1`.
  pub fn with_major_tick_ratio(mut self, ratio: u32) -> Self {
    self.major_tick_ratio = ratio.max(1);
    self
  }

  /// Enables ticks without a rebuild.
  pub fn set_ticks(&mut self, ticks: bool, cx: &mut Context<Self>) {
    if self.ticks != ticks {
      self.ticks = ticks;
      cx.notify();
    }
  }

  /// Whether a drag moves the value immediately (default) or only on release.
  ///
  /// With `false` the slider emits [`SliderEvent::Preview`] while the pointer
  /// moves and [`SliderEvent::Commit`] when it is released, and paints the
  /// pending thumb separately from the real one. Ticks and a non-default
  /// origin still apply to the committed value.
  pub fn with_immediate(mut self, immediate: bool) -> Self {
    self.immediate = immediate;
    self
  }

  /// Toggles immediate mode without a rebuild.
  pub fn set_immediate(&mut self, immediate: bool, cx: &mut Context<Self>) {
    if self.immediate != immediate {
      self.immediate = immediate;
      if immediate {
        self.drag_value = None;
      }
      cx.notify();
    }
  }

  /// The origin as a percentage of the track.
  fn origin_percentage(&self) -> f32 {
    self.origin.percentage()
  }

  fn sync_percentage(&mut self) {
    match self.value {
      SliderValue::Single(value) => {
        let p = self.value_to_percentage(value.clamp(self.min, self.max));
        self.percentage = 0.0..p;
      }
      SliderValue::Range(start, end) => {
        let start = start.clamp(self.min, self.max);
        let end = end.clamp(self.min, self.max);
        self.percentage = self.value_to_percentage(start)..self.value_to_percentage(end);
      }
    }
  }

  fn set_geometry(&mut self, bounds: Bounds<Pixels>, travel_inset: Pixels) {
    self.bounds = bounds;
    self.travel_inset = travel_inset;
  }

  /// Length of the travel range along `axis`, in pixels.
  ///
  /// Zero until the first layout, and while a pane is collapsed; callers treat
  /// that as "no geometry yet" rather than as an empty track.
  fn travel_length(&self, axis: Axis) -> Pixels {
    let total = match axis {
      Axis::Horizontal => self.bounds.size.width,
      Axis::Vertical => self.bounds.size.height,
    };
    (total - self.travel_inset * 2.0).max(px(0.))
  }

  /// The percentage under a pointer position.
  ///
  /// The rail is wider than the distance the thumb travels: a thumb's centre
  /// stops half its cross size short of either end, so the band's ends are not
  /// part of the travel range. Mapping the raw band width instead would put a
  /// value under the pointer that the thumb then does not reach.
  fn percentage_at(&self, axis: Axis, position: gpui::Point<Pixels>) -> Option<f32> {
    let length = self.travel_length(axis);
    if length <= px(0.) {
      return None;
    }

    let offset = match axis {
      Axis::Horizontal => (position.x - self.bounds.origin.x) - self.travel_inset,
      Axis::Vertical => (self.bounds.bottom() - self.travel_inset) - position.y,
    };
    Some((offset / length).clamp(0.0, 1.0))
  }

  fn snap_and_clamp(&self, value: SliderValue) -> SliderValue {
    let snap = |mut raw: f32| {
      raw = raw.clamp(self.min, self.max);
      if self.step > 0.0 {
        let steps = ((raw - self.min) / self.step).round();
        raw = self.min + steps * self.step;
      }
      raw.clamp(self.min, self.max)
    };

    match value {
      SliderValue::Single(value) => SliderValue::Single(snap(value)),
      SliderValue::Range(start, end) => {
        let start = snap(start);
        let end = snap(end).max(start);
        SliderValue::Range(start, end)
      }
    }
  }

  fn percentage_to_value(&self, percentage: f32) -> f32 {
    match self.scale {
      SliderScale::Linear => self.min + (self.max - self.min) * percentage,
      SliderScale::Logarithmic => {
        let base = self.max / self.min;
        (base.powf(percentage) * self.min).clamp(self.min, self.max)
      }
    }
  }

  fn value_to_percentage(&self, value: f32) -> f32 {
    match self.scale {
      SliderScale::Linear => {
        let range = self.max - self.min;
        if range <= 0.0 {
          0.0
        } else {
          ((value - self.min) / range).clamp(0.0, 1.0)
        }
      }
      SliderScale::Logarithmic => {
        let base = self.max / self.min;
        (value / self.min).log(base).clamp(0.0, 1.0)
      }
    }
  }

  fn choose_active_thumb(&mut self, axis: Axis, position: gpui::Point<Pixels>) {
    if !self.value.is_range() {
      self.active_thumb_start = false;
      return;
    }

    let Some(percentage) = self.percentage_at(axis, position) else {
      self.active_thumb_start = false;
      return;
    };

    // Which thumb the gesture grabs is a question about the range in the
    // middle of the track, so it is answered in percentages and never needs
    // the rail's pixel width.
    let center = (self.percentage.start + self.percentage.end) * 0.5;
    self.active_thumb_start = percentage < center;
  }

  fn update_by_position(
    &mut self, axis: Axis, position: gpui::Point<Pixels>, is_start: bool, cx: &mut Context<Self>,
  ) {
    let Some(raw_percentage) = self.percentage_at(axis, position) else {
      return;
    };
    let percentage = if is_start {
      raw_percentage.clamp(0.0, self.percentage.end)
    } else {
      raw_percentage.clamp(self.percentage.start, 1.0)
    };

    let value = self.percentage_to_value(percentage);
    let value = match self.snap_and_clamp(SliderValue::Single(value)) {
      SliderValue::Single(v) => v,
      SliderValue::Range(..) => value,
    };

    if self.immediate {
      let old_value = self.value;
      if is_start {
        self.value.set_start(value);
      } else {
        self.value.set_end(value);
      }
      self.value = self.snap_and_clamp(self.value);
      self.sync_percentage();
      if self.value != old_value {
        cx.emit(SliderEvent::Change(self.value));
        cx.notify();
      }
      return;
    }

    // Non-immediate: the drag previews a value and the committed one only
    // follows on release, so a host that is slow to apply a value (a seek, a
    // network write) does not fight the pointer.
    let mut preview = self.drag_value.unwrap_or(self.value);
    if is_start {
      preview.set_start(value);
    } else {
      preview.set_end(value);
    }
    let preview = self.snap_and_clamp(preview);
    if self.drag_value != Some(preview) {
      self.drag_value = Some(preview);
      cx.emit(SliderEvent::Preview(preview));
      cx.notify();
    }
  }

  /// Ends a drag gesture, committing the preview in non-immediate mode.
  fn end_drag(&mut self, cx: &mut Context<Self>) {
    if !self.dragging {
      return;
    }
    self.dragging = false;
    if let Some(preview) = self.drag_value.take() {
      self.value = preview;
      self.sync_percentage();
      cx.emit(SliderEvent::Commit(self.value));
    }
    cx.notify();
  }
}

/// Geometry and colour of one thumb pill, shared by the real thumbs and the
/// thumb a deferred drag previews.
#[derive(Clone, Copy)]
struct ThumbPill {
  main: Pixels,
  cross: Pixels,
  radius: gpui::Rems,
  color: gpui::Hsla,
}

/// What one span of the rail means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrackSegment {
  /// Nothing fills it: the rail's own recess colour.
  Track,
  /// From the origin to the committed thumb.
  Primary,
  /// From the committed thumb to the thumb a non-immediate drag is previewing.
  Warning,
}

/// Splits the track into the spans the painter draws, as
/// `(from, to, kind)` percentages in ascending order.
///
/// The three interesting positions — the origin, the committed thumb and the
/// dragged thumb — have no fixed order: an origin at the middle can sit above
/// or below either thumb, and a drag can move backwards past the committed
/// value. The spans are therefore derived by sorting the boundaries and
/// classifying each interval by what it lies between, never by drawing them in
/// a fixed sequence.
fn track_segments(origin: f32, actual: f32, drag: Option<f32>) -> Vec<(f32, f32, TrackSegment)> {
  let mut boundaries = vec![0.0_f32, 1.0, origin, actual];
  if let Some(drag) = drag {
    boundaries.push(drag);
  }
  boundaries.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
  boundaries.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);

  let inside = |value: f32, a: f32, b: f32| {
    let (low, high) = if a <= b { (a, b) } else { (b, a) };
    value >= low - f32::EPSILON && value <= high + f32::EPSILON
  };

  let mut segments: Vec<(f32, f32, TrackSegment)> = Vec::new();
  for pair in boundaries.windows(2) {
    let (from, to) = (pair[0], pair[1]);
    if to - from <= f32::EPSILON {
      continue;
    }
    let middle = (from + to) * 0.5;
    // The warning span wins where both could claim an interval: a drag towards
    // an end origin lands inside the primary span, and the pending value must
    // stay visible there (it is also painted last, on top).
    let kind = if drag.is_some_and(|drag| inside(middle, actual, drag)) {
      TrackSegment::Warning
    } else if inside(middle, origin, actual) {
      TrackSegment::Primary
    } else {
      continue;
    };
    // Splitting at the origin would otherwise leave a hairline seam between two
    // spans of the same kind.
    match segments.last_mut() {
      Some((_, previous_to, previous_kind))
        if *previous_kind == kind && (*previous_to - from).abs() < f32::EPSILON =>
      {
        *previous_to = to;
      }
      _ => segments.push((from, to, kind)),
    }
  }
  segments
}

/// Which thumb of the slider one pill is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ThumbKind {
  /// The low end of a range.
  RangeStart,
  /// The committed value.
  Value,
  /// The value a non-immediate drag is previewing.
  Pending,
}

/// One child of the rail, in flow order.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RailPart {
  /// A span of the travel range, as fractions of it.
  Span {
    from: f32,
    to: f32,
    kind: TrackSegment,
  },
  /// The zero-width slot a thumb hangs off, at a fraction of the travel range.
  Thumb { at: f32 },
}

/// The rail's children, in flow order, with `thumbs` sorted by position.
///
/// Every boundary is a cut, so each span carries one colour: the origin, the
/// committed value and the dragged value split the range between them, and the
/// gaps between the painted spans become spans of their own, coloured plain
/// track. That is also what handles an origin inside the track — the fill is
/// then two spans rather than one.
///
/// Thumbs are cut in as well, and that is the whole point: a thumb is placed by
/// standing between two spans, so the flow puts it on its value and nothing has
/// to be computed for it. The cut is what makes this work for a value with
/// nothing to fill (a middle origin and a value in the middle, where the
/// segments have no boundary at all) as much as for the usual case.
///
/// Each thumb is flushed *before* the span that starts at its position, since
/// that is where it stands, and the ones left over — at the maximum — are
/// appended.
fn rail_parts(segments: &[(f32, f32, TrackSegment)], thumbs: &[f32]) -> Vec<RailPart> {
  let mut boundaries = vec![0.0_f32, 1.0];
  for &(from, to, _) in segments {
    boundaries.push(from.clamp(0.0, 1.0));
    boundaries.push(to.clamp(0.0, 1.0));
  }
  boundaries.extend(thumbs.iter().map(|at| at.clamp(0.0, 1.0)));
  boundaries.sort_by(f32::total_cmp);
  boundaries.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);

  // One span per interval, coloured by the painted span it lies in.
  let mut spans: Vec<(f32, f32, TrackSegment)> = Vec::with_capacity(boundaries.len());
  for pair in boundaries.windows(2) {
    let (from, to) = (pair[0], pair[1]);
    if to - from <= f32::EPSILON {
      continue;
    }
    let middle = (from + to) * 0.5;
    let kind = segments
      .iter()
      .find(|(segment_from, segment_to, _)| middle >= *segment_from && middle <= *segment_to)
      .map(|(_, _, kind)| *kind)
      .unwrap_or(TrackSegment::Track);
    spans.push((from, to, kind));
  }

  let mut parts = Vec::with_capacity(spans.len() + thumbs.len());
  let mut pending = thumbs.iter().copied().peekable();
  for (from, to, kind) in spans {
    while let Some(at) = pending.next_if(|at| *at <= from + f32::EPSILON) {
      parts.push(RailPart::Thumb { at });
    }
    parts.push(RailPart::Span { from, to, kind });
  }
  // A thumb at the maximum has no span to sit in front of.
  parts.extend(pending.map(|at| RailPart::Thumb { at }));
  parts
}

/// One span of the rail, as a fixed pixel length.
///
/// Nothing here is proportional: the rail's width is known once it has been
/// laid out, so every length on it is arithmetic done up front (see
/// [`rail_layout`]) and the span only has to be that wide.
fn rail_span(axis: Axis, length: Pixels, thickness: Rems, color: Hsla) -> Div {
  div()
    .flex_shrink_0()
    .bg(color)
    .when(matches!(axis, Axis::Horizontal), |this| {
      this.w(length).h(thickness)
    })
    .when(matches!(axis, Axis::Vertical), |this| {
      this.h(length).w(thickness)
    })
}

/// The fixed end of the rail in front of (or behind) the travel range.
///
/// A thumb's centre stops half its cross size short of the rail's end, so the
/// stub is exactly that half — a constant width, which is what keeps a thumb
/// growing on hover from moving anything else on the rail. Colouring it by the
/// span it continues is what keeps a thumb dragged to an extreme from leaving a
/// bare notch beside itself, and rounding only its outer corners keeps the rail
/// reading as one pill even though each end is a child of its own.
fn rail_stub(
  axis: Axis, length: Pixels, thickness: Rems, radius: Rems, color: Hsla, leading: bool,
) -> Div {
  let stub = div().flex_shrink_0().bg(color);
  let stub = match axis {
    Axis::Horizontal => stub.w(length).h(thickness),
    Axis::Vertical => stub.h(length).w(thickness),
  };
  match (leading, matches!(axis, Axis::Horizontal)) {
    (true, true) => stub.rounded_l(radius),
    (false, true) => stub.rounded_r(radius),
    (true, false) => stub.rounded_b(radius),
    (false, false) => stub.rounded_t(radius),
  }
}

/// The zero-width slot a thumb hangs off.
///
/// A thumb takes no room in the rail's flow: it is a slot standing where its
/// value is, as tall (or as wide) as the rail, with the pill overflowing either
/// side of it by half its own width. So the flow places the thumb exactly the
/// way it places the spans — no offset is computed for it anywhere — and two
/// values that land on one position put two pills in one place.
fn thumb_slot(axis: Axis, cross: Pixels) -> Div {
  match axis {
    Axis::Horizontal => div().flex_shrink_0().w(px(0.)).h(cross),
    Axis::Vertical => div().flex_shrink_0().w(cross).h(px(0.)),
  }
}

/// Centres a pill on the slot it hangs off.
///
/// The slot stands on the value's position, so the pill — a child of the slot —
/// would otherwise start at its leading corner. Pulling it back by half its own
/// width is what makes the slot the pill's centre: the only length involved,
/// and it is half the pill's own size rather than anything derived from the
/// value.
fn centre_on_slot<T: Styled>(axis: Axis, pill: T, main: Pixels) -> T {
  match axis {
    Axis::Horizontal => pill.ml(-main / 2.0),
    Axis::Vertical => pill.mt(-main / 2.0),
  }
}

/// The rail's children with the pixel length each of them takes.
struct RailLayout {
  /// The stub in front of the travel range, half a thumb's cross size wide.
  lead: Pixels,
  /// One length per [`RailPart`], in the same order.
  widths: Vec<Pixels>,
  /// The stub behind the travel range: what the subtraction left over.
  trail: Pixels,
}

/// Lays the rail out in pixels, given the width it ended up with.
///
/// Every length is a share of the travel range — the rail less half a thumb's
/// cross size at either end — so the spans add up to it exactly and a thumb,
/// being a zero-width slot, adds nothing. That is the whole arithmetic: no
/// position is computed, and the flow places the thumbs where the spans meet.
fn rail_layout(parts: &[RailPart], rail: Pixels, inset: Pixels) -> RailLayout {
  let travel = (rail - inset * 2.0).max(px(0.));
  let lead = inset.min(rail);

  let widths: Vec<Pixels> = parts
    .iter()
    .map(|part| match part {
      RailPart::Span { from, to, .. } => (travel * (to - from).max(0.0)).min(rail),
      // A thumb is a slot, not a span: it takes no room, and the pill that
      // hangs off it is drawn on top of the rail instead of being laid out in
      // it.
      RailPart::Thumb { .. } => px(0.),
    })
    .collect();

  let trail = (rail - lead - widths.iter().copied().fold(px(0.), |sum, w| sum + w)).max(px(0.));

  RailLayout {
    lead,
    widths,
    trail,
  }
}

/// The track positions of the ticks, as `(percentage, is_major)`.
///
/// A step that would put more than [`MAX_TICKS`] marks on the track falls back
/// to the long ones only, and disappears entirely when even those are too
/// dense: a slider is a control, not a ruler.
fn tick_positions(
  min: f32, max: f32, step: f32, major_ratio: u32, to_percentage: impl Fn(f32) -> f32,
) -> Vec<(f32, bool)> {
  if step <= 0.0 || max <= min {
    return Vec::new();
  }
  let ratio = major_ratio.max(1);
  let count = ((max - min) / step).floor() as u32;
  if count == 0 {
    return Vec::new();
  }

  let stride = if count > MAX_TICKS {
    if count / ratio > MAX_TICKS {
      return Vec::new();
    }
    ratio
  } else {
    1
  };

  (0..=count)
    .step_by(stride as usize)
    .map(|index| {
      let value = (min + index as f32 * step).min(max);
      (to_percentage(value), index % ratio == 0)
    })
    .collect()
}

impl EventEmitter<SliderEvent> for SliderState {}

#[derive(IntoElement)]
pub struct Slider {
  id: SharedString,
  state: Entity<SliderState>,
  style: StyleRefinement,
  size: Size,
  disabled: bool,
  axis: Axis,
}

impl Slider {
  pub fn new(id: impl Into<SharedString>, state: &Entity<SliderState>) -> Self {
    Self {
      id: id.into(),
      state: state.clone(),
      style: StyleRefinement::default(),
      size: Size::default(),
      disabled: false,
      axis: Axis::Horizontal,
    }
  }

  pub fn horizontal(mut self) -> Self {
    self.axis = Axis::Horizontal;
    self
  }

  pub fn vertical(mut self) -> Self {
    self.axis = Axis::Vertical;
    self
  }

  /// The pill a thumb or a drag preview is drawn as, without any interaction.
  ///
  /// The pill is a flex child of the rail, between the spans on either side of
  /// it, exactly like the spans themselves: nothing here knows where the thumb
  /// is, so nothing here can disagree with the track's colouring.
  fn thumb_pill(&self, axis: Axis, pill: ThumbPill) -> Div {
    let ThumbPill {
      main,
      cross,
      radius,
      color,
    } = pill;
    div()
      .flex_shrink_0()
      .rounded(radius)
      .bg(color)
      .when(matches!(axis, Axis::Horizontal), |this| {
        this.w(main).h(cross)
      })
      .when(matches!(axis, Axis::Vertical), |this| this.w(cross).h(main))
  }

  /// A draggable thumb: the pill plus the gesture that moves it.
  ///
  /// `is_start` only selects the element id and which thumb the drag events
  /// belong to; the geometry and colour arrive pre-built in `pill`, because the
  /// rail needs them before it lays the thumbs out.
  fn thumb(
    &self, axis: Axis, pill: ThumbPill, is_start: bool, state: Entity<SliderState>,
  ) -> Stateful<Div> {
    let thumb_id = if is_start {
      "slider-thumb-start"
    } else {
      "slider-thumb-end"
    };
    let entity_id = state.entity_id();
    self
      .thumb_pill(axis, pill)
      .id((thumb_id, entity_id.as_u64()))
      .when(!self.disabled, |this| {
        this
          .cursor_pointer()
          .on_mouse_down(MouseButton::Left, {
            let state = state.clone();
            move |_, _, cx| {
              cx.stop_propagation();
              state.update(cx, |state, cx| {
                state.dragging = true;
                cx.notify();
              });
            }
          })
          .on_drag(DragThumb((entity_id, is_start)), |drag, _, _, cx| {
            cx.stop_propagation();
            cx.new(|_| drag.clone())
          })
          .on_drag_move({
            move |e: &DragMoveEvent<DragThumb>, _, cx| {
              let DragThumb((id, drag_is_start)) = e.drag(cx).clone();
              if id != entity_id || drag_is_start != is_start {
                return;
              }

              let position = e.event.position;
              state.update(cx, |state, cx| {
                state.update_by_position(axis, position, is_start, cx);
              });
            }
          })
      })
  }
}

impl_disableable!(Slider);
impl_sizable!(Slider);
impl_styled!(Slider);

impl RenderOnce for Slider {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // `Range` is not `Copy`, so clone the (8-byte) value out of the state. The
    // rail's measured length comes out with everything else, because the whole
    // layout is arithmetic on it.
    let (percentage, is_range, active_thumb_start, origin_pct, drag_pct, tick_marks, rail, travel) = {
      let state = self.state.read(cx);
      let drag_pct = match state.drag_value {
        Some(value) if !state.value.is_range() => Some(state.value_to_percentage(value.start())),
        _ => None,
      };
      let tick_marks = if state.ticks {
        tick_positions(
          state.min,
          state.max,
          state.step,
          state.major_tick_ratio,
          |value| state.value_to_percentage(value),
        )
      } else {
        Vec::new()
      };
      let rail = match self.axis {
        Axis::Horizontal => state.bounds.size.width,
        Axis::Vertical => state.bounds.size.height,
      };
      (
        state.percentage.clone(),
        state.value.is_range(),
        state.active_thumb_start,
        state.origin_percentage(),
        drag_pct,
        tick_marks,
        rail,
        state.travel_length(self.axis),
      )
    };

    let axis = self.axis;
    let entity_id = self.state.entity_id();
    let rem = window.rem_size();

    // The hover flag that drives the thumb's grow-and-brighten transition;
    // fed by the container's hover listener.
    let hover_state =
      window.use_keyed_state(("slider-thumb-hovered", entity_id.as_u64()), cx, |_, _| {
        false
      });
    // One flag drives every thumb, so range thumbs grow together, and a pressed
    // thumb stays expanded for the whole gesture even when the pointer leaves
    // the rail.
    let hover_t = transition(
      (("slider-thumb-hovered", entity_id.as_u64()), "t"),
      if *hover_state.read(cx) || self.state.read(cx).dragging {
        1.0
      } else {
        0.0
      },
      Transition::new(duration::THUMB_HOVER).easing(Easing::EaseOut),
      window,
      cx,
    );

    // The pill's cross size is one em; along the rail it is 0.25em at rest and
    // 0.5em on hover.
    let thumb_cross = self.size.thumb_size().to_pixels(rem);
    let thumb_main_rest = self.size.em(0.25).to_pixels(rem);
    let thumb_main_hover = self.size.em(0.5).to_pixels(rem);
    let thumb_main = thumb_main_rest + (thumb_main_hover - thumb_main_rest) * hover_t;
    let thumb_opacity = 0.6 + 0.2 * hover_t;
    // A thumb's centre stops half its cross size short of the rail's ends, so
    // that is where the travel range starts and ends.
    let travel_inset = thumb_cross / 2.0;

    // The pill every thumb is drawn as. Built once and shared, so a previewed
    // thumb is indistinguishable from the real one apart from its colour — and
    // in particular is the same width, since the rail's layout is arithmetic on
    // the thumb's width and a rail mixing two widths would place its spans
    // around a thumb that is not there.
    let pill = ThumbPill {
      main: thumb_main,
      cross: thumb_cross,
      radius: self.size.em(0.125),
      // The control's own neutral with 20% of the primary colour, so the
      // thumb belongs to the track it fills without matching it exactly.
      color: cx
        .theme()
        .foreground
        .blend(cx.theme().primary.opacity(0.2))
        .opacity(thumb_opacity),
    };

    // The pill a pending value is previewed as: the same geometry, tinted so it
    // reads as a thumb rather than as a different kind of mark.
    let pending_pill = ThumbPill {
      color: cx
        .theme()
        .foreground
        .blend(cx.theme().warning.opacity(0.2))
        .opacity(0.8),
      ..pill
    };

    // The value a non-immediate drag is previewing, if it has one to show: a
    // pending value that equals the committed one would only repaint the same
    // pill, so it is not drawn at all.
    let preview_pct = drag_pct.filter(|pct| *pct != percentage.end);

    // Primary runs from the origin to the committed thumb; a dragging thumb in
    // non-immediate mode adds the warning span between the two. A range fills
    // between its thumbs instead.
    let segments = if is_range {
      vec![(percentage.start, percentage.end, TrackSegment::Primary)]
    } else {
      track_segments(origin_pct, percentage.end, drag_pct)
    };
    // The thumbs in flow order. A later child draws over an earlier one, so the
    // pending thumb comes last and a stable sort keeps it on top when two
    // thumbs land on the same position.
    let mut thumb_kinds: Vec<(f32, ThumbKind)> = Vec::with_capacity(3);
    if is_range {
      thumb_kinds.push((percentage.start, ThumbKind::RangeStart));
    }
    thumb_kinds.push((percentage.end, ThumbKind::Value));
    if let Some(pct) = preview_pct {
      thumb_kinds.push((pct, ThumbKind::Pending));
    }
    thumb_kinds.sort_by(|a, b| a.0.total_cmp(&b.0));
    let thumb_positions: Vec<f32> = thumb_kinds.iter().map(|(at, _)| *at).collect();

    // The rail's spans, with a slot for each thumb cut in between them: that is
    // what puts a thumb where its value is, by the flow and not by an offset.
    let parts = rail_parts(&segments, &thumb_positions);

    // The end stubs continue the span next to them, so a value at an extreme
    // does not leave a bare notch beside the thumb.
    let span_kind = |part: &RailPart| match part {
      RailPart::Span { kind, .. } => Some(*kind),
      RailPart::Thumb { .. } => None,
    };
    let lead_kind = parts
      .iter()
      .find_map(span_kind)
      .unwrap_or(TrackSegment::Track);
    let tail_kind = parts
      .iter()
      .rev()
      .find_map(span_kind)
      .unwrap_or(TrackSegment::Track);

    // The pills, each hung off its slot. A pill overflows its slot by half its
    // own width on either side, so two values close enough share their space
    // instead of being pushed apart. It is deferred so that it is painted after
    // the rail: a sibling laid out before the track would be painted over by
    // the span on its other side — a question of paint order, not of position.
    let pills: Vec<AnyElement> = thumb_kinds
      .iter()
      .map(|(_, kind)| {
        let pill: AnyElement = match kind {
          ThumbKind::Pending => {
            centre_on_slot(axis, self.thumb_pill(axis, pending_pill), thumb_main).into_any_element()
          }
          ThumbKind::RangeStart => centre_on_slot(
            axis,
            self.thumb(axis, pill, true, self.state.clone()),
            thumb_main,
          )
          .into_any_element(),
          ThumbKind::Value => centre_on_slot(
            axis,
            self.thumb(axis, pill, false, self.state.clone()),
            thumb_main,
          )
          .into_any_element(),
        };
        deferred(thumb_slot(axis, thumb_cross).child(pill)).into_any_element()
      })
      .collect();

    let primary = cx.theme().primary;
    let warning = cx.theme().warning;
    let track = crate::track_color(cx.theme());
    let color_of = |kind: TrackSegment| match kind {
      TrackSegment::Track => track,
      TrackSegment::Primary => primary,
      TrackSegment::Warning => warning,
    };

    let thickness = self.size.track_thickness();
    // Half the track's thickness: the rail's ends are rounded as one pill,
    // which is what the two end stubs' outer corners carry.
    let radius = rems(thickness.0 * 0.5);

    let state_for_down = self.state.clone();
    let drag_start = active_thumb_start;

    // The rail's children: the two end stubs and the spans between them, all
    // flex children of a known width, and then the thumbs. Until the rail has
    // been measured there is no width to divide up, so it draws nothing rather
    // than laying itself out against a guess and shifting on the next frame.
    let mut children: Vec<AnyElement> = Vec::new();
    if rail > px(0.) && travel > px(0.) {
      // Ticks are the one part that is not a span: they mark positions instead
      // of occupying them, so they overlay the rail, and they are painted
      // before the spans so the fill reads over them.
      children.push(
        div()
          .absolute()
          .inset_0()
          .children({
            let minor = self.size.em(TICK_HEIGHT_EM).to_pixels(rem);
            let major = self.size.em(MAJOR_TICK_HEIGHT_EM).to_pixels(rem);
            let border = cx.theme().muted_foreground.opacity(0.45);
            tick_marks
              .into_iter()
              .map(|(pct, is_major)| {
                let height = if is_major { major } else { minor };
                // A tick marks a position on the travel range, which is the
                // rail less half a thumb's cross size at either end.
                let offset = travel_inset * (1.0 - 2.0 * pct);
                div()
                  .absolute()
                  .w(px(TICK_WIDTH_PX))
                  .h(height)
                  .bg(border)
                  .when(matches!(axis, Axis::Horizontal), |this| {
                    this
                      .left(relative(pct))
                      .ml(offset - px(TICK_WIDTH_PX / 2.0))
                      .top(relative(0.5))
                      .mt(-height / 2.0)
                  })
                  .when(matches!(axis, Axis::Vertical), |this| {
                    this
                      .bottom(relative(pct))
                      .mb(offset - px(TICK_WIDTH_PX / 2.0))
                      .left(relative(0.5))
                      .ml(-height / 2.0)
                      .w(height)
                      .h(px(TICK_WIDTH_PX))
                  })
                  .into_any_element()
              })
              .collect::<Vec<_>>()
          })
          .into_any_element(),
      );

      let layout = rail_layout(&parts, rail, travel_inset);
      let mut pills = pills.into_iter();

      children.push(
        rail_stub(
          axis,
          layout.lead,
          thickness,
          radius,
          color_of(lead_kind),
          true,
        )
        .into_any_element(),
      );
      for (part, width) in parts.iter().zip(&layout.widths) {
        match part {
          RailPart::Span { kind, .. } => {
            children.push(rail_span(axis, *width, thickness, color_of(*kind)).into_any_element());
          }
          RailPart::Thumb { .. } => {
            if let Some(pill) = pills.next() {
              children.push(pill);
            }
          }
        }
      }
      children.push(
        rail_stub(
          axis,
          layout.trail,
          thickness,
          radius,
          color_of(tail_kind),
          false,
        )
        .into_any_element(),
      );
    }

    h_flex()
      .id(self.id)
      .when(matches!(axis, Axis::Horizontal), |this| {
        this.h(self.size.component_height()).w_full()
      })
      .when(matches!(axis, Axis::Vertical), |this| {
        this.w(self.size.component_height()).h(rems(8.))
      })
      .items_center()
      .justify_center()
      .when(!self.disabled, |this| {
        this
          .on_hover({
            let hover_state = hover_state.clone();
            move |hovered, _, cx| {
              hover_state.update(cx, |state, cx| {
                if *state != *hovered {
                  *state = *hovered;
                  cx.notify();
                }
              });
            }
          })
          // End the drag gesture wherever the pointer is released.
          .on_mouse_up(MouseButton::Left, {
            let state = self.state.clone();
            move |_, _, cx| {
              state.update(cx, |state, cx| state.end_drag(cx));
            }
          })
          .on_mouse_up_out(MouseButton::Left, {
            let state = self.state.clone();
            move |_, _, cx| {
              state.update(cx, |state, cx| state.end_drag(cx));
            }
          })
          // Safety net: if the release itself was missed (e.g. the pointer
          // was released outside the window), clear the gesture on the next
          // unpressed move over the slider.
          .on_mouse_move({
            let state = self.state.clone();
            move |e: &MouseMoveEvent, _, cx| {
              if e.pressed_button.is_none() && state.read(cx).dragging {
                state.update(cx, |state, cx| state.end_drag(cx));
              }
            }
          })
      })
      .child(
        // The control is a band one em tall, not a hairline: the pointer has to
        // be able to land on it. It is also the hit target for pressing
        // anywhere the thumb is not — pressing a thumb stops the event before
        // it gets here — so the band needs no separate trigger layer.
        div()
          .id(("slider-track", entity_id.as_u64()))
          .when(matches!(axis, Axis::Horizontal), |this| {
            this.h(self.size.thumb_size()).w_full()
          })
          .when(matches!(axis, Axis::Vertical), |this| {
            this.w(self.size.thumb_size()).h_full()
          })
          .flex()
          // Reversed on the vertical axis so the first child is at the bottom:
          // the flow order then means the same thing on both axes, a fraction
          // measured from the minimum.
          .when(matches!(axis, Axis::Horizontal), |this| {
            this.flex_row().items_center()
          })
          .when(matches!(axis, Axis::Vertical), |this| {
            this.flex_col_reverse().items_center()
          })
          .when(!self.disabled, |this| this.cursor_pointer())
          .on_prepaint({
            let state = self.state.clone();
            move |bounds, window, cx| {
              state.update(cx, |state, _cx| {
                // The rail lays its children out from this width, so a rail
                // that has just been measured (or resized) has to be rebuilt
                // once, and a draw-phase invalidation cannot dirty the window —
                // the frame has to be asked for explicitly. Only a real change
                // asks, so an unchanged frame does not request its own
                // successor.
                let changed =
                  state.bounds.size != bounds.size || state.travel_inset != travel_inset;
                state.set_geometry(bounds, travel_inset);
                if changed {
                  window.request_animation_frame();
                }
              });
            }
          })
          .when(!self.disabled, |this| {
            this
              .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, cx| {
                state_for_down.update(cx, |state, cx| {
                  state.dragging = true;
                  state.choose_active_thumb(axis, e.position);
                  state.update_by_position(axis, e.position, state.active_thumb_start, cx);
                  // Notify even when the value did not change: the rebuilt
                  // element must carry the fresh `active_thumb_start` for the
                  // drag below.
                  cx.notify();
                });
              })
              .on_drag(DragThumb((entity_id, drag_start)), |drag, _, _, cx| {
                cx.stop_propagation();
                cx.new(|_| drag.clone())
              })
              .on_drag_move({
                let state = self.state.clone();
                move |e: &DragMoveEvent<DragThumb>, _, cx| {
                  let DragThumb((id, is_start)) = e.drag(cx).clone();
                  if id != entity_id {
                    return;
                  }
                  let position = e.event.position;
                  state.update(cx, |state, cx| {
                    state.update_by_position(axis, position, is_start, cx);
                  });
                }
              })
          })
          .children(children),
      )
      .opacity(if self.disabled {
        opacity::DISABLED
      } else {
        1.0
      })
      .refine_style(&self.style)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn kinds(segments: &[(f32, f32, TrackSegment)]) -> Vec<TrackSegment> {
    segments.iter().map(|(_, _, kind)| *kind).collect()
  }

  #[test]
  fn primary_runs_from_the_origin_to_the_thumb() {
    let segments = track_segments(0.0, 0.75, None);
    assert_eq!(segments.len(), 1);
    assert_eq!(kinds(&segments), vec![TrackSegment::Primary]);
    assert_eq!((segments[0].0, segments[0].1), (0.0, 0.75));
  }

  /// The colour of the padding stub at one end of the track.
  ///
  /// Padding takes the colour of whatever span touches it: a fill that reaches
  /// an end means the thumb has run to the extreme, and the stub beside it has
  /// to continue the fill or the slider would show a track-coloured notch at
  /// the very position the user dragged to.
  fn end_colors(
    segments: &[(f32, f32, TrackSegment)],
  ) -> (Option<TrackSegment>, Option<TrackSegment>) {
    let leading = segments
      .iter()
      .find(|(from, ..)| *from <= f32::EPSILON)
      .map(|(_, _, kind)| *kind);
    let trailing = segments
      .iter()
      .find(|(_, to, _)| *to >= 1.0 - f32::EPSILON)
      .map(|(_, _, kind)| *kind);
    (leading, trailing)
  }

  #[test]
  fn padding_continues_the_fill_at_both_extremes() {
    // Dragged to the maximum: the fill reaches 1.0, so the trailing padding is
    // filled too and no track colour shows at the end.
    let at_max = track_segments(0.0, 1.0, None);
    assert_eq!(
      end_colors(&at_max),
      (Some(TrackSegment::Primary), Some(TrackSegment::Primary))
    );

    // Dragged to the minimum with a start origin: the fill collapses, so both
    // stubs stay track-coloured.
    let at_min = track_segments(0.0, 0.0, None);
    assert_eq!(end_colors(&at_min), (None, None));
  }

  #[test]
  fn padding_keeps_the_track_colour_when_the_origin_is_inside() {
    // A mid-track origin must not paint the padding: the ends of the track are
    // outside the fill, so they stay track-coloured.
    let middle = track_segments(0.5, 0.75, None);
    assert_eq!(end_colors(&middle), (None, None));

    // An end origin fills backwards towards the thumb: the fill touches 1.0, so
    // the trailing stub is filled and the leading one is not.
    let end_origin = track_segments(1.0, 0.25, None);
    assert_eq!(end_colors(&end_origin), (None, Some(TrackSegment::Primary)));
  }

  #[test]
  fn a_middle_origin_fills_towards_the_thumb_in_both_directions() {
    let above = track_segments(0.5, 0.8, None);
    assert_eq!((above[0].0, above[0].1), (0.5, 0.8));

    let below = track_segments(0.5, 0.2, None);
    assert_eq!((below[0].0, below[0].1), (0.2, 0.5));
  }

  #[test]
  fn an_end_origin_fills_backwards() {
    let segments = track_segments(1.0, 0.3, None);
    assert_eq!((segments[0].0, segments[0].1), (0.3, 1.0));
  }

  #[test]
  fn a_dragged_thumb_adds_the_warning_span_between_the_two() {
    let segments = track_segments(0.0, 0.4, Some(0.7));
    assert_eq!(
      segments,
      vec![
        (0.0, 0.4, TrackSegment::Primary),
        (0.4, 0.7, TrackSegment::Warning)
      ]
    );
  }

  #[test]
  fn the_spans_follow_the_positions_not_a_fixed_order() {
    // Dragging backwards past the committed thumb: the warning sits below the
    // primary instead of above it.
    // Dragging back past the origin: the pending span covers the primary one,
    // so the warning stays one merged span painted over it.
    let backwards = track_segments(0.5, 0.6, Some(0.2));
    assert_eq!(backwards, vec![(0.2, 0.6, TrackSegment::Warning)]);

    // An end origin with a dragged thumb beyond it keeps the primary on the
    // side of the committed value.
    let end_origin = track_segments(1.0, 0.8, Some(0.9));
    assert_eq!(
      end_origin,
      vec![
        (0.8, 0.9, TrackSegment::Warning),
        (0.9, 1.0, TrackSegment::Primary)
      ]
    );
  }

  #[test]
  fn ticks_are_placed_at_every_step_with_majors_on_the_ratio() {
    let ticks = tick_positions(0.0, 10.0, 1.0, 5, |value| value / 10.0);
    assert_eq!(ticks.len(), 11);
    let majors: Vec<f32> = ticks
      .iter()
      .filter(|(_, major)| *major)
      .map(|(pct, _)| *pct)
      .collect();
    assert_eq!(majors, vec![0.0, 0.5, 1.0]);
  }

  #[test]
  fn a_dense_step_falls_back_to_major_ticks_only() {
    let ticks = tick_positions(0.0, 1000.0, 1.0, 5, |value| value / 1000.0);
    assert!(!ticks.is_empty());
    assert_eq!(ticks.len(), 201);
    assert!(ticks.iter().all(|(_, major)| *major));

    // Nothing readable even at the major stride: draw none at all.
    let hopeless = tick_positions(0.0, 100_000.0, 1.0, 5, |value| value / 100_000.0);
    assert!(hopeless.is_empty());
  }

  #[test]
  fn degenerate_bounds_draw_no_ticks() {
    assert!(tick_positions(0.0, 0.0, 1.0, 5, |_| 0.0).is_empty());
    assert!(tick_positions(0.0, 10.0, 0.0, 5, |_| 0.0).is_empty());
  }

  #[test]
  fn origins_map_to_track_fractions() {
    assert_eq!(SliderOrigin::Start.percentage(), 0.0);
    assert_eq!(SliderOrigin::Middle.percentage(), 0.5);
    assert_eq!(SliderOrigin::End.percentage(), 1.0);
    assert_eq!(SliderOrigin::default(), SliderOrigin::Start);
  }

  #[test]
  fn a_drag_preview_survives_an_external_value_update() {
    let mut state = SliderState::new()
      .min(0.0)
      .max(100.0)
      .step(1.0)
      .with_immediate(false);
    state.dragging = true;
    state.drag_value = Some(SliderValue::Single(80.0));
    // A player reporting its own position must not cancel the user's drag.
    state.value = SliderValue::Single(12.0);
    state.sync_percentage();
    assert_eq!(state.drag_value, Some(SliderValue::Single(80.0)));
    assert_eq!(state.value, SliderValue::Single(12.0));
  }

  /// A rail of 268px with a 16px cross and a 4px thumb: the numbers a medium
  /// slider in a 300px window ends up with.
  const RAIL: f32 = 268.0;
  const INSET: f32 = 8.0;
  const MAIN: f32 = 4.0;

  fn layout(segments: &[(f32, f32, TrackSegment)], thumbs: &[f32]) -> (Vec<RailPart>, RailLayout) {
    let parts = rail_parts(segments, thumbs);
    let layout = rail_layout(&parts, px(RAIL), px(INSET));
    (parts, layout)
  }

  /// Where each thumb's centre lands, walked through the flow: a thumb is a
  /// zero-width slot, so its centre is simply the position the flow gave it.
  fn thumb_centres(parts: &[RailPart], layout: &RailLayout) -> Vec<f32> {
    let mut cursor: f32 = layout.lead.into();
    let mut centres = Vec::new();
    for (part, width) in parts.iter().zip(&layout.widths) {
      if matches!(part, RailPart::Thumb { .. }) {
        centres.push(cursor);
      }
      cursor += f32::from(*width);
    }
    centres
  }

  /// The rail itself — both stubs and every span — covers its width exactly.
  fn assert_rail_fills_the_width(parts: &[RailPart], layout: &RailLayout) {
    let total: f32 = f32::from(layout.lead)
      + layout.widths.iter().map(|w| f32::from(*w)).sum::<f32>()
      + f32::from(layout.trail);
    assert!(
      (total - RAIL).abs() < 0.01,
      "the spans and the two stubs must add up to the rail: {total}"
    );
    assert_eq!(parts.len(), layout.widths.len());
    // A thumb is a slot and takes nothing: only the spans carry length.
    for (part, width) in parts.iter().zip(&layout.widths) {
      if matches!(part, RailPart::Thumb { .. }) {
        assert_eq!(f32::from(*width), 0.0);
      }
    }
  }

  #[test]
  fn spans_cover_the_travel_range_without_a_seam() {
    // The origin cuts the fill in two, so a middle origin yields three spans
    // around the thumb, and the widths add up to the travel range exactly.
    let (parts, layout) = layout(&track_segments(0.5, 0.8, None), &[0.8]);
    let spans: Vec<_> = parts
      .iter()
      .filter_map(|part| match part {
        RailPart::Span { from, to, kind } => Some((*from, *to, *kind)),
        RailPart::Thumb { .. } => None,
      })
      .collect();
    assert_eq!(
      spans,
      vec![
        (0.0, 0.5, TrackSegment::Track),
        (0.5, 0.8, TrackSegment::Primary),
        (0.8, 1.0, TrackSegment::Track),
      ]
    );
    assert_rail_fills_the_width(&parts, &layout);
    assert!((f32::from(layout.widths[1]) - 0.3 * (RAIL - INSET * 2.0)).abs() < 0.01);
  }

  #[test]
  fn a_dragged_thumb_splits_the_pending_span_out_of_the_fill() {
    // Origin in the middle, committed at 0.4 and pending at 0.7: the pending
    // span runs from the committed thumb to the dragged one and reaches past
    // the origin, so it merges with the fill into one warning span.
    let (parts, _) = layout(&track_segments(0.5, 0.4, Some(0.7)), &[0.4, 0.7]);
    let spans: Vec<_> = parts
      .iter()
      .filter_map(|part| match part {
        RailPart::Span { from, to, kind } => Some((*from, *to, *kind)),
        RailPart::Thumb { .. } => None,
      })
      .collect();
    assert_eq!(
      spans,
      vec![
        (0.0, 0.4, TrackSegment::Track),
        (0.4, 0.7, TrackSegment::Warning),
        (0.7, 1.0, TrackSegment::Track),
      ]
    );
  }

  #[test]
  fn a_thumb_on_a_span_boundary_is_cut_in_before_the_span_it_starts() {
    // A value is a boundary, never the inside of a span: the thumb is a slot
    // between the two spans, so the flow puts it on its value.
    let (parts, _) = layout(&track_segments(0.0, 0.5, None), &[0.5]);
    assert_eq!(
      parts,
      vec![
        RailPart::Span {
          from: 0.0,
          to: 0.5,
          kind: TrackSegment::Primary
        },
        RailPart::Thumb { at: 0.5 },
        RailPart::Span {
          from: 0.5,
          to: 1.0,
          kind: TrackSegment::Track
        },
      ]
    );
  }

  #[test]
  fn a_thumb_with_nothing_to_fill_still_sits_on_its_value() {
    // A middle origin with the value in the middle has an empty fill, so no
    // segment boundary lands on the thumb at all (this was the treble band of
    // the slider example). The cut is made for the thumb itself, so there is
    // nothing left that could push it anywhere else.
    let (parts, layout) = layout(&track_segments(0.5, 0.5, None), &[0.5]);
    let centres = thumb_centres(&parts, &layout);
    assert!((centres[0] - (INSET + 0.5 * (RAIL - INSET * 2.0))).abs() < 0.01);
    assert_rail_fills_the_width(&parts, &layout);
  }

  #[test]
  fn a_thumb_travels_the_rail_less_two_insets_at_any_value() {
    for at in [0.0, 0.25, 0.5, 0.9, 1.0] {
      let (parts, layout) = layout(&track_segments(0.0, at, None), &[at]);
      let centres = thumb_centres(&parts, &layout);
      let expected = INSET + at * (RAIL - INSET * 2.0);
      assert!(
        (centres[0] - expected).abs() < 0.01,
        "at {at}: {} instead of {expected}",
        centres[0]
      );
      assert_rail_fills_the_width(&parts, &layout);
    }
  }

  #[test]
  fn a_thumb_at_an_extreme_stops_half_a_cross_size_short_of_the_end() {
    // The two ends are what a drag to the minimum and the maximum reach: the
    // end stubs are half a thumb's *cross* size, so the pill's centre stops
    // there and its outer half still sits on the rail.
    for (at, centre) in [(0.0, INSET), (1.0, RAIL - INSET)] {
      let (parts, layout) = layout(&track_segments(0.0, at, None), &[at]);
      let centres = thumb_centres(&parts, &layout);
      assert!((centres[0] - centre).abs() < 0.01);
    }
  }

  #[test]
  fn thumbs_cost_the_travel_range_nothing() {
    // A thumb is a zero-width slot, so neither one thumb nor two nor three take
    // a length out of the travel range: the spans together are the travel range
    // either way, which is what makes a range fill exact.
    let travel = RAIL - INSET * 2.0;
    for thumbs in [vec![0.5], vec![0.25, 0.75], vec![0.2, 0.5, 0.8]] {
      let (parts, layout) = layout(&track_segments(0.0, 0.5, None), &thumbs);
      let total: f32 = layout.widths.iter().map(|w| f32::from(*w)).sum();
      assert!((total - travel).abs() < 0.01, "{thumbs:?} took {total}");
      assert_rail_fills_the_width(&parts, &layout);
    }
  }

  #[test]
  fn thumbs_at_the_same_value_overlap_instead_of_sitting_side_by_side() {
    // Two values in the same place are one place: both slots land on it and the
    // pills, being centred on their slots, are exactly on top of each other.
    let (stacked, stacked_layout) = layout(&track_segments(0.0, 0.5, None), &[0.5, 0.5]);
    let centres = thumb_centres(&stacked, &stacked_layout);
    assert_eq!(centres[0], centres[1]);
    assert!((centres[0] - (INSET + 0.5 * (RAIL - INSET * 2.0))).abs() < 0.01);

    // Thumbs whose values are close are close, not spread apart: their pills
    // overlap as much as the distance between them allows.
    let (near, near_layout) = layout(&track_segments(0.0, 0.5, None), &[0.5, 0.51]);
    let centres = thumb_centres(&near, &near_layout);
    let travel = RAIL - INSET * 2.0;
    assert!((centres[1] - centres[0] - 0.01 * travel).abs() < 0.01);
    assert!((centres[1] - centres[0]) < MAIN, "the pills must overlap");
  }

  #[test]
  fn both_thumbs_of_a_range_keep_their_centres() {
    let at = (0.25, 0.75);
    let segments = vec![(at.0, at.1, TrackSegment::Primary)];
    let (parts, layout) = layout(&segments, &[at.0, at.1]);
    let centres = thumb_centres(&parts, &layout);
    assert!((centres[0] - (INSET + at.0 * (RAIL - INSET * 2.0))).abs() < 0.01);
    assert!((centres[1] - (INSET + at.1 * (RAIL - INSET * 2.0))).abs() < 0.01);
    assert_rail_fills_the_width(&parts, &layout);
  }

  #[test]
  fn the_trailing_stub_takes_the_rounding_so_the_rail_never_overflows() {
    // Fractions that do not divide a pixel evenly: the rail still adds up, and
    // the leftover lands in the trailing stub rather than in a seam.
    let at = 1.0 / 3.0;
    let (parts, layout) = layout(&track_segments(0.0, at, None), &[at]);
    assert_rail_fills_the_width(&parts, &layout);
  }
}

#[cfg(test)]
mod render_tests {
  use std::ops::Deref as _;

  use gpui::{
    Context, IntoElement, Render, TestAppContext, VisualTestContext, Window, div, px, size,
  };

  use super::*;

  /// A 300px window with 16px padding leaves the rail 268px wide, starting at
  /// x = 16 and 24px down; the medium thumb is one em (16px) tall, rest 4px
  /// wide and 8px wide while hovered or pressed.
  struct Host {
    state: Entity<SliderState>,
  }

  impl Render for Host {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      div().size_full().p_4().child(Slider::new("s", &self.state))
    }
  }

  struct VerticalHost {
    state: Entity<SliderState>,
  }

  impl Render for VerticalHost {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      div()
        .size_full()
        .p_4()
        .child(Slider::new("v", &self.state).vertical())
    }
  }

  fn open_window_with(cx: &mut TestAppContext, state: Entity<SliderState>) -> VisualTestContext {
    cx.set_global(crate::Theme::default());
    let window = cx.open_window(size(px(300.), px(80.)), move |_, _| Host { state });
    let visual = VisualTestContext::from_window(*window.deref(), cx);
    visual.run_until_parked();
    visual
  }

  /// Open a slider and deliver the frame its first prepaint asked for.
  ///
  /// The rail lays its children out from its measured width, so the frame that
  /// measures it cannot draw them; the prepaint of that frame requests the next
  /// one, which tests have to deliver by hand.
  fn open(cx: &mut TestAppContext, state: Entity<SliderState>) -> VisualTestContext {
    let mut visual = open_window_with(cx, state);
    visual.update(|window, cx| {
      window.simulate_next_frame(cx);
    });
    visual.run_until_parked();
    visual
  }

  fn painted(cx: &mut VisualTestContext) -> (f32, Vec<gpui::Quad>) {
    cx.update(|window, _| (window.scale_factor(), window.painted_quads()))
  }

  /// The one painted quad of a given logical size, as `(left, top, right,
  /// bottom)`.
  fn quad_of_size(
    quads: &[gpui::Quad], width: f32, height: f32, scale: f32,
  ) -> (f32, f32, f32, f32) {
    let quad = quads
      .iter()
      .find(|quad| {
        (quad.bounds.size.width.as_f32() / scale - width).abs() < 0.5
          && (quad.bounds.size.height.as_f32() / scale - height).abs() < 0.5
      })
      .unwrap_or_else(|| panic!("no quad {width}x{height}px"));
    let left = quad.bounds.origin.x.as_f32() / scale;
    let top = quad.bounds.origin.y.as_f32() / scale;
    (left, top, left + width, top + height)
  }

  /// Where the painted rail ends: the right edge of everything on it.
  fn rail_end(quads: &[gpui::Quad], scale: f32) -> f32 {
    quads
      .iter()
      .map(|quad| quad.bounds.right().as_f32() / scale)
      .fold(f32::MIN, f32::max)
  }

  /// The centres of the painted thumbs, left to right: the quads as tall as the
  /// thumb's cross size. Nothing else on the rail is that tall.
  fn painted_thumbs(quads: &[gpui::Quad], scale: f32) -> Vec<f32> {
    let mut centres: Vec<f32> = quads
      .iter()
      .filter(|quad| (quad.bounds.size.height.as_f32() / scale - 16.0).abs() < 0.5)
      .map(|quad| (quad.bounds.origin.x + quad.bounds.size.width / 2.0).as_f32() / scale)
      .collect();
    centres.sort_by(f32::total_cmp);
    centres
  }

  #[gpui::test]
  fn an_unmeasured_rail_draws_nothing(cx: &mut TestAppContext) {
    // Every length on the rail is exact pixels, so the frame that measures it
    // has no width to divide up yet: it draws nothing and asks for the next
    // frame instead of laying itself out against a guess.
    let state = cx.new(|_| SliderState::new().default_value(50.0));
    let mut visual = open_window_with(cx, state);
    assert!(visual.update(|window, _| window.painted_quads().is_empty()));
  }

  #[gpui::test]
  fn the_fill_and_the_thumb_land_where_their_values_are(cx: &mut TestAppContext) {
    // Half a 252px travel range is 126px, measured from 8px in from the rail's
    // start, so a half-way thumb's centre sits at 16 + 8 + 126 = 150.
    let state = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(50.0)
    });
    let mut visual = open(cx, state);
    let (scale, quads) = painted(&mut visual);

    let thumbs = painted_thumbs(&quads, scale);
    assert_eq!(thumbs.len(), 1);
    assert!(
      (thumbs[0] - 150.0).abs() < 0.5,
      "a half-way thumb belongs at 150, not {}",
      thumbs[0]
    );

    let (fill_left, _, fill_right, _) = quad_of_size(&quads, 126.0, 2.0, scale);
    assert!(
      (fill_left - 24.0).abs() < 0.5 && (fill_right - 150.0).abs() < 0.5,
      "the fill belongs between 24 and 150, not {fill_left} and {fill_right}"
    );

    assert!(
      (rail_end(&quads, scale) - 284.0).abs() < 0.5,
      "the rail must reach 284, not {}",
      rail_end(&quads, scale)
    );

    // And the pill is painted after the rail: a slider sibling laid out in the
    // flow would be painted under the span on its other side, which is why the
    // pill's paint is deferred.
    let thumb = quads
      .iter()
      .find(|quad| (quad.bounds.size.height.as_f32() / scale - 16.0).abs() < 0.5)
      .expect("the thumb quad");
    let fill = quads
      .iter()
      .find(|quad| (quad.bounds.size.width.as_f32() / scale - 126.0).abs() < 0.5)
      .expect("the fill quad");
    assert!(
      thumb.order > fill.order,
      "the thumb must paint over the track, not under it"
    );
  }

  #[gpui::test]
  fn a_middle_origin_fills_from_the_middle_back_to_the_thumb(cx: &mut TestAppContext) {
    // A quarter of the way up from the minimum with the origin in the middle:
    // the fill runs from the middle back to the left, so it is 63px of the
    // 252px travel range, and the thumb sits on it at 16 + 8 + 63 = 87.
    let state = cx.new(|_| {
      SliderState::new()
        .min(-12.0)
        .max(12.0)
        .step(1.0)
        .default_value(-6.0)
        .with_origin(SliderOrigin::Middle)
    });
    let mut visual = open(cx, state);
    let (scale, quads) = painted(&mut visual);

    let (fill_left, _, fill_right, _) = quad_of_size(&quads, 63.0, 2.0, scale);
    assert!(
      (fill_left - 24.0).abs() < 0.5 && (fill_right - 87.0).abs() < 0.5,
      "the fill belongs between 24 and 87, not {fill_left} and {fill_right}"
    );
    let thumbs = painted_thumbs(&quads, scale);
    assert!(
      (thumbs[0] - 87.0).abs() < 0.5,
      "the thumb belongs at 87, not {}",
      thumbs[0]
    );
  }

  #[gpui::test]
  fn a_centred_value_leaves_the_thumb_in_the_middle(cx: &mut TestAppContext) {
    // The treble band of the slider example: a middle origin and a value at the
    // middle, so there is no fill at all and the thumb has to stay where the
    // value is instead of drifting to the end of the rail.
    let state = cx.new(|_| {
      SliderState::new()
        .min(-12.0)
        .max(12.0)
        .step(1.0)
        .default_value(0.0)
        .with_origin(SliderOrigin::Middle)
    });
    let mut visual = open(cx, state);
    let (scale, quads) = painted(&mut visual);

    let thumbs = painted_thumbs(&quads, scale);
    assert!(
      (thumbs[0] - 150.0).abs() < 0.5,
      "a value in the middle belongs at 150, not {}",
      thumbs[0]
    );
  }

  #[gpui::test]
  fn a_vertical_slider_measures_upwards_from_its_bottom(cx: &mut TestAppContext) {
    // A vertical slider is eight rems tall, so its rail is 128px from 16 down
    // to 144: the travel range is 112px, and a quarter of it up from the bottom
    // puts the thumb's centre at 144 - 8 - 28 = 108.
    let state = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(25.0)
    });
    cx.set_global(crate::Theme::default());
    let window = cx.open_window(size(px(120.), px(200.)), |_, _| VerticalHost {
      state: state.clone(),
    });
    let mut visual = VisualTestContext::from_window(*window.deref(), cx);
    visual.run_until_parked();
    visual.update(|window, cx| {
      window.simulate_next_frame(cx);
    });
    visual.run_until_parked();

    let (scale, quads) = painted(&mut visual);
    let quad = quads
      .iter()
      .find(|quad| (quad.bounds.size.width.as_f32() / scale - 16.0).abs() < 0.5)
      .expect("the thumb quad");
    let top = quad.bounds.origin.y.as_f32() / scale;
    let centre = top + quad.bounds.size.height.as_f32() / scale / 2.0;
    assert!(
      (centre - 108.0).abs() < 0.5,
      "a quarter-way thumb belongs at 108 from the top, not {centre}"
    );
  }

  #[gpui::test]
  fn ticks_are_marks_under_the_fill_rather_than_spans_on_it(cx: &mut TestAppContext) {
    // Ticks mark positions, so they overlay the rail instead of taking part in
    // its flow — and they are painted before the spans, which is what makes the
    // fill read over them. Both facts are visible in the paint order.
    let state = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(50.0)
        .with_ticks(true)
    });
    let mut visual = open(cx, state);
    let (scale, quads) = painted(&mut visual);

    let fill = quads
      .iter()
      .find(|quad| (quad.bounds.size.width.as_f32() / scale - 126.0).abs() < 0.5)
      .expect("the fill quad");
    let ticks: Vec<&gpui::Quad> = quads
      .iter()
      .filter(|quad| (quad.bounds.size.width.as_f32() / scale - 1.0).abs() < 0.5)
      .collect();
    assert!(!ticks.is_empty(), "the slider was asked for ticks");
    for tick in &ticks {
      assert!(tick.order < fill.order, "a tick must paint under the fill");
      let left = tick.bounds.origin.x.as_f32() / scale;
      let right = tick.bounds.right().as_f32() / scale;
      // The travel range runs 8px in from either end of the 268px rail.
      assert!(
        left >= 16.0 + 8.0 - 1.0 && right <= 16.0 + 268.0 - 8.0 + 1.0,
        "a tick must stay on the travel range, not {left}..{right}"
      );
    }
  }

  #[gpui::test]
  fn a_pending_value_on_the_committed_one_is_not_drawn(cx: &mut TestAppContext) {
    // A deferred drag that lands on the committed value has nothing to preview:
    // the two pills would be the same pill, so only the committed thumb is
    // drawn.
    let state = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(50.0)
        .with_immediate(false)
    });
    let mut visual = open(cx, state.clone());

    visual.update(|_, cx| {
      state.update(cx, |state, cx| {
        state.drag_value = Some(SliderValue::Single(50.0));
        cx.notify();
      });
    });
    let (scale, quads) = painted(&mut visual);
    assert_eq!(
      painted_thumbs(&quads, scale).len(),
      1,
      "a pending value equal to the committed one draws one thumb"
    );

    // A pending value elsewhere gets a thumb of its own, and two values that
    // are close enough share the space between them rather than being spread
    // apart.
    visual.update(|_, cx| {
      state.update(cx, |state, cx| {
        state.drag_value = Some(SliderValue::Single(80.0));
        cx.notify();
      });
    });
    let (scale, quads) = painted(&mut visual);
    let thumbs = painted_thumbs(&quads, scale);
    assert_eq!(thumbs.len(), 2);
    assert!((thumbs[0] - 150.0).abs() < 0.5);
    assert!((thumbs[1] - 225.6).abs() < 0.5);

    // Two values one percent apart are two values, and their pills overlap
    // instead of being pushed apart: the committed thumb does not move because
    // a pending one appeared.
    visual.update(|_, cx| {
      state.update(cx, |state, cx| {
        state.drag_value = Some(SliderValue::Single(51.0));
        cx.notify();
      });
    });
    let (scale, quads) = painted(&mut visual);
    let thumbs = painted_thumbs(&quads, scale);
    assert_eq!(thumbs.len(), 2);
    assert!(
      (thumbs[1] - thumbs[0]).abs() < 4.0,
      "pills 4px wide must overlap, not sit {} apart",
      thumbs[1] - thumbs[0]
    );
    assert!(
      (thumbs[0] - 150.0).abs() < 0.5,
      "the committed thumb belongs where its value is, not at {}",
      thumbs[0]
    );
  }
}
