use std::time::Instant;

use gpui::{
  App, Context, Entity, EventEmitter, FocusHandle, Focusable, InteractiveElement as _, IntoElement,
  KeyDownEvent, MouseButton, ParentElement as _, Render, RenderOnce, SharedString, Styled, Window,
  div, prelude::FluentBuilder as _,
};

use super::InputEvent;
use crate::{ActiveTheme as _, CARET_STEADY_DURATION, Size, StyleSized, h_flex, render_caret};

pub struct OtpState {
  focus_handle: FocusHandle,
  value: SharedString,
  caret_steady_until: Option<Instant>,
  masked: bool,
  length: usize,
  disabled: bool,
}

impl OtpState {
  pub fn new(length: usize, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      value: SharedString::default(),
      caret_steady_until: None,
      masked: false,
      length: length.max(1),
      disabled: false,
    }
  }

  pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
    self.value = value.into();
    self
  }

  pub fn set_value(
    &mut self, value: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.value = value.into();
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  pub fn value(&self) -> &SharedString {
    &self.value
  }

  pub fn masked(mut self, masked: bool) -> Self {
    self.masked = masked;
    self
  }

  pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.masked = masked;
    cx.notify();
  }

  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    self.focus_handle.focus(window, cx);
  }

  fn hold_caret_visible(&mut self) {
    self.caret_steady_until = Some(Instant::now() + CARET_STEADY_DURATION);
  }

  fn should_animate_caret(&self) -> bool {
    self
      .caret_steady_until
      .is_none_or(|until| Instant::now() >= until)
  }

  fn on_input_mouse_down(
    &mut self, _: &gpui::MouseDownEvent, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.hold_caret_visible();
    self.focus(window, cx);
  }

  fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    if self.disabled {
      return;
    }

    let mut chars: Vec<char> = self.value.chars().collect();
    let key = event.keystroke.key.as_str();

    match key {
      "backspace" => {
        if !chars.is_empty() {
          chars.pop();
          self.value = chars.iter().collect::<String>().into();
          self.hold_caret_visible();
          cx.emit(InputEvent::Change);
          cx.notify();
          window.prevent_default();
          cx.stop_propagation();
        }
      }
      _ => {
        if let Some(ch) = key.chars().next()
          && ch.is_ascii_digit()
          && chars.len() < self.length
        {
          chars.push(ch);
          self.value = chars.iter().collect::<String>().into();
          self.hold_caret_visible();
          cx.emit(InputEvent::Change);
          cx.notify();
          window.prevent_default();
          cx.stop_propagation();
        }
      }
    }
  }
}

impl EventEmitter<InputEvent> for OtpState {}

impl Focusable for OtpState {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for OtpState {
  // `OtpState` is never rendered directly; `OtpInput` draws the actual cells.
  // This impl exists only so the state entity can be embedded as a child
  // element of the OTP view.
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
  }
}

#[derive(IntoElement)]
pub struct OtpInput {
  state: Entity<OtpState>,
  number_of_groups: usize,
  size: Size,
  disabled: bool,
}

impl OtpInput {
  pub fn new(state: &Entity<OtpState>) -> Self {
    Self {
      state: state.clone(),
      number_of_groups: 2,
      size: Size::default(),
      disabled: false,
    }
  }

  pub fn groups(mut self, n: usize) -> Self {
    self.number_of_groups = n.max(1);
    self
  }
}

impl_disableable!(OtpInput);
impl_sizable!(OtpInput);

impl RenderOnce for OtpInput {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let is_focused = state.focus_handle.is_focused(window);
    let animate_caret = state.should_animate_caret();
    let caret_color = cx.theme().primary;
    let cursor_ix = state
      .value
      .chars()
      .count()
      .min(state.length.saturating_sub(1));
    let group_count = self.number_of_groups.max(1).min(state.length);
    let base_group_size = state.length / group_count;
    let extra = state.length % group_count;

    // Build each group directly from its cell range in one pass, avoiding a
    // nested Vec<Vec<AnyElement>> allocation per frame.
    let mut groups = Vec::with_capacity(group_count);
    let mut cell_ix = 0usize;
    for group_ix in 0..group_count {
      let group_size = base_group_size + usize::from(group_ix < extra);
      let mut group = h_flex().items_center().gap(self.size.container_gap());
      for _ in 0..group_size {
        let ch = state.value.chars().nth(cell_ix);
        let focused_cell = is_focused && cell_ix == cursor_ix;

        group = group.child(
          h_flex()
            .border_1()
            .border_color(if focused_cell {
              cx.theme().ring
            } else {
              cx.theme().input
            })
            .bg(if self.disabled {
              cx.theme().muted
            } else {
              cx.theme().background
            })
            .rounded(cx.theme().radius)
            .items_center()
            .justify_center()
            .text_color(if self.disabled {
              cx.theme().muted_foreground
            } else {
              cx.theme().foreground
            })
            .component_h(self.size)
            .w(self.size.component_height())
            .on_mouse_down(
              MouseButton::Left,
              window.listener_for(&self.state, OtpState::on_input_mouse_down),
            )
            .child(match ch {
              Some(c) => {
                if state.masked {
                  "•".to_string().into_any_element()
                } else {
                  c.to_string().into_any_element()
                }
              }
              None => {
                if focused_cell {
                  // The caret matches the tier font size (1em), mirroring the
                  // text input caret, instead of a fixed rem height.
                  div()
                    .h(self.size.text_size())
                    .child(render_caret(caret_color, animate_caret, "otp-caret-blink"))
                    .into_any_element()
                } else {
                  div().into_any_element()
                }
              }
            }),
        );

        cell_ix += 1;
      }
      groups.push(group);
    }

    h_flex()
      .id(("otp-input", self.state.entity_id()))
      .track_focus(&state.focus_handle)
      .when(!self.disabled, |this| {
        this.on_key_down(window.listener_for(&self.state, OtpState::on_key_down))
      })
      .items_center()
      .gap(self.size.container_gap() * 2.0)
      .children(groups)
  }
}
