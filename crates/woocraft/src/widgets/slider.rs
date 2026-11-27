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
  AnyElement, App, AppContext as _, Axis, Bounds, Context, Div, DragMoveEvent, Empty, Entity,
  EntityId, EventEmitter, Hsla, InteractiveElement as _, IntoElement, Length, MouseButton,
  MouseDownEvent, MouseMoveEvent, ParentElement, Pixels, Rems, Render, RenderOnce, SharedString,
  Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, deferred, div,
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
    // which is what the two stubs' outer corners carry.
    let radius = rems(thickness.0 * 0.5);

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
          )),
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
}
