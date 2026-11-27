//! Numeric slider with one thumb (a value) or two (a range).
//!
//! Nothing on the rail computes a position: it is a stub, the travel range and
//! the same stub again, each span takes a fraction of the range as its flex
//! basis, and a thumb is a zero-width slot cut in between two spans. So a thumb
//! stands where its value is because that is where the flow puts it, and a
//! span's colouring and a thumb's position cannot disagree.
//!
//! The thumb's position is the one thing that is not sprung: it is a
//! layout-relative fraction, so animating it would re-run layout every frame.
//! The pill's hover growth does animate, and takes no width out of the flow.
//!
//! A preview ([`Slider::preview`]) is not a tooltip bound to a pill: it is an
//! overlay anchored to the value under the pointer, or to the thumb a drag is
//! moving, and its content is any element. That is what a media player's seek
//! bar needs — a frame thumbnail for the second the pointer names — so the
//! builder is asked again on every frame of the gesture, and the overlay is
//! deferred to paint over the frame behind the slider rather than under it.
//!
//! # Example
//! ```rust,ignore
//! let volume = cx.new(|_| SliderState::new().min(0.0).max(100.0).step(1.0));
//! let price = cx.new(|_| {
//!   SliderState::new()
//!     .min(0.0)
//!     .max(100.0)
//!     .default_value((25.0, 75.0))
//! });
//! ```

use gpui::{
  Anchor, AnyElement, App, AppContext as _, Axis, Bounds, Context, Div, DragMoveEvent, Empty,
  Entity, EntityId, EventEmitter, Hsla, InteractiveElement as _, IntoElement, Length, MouseButton,
  MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Rems, Render, RenderOnce, SharedString,
  Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, anchored, deferred,
  div, point, prelude::FluentBuilder as _, px, relative, rems,
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

/// Numeric value: one thumb, or two kept in order (`start <= end`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
  /// One thumb.
  Single(f32),
  /// Two thumbs, kept in order.
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

/// Linear values are evenly spaced across the track, logarithmic ones
/// exponentially (for audio and other exponential data; needs `min > 0`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderScale {
  /// Evenly spaced. Default.
  #[default]
  Linear,
  /// Exponential spacing (min must be > 0).
  Logarithmic,
}

/// Every n-th tick is drawn longer unless the caller overrides it.
const DEFAULT_MAJOR_TICK_RATIO: u32 = 5;
/// Height of a minor tick, in ems of the slider's size tier.
const TICK_HEIGHT_EM: f32 = 0.3;
/// Gap between a preview and the rail it stands over.
const PREVIEW_GAP_PX: f32 = 4.0;
/// How far a preview keeps clear of the window's edges, so it is never cut off.
const PREVIEW_MARGIN_PX: f32 = 8.0;
/// Height of a major tick: only a little taller, so the marks read as a ruler
/// rather than as a second track.
const MAJOR_TICK_HEIGHT_EM: f32 = 0.5;
/// Tick thickness in device pixels: a hairline, like a border.
const TICK_WIDTH_PX: f32 = 1.0;
/// Above this many ticks per track they are skipped: a 0.01 step over 0..=100
/// would otherwise paint ten thousand lines nobody can read.
const MAX_TICKS: u32 = 200;

#[derive(Clone)]
/// What a drag reports.
pub enum SliderEvent {
  /// The value moved: an immediate drag, or a programmatic
  /// [`SliderState::set_value`].
  Change(SliderValue),
  /// A non-immediate drag is in progress; render the pending thumb from
  /// [`SliderState::drag_value`]. The value itself is unchanged until release.
  Preview(SliderValue),
  /// A non-immediate drag ended; the value is now the previewed one.
  Commit(SliderValue),
}

/// Where the primary track starts: the fill runs from here to the thumb.
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

/// What shows a slider's preview.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SliderPreviewTrigger {
  /// Hovering anywhere on the rail (default), so the preview follows the
  /// pointer.
  #[default]
  Rail,
  /// Only hovering a thumb, or dragging one.
  Thumb,
}

/// What a preview is asked to show.
///
/// It describes one point of the slider: the value under the pointer while the
/// rail is hovered, a thumb's own value while it is hovered, or the value a
/// gesture is moving — the pending one of a deferred drag.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SliderPreview {
  /// The value at the previewed position, snapped and clamped.
  pub value: f32,
  /// Where that value sits, as a fraction of the travel range.
  pub percentage: f32,
  /// Whether a drag rather than a hover is showing the preview.
  pub dragging: bool,
}

/// The slider's value, bounds and drag state.
pub struct SliderState {
  min: f32,
  max: f32,
  step: f32,
  value: SliderValue,
  percentage: std::ops::Range<f32>,
  /// Where the rail was laid out, for mapping pointer positions back to values.
  bounds: Bounds<Pixels>,
  /// Half a thumb's cross size: a thumb's centre travels that far in from
  /// either end of the rail, so this is where the travel range starts.
  travel_inset: Pixels,
  scale: SliderScale,
  active_thumb_start: bool,
  /// Whether a pointer is pressed on the slider: keeps the thumb expanded for
  /// the whole gesture, even when the pointer leaves the hover area mid-drag.
  dragging: bool,
  /// Where the primary track starts.
  origin: SliderOrigin,
  /// Draw a tick at every step.
  ticks: bool,
  /// Every n-th tick is drawn longer; `0` is treated as `1`.
  major_tick_ratio: u32,
  /// Whether dragging moves the value immediately. When false the value follows
  /// on release instead, and the drag is previewed separately.
  immediate: bool,
  /// The value a non-immediate drag is previewing, cleared when it commits.
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

  /// The minimum value (default `0.0`); must be `> 0` on a logarithmic scale.
  pub fn min(mut self, min: f32) -> Self {
    self.min = min;
    self.sync_percentage();
    self
  }

  /// The maximum value (default `100.0`); must be greater than [`Self::min`].
  pub fn max(mut self, max: f32) -> Self {
    self.max = max;
    self.sync_percentage();
    self
  }

  /// The quantum the value snaps to (default `1.0`).
  pub fn step(mut self, step: f32) -> Self {
    self.step = step.max(0.000_001);
    self
  }

  /// Linear (default) or logarithmic spacing.
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

  pub fn set_origin(&mut self, origin: SliderOrigin, cx: &mut Context<Self>) {
    if self.origin != origin {
      self.origin = origin;
      cx.notify();
    }
  }

  /// Draw a tick at every step, every `major_tick_ratio`-th one longer.
  pub fn with_ticks(mut self, ticks: bool) -> Self {
    self.ticks = ticks;
    self
  }

  /// How many ticks apart the long ones are; `0` is treated as `1`.
  pub fn with_major_tick_ratio(mut self, ratio: u32) -> Self {
    self.major_tick_ratio = ratio.max(1);
    self
  }

  pub fn set_ticks(&mut self, ticks: bool, cx: &mut Context<Self>) {
    if self.ticks != ticks {
      self.ticks = ticks;
      cx.notify();
    }
  }

  /// Whether a drag moves the value immediately (default) or only on release,
  /// with the dragged value previewed and [`SliderEvent::Preview`] emitted
  /// until [`SliderEvent::Commit`] on release.
  pub fn with_immediate(mut self, immediate: bool) -> Self {
    self.immediate = immediate;
    self
  }

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

  /// Records where the rail was laid out, for mapping pointer positions back to
  /// values. The layout never reads this: the flow resolves the spans, so only
  /// dragging needs the geometry.
  fn set_geometry(&mut self, bounds: Bounds<Pixels>, travel_inset: Pixels) {
    self.bounds = bounds;
    self.travel_inset = travel_inset;
  }

  /// The travel range's length along `axis`; zero before the first layout.
  fn travel_length(&self, axis: Axis) -> Pixels {
    let total = match axis {
      Axis::Horizontal => self.bounds.size.width,
      Axis::Vertical => self.bounds.size.height,
    };
    (total - self.travel_inset * 2.0).max(px(0.))
  }

  /// The percentage under a pointer position, or `None` with no geometry yet.
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

    // Which thumb the gesture grabs is a question about the middle of the
    // range, so it is answered in percentages.
    let center = (self.percentage.start + self.percentage.end) * 0.5;
    self.active_thumb_start = percentage < center;
  }

  fn update_by_position(
    &mut self, axis: Axis, position: gpui::Point<Pixels>, is_start: bool, cx: &mut Context<Self>,
  ) {
    // A gesture on a pill carries its own end, so record it: otherwise the flag
    // still names the end the previous gesture moved.
    self.active_thumb_start = is_start;

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

    // Non-immediate: the drag previews a value and the committed one follows on
    // release, so a host slow to apply a value does not fight the pointer.
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

  /// The snapped value a fraction of the travel range stands for.
  fn value_at(&self, percentage: f32) -> f32 {
    let raw = self.percentage_to_value(percentage);
    match self.snap_and_clamp(SliderValue::Single(raw)) {
      SliderValue::Single(value) => value,
      SliderValue::Range(..) => raw,
    }
  }

  /// The snapped value the pointer at `position` names, or `None` with no
  /// geometry yet. Snapping here is what keeps a pointer that moves inside one
  /// step from re-rendering.
  fn value_at_position(&self, axis: Axis, position: gpui::Point<Pixels>) -> Option<f32> {
    self
      .percentage_at(axis, position)
      .map(|percentage| self.value_at(percentage))
  }

  /// Where a fraction of the travel range stands along the axis, in the track's
  /// own coordinates: the cross position is the track's edge, which is what an
  /// anchor on the rail is anchored to.
  fn thumb_offset(&self, axis: Axis, percentage: f32) -> Pixels {
    let travelled = self.travel_length(axis) * percentage;
    match axis {
      Axis::Horizontal => self.travel_inset + travelled,
      Axis::Vertical => self.bounds.size.height - self.travel_inset - travelled,
    }
  }

  /// What a preview should show right now: a drag always previews, a hover does
  /// what the trigger allows.
  fn preview(&self, trigger: SliderPreviewTrigger, hover: &PreviewHover) -> Option<SliderPreview> {
    if self.dragging {
      // A deferred drag has not moved the value yet, so its pending value is
      // the one under the pointer; an immediate drag has already moved it.
      let start = self.value.is_range() && self.active_thumb_start;
      let (value, percentage) = match self.drag_value {
        Some(pending) if start => (pending.start(), self.value_to_percentage(pending.start())),
        Some(pending) => (pending.end(), self.value_to_percentage(pending.end())),
        None if start => (self.value.start(), self.percentage.start),
        None => (self.value.end(), self.percentage.end),
      };
      return Some(SliderPreview {
        value,
        percentage,
        dragging: true,
      });
    }

    match trigger {
      // Anywhere on the rail: the value the pointer names.
      SliderPreviewTrigger::Rail => hover.value.map(|value| SliderPreview {
        value,
        percentage: self.value_to_percentage(value),
        dragging: false,
      }),
      // Only a thumb, and then its own value rather than the pointer's.
      SliderPreviewTrigger::Thumb => {
        let (value, percentage) = match hover.thumb? {
          ThumbKind::RangeStart => (self.value.start(), self.percentage.start),
          ThumbKind::Value => (self.value.end(), self.percentage.end),
          ThumbKind::Pending => return None,
        };
        Some(SliderPreview {
          value,
          percentage,
          dragging: false,
        })
      }
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

/// Geometry and colour of one thumb pill.
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
  /// From the committed thumb to the thumb a deferred drag previews.
  Warning,
}

/// Where the pointer stands for a preview.
///
/// This is keyed window state rather than slider state: a slider that stops
/// rendering — a floating bar that hides itself — forgets a hover it can no
/// longer see, instead of resurrecting a stale preview when it comes back.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct PreviewHover {
  /// The snapped value under the pointer over the rail.
  value: Option<f32>,
  /// The thumb pill the pointer is over.
  thumb: Option<ThumbKind>,
}

impl PreviewHover {
  /// Records the value under the pointer; returns whether a preview would now
  /// show another value.
  fn set_value(&mut self, value: Option<f32>) -> bool {
    if self.value == value {
      return false;
    }
    self.value = value;
    true
  }

  /// Records the thumb pill under the pointer; returns whether it changed.
  fn set_thumb(&mut self, thumb: Option<ThumbKind>) -> bool {
    if self.thumb == thumb {
      return false;
    }
    self.thumb = thumb;
    true
  }
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

/// One child of the travel range, in flow order.
#[derive(Clone, Copy, Debug, PartialEq)]
enum RailPart {
  /// A share of the travel range, as fractions of it.
  Span {
    from: f32,
    to: f32,
    kind: TrackSegment,
  },
  /// The zero-width slot a thumb hangs off, at a fraction of the range.
  Thumb { at: f32 },
}

/// Sizes an element along the rail by `main`, and across it by `cross`.
fn axis_size<T: Styled>(
  axis: Axis, element: T, main: impl Into<Length> + Clone, cross: impl Into<Length> + Clone,
) -> T {
  match axis {
    Axis::Horizontal => element.w(main).h(cross),
    Axis::Vertical => element.w(cross).h(main),
  }
}

/// The rail's children in flow order.
///
/// Every value is a cut: the origin, the committed thumb and the dragged thumb
/// split the range into spans, and each thumb is cut in as a zero-width slot
/// standing where it is, so the flow places the thumbs. The values have no
/// fixed order, so the spans come from sorting the cuts.
fn rail_parts(origin: f32, actual: f32, drag: Option<f32>, thumbs: &[f32]) -> Vec<RailPart> {
  let mut cuts = vec![0.0_f32, 1.0, origin, actual];
  if let Some(drag) = drag {
    cuts.push(drag);
  }
  cuts.extend(thumbs.iter().copied());
  cuts.sort_by(f32::total_cmp);
  cuts.dedup_by(|a, b| (*a - *b).abs() < f32::EPSILON);

  let inside = |value: f32, a: f32, b: f32| {
    let (low, high) = if a <= b { (a, b) } else { (b, a) };
    value >= low - f32::EPSILON && value <= high + f32::EPSILON
  };

  let mut sorted_thumbs = thumbs.to_vec();
  sorted_thumbs.sort_by(f32::total_cmp);
  let mut pending = sorted_thumbs.into_iter().peekable();

  let mut parts: Vec<RailPart> = Vec::with_capacity(cuts.len() * 2);
  for pair in cuts.windows(2) {
    let (from, to) = (pair[0], pair[1]);
    // A thumb stands in front of the span that starts at its position; the ones
    // at the maximum are appended at the end.
    while let Some(at) = pending.next_if(|at| *at <= from + f32::EPSILON) {
      parts.push(RailPart::Thumb { at });
    }
    if to - from <= f32::EPSILON {
      continue;
    }

    let middle = (from + to) * 0.5;
    // The warning wins where both could claim an interval: a drag towards an
    // end origin lands inside the fill, and the pending value must stay
    // visible.
    let kind = if drag.is_some_and(|drag| inside(middle, actual, drag)) {
      TrackSegment::Warning
    } else if inside(middle, origin, actual) {
      TrackSegment::Primary
    } else {
      TrackSegment::Track
    };
    // A cut that changes nothing (the origin sitting on the thumb) must not
    // leave two spans of one colour meeting at a seam.
    match parts.last_mut() {
      Some(RailPart::Span {
        to: previous_to,
        kind: previous_kind,
        ..
      }) if *previous_kind == kind => *previous_to = to,
      _ => parts.push(RailPart::Span { from, to, kind }),
    }
  }
  // A thumb at the maximum has no span to stand in front of.
  parts.extend(pending.map(|at| RailPart::Thumb { at }));
  parts
}

/// One span of the track: its share of the travel range is its flex basis, so
/// the flex layout resolves it against the range and nothing is measured here.
fn rail_span(axis: Axis, share: f32, thickness: Rems, color: Hsla) -> Div {
  let span = div().flex_shrink_0().flex_basis(relative(share)).bg(color);
  match axis {
    Axis::Horizontal => span.h(thickness),
    Axis::Vertical => span.w(thickness),
  }
}

/// The fixed end of the rail outside the travel range: half a thumb's cross
/// size, so a thumb's centre stops there. Only its outer corners are rounded,
/// and it takes the colour of the span beside it.
fn rail_stub(
  axis: Axis, length: Pixels, thickness: Rems, radius: Rems, color: Hsla, leading: bool,
) -> Div {
  // Only the stubs may shrink: a rail narrower than its two halves gives up
  // padding rather than painting outside itself, and a share that does not
  // divide a pixel leaves its leftover here instead of in a seam.
  let stub = axis_size(axis, div().bg(color), length, thickness);
  match (leading, matches!(axis, Axis::Horizontal)) {
    (true, true) => stub.rounded_l(radius),
    (false, true) => stub.rounded_r(radius),
    (true, false) => stub.rounded_b(radius),
    (false, false) => stub.rounded_t(radius),
  }
}

/// The zero-width slot a thumb hangs off: it takes no room in the flow, so the
/// flow places it exactly the way it places the spans, and the pill overflows
/// it by half its own width on either side.
fn thumb_slot(axis: Axis, cross: Pixels) -> Div {
  axis_size(axis, div().flex_shrink_0(), px(0.), cross)
}

/// Centres a pill on its slot: the slot stands on the value, so the pill is
/// pulled back by half its own width to straddle it.
fn centre_on_slot<T: Styled>(axis: Axis, pill: T, main: Pixels) -> T {
  match axis {
    Axis::Horizontal => pill.ml(-main / 2.0),
    Axis::Vertical => pill.mt(-main / 2.0),
  }
}

/// The track positions of the ticks, as `(percentage, is_major)`.
///
/// Above [`MAX_TICKS`] marks only the long ones are drawn, and above that none
/// at all: a slider is a control, not a ruler.
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

/// Builds a preview from what the slider is pointing at.
type SliderPreviewBuilder = Box<dyn Fn(SliderPreview, &mut Window, &mut App) -> AnyElement>;

#[derive(IntoElement)]
pub struct Slider {
  id: SharedString,
  state: Entity<SliderState>,
  style: StyleRefinement,
  size: Size,
  disabled: bool,
  axis: Axis,
  preview: Option<SliderPreviewBuilder>,
  preview_trigger: SliderPreviewTrigger,
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
      preview: None,
      preview_trigger: SliderPreviewTrigger::default(),
    }
  }

  /// Show a preview for the point the pointer names, or for the thumb a drag is
  /// moving: the content is any element, not only a text tooltip. A media
  /// player's seek bar is the typical case — render a frame thumbnail for
  /// [`SliderPreview::value`] and the slider places it over the rail.
  ///
  /// The builder is asked again on every render while the preview is visible,
  /// so it sees a drag's live value; and since a slider is a `RenderOnce`
  /// element with no state of its own, prefer plain elements over a new view
  /// unless the content needs state that outlives one frame.
  ///
  /// The overlay is deferred, so it paints over whatever stands behind the
  /// slider — an ancestor that hides its own overflow still clips it. It is
  /// laid out inside the slider's track, so auto-width content is measured
  /// against that: one em wide on a vertical slider, where the content should
  /// bring a width of its own.
  pub fn preview<E, F>(mut self, builder: F) -> Self
  where
    E: IntoElement,
    F: Fn(SliderPreview, &mut Window, &mut App) -> E + 'static, {
    self.preview = Some(Box::new(move |preview, window, cx| {
      builder(preview, window, cx).into_any_element()
    }));
    self
  }

  /// What shows the preview: anywhere on the rail (the default) or only on a
  /// thumb. A drag shows it either way.
  pub fn preview_trigger(mut self, trigger: SliderPreviewTrigger) -> Self {
    self.preview_trigger = trigger;
    self
  }

  pub fn horizontal(mut self) -> Self {
    self.axis = Axis::Horizontal;
    self
  }

  pub fn vertical(mut self) -> Self {
    self.axis = Axis::Vertical;
    self
  }

  /// The pill a thumb is drawn as, without any interaction. It knows nothing
  /// about where the thumb is: the flow places it.
  fn thumb_pill(&self, axis: Axis, pill: ThumbPill) -> Div {
    let ThumbPill {
      main,
      cross,
      radius,
      color,
    } = pill;
    axis_size(
      axis,
      div().flex_shrink_0().rounded(radius).bg(color),
      main,
      cross,
    )
  }

  /// A draggable thumb: the pill plus the gesture that moves it. `is_start`
  /// selects the element id and which thumb the drag belongs to.
  fn thumb(
    &self, axis: Axis, pill: ThumbPill, is_start: bool, state: Entity<SliderState>,
    preview_hover: Option<Entity<PreviewHover>>,
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
            let state = state.clone();
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
          // A thumb-scoped preview is about the pill, not about the rail under
          // it, so the pill's own hitbox decides.
          .when_some(
            preview_hover.filter(|_| self.preview_trigger == SliderPreviewTrigger::Thumb),
            |this, hover| {
              let thumb = if is_start {
                ThumbKind::RangeStart
              } else {
                ThumbKind::Value
              };
              this.on_hover({
                move |hovered, _, cx| {
                  let thumb = hovered.then_some(thumb);
                  hover.update(cx, |hover, cx| {
                    if hover.set_thumb(thumb) {
                      cx.notify();
                    }
                  });
                }
              })
            },
          )
      })
  }
}

impl_disableable!(Slider);
impl_sizable!(Slider);
impl_styled!(Slider);

impl RenderOnce for Slider {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let axis = self.axis;
    let entity_id = self.state.entity_id();

    // Where the pointer stands, for a preview. Keyed state, so it dies with the
    // element: a slider that leaves the frame has no hover either.
    let preview_hover = self.preview.is_some().then(|| {
      window.use_keyed_state(("slider-preview", entity_id.as_u64()), cx, |_, _| {
        PreviewHover::default()
      })
    });

    // `Range` is not `Copy`, so clone the (8-byte) value out of the state.
    let (percentage, is_range, active_thumb_start, origin_pct, drag_pct, tick_marks, preview) = {
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
      // A preview is only resolved for an enabled slider with one configured:
      // without a builder the hover listeners are not wired either, so this is
      // the one place that decides.
      let preview = match preview_hover.as_ref() {
        Some(hover) if !self.disabled => state.preview(self.preview_trigger, hover.read(cx)),
        _ => None,
      };
      (
        state.percentage.clone(),
        state.value.is_range(),
        state.active_thumb_start,
        state.origin_percentage(),
        drag_pct,
        tick_marks,
        preview,
      )
    };

    let rem = window.rem_size();

    // The hover flag behind the thumb's grow-and-brighten transition, fed by
    // the container's listener. One flag drives every thumb, and a pressed
    // thumb stays expanded for the whole gesture.
    let hover_state =
      window.use_keyed_state(("slider-thumb-hovered", entity_id.as_u64()), cx, |_, _| {
        false
      });
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
    let travel_inset = thumb_cross / 2.0;

    // The pill every thumb is drawn as, built once so a pending thumb differs
    // only in colour — and in particular is the same width, since the flow
    // places the spans around the slot the pill hangs off.
    let pill = ThumbPill {
      main: thumb_main,
      cross: thumb_cross,
      radius: self.size.em(0.125),
      // The control's own neutral with 20% of the primary colour, so the thumb
      // belongs to the track it fills without matching it exactly.
      color: cx
        .theme()
        .foreground
        .blend(cx.theme().primary.opacity(0.2))
        .opacity(thumb_opacity),
    };

    // The same pill tinted with the warning colour, so a pending value reads as
    // a thumb rather than as a different kind of mark.
    let pending_pill = ThumbPill {
      color: cx
        .theme()
        .foreground
        .blend(cx.theme().warning.opacity(0.2))
        .opacity(0.8),
      ..pill
    };

    // A pending value equal to the committed one has nothing to show: it would
    // only repaint the same pill.
    let preview_pct = drag_pct.filter(|pct| *pct != percentage.end);

    // A range fills between its thumbs, so its low end takes the origin's part.
    let (origin, actual) = if is_range {
      (percentage.start, percentage.end)
    } else {
      (origin_pct, percentage.end)
    };
    // The thumbs in flow order. A later child draws over an earlier one, so a
    // stable sort keeps the pending thumb on top when two land on one spot.
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

    // The spans, with a slot for each thumb cut in between them.
    let parts = rail_parts(origin, actual, drag_pct, &thumb_positions);

    // Each end stub continues the span next to it.
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

    // The pills, each hung off its slot and deferred so that it paints over the
    // spans on either side of it: a sibling laid out before the track would be
    // painted over by the span on its other side.
    let pills: Vec<AnyElement> = thumb_kinds
      .iter()
      .map(|(_, kind)| {
        let pill: AnyElement = match kind {
          ThumbKind::Pending => {
            centre_on_slot(axis, self.thumb_pill(axis, pending_pill), thumb_main).into_any_element()
          }
          ThumbKind::RangeStart => centre_on_slot(
            axis,
            self.thumb(axis, pill, true, self.state.clone(), preview_hover.clone()),
            thumb_main,
          )
          .into_any_element(),
          ThumbKind::Value => centre_on_slot(
            axis,
            self.thumb(axis, pill, false, self.state.clone(), preview_hover.clone()),
            thumb_main,
          )
          .into_any_element(),
        };
        deferred(thumb_slot(axis, thumb_cross).child(pill)).into_any_element()
      })
      .collect();

    let primary = cx.theme().primary;
    let warning = cx.theme().warning;
    // The rail is the same hue as the fill it sits beside, stepped back toward
    // the surface so the filled part stays the strong one.
    let track = crate::fill_track_color(cx.theme(), primary);
    let color_of = |kind: TrackSegment| match kind {
      TrackSegment::Track => track,
      TrackSegment::Primary => primary,
      TrackSegment::Warning => warning,
    };

    let thickness = self.size.track_thickness();
    // Half the track's thickness: the rail's ends are rounded as one pill,
    // which is what the two stubs' outer corners carry.
    let radius = rems(thickness.0 * 0.5);

    // The preview, if anything shows it: an overlay the flow does not see, hung
    // off the point it describes and anchored to whichever side has room, so a
    // preview near a window edge slides in instead of being cut off. It is
    // deferred because a preview over the frame behind the slider is the whole
    // point of having one. The anchor is a window position because the track's
    // own layout is the flow's business: a slot standing on it would leave the
    // overlay to whatever alignment the rail happens to use.
    let preview = preview.and_then(|preview| {
      let build = self.preview.as_ref()?;
      let content = build(preview, window, cx);
      let (anchor, position) = {
        let state = self.state.read(cx);
        let offset = state.thumb_offset(axis, preview.percentage);
        match axis {
          // Over the rail, centred on the value.
          Axis::Horizontal => (
            Anchor::BottomCenter,
            point(
              state.bounds.origin.x + offset,
              state.bounds.origin.y - px(PREVIEW_GAP_PX),
            ),
          ),
          // Beside the rail, centred on the value.
          Axis::Vertical => (
            Anchor::RightCenter,
            point(
              state.bounds.right() + px(PREVIEW_GAP_PX),
              state.bounds.origin.y + offset,
            ),
          ),
        }
      };
      Some(
        deferred(
          anchored()
            .anchor(anchor)
            .position(position)
            .snap_to_window_with_margin(px(PREVIEW_MARGIN_PX))
            .child(content),
        )
        .with_priority(1),
      )
    });

    let state_for_down = self.state.clone();
    let drag_start = active_thumb_start;

    let mut travel_range: Vec<AnyElement> = Vec::with_capacity(parts.len() + 1);
    // Ticks are marks rather than spans: they overlay the travel range at a
    // fraction of it, painted before the spans so the fill reads over them.
    if !tick_marks.is_empty() {
      travel_range.push(
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
                div()
                  .absolute()
                  .w(px(TICK_WIDTH_PX))
                  .h(height)
                  .bg(border)
                  .when(matches!(axis, Axis::Horizontal), |this| {
                    this
                      .left(relative(pct))
                      .ml(-px(TICK_WIDTH_PX / 2.0))
                      .top(relative(0.5))
                      .mt(-height / 2.0)
                  })
                  .when(matches!(axis, Axis::Vertical), |this| {
                    this
                      .bottom(relative(pct))
                      .mb(-px(TICK_WIDTH_PX / 2.0))
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
    }

    let mut pills = pills.into_iter();
    for part in &parts {
      match part {
        RailPart::Span { from, to, kind } => travel_range
          .push(rail_span(axis, to - from, thickness, color_of(*kind)).into_any_element()),
        RailPart::Thumb { .. } => {
          if let Some(pill) = pills.next() {
            travel_range.push(pill);
          }
        }
      }
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
          // A release anywhere ends the gesture; if the release itself was
          // missed, the next unpressed move does.
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
        // A band one em tall, not a hairline, so the pointer can land on it. It
        // is also the hit target for presses away from a thumb, which stop the
        // event before it gets here.
        div()
          .id(("slider-track", entity_id.as_u64()))
          .when(matches!(axis, Axis::Horizontal), |this| {
            this.h(self.size.thumb_size()).w_full()
          })
          .when(matches!(axis, Axis::Vertical), |this| {
            this.w(self.size.thumb_size()).h_full()
          })
          .flex()
          // Reversed on the vertical axis so the first child is at the bottom, so
          // the flow order means the same thing on both axes.
          .when(matches!(axis, Axis::Horizontal), |this| {
            this.flex_row().items_center()
          })
          .when(matches!(axis, Axis::Vertical), |this| {
            this.flex_col_reverse().items_center()
          })
          .when(!self.disabled, |this| this.cursor_pointer())
          .on_prepaint({
            let state = self.state.clone();
            move |bounds, _, cx| {
              // Only dragging reads the geometry: the layout is the flow's.
              state.update(cx, |state, _cx| state.set_geometry(bounds, travel_inset));
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
          .when_some(preview_hover.filter(|_| !self.disabled), |this, hover| {
            this
              .on_mouse_move({
                let state = self.state.clone();
                let hover = hover.clone();
                move |e: &MouseMoveEvent, _, cx| {
                  let value = state.read(cx).value_at_position(axis, e.position);
                  hover.update(cx, |hover, cx| {
                    if hover.set_value(value) {
                      cx.notify();
                    }
                  });
                }
              })
              .on_hover({
                move |hovered, _, cx| {
                  if *hovered {
                    return;
                  }
                  hover.update(cx, |hover, cx| {
                    if hover.set_value(None) {
                      cx.notify();
                    }
                  });
                }
              })
          })
          .child(rail_stub(
            axis,
            travel_inset,
            thickness,
            radius,
            color_of(lead_kind),
            true,
          ))
          .child(
            // The travel range: the space a thumb's centre moves through, and
            // the one container the spans are resolved against.
            div()
              .flex_1()
              .flex()
              .items_center()
              .when(matches!(axis, Axis::Horizontal), |this| this.flex_row())
              .when(matches!(axis, Axis::Vertical), |this| {
                this.flex_col_reverse()
              })
              .children(travel_range),
          )
          .child(rail_stub(
            axis,
            travel_inset,
            thickness,
            radius,
            color_of(tail_kind),
            false,
          ))
          // The overlay is a child of the track so that its anchor is measured
          // from the same origin the thumb positions are.
          .children(preview),
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
  use gpui::{Modifiers, Render, TestAppContext, Window};

  use super::*;
  use crate::Theme;

  fn spans(parts: &[RailPart]) -> Vec<(f32, f32, TrackSegment)> {
    parts
      .iter()
      .filter_map(|part| match part {
        RailPart::Span { from, to, kind } => Some((*from, *to, *kind)),
        RailPart::Thumb { .. } => None,
      })
      .collect()
  }

  /// Where each thumb lands, as a fraction of the travel range: the flow puts
  /// it where the spans before it add up to.
  fn thumb_shares(parts: &[RailPart]) -> Vec<f32> {
    let mut cursor = 0.0_f32;
    let mut shares = Vec::new();
    for part in parts {
      match part {
        RailPart::Span { from, to, .. } => cursor += to - from,
        RailPart::Thumb { .. } => shares.push(cursor),
      }
    }
    shares
  }

  #[test]
  fn the_fill_runs_from_the_origin_to_the_thumb() {
    // A start origin fills forwards, a middle one in both directions, and an
    // end origin backwards.
    let from_start = spans(&rail_parts(0.0, 0.75, None, &[0.75]));
    assert_eq!(
      from_start,
      vec![
        (0.0, 0.75, TrackSegment::Primary),
        (0.75, 1.0, TrackSegment::Track),
      ]
    );

    assert_eq!(
      spans(&rail_parts(0.5, 0.8, None, &[0.8]))[1],
      (0.5, 0.8, TrackSegment::Primary)
    );
    assert_eq!(
      spans(&rail_parts(0.5, 0.2, None, &[0.2]))[1],
      (0.2, 0.5, TrackSegment::Primary)
    );
    assert_eq!(
      spans(&rail_parts(1.0, 0.3, None, &[0.3]))[1],
      (0.3, 1.0, TrackSegment::Primary)
    );
  }

  #[test]
  fn the_end_stubs_take_the_colour_of_the_span_beside_them() {
    // The stubs are the rail outside the travel range, so they continue
    // whatever span touches them: a fill that reaches an end has to carry into
    // the stub, or the slider shows a bare notch at the value the user dragged
    // to.
    let ends = |origin, actual| {
      let spans = spans(&rail_parts(origin, actual, None, &[actual]));
      (spans[0].2, spans[spans.len() - 1].2)
    };
    assert_eq!(
      ends(0.0, 1.0),
      (TrackSegment::Primary, TrackSegment::Primary)
    );
    assert_eq!((ends(0.0, 0.0).0), TrackSegment::Track);
    assert_eq!((ends(0.5, 0.75).0), TrackSegment::Track);
    assert_eq!(ends(1.0, 0.25).1, TrackSegment::Primary);
    assert_eq!(ends(0.25, 0.75).0, TrackSegment::Track);
  }

  #[test]
  fn a_dragged_thumb_adds_the_warning_span_between_the_two() {
    let parts = rail_parts(0.0, 0.4, Some(0.7), &[0.4, 0.7]);
    assert_eq!(
      spans(&parts),
      vec![
        (0.0, 0.4, TrackSegment::Primary),
        (0.4, 0.7, TrackSegment::Warning),
        (0.7, 1.0, TrackSegment::Track),
      ]
    );
    assert_eq!(thumb_shares(&parts), vec![0.4, 0.7]);

    // The three values have no fixed order: dragging back past the origin
    // covers the fill with one warning span, and dragging past an end origin
    // leaves the fill on the far side of the committed thumb.
    let backwards = spans(&rail_parts(0.5, 0.6, Some(0.2), &[0.6, 0.2]));
    assert_eq!(backwards[1], (0.2, 0.6, TrackSegment::Warning));
    let end_origin = spans(&rail_parts(1.0, 0.8, Some(0.9), &[0.8, 0.9]));
    assert_eq!(end_origin[1], (0.8, 0.9, TrackSegment::Warning));
    assert_eq!(end_origin[2], (0.9, 1.0, TrackSegment::Primary));
  }

  #[test]
  fn a_thumb_is_cut_in_between_the_spans_at_its_value() {
    assert_eq!(
      rail_parts(0.0, 0.5, None, &[0.5]),
      vec![
        RailPart::Span {
          from: 0.0,
          to: 0.5,
          kind: TrackSegment::Primary,
        },
        RailPart::Thumb { at: 0.5 },
        RailPart::Span {
          from: 0.5,
          to: 1.0,
          kind: TrackSegment::Track,
        },
      ]
    );

    // A middle origin with the value in the middle has no fill and no span
    // boundary to stand on, and the thumb still stands on its value.
    let empty = rail_parts(0.5, 0.5, None, &[0.5]);
    assert_eq!(thumb_shares(&empty), vec![0.5]);
  }

  #[test]
  fn spans_tile_the_travel_range_and_thumbs_cost_it_nothing() {
    // A span's share is its flex basis, so the shares have to add up to the
    // range; a thumb is a zero-width slot and takes none of it, and two values
    // on one spot are one spot.
    for thumbs in [
      vec![0.0],
      vec![0.25, 0.75],
      vec![0.2, 0.5, 0.8],
      vec![0.5, 0.51],
    ] {
      let parts = rail_parts(0.0, 0.5, None, &thumbs);
      let total: f32 = spans(&parts).iter().map(|(from, to, _)| to - from).sum();
      assert!((total - 1.0).abs() < 1e-4, "{thumbs:?} tiles to {total}");
      assert_eq!(thumb_shares(&parts), thumbs);
    }
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
  fn a_dense_step_falls_back_to_major_ticks_and_then_to_none() {
    let ticks = tick_positions(0.0, 1000.0, 1.0, 5, |value| value / 1000.0);
    assert!(!ticks.is_empty());
    assert!(ticks.iter().all(|(_, major)| *major));

    // Nothing readable even at the major stride, and nothing at all for bounds
    // that cannot step.
    assert!(tick_positions(0.0, 100_000.0, 1.0, 5, |v| v / 100_000.0).is_empty());
    assert!(tick_positions(0.0, 0.0, 1.0, 5, |_| 0.0).is_empty());
    assert!(tick_positions(0.0, 10.0, 0.0, 5, |_| 0.0).is_empty());
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

  /// A laid-out slider: a 200px track whose thumbs may travel 8..192, so a
  /// percentage reads straight off the pointer.
  fn laid_out() -> SliderState {
    let mut state = SliderState::new().min(0.0).max(100.0).step(1.0);
    state.bounds = Bounds::new(point(px(0.), px(0.)), gpui::size(px(200.), px(16.)));
    state.travel_inset = px(8.);
    state
  }

  /// Points a preview's hover state at a position on a laid-out slider.
  fn hover_at(state: &SliderState, hover: &mut PreviewHover, x: f32) -> bool {
    hover.set_value(state.value_at_position(Axis::Horizontal, point(px(x), px(8.))))
  }

  #[test]
  fn a_rail_hover_previews_the_value_under_the_pointer() {
    let state = laid_out();
    let mut hover = PreviewHover::default();
    assert!(hover_at(&state, &mut hover, 100.));

    let preview = state.preview(SliderPreviewTrigger::Rail, &hover).unwrap();
    assert_eq!(preview.value, 50.0);
    assert!((preview.percentage - 0.5).abs() < 1e-6);
    assert!(!preview.dragging);
  }

  #[test]
  fn a_hover_inside_one_step_asks_for_no_other_frame() {
    let state = laid_out();
    let mut hover = PreviewHover::default();
    assert!(hover_at(&state, &mut hover, 100.));
    // 0.5px on is still the same step, so the preview cannot change.
    assert!(!hover_at(&state, &mut hover, 100.5));
    assert!(hover_at(&state, &mut hover, 120.));
  }

  #[test]
  fn the_trigger_decides_whether_the_rail_or_a_thumb_previews() {
    // Hovering the rail is not hovering a thumb.
    let state = laid_out();
    let mut hover = PreviewHover::default();
    hover_at(&state, &mut hover, 100.);
    assert!(state.preview(SliderPreviewTrigger::Thumb, &hover).is_none());

    // And a thumb previews its own value, not the one under the pointer.
    let state = laid_out().default_value(30.0);
    let hover = PreviewHover {
      value: None,
      thumb: Some(ThumbKind::Value),
    };
    let preview = state.preview(SliderPreviewTrigger::Thumb, &hover).unwrap();
    assert_eq!(preview.value, 30.0);
    assert!((preview.percentage - 0.3).abs() < 1e-6);
    assert!(state.preview(SliderPreviewTrigger::Rail, &hover).is_none());
  }

  #[test]
  fn a_deferred_drag_previews_the_end_it_moves() {
    let mut state = laid_out().default_value((20.0, 80.0)).with_immediate(false);
    let hover = PreviewHover::default();
    state.dragging = true;

    // The pending value, not the committed one, is what the gesture stands on.
    state.active_thumb_start = true;
    state.drag_value = Some(SliderValue::Range(45.0, 80.0));
    let preview = state.preview(SliderPreviewTrigger::Rail, &hover).unwrap();
    assert_eq!(preview.value, 45.0);
    assert!((preview.percentage - 0.45).abs() < 1e-6);
    assert!(preview.dragging);

    state.active_thumb_start = false;
    state.drag_value = Some(SliderValue::Range(45.0, 60.0));
    let preview = state.preview(SliderPreviewTrigger::Rail, &hover).unwrap();
    assert_eq!(preview.value, 60.0);
    assert!((preview.percentage - 0.6).abs() < 1e-6);
  }

  struct PreviewRoot {
    state: Entity<SliderState>,
    trigger: SliderPreviewTrigger,
  }

  impl Render for PreviewRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      div().size_full().pt(px(100.)).pl(px(50.)).child(
        div().w(px(200.)).child(
          Slider::new("preview-slider", &self.state)
            .preview(|_, _, _| {
              div()
                .id("slider-preview")
                .debug_selector(|| "SLIDER-PREVIEW".into())
                .w(px(60.))
                .h(px(30.))
            })
            .preview_trigger(self.trigger),
        ),
      )
    }
  }

  #[gpui::test]
  fn the_preview_stands_over_the_value_the_pointer_names(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    let (_, cx) = cx.add_window_view(|_, cx| PreviewRoot {
      state: cx.new(|_| SliderState::new().min(0.0).max(100.0).step(1.0)),
      trigger: SliderPreviewTrigger::Rail,
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("SLIDER-PREVIEW").is_none());

    // The slider sits at x 50, y 100 and is 200x32, so its 16px track band is
    // y 108..124 and its travel range runs x 58..242: the middle of it is 50.
    cx.simulate_mouse_move(point(px(150.), px(116.)), None, Modifiers::none());
    cx.run_until_parked();

    let bounds = cx.debug_bounds("SLIDER-PREVIEW").expect("preview on hover");
    assert_eq!(bounds.size, gpui::size(px(60.), px(30.)));
    // Centred on the value and standing just above the band.
    assert_eq!(bounds.center().x, px(150.));
    assert_eq!(bounds.bottom(), px(104.));

    // And leaving the rail takes it away again.
    cx.simulate_mouse_move(point(px(700.), px(500.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("SLIDER-PREVIEW").is_none());
  }

  #[gpui::test]
  fn a_thumb_trigger_previews_only_the_pill(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    let (_, cx) = cx.add_window_view(|_, cx| PreviewRoot {
      state: cx.new(|_| {
        SliderState::new()
          .min(0.0)
          .max(100.0)
          .step(1.0)
          .default_value(30.0)
      }),
      trigger: SliderPreviewTrigger::Thumb,
    });
    cx.run_until_parked();

    // The rail is not a thumb.
    cx.simulate_mouse_move(point(px(150.), px(116.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("SLIDER-PREVIEW").is_none());

    // The value thumb stands at 30% of x 58..242, so its pill straddles 113.
    cx.simulate_mouse_move(point(px(113.), px(116.)), None, Modifiers::none());
    cx.run_until_parked();
    let bounds = cx
      .debug_bounds("SLIDER-PREVIEW")
      .expect("preview on the thumb");
    assert!(
      (bounds.center().x - px(113.2)).abs() < px(1.),
      "preview at {:?}, expected over the thumb at 113",
      bounds.center().x
    );
  }

  #[gpui::test]
  fn a_drag_previews_while_the_button_is_down(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    let (_, cx) = cx.add_window_view(|_, cx| PreviewRoot {
      state: cx.new(|_| SliderState::new().min(0.0).max(100.0).step(1.0)),
      trigger: SliderPreviewTrigger::Rail,
    });
    cx.run_until_parked();

    cx.simulate_mouse_down(
      point(px(150.), px(116.)),
      MouseButton::Left,
      Modifiers::none(),
    );
    cx.run_until_parked();

    // A press drags the value to the pointer, and the preview is there before
    // the first move: a gesture on the rail starts where it landed.
    let bounds = cx
      .debug_bounds("SLIDER-PREVIEW")
      .expect("preview while dragging");
    assert_eq!(bounds.center().x, px(150.));
    assert_eq!(bounds.bottom(), px(104.));
  }

  struct ToggledRoot {
    state: Entity<SliderState>,
    shown: Entity<bool>,
  }

  impl Render for ToggledRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
      let shown = *self.shown.read(cx);
      div()
        .size_full()
        .pt(px(100.))
        .pl(px(50.))
        .children(shown.then(|| {
          div().w(px(200.)).child(
            Slider::new("preview-slider", &self.state).preview(|_, _, _| {
              div()
                .id("slider-preview")
                .debug_selector(|| "SLIDER-PREVIEW".into())
                .w(px(60.))
                .h(px(30.))
            }),
          )
        }))
    }
  }

  #[gpui::test]
  fn a_slider_that_leaves_and_returns_does_not_resurrect_a_hover(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    let (root, cx) = cx.add_window_view(|_, cx| ToggledRoot {
      state: cx.new(|_| SliderState::new().min(0.0).max(100.0).step(1.0)),
      shown: cx.new(|_| true),
    });
    cx.run_until_parked();

    cx.simulate_mouse_move(point(px(150.), px(116.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("SLIDER-PREVIEW").is_some());

    // A floating bar hides itself while the pointer rests on the rail; when it
    // comes back the hover is gone, not frozen at the old position.
    let shown = cx.update(|_, cx| root.read(cx).shown.clone());
    for visible in [false, true] {
      cx.update(|_, cx| {
        shown.update(cx, |shown, cx| {
          *shown = visible;
          cx.notify();
        });
      });
      cx.run_until_parked();
    }
    assert!(cx.debug_bounds("SLIDER-PREVIEW").is_none());

    // And the pointer has to come back for the preview to.
    cx.simulate_mouse_move(point(px(150.), px(116.)), None, Modifiers::none());
    cx.run_until_parked();
    assert!(cx.debug_bounds("SLIDER-PREVIEW").is_some());
  }
}
