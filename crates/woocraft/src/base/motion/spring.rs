//! Physical spring motion.
//!
//! A spring carries velocity across retargets, so interrupting a movement
//! continues from the current speed instead of restarting. [`spring`] is the
//! entry point; [`SpringError`] reports configurations gpui rejects.

use std::time::Duration;

use gpui::{App, ElementId, SpringConfig, SpringState, SpringTarget, Window};

use super::transition::TransitionId;

/// Matches GPUI's own default spring settling tolerance.
const SPRING_EPSILON: f32 = 0.001;

#[derive(Clone, Copy, Debug)]
pub struct Spring {
  response: Duration,
  damping: f32,
  epsilon: f32,
  travel: bool,
}

/// Invalid physical or settling parameters for a [`Spring`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SpringError {
  InvalidDamping,
  InvalidEpsilon,
}

impl std::fmt::Display for SpringError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidDamping => f.write_str("spring damping must be finite and non-negative"),
      Self::InvalidEpsilon => f.write_str("spring epsilon must be finite and greater than zero"),
    }
  }
}

impl std::error::Error for SpringError {}

impl Spring {
  /// Builds a spring that reaches its target in about `response` without
  /// overshooting it.
  ///
  /// `response` is not a duration in the sense [`Transition::new`] means one.
  /// A spring has no end to schedule: this is the period one full oscillation
  /// would take without damping, which is the scale the motion is felt at
  /// rather than the moment it stops. The remaining fraction of a percent
  /// keeps settling past it, until it is within the tolerance
  /// [`Self::with_epsilon`] sets.
  ///
  /// A zero response adopts the target on the spot, as a zero duration does
  /// for a transition. Say that with [`Self::with_travel`] where it is what
  /// you mean; a zero here is the degenerate case, defined so an infinitely
  /// stiff spring resolves rather than dividing by its own period.
  pub const fn new(response: Duration) -> Self {
    Self {
      response,
      damping: 1.0,
      epsilon: SPRING_EPSILON,
      travel: true,
    }
  }

  /// Sets the damping ratio, which is `1.0` — no overshoot — by default.
  ///
  /// Below `1.0` the spring passes its target and comes back; above `1.0` it
  /// approaches slowly. Overshoot suits a value with room to pass its target
  /// and nothing to collide with. A height, an opacity, or anything bounded by
  /// the geometry around it should stay at the default.
  ///
  /// This is $\zeta$, not GPUI's `SpringConfig::damping`, which is the
  /// coefficient $c = 2 \zeta \omega_0$.
  ///
  /// # Panics
  ///
  /// Panics when `ratio` is negative or non-finite. Use
  /// [`Self::try_with_damping`] when the value is not a trusted constant.
  pub const fn with_damping(self, ratio: f32) -> Self {
    match self.try_with_damping(ratio) {
      Ok(spring) => spring,
      Err(_) => panic!("spring damping must be finite and non-negative"),
    }
  }

  /// Checked form of [`Self::with_damping`].
  pub const fn try_with_damping(mut self, ratio: f32) -> Result<Self, SpringError> {
    if !ratio.is_finite() || ratio < 0.0 {
      return Err(SpringError::InvalidDamping);
    }
    self.damping = ratio;
    Ok(self)
  }

  /// Sets whether the spring travels to its target or adopts it on the spot.
  ///
  /// A value the pointer is already moving — a panel being dragged by its
  /// resize handle — must not lag behind the pointer, so the spring stops
  /// travelling for as long as the drag lasts. Retained state stays pinned to
  /// the target meanwhile, so travel resumes from the value the drag released
  /// rather than from wherever the spring was when it began.
  ///
  /// This says at the call that the motion is suspended, and it says it
  /// without disturbing the response, damping or tolerance the spring is
  /// configured with — which a policy swapped out for the length of the drag
  /// would have to restate or discard.
  pub const fn with_travel(mut self, travel: bool) -> Self {
    self.travel = travel;
    self
  }

  /// Sets the settling tolerance, expressed in the target's own units.
  ///
  /// The default suits targets that move within a normalized `0..1` range. A
  /// spring over pixels settles perceptibly sooner with a coarser tolerance,
  /// which also ends the animation frames that the remaining sub-pixel motion
  /// would otherwise request.
  ///
  /// # Panics
  ///
  /// Panics when `epsilon` is zero, negative, or non-finite. Use
  /// [`Self::try_with_epsilon`] when the value is not a trusted constant.
  pub const fn with_epsilon(self, epsilon: f32) -> Self {
    match self.try_with_epsilon(epsilon) {
      Ok(spring) => spring,
      Err(_) => panic!("spring epsilon must be finite and greater than zero"),
    }
  }

  /// Checked form of [`Self::with_epsilon`].
  pub const fn try_with_epsilon(mut self, epsilon: f32) -> Result<Self, SpringError> {
    if !epsilon.is_finite() || epsilon <= 0.0 {
      return Err(SpringError::InvalidEpsilon);
    }
    self.epsilon = epsilon;
    Ok(self)
  }

  /// Returns the settling tolerance in the target's own units.
  pub const fn epsilon(self) -> f32 {
    self.epsilon
  }

  /// The physical parameters GPUI integrates. The response must be non-zero;
  /// [`spring`] adopts the target before reaching here when it is not.
  ///
  /// Derived on use rather than stored, so the builders stay `const`: neither
  /// `Duration::as_secs_f32` nor the square root that recovers a damping ratio
  /// from a built config can be called from a `const fn`.
  fn config(&self) -> SpringConfig {
    let frequency = std::f32::consts::TAU / self.response.as_secs_f32();
    SpringConfig::new(frequency * frequency, 2.0 * self.damping * frequency, 1.0)
  }
}

#[derive(Clone, Copy)]
struct SpringTransition {
  state: SpringState,
  target: f32,
  updated_at: std::time::Instant,
}

/// Returns the current value for a spring travelling toward `target`.
///
/// State is keyed by `id` exactly as [`transition`] keys its own. The first
/// value is adopted immediately; later target changes preserve both the current
/// position and the current velocity, so an interrupted spring is redirected
/// rather than restarted.
///
/// Call this while rendering an element, where GPUI keyed element state is
/// available. A channel id must identify one value within that element.
pub fn spring<T>(
  id: impl Into<TransitionId>, target: T, policy: Spring, window: &mut Window, cx: &mut App,
) -> T::Output
where
  T: SpringTarget, {
  let id: ElementId = id.into().into();
  let now = cx.background_executor().now();
  let target_position = target.target();
  let state = window.use_keyed_state(id, cx, |_, _| SpringTransition {
    state: SpringState {
      position: target_position,
      velocity: 0.0,
    },
    target: target_position,
    updated_at: now,
  });

  let snapshot = *state.read(cx);
  let at_rest_on_target =
    snapshot.state.position == target_position && snapshot.state.velocity == 0.0;

  // The overwhelmingly common case: a spring nothing is currently moving. It
  // has no state to advance and no frame to ask for, so it never builds a
  // config or steps one — a settled spring costs a read and two comparisons.
  // Every branch below would return this same value and write nothing.
  //
  // Resting writes nothing, so `updated_at` goes stale for as long as the rest
  // lasts. The next retarget then steps a zero displacement at zero velocity
  // over that whole gap, which any elapsed time leaves where it is, so the
  // stale clock cannot move the value — it only has to not produce a NaN, and
  // every term the propagator scales is finite.
  if at_rest_on_target {
    return target.resolve(target_position);
  }

  let settle = |state: &mut SpringTransition| {
    state.state = SpringState {
      position: target_position,
      velocity: 0.0,
    };
    state.target = target_position;
    state.updated_at = now;
  };

  if cx.reduce_motion() || !policy.travel || policy.response.is_zero() {
    state.update(cx, |state, _| settle(state));
    return target.resolve(target_position);
  }

  // Advance over the frame that just elapsed, which the previous target
  // governed, before adopting the new one for the frame to come.
  let elapsed = now
    .saturating_duration_since(snapshot.updated_at)
    .as_secs_f32();
  let config = policy.config();
  let stepped = config.step(snapshot.state, snapshot.target, elapsed);

  if config.is_settled(stepped, target_position, policy.epsilon) {
    state.update(cx, |state, _| settle(state));
    return target.resolve(target_position);
  }

  state.update(cx, |state, _| {
    state.state = stepped;
    state.target = target_position;
    state.updated_at = now;
  });
  window.request_animation_frame();
  target.resolve(stepped.position)
}
