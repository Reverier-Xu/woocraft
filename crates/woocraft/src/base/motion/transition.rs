//! Duration-based transitions and keyframe playback.
//!
//! [`transition`] eases a value toward a target over a fixed duration and
//! restarts from the sampled value when the target changes mid-flight, so a
//! retarget never jumps. [`animate_keyframes`] plays a multi-stop track with
//! CSS-like [`Timing`](super::Timing).

use std::{rc::Rc, time::Duration};

use gpui::{App, ElementId, SharedString, Window};

use super::{
  Easing, Interpolate, MotionStatus, SignedDuration, Timing, easing::ease_out_cubic,
  keyframes::Keyframes,
};

#[derive(Clone)]
pub struct Transition {
  pub(super) duration: Duration,
  pub(super) delay: SignedDuration,
  pub(super) easing: Easing,
}

impl Transition {
  pub fn new(duration: Duration) -> Self {
    Self {
      duration,
      delay: SignedDuration::ZERO,
      easing: Easing::Custom(Rc::new(ease_out_cubic)),
    }
  }

  pub fn delay(mut self, delay: impl Into<SignedDuration>) -> Self {
    self.delay = delay.into();
    self
  }

  /// Sets a custom easing function.
  pub fn ease(mut self, easing: impl Fn(f32) -> f32 + 'static) -> Self {
    self.easing = Easing::Custom(Rc::new(easing));
    self
  }

  /// Sets a named easing policy.
  pub fn easing(mut self, easing: Easing) -> Self {
    self.easing = easing;
    self
  }

  pub(super) fn sample(&self, progress: f32) -> f32 {
    self.easing.sample(progress)
  }

  pub(super) fn progress(&self, elapsed: Duration, duration: Duration) -> (f32, MotionStatus) {
    let Some(active_elapsed) = self.delay.active_elapsed(elapsed) else {
      return (0.0, MotionStatus::Delayed);
    };
    if duration.is_zero() || active_elapsed >= duration {
      return (1.0, MotionStatus::Finished);
    }
    (
      active_elapsed.as_secs_f32() / duration.as_secs_f32(),
      MotionStatus::Running,
    )
  }

  /// The elapsed time at which [`Self::progress`] over the transition's own
  /// duration first reports `Finished`.
  pub(super) fn finishes_after(&self) -> Duration {
    match self.delay {
      SignedDuration::Positive(delay) => delay.saturating_add(self.duration),
      SignedDuration::Negative(delay) => self.duration.saturating_sub(delay),
    }
  }
}

impl From<Duration> for SignedDuration {
  fn from(duration: Duration) -> Self {
    Self::positive(duration)
  }
}

/// Identifies one independently transitioning value.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TransitionId(pub(super) ElementId);

impl From<ElementId> for TransitionId {
  fn from(id: ElementId) -> Self {
    Self(id)
  }
}

impl From<&'static str> for TransitionId {
  fn from(id: &'static str) -> Self {
    Self(id.into())
  }
}

impl From<String> for TransitionId {
  fn from(id: String) -> Self {
    Self(id.into())
  }
}

impl From<SharedString> for TransitionId {
  fn from(id: SharedString) -> Self {
    Self(id.into())
  }
}

impl From<usize> for TransitionId {
  fn from(id: usize) -> Self {
    Self(id.into())
  }
}

impl From<i32> for TransitionId {
  fn from(id: i32) -> Self {
    Self(id.into())
  }
}

impl From<TransitionId> for ElementId {
  fn from(id: TransitionId) -> Self {
    ElementId::NamedChild(id.0.into(), "__woocraft-transition-state".into())
  }
}

impl<I, C> From<(I, C)> for TransitionId
where
  I: Into<ElementId>,
  C: Into<SharedString>,
{
  fn from((id, channel): (I, C)) -> Self {
    Self(ElementId::NamedChild(id.into().into(), channel.into()))
  }
}

#[derive(Clone)]
struct ValueTransition<T> {
  from: T,
  target: T,
  started_at: std::time::Instant,
  reversing_factor: f32,
  duration: Duration,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionValue<T> {
  pub value: T,
  pub status: MotionStatus,
}

/// Returns the current value for a CSS-like transition toward `target`.
///
/// State is keyed by `id`. The first value is adopted immediately; later target
/// changes transition from the value sampled at that instant. Components opt
/// into this function explicitly — base components do not install default
/// motion.
///
/// Call this while rendering an element, where GPUI keyed element state is
/// available. A channel id must identify one value type within that element.
pub fn transition<T>(
  id: impl Into<TransitionId>, target: T, policy: Transition, window: &mut Window, cx: &mut App,
) -> T
where
  T: Interpolate + PartialEq + 'static, {
  transition_with_status(id, target, policy, window, cx).value
}

pub fn transition_with_status<T>(
  id: impl Into<TransitionId>, target: T, policy: Transition, window: &mut Window, cx: &mut App,
) -> MotionValue<T>
where
  T: Interpolate + PartialEq + 'static, {
  let id: ElementId = id.into().into();
  let now = cx.background_executor().now();
  let state = window.use_keyed_state(id, cx, |_, _| ValueTransition {
    from: target.clone(),
    target: target.clone(),
    started_at: now,
    reversing_factor: 1.0,
    duration: policy.duration,
  });

  let snapshot = state.read(cx).clone();

  if cx.reduce_motion() || policy.duration.is_zero() {
    if snapshot.from != target || snapshot.target != target {
      state.update(cx, |state, _| {
        state.from = target.clone();
        state.target = target.clone();
        state.started_at = now;
        state.reversing_factor = 1.0;
        state.duration = policy.duration;
      });
    }
    return MotionValue {
      value: target,
      status: MotionStatus::Finished,
    };
  }

  let elapsed = now.saturating_duration_since(snapshot.started_at);
  let (progress, status) = policy.progress(elapsed, snapshot.duration);
  let sampled = snapshot
    .from
    .interpolate(&snapshot.target, policy.sample(progress));

  let (value, status) = if snapshot.target != target {
    let reversing = target == snapshot.from;
    let reversing_factor = if reversing {
      (policy.sample(progress) * snapshot.reversing_factor + (1.0 - snapshot.reversing_factor))
        .clamp(0.0, 1.0)
    } else {
      1.0
    };
    let duration = policy.duration.mul_f32(reversing_factor);
    state.update(cx, |state, _| {
      state.from = sampled.clone();
      state.target = target.clone();
      state.started_at = now;
      state.reversing_factor = reversing_factor;
      state.duration = duration;
    });
    let (initial_progress, initial_status) = policy.progress(Duration::ZERO, duration);
    (
      sampled.interpolate(&target, policy.sample(initial_progress)),
      initial_status,
    )
  } else {
    (
      sampled,
      if snapshot.from == snapshot.target {
        MotionStatus::Idle
      } else {
        status
      },
    )
  };
  if matches!(status, MotionStatus::Delayed | MotionStatus::Running) {
    window.request_animation_frame();
  }
  MotionValue { value, status }
}

#[derive(Clone, Copy)]
struct KeyframePlayback {
  started_at: std::time::Instant,
}

/// Samples a keyed keyframe playback and requests frames while it is active.
///
/// The stable `id` owns the playback's start time. Re-rendering with the same
/// ID continues that playback; it does not restart when `keyframes` or `timing`
/// is reconstructed. To replay a sequence, include an application-owned
/// generation in the ID, for example `("notification-enter", generation)`.
pub fn animate_keyframes<T>(
  id: impl Into<TransitionId>, keyframes: &Keyframes<T>, timing: Timing, window: &mut Window,
  cx: &mut App,
) -> MotionValue<T>
where
  T: Interpolate + 'static, {
  let id: TransitionId = id.into();
  let id = ElementId::NamedChild(ElementId::from(id).into(), "__keyframes".into());
  let now = cx.background_executor().now();
  let state = window.use_keyed_state(id, cx, |_, _| KeyframePlayback { started_at: now });
  let started_at = state.read(cx).started_at;

  if cx.reduce_motion() {
    return MotionValue {
      value: keyframes.sample(1.0),
      status: MotionStatus::Finished,
    };
  }

  let sample = timing.sample(now.saturating_duration_since(started_at));
  let status = sample.phase;
  if matches!(status, MotionStatus::Delayed | MotionStatus::Running) {
    window.request_animation_frame();
  }
  MotionValue {
    value: keyframes.sample(sample.directed_progress),
    status,
  }
}
