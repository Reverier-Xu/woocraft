//! Shared debug utilities for woocraft examples.
//!
//! Every example target declares `mod common;` to include this file. Cargo
//! only auto-discovers `examples/*.rs` and `examples/*/main.rs` as targets,
//! so this module is never built standalone.

use gpui::{App, InteractiveElement as _, IntoElement, ParentElement, Styled, Window, div};
use woocraft::{
  ActiveTheme as _, Button, ButtonVariants as _, Disableable as _, Sizable as _, h_flex,
};

/// Rem-size adjustment step and bounds for the debug control.
const REM_STEP: f32 = 1.0;
const REM_MIN: f32 = 8.0;
const REM_MAX: f32 = 32.0;

/// Floating debug overlay with − / + buttons that override the window's base
/// rem size, for eyeballing how em-relative component metrics scale.
///
/// `set_rem_size` does not refresh on its own, so the buttons refresh the
/// window explicitly to re-layout at the new base size.
pub fn rem_size_control(window: &Window, cx: &App) -> impl IntoElement {
  let rem_size = window.rem_size().as_f32();

  div().absolute().bottom_2().right_2().child(
    h_flex()
      .id("rem-size-control")
      .occlude()
      .items_center()
      .gap_1()
      .p_1()
      .rounded(cx.theme().radius)
      .bg(cx.theme().background.opacity(0.9))
      .border_1()
      .border_color(cx.theme().border)
      .child(
        Button::new("rem-size-dec")
          .flat()
          .small()
          .label("-")
          .disabled(rem_size <= REM_MIN)
          .on_click(|_, window, _| {
            let next = (window.rem_size().as_f32() - REM_STEP).max(REM_MIN);
            window.set_rem_size(next);
            window.refresh();
          }),
      )
      .child(
        div()
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child(format!("{rem_size:.0}px")),
      )
      .child(
        Button::new("rem-size-inc")
          .flat()
          .small()
          .label("+")
          .disabled(rem_size >= REM_MAX)
          .on_click(|_, window, _| {
            let next = (window.rem_size().as_f32() + REM_STEP).min(REM_MAX);
            window.set_rem_size(next);
            window.refresh();
          }),
      ),
  )
}
