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
  ActiveTheme, Easing, ElementExt, Size, StyledExt, Transition, duration, opacity, transition,
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

#[derive(Clone)]
/// Events emitted by the slider when value changes.
pub enum SliderEvent {
  /// Emitted when user drags thumb or changes value via keyboard. Contains new
  /// value.
  Change(SliderValue),
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
  }
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

    div()
      .id((thumb_id, entity_id.as_u64()))
      .absolute()
      .rounded(thumb_radius)
      .bg(cx.theme().foreground.opacity(thumb_opacity))
      .when(matches!(axis, Axis::Horizontal), |this| {
        this
          .w(thumb_main)
          .h(thumb_cross)
          .top(-(thumb_cross - track_thickness) / 2.0)
          .left(relative(pct))
          .ml(main_offset)
      })
      .when(matches!(axis, Axis::Vertical), |this| {
        this
          .w(thumb_cross)
          .h(thumb_main)
          .left(-(thumb_cross - track_thickness) / 2.0)
          .bottom(relative(pct))
          .mb(main_offset)
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
    let (percentage, is_range, active_thumb_start) = {
      let state = self.state.read(cx);
      (
        state.percentage.clone(),
        state.value.is_range(),
        state.active_thumb_start,
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

    let state_for_down = self.state.clone();

    let bar_start = relative(percentage.start);
    let bar_end = relative(1. - percentage.end);
    // The fill runs between the thumb centers, which travel 0.5em in from
    // the track ends — correct the percentage insets by that padding so the
    // fill neither overshoots the thumb nor leaves a stub at the extremes.
    let bar_inset = self.size.em(0.5).to_pixels(window.rem_size());
    let bar_start_correction = bar_inset * (1.0 - 2.0 * percentage.start);
    let bar_end_correction = bar_inset * (2.0 * percentage.end - 1.0);

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

    div()
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
              state.update(cx, |state, cx| {
                if state.dragging {
                  state.dragging = false;
                  cx.notify();
                }
              });
            }
          })
          .on_mouse_up_out(MouseButton::Left, {
            let state = self.state.clone();
            move |_, _, cx| {
              state.update(cx, |state, cx| {
                if state.dragging {
                  state.dragging = false;
                  cx.notify();
                }
              });
            }
          })
          // Safety net: if the release itself was missed (e.g. the pointer
          // was released outside the window), clear the gesture on the next
          // unpressed move over the slider.
          .on_mouse_move({
            let state = self.state.clone();
            move |e: &MouseMoveEvent, _, cx| {
              if e.pressed_button.is_none() && state.read(cx).dragging {
                state.update(cx, |state, cx| {
                  state.dragging = false;
                  cx.notify();
                });
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
          .child(
            div()
              .absolute()
              .when(matches!(axis, Axis::Horizontal), |this| {
                this
                  .left(bar_start)
                  .ml(bar_start_correction)
                  .right(bar_end)
                  .mr(bar_end_correction)
                  .top_0()
                  .bottom_0()
              })
              .when(matches!(axis, Axis::Vertical), |this| {
                this
                  .bottom(bar_start)
                  .mb(bar_start_correction)
                  .top(bar_end)
                  .mt(bar_end_correction)
                  .left_0()
                  .right_0()
              })
              .rounded_full()
              .bg(cx.theme().primary),
          )
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
