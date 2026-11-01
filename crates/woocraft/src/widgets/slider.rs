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
//!   skipped when the step would make them unreadable
//! - **Deferred dragging**: `with_immediate(false)` previews the dragged value
//!   with its own thumb and a warning track, emits `Preview` while the pointer
//!   moves and `Commit` on release, and leaves the committed value alone until
//!   then — so a host that reports its own value back cannot cancel the drag
//!
//! Motion note: the thumb position is not sprung. A spring would have to
//! suspend travel while the pointer drags (`Spring::with_travel`), but
//! `SliderState` does not track the drag lifecycle, and the thumb position is
//! a layout-relative percentage — animating it would re-run layout every
//! frame instead of staying paint-only. Hover feedback (the thumb grows and
//! brightens) does animate, through a keyed target-value transition whose
//! target is `hovered || dragging`, so a pressed thumb stays expanded even
//! when the pointer leaves the hover area mid-drag.
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
  App, AppContext as _, Axis, Bounds, Context, Div, DragMoveEvent, Empty, Entity, EntityId,
  EventEmitter, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
  ParentElement, Pixels, Render, RenderOnce, SharedString, Stateful,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
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
  bounds: Bounds<Pixels>,
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

  fn set_bounds(&mut self, bounds: Bounds<Pixels>) {
    self.bounds = bounds;
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

    let total = if matches!(axis, Axis::Horizontal) {
      self.bounds.size.width
    } else {
      self.bounds.size.height
    };

    if total <= px(0.) {
      self.active_thumb_start = false;
      return;
    }

    let inner_pos = if matches!(axis, Axis::Horizontal) {
      position.x - self.bounds.left()
    } else {
      self.bounds.bottom() - position.y
    };

    let center =
      ((self.percentage.end - self.percentage.start) * 0.5 + self.percentage.start) * total;
    self.active_thumb_start = inner_pos < center;
  }

  fn update_by_position(
    &mut self, axis: Axis, position: gpui::Point<Pixels>, is_start: bool, cx: &mut Context<Self>,
  ) {
    let total = if matches!(axis, Axis::Horizontal) {
      self.bounds.size.width
    } else {
      self.bounds.size.height
    };

    if total <= px(0.) {
      return;
    }

    let inner_pos = if matches!(axis, Axis::Horizontal) {
      position.x - self.bounds.left()
    } else {
      self.bounds.bottom() - position.y
    };

    let raw_percentage = (inner_pos / total).clamp(0.0, 1.0);
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
struct ThumbPill {
  main: Pixels,
  cross: Pixels,
  track_thickness: Pixels,
  radius: gpui::Rems,
  color: gpui::Hsla,
}

/// What one span of the track between two boundaries means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrackSegment {
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

  /// Build a draggable range thumb positioned at `pct` (0..1). Shared by the
  /// start and end thumbs; `is_start` only selects the element id and which
  /// thumb the drag events belong to.
  ///
  /// The thumb is a borderless text-colored pill whose center stays 0.5em
  /// away from the track ends; on hover it grows along the track and
  /// brightens (via the shared keyed hover flag).
  /// The pill every thumb is drawn as: the geometry, rounding and placement
  /// live here once so a previewed thumb is indistinguishable from the real
  /// one apart from its colour.
  fn thumb_pill(&self, axis: Axis, pct: f32, pill: ThumbPill) -> Div {
    let ThumbPill {
      main,
      cross,
      track_thickness,
      radius,
      color,
    } = pill;
    div()
      .absolute()
      .rounded(radius)
      .bg(color)
      .when(matches!(axis, Axis::Horizontal), |this| {
        this
          .w(main)
          .h(cross)
          .top(-(cross - track_thickness) / 2.0)
          .left(relative(pct))
      })
      .when(matches!(axis, Axis::Vertical), |this| {
        this
          .w(cross)
          .h(main)
          .left(-(cross - track_thickness) / 2.0)
          .bottom(relative(pct))
      })
  }

  fn thumb(
    &self, window: &mut Window, cx: &mut App, axis: Axis, is_start: bool, pct: f32,
    state: Entity<SliderState>,
  ) -> Stateful<Div> {
    let thumb_id = if is_start {
      "slider-thumb-start"
    } else {
      "slider-thumb-end"
    };
    let entity_id = state.entity_id();
    let rem_size = window.rem_size();
    let track_thickness = self.size.track_thickness().to_pixels(rem_size);
    // Cross-axis size: 1em. Along-axis size: 0.25em at rest, 0.5em on hover.
    let thumb_cross = self.size.thumb_size().to_pixels(rem_size);
    let thumb_main_rest = self.size.em(0.25).to_pixels(rem_size);
    let thumb_main_hover = self.size.em(0.5).to_pixels(rem_size);
    let thumb_inset = self.size.em(0.5).to_pixels(rem_size);
    let thumb_radius = self.size.em(0.125);

    // Both thumbs sample the same keyed hover transition, so range thumbs
    // grow together when the slider is hovered. A pressed/dragged thumb
    // stays expanded for the whole gesture.
    let hover_t = transition(
      (("slider-thumb-hovered", entity_id.as_u64()), "t"),
      if is_slider_hovered(window, cx, entity_id) || state.read(cx).dragging {
        1.0
      } else {
        0.0
      },
      Transition::new(duration::THUMB_HOVER).easing(Easing::EaseOut),
      window,
      cx,
    );
    let thumb_main = thumb_main_rest + (thumb_main_hover - thumb_main_rest) * hover_t;
    let thumb_opacity = 0.6 + 0.2 * hover_t;
    // Margin correction that keeps the thumb center at
    // `0.5em + pct * (track_len - 1em)` while `left`/`bottom` is `pct` of the
    // full track length.
    let main_offset = thumb_inset * (1.0 - 2.0 * pct) - thumb_main / 2.0;

    self
      .thumb_pill(
        axis,
        pct,
        ThumbPill {
          main: thumb_main,
          cross: thumb_cross,
          track_thickness,
          radius: thumb_radius,
          // The control's own neutral with 20% of the primary colour, so the
          // thumb belongs to the track it fills without matching it exactly.
          color: cx
            .theme()
            .foreground
            .blend(cx.theme().primary.opacity(0.2))
            .opacity(thumb_opacity),
        },
      )
      .id((thumb_id, entity_id.as_u64()))
      .map(|this| match axis {
        Axis::Horizontal => this.ml(main_offset),
        Axis::Vertical => this.mb(main_offset),
      })
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

/// Read the slider's keyed hover flag (created by [`Slider::render`]).
fn is_slider_hovered(window: &mut Window, cx: &mut App, entity_id: EntityId) -> bool {
  *window
    .use_keyed_state(("slider-thumb-hovered", entity_id.as_u64()), cx, |_, _| {
      false
    })
    .read(cx)
}

impl_disableable!(Slider);
impl_sizable!(Slider);
impl_styled!(Slider);

impl RenderOnce for Slider {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // `Range` is not `Copy`, so clone the (8-byte) value out of the state.
    let (percentage, is_range, active_thumb_start, origin_pct, drag_pct, tick_marks) = {
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
      (
        state.percentage.clone(),
        state.value.is_range(),
        state.active_thumb_start,
        state.origin_percentage(),
        drag_pct,
        tick_marks,
      )
    };

    let axis = self.axis;
    let entity_id = self.state.entity_id();

    // The hover flag that drives the thumbs' grow-and-brighten transition
    // (sampled in `Slider::thumb`); fed by the container's hover listener.
    let hover_state =
      window.use_keyed_state(("slider-thumb-hovered", entity_id.as_u64()), cx, |_, _| {
        false
      });

    // Build the thumbs before the builder chain below moves fields out of
    // `self` (`.id(self.id)`), otherwise the closures could not borrow `self`.
    let start_thumb =
      is_range.then(|| self.thumb(window, cx, axis, true, percentage.start, self.state.clone()));
    let end_thumb = self.thumb(window, cx, axis, false, percentage.end, self.state.clone());

    // The thumb a non-immediate drag previews: the same pill as the real
    // thumb, expanded as if the pointer were holding it, tinted so the pending
    // value stays distinguishable while the real thumb keeps following
    // playback. Built here because `self` is consumed by the chain below.
    let preview_pill = drag_pct.map(|pct| {
      let rem = window.rem_size();
      let cross = self.size.thumb_size().to_pixels(rem);
      let main = self.size.em(0.5).to_pixels(rem);
      let thickness = self.size.track_thickness().to_pixels(rem);
      let inset = self.size.em(0.5).to_pixels(rem);
      let offset = inset * (1.0 - 2.0 * pct) - main / 2.0;
      self
        .thumb_pill(
          axis,
          pct,
          ThumbPill {
            main,
            cross,
            track_thickness: thickness,
            radius: self.size.em(0.125),
            // The real thumb's neutral at 80% with 20% of the pending colour:
            // the preview reads as a thumb, not as a different mark, and the
            // tint is what distinguishes it.
            color: cx
              .theme()
              .foreground
              .blend(cx.theme().warning.opacity(0.2))
              .opacity(0.8),
          },
        )
        .map(|this| match axis {
          Axis::Horizontal => this.ml(offset),
          Axis::Vertical => this.mb(offset),
        })
    });

    let state_for_down = self.state.clone();

    // The fill runs between the thumb centers, which travel 0.5em in from
    // the track ends — correct the percentage insets by that padding so the
    // fill neither overshoots the thumb nor leaves a stub at the extremes.
    let bar_inset = self.size.em(0.5).to_pixels(window.rem_size());

    // Hit area: a band as wide as the track and as tall as the thumb,
    // layered behind the thumbs — the 0.125em track line itself is far too
    // small a target. Pressing it jumps to the position and starts a real
    // drag, so moving off the band mid-drag keeps updating.
    let thumb_cross = self.size.thumb_size().to_pixels(window.rem_size());
    let track_thickness = self.size.track_thickness().to_pixels(window.rem_size());
    let hit_area = div()
      .id(("slider-hit-area", entity_id.as_u64()))
      .absolute()
      .cursor_pointer()
      .when(matches!(axis, Axis::Horizontal), |this| {
        this
          .left_0()
          .right_0()
          .top(-(thumb_cross - track_thickness) / 2.0)
          .h(thumb_cross)
      })
      .when(matches!(axis, Axis::Vertical), |this| {
        this
          .top_0()
          .bottom_0()
          .left(-(thumb_cross - track_thickness) / 2.0)
          .w(thumb_cross)
      })
      .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, cx| {
        state_for_down.update(cx, |state, cx| {
          state.dragging = true;
          state.choose_active_thumb(axis, e.position);
          state.update_by_position(axis, e.position, state.active_thumb_start, cx);
          // Notify even when the value did not change: the rebuilt element
          // must carry the fresh `active_thumb_start` for the drag below.
          cx.notify();
        });
      })
      .on_drag(
        DragThumb((entity_id, active_thumb_start)),
        |drag, _, _, cx| {
          cx.stop_propagation();
          cx.new(|_| drag.clone())
        },
      )
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
      });

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
        div()
          .id(("slider-track", self.state.entity_id().as_u64()))
          .relative()
          .when(matches!(axis, Axis::Horizontal), |this| {
            this.h(self.size.track_thickness()).w_full()
          })
          .when(matches!(axis, Axis::Vertical), |this| {
            this.w(self.size.track_thickness()).h_full()
          })
          .rounded_full()
          .bg(cx.theme().muted)
          .on_prepaint({
            let state = self.state.clone();
            move |bounds, _, cx| {
              state.update(cx, |s, _| s.set_bounds(bounds));
            }
          })
          .children({
            // Primary runs from the origin to the committed thumb; a dragging
            // thumb in non-immediate mode adds the warning span between the
            // two. Both are painted from the same sorted boundary list, so the
            // order of origin, committed thumb and dragged thumb is never
            // assumed.
            let segments = if is_range {
              vec![(percentage.start, percentage.end, TrackSegment::Primary)]
            } else {
              track_segments(origin_pct, percentage.end, drag_pct)
            };
            let primary = cx.theme().primary;
            let warning = cx.theme().warning;
            let inset = bar_inset;
            // Warning spans are painted last so a drag that overlaps the
            // primary track still reads as the pending value.
            let mut ordered = segments;
            ordered.sort_by_key(|(_, _, kind)| matches!(kind, TrackSegment::Warning));
            ordered
              .into_iter()
              .map(|(from, to, kind)| {
                let color = match kind {
                  TrackSegment::Primary => primary,
                  TrackSegment::Warning => warning,
                };
                let from_correction = inset * (1.0 - 2.0 * from);
                let to_correction = inset * (2.0 * to - 1.0);
                div()
                  .absolute()
                  .when(matches!(axis, Axis::Horizontal), |this| {
                    this
                      .left(relative(from))
                      .ml(from_correction)
                      .right(relative(1. - to))
                      .mr(to_correction)
                      .top_0()
                      .bottom_0()
                  })
                  .when(matches!(axis, Axis::Vertical), |this| {
                    this
                      .bottom(relative(from))
                      .mb(from_correction)
                      .top(relative(1. - to))
                      .mt(to_correction)
                      .left_0()
                      .right_0()
                  })
                  .rounded_full()
                  .bg(color)
                  .into_any_element()
              })
              .collect::<Vec<_>>()
          })
          // Ticks sit above the fill so the long ones stay readable where the
          // primary track covers them.
          .children({
            let rem = window.rem_size();
            let minor = self.size.em(TICK_HEIGHT_EM).to_pixels(rem);
            let major = self.size.em(MAJOR_TICK_HEIGHT_EM).to_pixels(rem);
            let border = cx.theme().muted_foreground.opacity(0.45);
            tick_marks
              .into_iter()
              .map(|(pct, is_major)| {
                let height = if is_major { major } else { minor };
                div()
                  .absolute()
                  .w(px(TICK_WIDTH_PX))
                  .h(height)
                  .bg(border)
                  // Ticks share the track's endpoint padding: a thumb travels
                  // 0.5em in from each end, so an unpadded tick would sit
                  // outside the values it marks.
                  .when(matches!(axis, Axis::Horizontal), |this| {
                    this
                      .left(relative(pct))
                      .ml(bar_inset * (1.0 - 2.0 * pct) - px(TICK_WIDTH_PX / 2.0))
                      .top(relative(0.5))
                      .mt(-height / 2.0)
                  })
                  .when(matches!(axis, Axis::Vertical), |this| {
                    this
                      .bottom(relative(pct))
                      .mb(bar_inset * (1.0 - 2.0 * pct) - px(TICK_WIDTH_PX / 2.0))
                      .left(relative(0.5))
                      .ml(-height / 2.0)
                      .w(height)
                      .h(px(TICK_WIDTH_PX))
                  })
                  .into_any_element()
              })
              .collect::<Vec<_>>()
          })
          // Painted above the triggers and below the real thumbs.
          .when_some(preview_pill, |this, thumb| this.child(thumb))
          // Layered behind the thumbs so a press exactly on a thumb still
          // starts the thumb's own drag.
          .when(!self.disabled, |this| this.child(hit_area))
          .when_some(start_thumb, |this, thumb| this.child(thumb))
          .child(end_thumb),
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
}
