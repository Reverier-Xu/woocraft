use std::time::Duration;

use gpui::{
  Animation, AnimationExt as _, AnyElement, Bounds, Hsla, IntoElement, Pixels, Styled as _, Window,
  div, fill, linear, px,
};

pub const CARET_THICKNESS: Pixels = px(2.);
pub const CARET_BLINK_DURATION: Duration = Duration::from_millis(1200);
pub const CARET_STEADY_DURATION: Duration = Duration::from_millis(700);

#[inline]
pub fn caret_opacity(delta: f32) -> f32 {
  let wave = (1.0 + (delta * std::f32::consts::TAU).cos()) * 0.5;
  0.1 + wave * 0.9
}

/// Shared caret element for input-like widgets.
pub fn render_caret(color: Hsla, animate: bool, animation_id: &'static str) -> AnyElement {
  let caret = div().h_full().w(CARET_THICKNESS).bg(color);

  if animate {
    caret
      .with_animation(
        animation_id,
        Animation::new(CARET_BLINK_DURATION)
          .repeat()
          .with_easing(linear),
        |this, delta| this.opacity(caret_opacity(delta)),
      )
      .into_any_element()
  } else {
    caret.into_any_element()
  }
}

/// Shared caret paint helper for canvas-style widgets.
pub fn paint_caret(window: &mut Window, bounds: Bounds<Pixels>, color: Hsla, opacity: f32) {
  // Snap to physical pixel boundaries to avoid subpixel anti-aliasing that
  // makes the caret look thinner than the DOM-styled input caret.
  let mut snapped = bounds;
  snapped.origin.x = px(f32::from(snapped.origin.x).round());
  snapped.size.width = px(f32::from(snapped.size.width).max(1.).round());

  window.paint_quad(fill(snapped, color.opacity(opacity)));
}
