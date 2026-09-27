use gpui::{
  App, Entity, EventEmitter, IntoElement, RenderOnce, SharedString, StyleRefinement, Window,
};

use super::{Input, InputState};
use crate::{
  Button, ButtonVariants as _, Disableable, Icon, IconName, Sizable, Size, StyledExt, WidgetGroup,
  WidgetGroupChild,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepAction {
  Decrement,
  Increment,
}

pub enum NumberInputEvent {
  Step(StepAction),
}

/// Emission host for [`NumberInputEvent`]: `NumberInput::apply_step` emits
/// step events through the input state's context, and subscribers listen on
/// the same `Entity<InputState>`. This impl is load-bearing, not dead code.
impl EventEmitter<NumberInputEvent> for InputState {}

#[derive(IntoElement)]
pub struct NumberInput {
  state: Entity<InputState>,
  placeholder: SharedString,
  size: Size,
  appearance: bool,
  disabled: bool,
  style: StyleRefinement,
  step: f64,
  min: Option<f64>,
  max: Option<f64>,
}

impl NumberInput {
  pub fn new(state: &Entity<InputState>) -> Self {
    Self {
      state: state.clone(),
      placeholder: SharedString::default(),
      size: Size::default(),
      appearance: true,
      disabled: false,
      style: StyleRefinement::default(),
      step: 1.0,
      min: None,
      max: None,
    }
  }

  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = placeholder.into();
    self
  }

  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  pub fn step(mut self, step: f64) -> Self {
    self.step = step.max(0.000_001);
    self
  }

  pub fn min(mut self, min: f64) -> Self {
    self.min = Some(min);
    self
  }

  pub fn max(mut self, max: f64) -> Self {
    self.max = Some(max);
    self
  }

  fn format_number(v: f64) -> String {
    let rounded = (v * 1_000_000.0).round() / 1_000_000.0;
    let mut s = rounded.to_string();
    if s.contains('.') {
      while s.ends_with('0') {
        s.pop();
      }
      if s.ends_with('.') {
        s.pop();
      }
    }
    s
  }

  fn apply_step(
    state: &Entity<InputState>, action: StepAction, step: f64, min: Option<f64>, max: Option<f64>,
    window: &mut Window, cx: &mut App,
  ) {
    state.update(cx, |state, cx| {
      if state.disabled {
        return;
      }
      let current = state.text.parse::<f64>().unwrap_or(0.0);
      let mut next = match action {
        StepAction::Increment => current + step,
        StepAction::Decrement => current - step,
      };
      if let Some(min) = min {
        next = next.max(min);
      }
      if let Some(max) = max {
        next = next.min(max);
      }
      state.set_value(Self::format_number(next), window, cx);
      state.focus(window, cx);
      cx.emit(NumberInputEvent::Step(action));
    });
  }
}

impl_disableable!(NumberInput);
impl_sizable!(NumberInput);
impl_styled!(NumberInput);

impl RenderOnce for NumberInput {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    self.state.update(cx, |state, _cx| {
      // Only write back when the value actually changed to avoid touching the
      // entity on every parent re-render.
      if !self.placeholder.is_empty() && state.placeholder != self.placeholder {
        state.placeholder = self.placeholder.clone();
      }
    });

    WidgetGroup::new(("number-input", self.state.entity_id()))
      .with_size(self.size)
      .disabled(self.disabled)
      .refine_style(&self.style)
      .children([
        WidgetGroupChild::from(
          Button::new(("minus", self.state.entity_id()))
            .with_size(self.size)
            .default()
            .icon(Icon::new(IconName::Subtract))
            .tab_stop(false)
            .disabled(self.disabled)
            .on_click({
              let state = self.state.clone();
              let step = self.step;
              let min = self.min;
              let max = self.max;
              move |_, window, cx| {
                Self::apply_step(&state, StepAction::Decrement, step, min, max, window, cx);
              }
            }),
        ),
        WidgetGroupChild::from(
          Input::new(&self.state)
            .appearance(self.appearance)
            .with_size(self.size)
            .disabled(self.disabled),
        ),
        WidgetGroupChild::from(
          Button::new(("plus", self.state.entity_id()))
            .with_size(self.size)
            .default()
            .icon(Icon::new(IconName::Add))
            .tab_stop(false)
            .disabled(self.disabled)
            .on_click({
              let state = self.state.clone();
              let step = self.step;
              let min = self.min;
              let max = self.max;
              move |_, window, cx| {
                Self::apply_step(&state, StepAction::Increment, step, min, max, window, cx);
              }
            }),
        ),
      ])
  }
}
