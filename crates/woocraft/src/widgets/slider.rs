use gpui::{
  App, AppContext as _, Axis, Bounds, Context, DragMoveEvent, Empty, Entity, EntityId,
  EventEmitter, InteractiveElement as _, IntoElement, MouseButton, MouseDownEvent, MouseMoveEvent,
  ParentElement, Pixels, Render, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px, relative,
};

use crate::{ActiveTheme, ElementExt, Size, StyledExt, opacity};

#[derive(Clone)]
struct DragThumb((EntityId, bool));

impl Render for DragThumb {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SliderValue {
  Single(f32),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SliderScale {
  #[default]
  Linear,
  Logarithmic,
}

#[derive(Clone)]
pub enum SliderEvent {
  Change(SliderValue),
}

pub struct SliderState {
  min: f32,
  max: f32,
  step: f32,
  value: SliderValue,
  percentage: std::ops::Range<f32>,
  bounds: Bounds<Pixels>,
  scale: SliderScale,
  active_thumb_start: bool,
}

impl Default for SliderState {
  fn default() -> Self {
    Self::new()
  }
}

impl SliderState {
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
    };
    this.sync_percentage();
    this
  }

  pub fn min(mut self, min: f32) -> Self {
    self.min = min;
    self.sync_percentage();
    self
  }

  pub fn max(mut self, max: f32) -> Self {
    self.max = max;
    self.sync_percentage();
    self
  }

  pub fn step(mut self, step: f32) -> Self {
    self.step = step.max(0.000_001);
    self
  }

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

    if total <= px(0.0) {
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

    if total <= px(0.0) {
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

    if is_start {
      self.value.set_start(value);
    } else {
      self.value.set_end(value);
    }
    self.value = self.snap_and_clamp(self.value);
    self.sync_percentage();
    cx.emit(SliderEvent::Change(self.value));
    cx.notify();
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
}

impl_disableable!(Slider);
impl_sizable!(Slider);
impl_styled!(Slider);

impl RenderOnce for Slider {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let percentage = state.percentage.clone();
    let is_range = state.value.is_range();
    let _ = state;

    let axis = self.axis;
    let entity_id = self.state.entity_id();
    let state_for_down = self.state.clone();
    let state_for_move = self.state.clone();

    let bar_start = relative(percentage.start);
    let bar_end = relative(1. - percentage.end);

    div()
      .id(self.id)
      .when(matches!(axis, Axis::Horizontal), |this| {
        this.h(self.size.component_height()).w_full()
      })
      .when(matches!(axis, Axis::Vertical), |this| {
        this.w(self.size.component_height()).h(px(120.0))
      })
      .items_center()
      .justify_center()
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
          .when(!self.disabled, |this| {
            this
              .cursor_pointer()
              .on_mouse_down(MouseButton::Left, move |e: &MouseDownEvent, _, cx| {
                state_for_down.update(cx, |state, cx| {
                  state.choose_active_thumb(axis, e.position);
                  state.update_by_position(axis, e.position, state.active_thumb_start, cx);
                });
              })
              .on_mouse_move(move |e: &MouseMoveEvent, _, cx| {
                if e.pressed_button == Some(MouseButton::Left) {
                  state_for_move.update(cx, |state, cx| {
                    state.update_by_position(axis, e.position, state.active_thumb_start, cx);
                  });
                }
              })
          })
          .child(
            div()
              .absolute()
              .when(matches!(axis, Axis::Horizontal), |this| {
                this.left(bar_start).right(bar_end).top_0().bottom_0()
              })
              .when(matches!(axis, Axis::Vertical), |this| {
                this.bottom(bar_start).top(bar_end).left_0().right_0()
              })
              .rounded_full()
              .bg(cx.theme().primary),
          )
          .when(is_range, |this| {
            this.child(
              div()
                .id(("slider-thumb-start", self.state.entity_id().as_u64()))
                .absolute()
                .size(self.size.thumb_size())
                .rounded_full()
                .border_2()
                .border_color(cx.theme().primary)
                .bg(cx.theme().background)
                .when(matches!(axis, Axis::Horizontal), |this| {
                  this
                    .top(-(self.size.thumb_size() - self.size.track_thickness()) / 2.0)
                    .left(relative(percentage.start))
                    .ml(-self.size.thumb_size() / 2.0)
                })
                .when(matches!(axis, Axis::Vertical), |this| {
                  this
                    .left(-(self.size.thumb_size() - self.size.track_thickness()) / 2.0)
                    .bottom(relative(percentage.start))
                    .mb(-self.size.thumb_size() / 2.0)
                })
                .when(!self.disabled, |this| {
                  this
                    .cursor_pointer()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                      cx.stop_propagation();
                    })
                    .on_drag(DragThumb((entity_id, true)), |drag, _, _, cx| {
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
                }),
            )
          })
          .child(
            div()
              .id(("slider-thumb-end", self.state.entity_id().as_u64()))
              .absolute()
              .size(self.size.thumb_size())
              .rounded_full()
              .border_2()
              .border_color(cx.theme().primary)
              .bg(cx.theme().background)
              .when(matches!(axis, Axis::Horizontal), |this| {
                this
                  .top(-(self.size.thumb_size() - self.size.track_thickness()) / 2.0)
                  .left(relative(percentage.end))
                  .ml(-self.size.thumb_size() / 2.0)
              })
              .when(matches!(axis, Axis::Vertical), |this| {
                this
                  .left(-(self.size.thumb_size() - self.size.track_thickness()) / 2.0)
                  .bottom(relative(percentage.end))
                  .mb(-self.size.thumb_size() / 2.0)
              })
              .when(!self.disabled, |this| {
                this
                  .cursor_pointer()
                  .on_mouse_down(MouseButton::Left, |_, _, cx| {
                    cx.stop_propagation();
                  })
                  .on_drag(DragThumb((entity_id, false)), |drag, _, _, cx| {
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
              }),
          ),
      )
      .opacity(if self.disabled { opacity::DISABLED } else { 1.0 })
      .refine_style(&self.style)
  }
}
