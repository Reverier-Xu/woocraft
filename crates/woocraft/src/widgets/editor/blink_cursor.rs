use std::time::Instant;

use gpui::{Context, Pixels, Task};

use crate::{
  CARET_BLINK_DURATION, CARET_STEADY_DURATION, CARET_THICKNESS, caret_opacity,
  duration::ANIMATION_FRAME,
};

pub(super) const CURSOR_WIDTH: Pixels = CARET_THICKNESS;

/// Editor caret animation controller.
/// Uses the same timing and opacity curve as widgets/input caret.
pub(crate) struct BlinkCursor {
  active: bool,
  epoch: usize,
  started_at: Instant,
  steady_until: Option<Instant>,

  _task: Task<()>,
}

impl BlinkCursor {
  pub fn new() -> Self {
    Self {
      active: false,
      epoch: 0,
      started_at: Instant::now(),
      steady_until: None,
      _task: Task::ready(()),
    }
  }

  /// Start the blinking
  pub fn start(&mut self, cx: &mut Context<Self>) {
    self.active = true;
    self.started_at = Instant::now();
    self.pause(cx);
    self.tick(self.epoch, cx);
  }

  pub fn stop(&mut self, cx: &mut Context<Self>) {
    self.active = false;
    self.steady_until = None;
    self.next_epoch();
    cx.notify();
  }

  fn next_epoch(&mut self) -> usize {
    self.epoch += 1;
    self.epoch
  }

  fn tick(&mut self, epoch: usize, cx: &mut Context<Self>) {
    if !self.active || epoch != self.epoch {
      return;
    }

    cx.notify();

    // Schedule the next animation frame.
    let epoch = self.next_epoch();
    self._task = cx.spawn(async move |this, cx| {
      cx.background_executor().timer(ANIMATION_FRAME).await;
      if let Some(this) = this.upgrade() {
        _ = this.update(cx, |this, cx| this.tick(epoch, cx));
      }
    });
  }

  pub fn is_active(&self) -> bool {
    self.active
  }

  pub fn opacity(&self) -> f32 {
    if !self.active {
      return 1.0;
    }

    if self
      .steady_until
      .is_some_and(|until| Instant::now() < until)
    {
      return 1.0;
    }

    let period = CARET_BLINK_DURATION.as_secs_f32().max(f32::EPSILON);
    let elapsed = Instant::now().duration_since(self.started_at).as_secs_f32();
    let delta = (elapsed / period).fract();
    caret_opacity(delta)
  }

  /// Keep caret solid for a short steady period after user interaction.
  pub fn pause(&mut self, cx: &mut Context<Self>) {
    self.steady_until = Some(Instant::now() + CARET_STEADY_DURATION);
    cx.notify();
  }
}
