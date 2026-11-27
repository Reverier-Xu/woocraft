//! Deterministic motion sampling and lifecycle for value-level animation.
//!
//! This module owns keyed retained state, duration/easing policies,
//! interruption and reversal semantics, animation-frame requests, and
//! reduced-motion behavior — while leaving every visual choice (which
//! property animates, with what timing) to the caller.
//!
//! The main entry points:
//!
//! - [`transition`]/[`transition_with_status`]: a value easing toward a target
//!   over a fixed duration, restarting from the sampled value on retarget.
//! - [`spring`]: a physical spring that preserves velocity across retargets.
//! - [`animate_keyframes`]: a keyed multi-stop playback with CSS-like
//!   [`Timing`] (delay, iterations, direction).
//! - [`Presence`]: mount/unmount lifecycle with enter and exit phases.
//! - [`Sequence`]: chained transitions that hand over at their boundaries.
//! - [`Stagger`]: allocation-free timing offsets across a list.
//! - [`MotionReveal`]: a measured, clipped vertical reveal element.
//!
//! Every primitive consults `App::reduce_motion` and resolves immediately
//! without requesting frames when it is set.
//!
//! # Component adoption
//!
//! - **switch** — thumb slide, fill width, and thumb colors transition via
//!   [`transition`] (replaces the former settle-timer scheme; interruptions
//!   reverse from the sampled value).
//! - **checkbox** — checkmark fade plus fill/border cross-fade.
//! - **dialog**, **popover**, **tooltip**, **notification** cards — enter
//!   animations via [`Presence`]; their overlay/card subtrees unmount when
//!   closed, so keyed state drops and each open replays the entrance. Exit
//!   animations are not installed (would require exit-phase mounting support in
//!   the overlay/queue lifecycles).
//!
//! Deliberately not on this system, with reasons recorded at each site:
//! caret blink and spinner rotation (continuous loops, GPUI's repeating
//! `with_animation` is the right tool), the editor's blink cursor (focus- and
//! typing-coupled controller), the scrollbar fade (hover/scroll/idle state
//! machine evaluated in paint), the notification countdown bar (one shared
//! tick loop for the whole queue), `Progress` (no stable element id), tree
//! expand/collapse (flattened index-keyed rows), and the slider thumb (no
//! drag-lifecycle state; layout-relative positions would re-layout per
//! frame).

use gpui::App;

mod easing;
mod interpolate;
mod keyframes;
mod presence;
mod reduce_motion;
mod reveal;
mod sequence;
mod spring;
mod stagger;
mod timing;
mod transition;

pub use easing::{Easing, EasingError, LinearStop, StepPosition};
pub use interpolate::{Interpolate, MotionTransform};
pub use keyframes::{Discrete, DiscreteError, Keyframe, KeyframeError, Keyframes};
pub use presence::{Presence, PresencePhase, PresenceSample};
pub use reduce_motion::apply_system_reduce_motion;
pub use reveal::MotionReveal;
pub use sequence::{Sequence, SequenceSample, SequenceStep};
pub use spring::{Spring, SpringError, spring};
pub use stagger::{Stagger, StaggerOrigin};
pub use timing::{
  IterationCount, MotionStatus, PlaybackDirection, SignedDuration, Timing, TimingSample,
};
pub use transition::{
  MotionValue, Transition, TransitionId, animate_keyframes, transition, transition_with_status,
};

/// Reads the operating system's reduced-motion preference into GPUI's
/// reduce-motion flag. Called by [`crate::base::init`].
pub(crate) fn init(cx: &mut App) {
  reduce_motion::init(cx);
}

#[cfg(test)]
mod css_timing_tests {
  use std::time::Duration;

  use super::{
    Easing, IterationCount, LinearStop, MotionStatus, PlaybackDirection, SignedDuration,
    StepPosition, Timing,
  };

  #[test]
  fn css_keyword_easing_matches_published_reference_samples() {
    for (easing, samples) in [
      (Easing::Ease, [(0.2, 0.295), (0.5, 0.802), (0.8, 0.976)]),
      (Easing::EaseIn, [(0.2, 0.062), (0.5, 0.315), (0.8, 0.692)]),
      (Easing::EaseOut, [(0.2, 0.308), (0.5, 0.685), (0.8, 0.938)]),
      (Easing::EaseInOut, [(0.2, 0.082), (0.5, 0.5), (0.8, 0.918)]),
    ] {
      for (progress, expected) in samples {
        let actual = easing.sample(progress);
        assert!(
          (actual - expected).abs() < 0.002,
          "{easing:?}({progress}) = {actual}, expected {expected}"
        );
      }
    }
  }

  #[test]
  fn step_easing_observes_css_jump_positions() {
    let start = Easing::steps(4, StepPosition::JumpStart).unwrap();
    let end = Easing::steps(4, StepPosition::JumpEnd).unwrap();

    assert_eq!(start.sample(0.0), 0.25);
    assert_eq!(start.sample(0.24), 0.25);
    assert_eq!(start.sample(0.25), 0.5);
    assert_eq!(end.sample(0.0), 0.0);
    assert_eq!(end.sample(0.24), 0.0);
    assert_eq!(end.sample(0.25), 0.25);
    assert!(Easing::steps(0, StepPosition::JumpEnd).is_err());

    let none = Easing::steps(4, StepPosition::JumpNone).unwrap();
    let both = Easing::steps(4, StepPosition::JumpBoth).unwrap();
    assert_eq!(none.sample(0.0), 0.0);
    assert!((none.sample(0.5) - 2.0 / 3.0).abs() < f32::EPSILON);
    assert_eq!(none.sample(1.0), 1.0);
    assert_eq!(both.sample(0.0), 0.2);
    assert_eq!(both.sample(1.0), 1.0);
    assert!(Easing::steps(1, StepPosition::JumpNone).is_err());
  }

  #[test]
  fn linear_stops_fill_omitted_positions_before_sampling() {
    let easing = Easing::linear_stops([
      LinearStop::at(0.0, 0.0),
      LinearStop::new(0.2),
      LinearStop::new(0.8),
      LinearStop::at(1.0, 1.0),
    ])
    .unwrap();

    assert!((easing.sample(1.0 / 3.0) - 0.2).abs() < 1e-6);
    assert!((easing.sample(0.5) - 0.5).abs() < 1e-6);
    assert!(Easing::linear_stops([LinearStop::at(0.0, 0.8), LinearStop::at(1.0, 0.2)]).is_err());
  }

  #[test]
  fn negative_delay_starts_inside_the_active_interval() {
    let timing = Timing::new(Duration::from_millis(100))
      .delay(SignedDuration::negative(Duration::from_millis(25)));
    let sample = timing.sample(Duration::ZERO);

    assert_eq!(sample.phase, MotionStatus::Running);
    assert!((sample.directed_progress - 0.25).abs() < f32::EPSILON);
  }

  #[test]
  fn alternate_direction_reverses_odd_iterations() {
    let timing = Timing::new(Duration::from_millis(100))
      .iterations(IterationCount::Finite(2))
      .direction(PlaybackDirection::Alternate)
      .ease(Easing::Linear);

    let first = timing.sample(Duration::from_millis(25));
    let second = timing.sample(Duration::from_millis(125));
    let finished = timing.sample(Duration::from_millis(200));

    assert_eq!(first.iteration, 0);
    assert_eq!(first.directed_progress, 0.25);
    assert_eq!(second.iteration, 1);
    assert_eq!(second.directed_progress, 0.75);
    assert_eq!(finished.phase, MotionStatus::Finished);
    assert_eq!(finished.directed_progress, 0.0);
  }
}

#[cfg(test)]
mod motion_track_tests {
  use std::time::Duration;

  use gpui::{Bounds, Point, Size, point, px, size};

  use super::{
    Discrete, Easing, Interpolate as _, Keyframe, KeyframeError, Keyframes, MotionTransform,
    Stagger, StaggerOrigin,
  };

  #[test]
  fn keyframes_validate_offsets_and_sample_each_segments_easing() {
    assert!(matches!(
      Keyframes::try_new([Keyframe::new(0.2, 0.0_f32), Keyframe::new(1.0, 1.0_f32),]),
      Err(KeyframeError::MissingEndpoint)
    ));
    assert!(matches!(
      Keyframes::try_new([
        Keyframe::new(0.0, 0.0_f32),
        Keyframe::new(0.8, 1.0_f32),
        Keyframe::new(0.7, 2.0_f32),
        Keyframe::new(1.0, 3.0_f32),
      ]),
      Err(KeyframeError::OffsetsNotMonotonic)
    ));

    let track = Keyframes::try_new([
      Keyframe::new(0.0, 0.0_f32).ease(Easing::steps(2, super::StepPosition::JumpEnd).unwrap()),
      Keyframe::new(0.5, 10.0_f32).ease(Easing::Linear),
      Keyframe::new(1.0, 20.0_f32),
    ])
    .unwrap();

    assert_eq!(track.sample(0.2), 0.0);
    assert_eq!(track.sample(0.3), 5.0);
    assert_eq!(track.sample(0.75), 15.0);
    assert_eq!(track.sample(1.0), 20.0);
  }

  #[test]
  fn discrete_values_switch_only_at_the_requested_progress() {
    let value = Discrete::new("old", "new").switch_at(0.75).unwrap();
    assert_eq!(value.sample(0.749), "old");
    assert_eq!(value.sample(0.75), "new");
    assert!(Discrete::new(0, 1).switch_at(f32::NAN).is_err());
  }

  #[test]
  fn stagger_origins_produce_stable_delays_without_allocating_a_schedule() {
    let interval = Duration::from_millis(20);
    let first = Stagger::new(interval, StaggerOrigin::First);
    let last = Stagger::new(interval, StaggerOrigin::Last);
    let center = Stagger::new(interval, StaggerOrigin::Center);

    assert_eq!(first.delay(3, 5), Duration::from_millis(60));
    assert_eq!(last.delay(3, 5), Duration::from_millis(20));
    assert_eq!(center.delay(2, 5), Duration::ZERO);
    assert_eq!(center.delay(0, 5), Duration::from_millis(40));
    assert_eq!(first.delay(7, 0), Duration::ZERO);
  }

  #[test]
  fn common_gpui_geometry_interpolates_channel_by_channel() {
    let from_size = size(px(10.0), px(20.0));
    let to_size = size(px(30.0), px(60.0));
    assert_eq!(
      from_size.interpolate(&to_size, 0.25),
      size(px(15.0), px(30.0))
    );

    let from = Bounds::new(point(px(0.0), px(10.0)), from_size);
    let to = Bounds::new(point(px(40.0), px(50.0)), to_size);
    assert_eq!(
      from.interpolate(&to, 0.5),
      Bounds::new(point(px(20.0), px(30.0)), size(px(20.0), px(40.0)))
    );

    let _: Point<gpui::Pixels> = from.origin;
    let _: Size<gpui::Pixels> = from.size;

    let transform = MotionTransform::identity().interpolate(
      &MotionTransform {
        translation: point(px(20.0), px(40.0)),
        scale: point(2.0, 0.5),
        rotation_radians: std::f32::consts::PI,
        opacity: 0.0,
      },
      0.5,
    );
    assert_eq!(transform.translation, point(px(10.0), px(20.0)));
    assert_eq!(transform.scale, point(1.5, 0.75));
    assert_eq!(transform.rotation_radians, std::f32::consts::FRAC_PI_2);
    assert_eq!(transform.opacity, 0.5);
  }
}

#[cfg(test)]
mod tests {
  use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
  };

  use gpui::{
    ElementId, Empty, IntoElement, Render, TestAppContext, Window, WindowHandle, px, size,
  };

  use super::*;

  struct StatusView {
    target: Rc<Cell<f32>>,
    policy: Transition,
    samples: Rc<RefCell<Vec<MotionValue<f32>>>>,
  }

  impl Render for StatusView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(transition_with_status(
        ("status-test", "value"),
        self.target.get(),
        self.policy.clone(),
        window,
        cx,
      ));
      Empty
    }
  }

  struct StatusFixture {
    window: WindowHandle<StatusView>,
    target: Rc<Cell<f32>>,
    samples: Rc<RefCell<Vec<MotionValue<f32>>>>,
  }

  impl StatusFixture {
    fn open(cx: &mut TestAppContext, policy: Transition) -> Self {
      let target = Rc::new(Cell::new(0.0));
      let samples = Rc::new(RefCell::new(Vec::new()));
      let window = cx.open_window(size(px(100.), px(100.)), {
        let target = target.clone();
        let samples = samples.clone();
        move |_, _| StatusView {
          target,
          policy,
          samples,
        }
      });
      cx.run_until_parked();
      Self {
        window,
        target,
        samples,
      }
    }

    fn render(&self, cx: &mut TestAppContext, target: f32) -> MotionValue<f32> {
      self.target.set(target);
      self
        .window
        .update(cx, |_, window, _| window.refresh())
        .unwrap();
      cx.run_until_parked();
      *self.samples.borrow().last().unwrap()
    }
  }

  #[gpui::test]
  fn status_transition_reports_delay_running_and_finished(cx: &mut TestAppContext) {
    let fixture = StatusFixture::open(
      cx,
      Transition::new(Duration::from_millis(100)).delay(Duration::from_millis(20)),
    );
    assert_eq!(fixture.render(cx, 1.0).status, MotionStatus::Delayed);

    cx.executor().advance_clock(Duration::from_millis(20));
    assert_eq!(fixture.render(cx, 1.0).status, MotionStatus::Running);
    cx.executor().advance_clock(Duration::from_millis(100));
    assert_eq!(fixture.render(cx, 1.0).status, MotionStatus::Finished);
  }

  #[gpui::test]
  fn negative_delay_samples_a_target_change_inside_its_interval(cx: &mut TestAppContext) {
    let fixture = StatusFixture::open(
      cx,
      Transition::new(Duration::from_millis(100))
        .delay(SignedDuration::negative(Duration::from_millis(25)))
        .ease(|t| t),
    );
    let sample = fixture.render(cx, 1.0);
    assert_eq!(sample.status, MotionStatus::Running);
    assert_eq!(sample.value, 0.25);
  }

  #[gpui::test]
  fn a_direct_reversal_shortens_the_return_transition(cx: &mut TestAppContext) {
    let fixture = StatusFixture::open(cx, Transition::new(Duration::from_millis(100)).ease(|t| t));
    assert_eq!(fixture.render(cx, 1.0).value, 0.0);
    cx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(fixture.render(cx, 0.0).value, 0.5);
    cx.executor().advance_clock(Duration::from_millis(25));
    assert_eq!(fixture.render(cx, 0.0).value, 0.25);
  }

  struct KeyframeView {
    track: Keyframes<f32>,
    timing: Timing,
    samples: Rc<RefCell<Vec<MotionValue<f32>>>>,
  }

  impl Render for KeyframeView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(animate_keyframes(
        "keyframe-test",
        &self.track,
        self.timing.clone(),
        window,
        cx,
      ));
      Empty
    }
  }

  #[gpui::test]
  fn keyed_keyframes_follow_timing_and_stop_after_completion(cx: &mut TestAppContext) {
    let samples = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.), px(100.)), {
      let samples = samples.clone();
      move |_, _| KeyframeView {
        track: Keyframes::try_new([Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 10.0)]).unwrap(),
        timing: Timing::new(Duration::from_millis(100)),
        samples,
      }
    });
    cx.run_until_parked();
    assert_eq!(samples.borrow().last().unwrap().value, 0.0);
    assert_eq!(
      samples.borrow().last().unwrap().status,
      MotionStatus::Running
    );

    cx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(
      window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap(),
      1
    );
    cx.run_until_parked();
    assert_eq!(samples.borrow().last().unwrap().value, 5.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    window.update(cx, |_, window, _| window.refresh()).unwrap();
    cx.run_until_parked();
    assert_eq!(
      samples.borrow().last().unwrap().status,
      MotionStatus::Finished
    );
    window
      .update(cx, |_, window, cx| window.simulate_next_frame(cx))
      .unwrap();
    cx.run_until_parked();
    assert_eq!(
      window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap(),
      0
    );
  }

  struct PresenceView {
    present: Rc<Cell<bool>>,
    samples: Rc<RefCell<Vec<PresenceSample>>>,
  }

  impl Render for PresenceView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(
        Presence::new("presence-test", self.present.get())
          .transition(Transition::new(Duration::from_millis(100)).ease(|t| t))
          .sample(window, cx),
      );
      Empty
    }
  }

  struct PresenceFixture {
    window: WindowHandle<PresenceView>,
    present: Rc<Cell<bool>>,
    samples: Rc<RefCell<Vec<PresenceSample>>>,
  }

  impl PresenceFixture {
    fn open(cx: &mut TestAppContext, initially_present: bool) -> Self {
      let present = Rc::new(Cell::new(initially_present));
      let samples = Rc::new(RefCell::new(Vec::new()));
      let window = cx.open_window(size(px(100.), px(100.)), {
        let present = present.clone();
        let samples = samples.clone();
        move |_, _| PresenceView { present, samples }
      });
      cx.run_until_parked();
      Self {
        window,
        present,
        samples,
      }
    }

    fn render(&self, cx: &mut TestAppContext, present: bool) -> PresenceSample {
      self.present.set(present);
      self
        .window
        .update(cx, |_, window, _| window.refresh())
        .unwrap();
      cx.run_until_parked();
      *self.samples.borrow().last().unwrap()
    }
  }

  #[gpui::test]
  fn presence_enters_exits_and_only_unmounts_after_exit(cx: &mut TestAppContext) {
    let fixture = PresenceFixture::open(cx, true);
    let entering = *fixture.samples.borrow().last().unwrap();
    assert_eq!(entering.phase, PresencePhase::Entering);
    assert_eq!(entering.progress, 0.0);
    assert!(entering.should_render());

    cx.executor().advance_clock(Duration::from_millis(100));
    let present = fixture.render(cx, true);
    assert_eq!(present.phase, PresencePhase::Present);
    assert_eq!(present.progress, 1.0);

    let exiting = fixture.render(cx, false);
    assert_eq!(exiting.phase, PresencePhase::Exiting);
    assert_eq!(exiting.progress, 1.0);
    assert!(exiting.should_render());

    cx.executor().advance_clock(Duration::from_millis(100));
    let absent = fixture.render(cx, false);
    assert_eq!(absent.phase, PresencePhase::Absent);
    assert_eq!(absent.progress, 0.0);
    assert!(!absent.should_render());
  }

  #[gpui::test]
  fn presence_reentry_reverses_from_the_exit_sample(cx: &mut TestAppContext) {
    let fixture = PresenceFixture::open(cx, true);
    cx.executor().advance_clock(Duration::from_millis(100));
    fixture.render(cx, true);
    fixture.render(cx, false);
    cx.executor().advance_clock(Duration::from_millis(40));
    let reentering = fixture.render(cx, true);

    assert_eq!(reentering.phase, PresencePhase::Entering);
    assert_eq!(reentering.progress, 0.6);
  }

  #[gpui::test]
  fn reduced_motion_resolves_presence_without_a_pending_frame(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let fixture = PresenceFixture::open(cx, true);
    assert_eq!(
      fixture.samples.borrow().last().unwrap().phase,
      PresencePhase::Present
    );
    assert_eq!(
      fixture
        .window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap(),
      0
    );
    assert_eq!(fixture.render(cx, false).phase, PresencePhase::Absent);
  }

  struct SequenceView {
    steps: Rc<RefCell<Vec<SequenceStep<f32>>>>,
    samples: Rc<RefCell<Vec<SequenceSample<f32>>>>,
  }

  impl Render for SequenceView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(
        Sequence::new("sequence-test", 0.0)
          .with_steps(self.steps.borrow().iter().cloned())
          .sample(window, cx),
      );
      Empty
    }
  }

  struct SequenceFixture {
    window: WindowHandle<SequenceView>,
    steps: Rc<RefCell<Vec<SequenceStep<f32>>>>,
    samples: Rc<RefCell<Vec<SequenceSample<f32>>>>,
  }

  impl SequenceFixture {
    fn open(cx: &mut TestAppContext, steps: Vec<SequenceStep<f32>>) -> Self {
      let steps = Rc::new(RefCell::new(steps));
      let samples = Rc::new(RefCell::new(Vec::new()));
      let window = cx.open_window(size(px(100.), px(100.)), {
        let steps = steps.clone();
        let samples = samples.clone();
        move |_, _| SequenceView { steps, samples }
      });
      cx.run_until_parked();
      Self {
        window,
        steps,
        samples,
      }
    }

    fn linear(millis: u64) -> Transition {
      Transition::new(Duration::from_millis(millis)).ease(|t| t)
    }

    fn last(&self) -> SequenceSample<f32> {
      *self.samples.borrow().last().unwrap()
    }

    fn render(&self, cx: &mut TestAppContext) -> SequenceSample<f32> {
      self
        .window
        .update(cx, |_, window, _| window.refresh())
        .unwrap();
      cx.run_until_parked();
      self.last()
    }

    fn advance(&self, cx: &mut TestAppContext, millis: u64) -> SequenceSample<f32> {
      cx.executor().advance_clock(Duration::from_millis(millis));
      self.render(cx)
    }

    fn pending_frame(&self, cx: &mut TestAppContext) -> usize {
      self
        .window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap()
    }
  }

  fn assert_sample(sample: SequenceSample<f32>, value: f32, step: usize, status: MotionStatus) {
    assert_eq!(
      (*sample.value(), sample.step(), sample.status()),
      (value, step, status)
    );
  }

  #[gpui::test]
  fn sequence_steps_advance_in_order_at_their_boundaries(cx: &mut TestAppContext) {
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
        SequenceStep::new(
          20.0,
          SequenceFixture::linear(100).delay(Duration::from_millis(50)),
        ),
      ],
    );
    assert_sample(fixture.last(), 0.0, 0, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 50), 5.0, 0, MotionStatus::Running);

    // The first step ends and the second begins within one frame; nothing
    // reports the first step as finished in between.
    assert_sample(fixture.advance(cx, 50), 10.0, 1, MotionStatus::Delayed);
    assert_sample(fixture.advance(cx, 50), 10.0, 1, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 50), 15.0, 1, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 50), 20.0, 1, MotionStatus::Finished);
  }

  #[gpui::test]
  fn a_sequence_starts_its_next_step_where_the_previous_ended_not_at_the_frame(
    cx: &mut TestAppContext,
  ) {
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
        SequenceStep::new(20.0, SequenceFixture::linear(100)),
      ],
    );
    // One frame at 50 ms, the next at 175 ms: the boundary at 100 ms fell
    // between them, and the second step is sampled as if it started there.
    fixture.advance(cx, 50);
    assert_sample(fixture.advance(cx, 125), 17.5, 1, MotionStatus::Running);
  }

  #[gpui::test]
  fn zero_duration_steps_complete_within_the_frame_that_reaches_them(cx: &mut TestAppContext) {
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(5.0, SequenceFixture::linear(0)),
        SequenceStep::new(6.0, SequenceFixture::linear(0)),
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
      ],
    );
    assert_sample(fixture.last(), 6.0, 2, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 50), 8.0, 2, MotionStatus::Running);
  }

  #[gpui::test]
  fn a_sequence_reports_finished_once_and_then_stops_requesting_frames(cx: &mut TestAppContext) {
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
        SequenceStep::new(20.0, SequenceFixture::linear(100)),
      ],
    );
    assert_eq!(fixture.pending_frame(cx), 1);
    cx.run_until_parked();

    cx.executor().advance_clock(Duration::from_millis(100));
    assert_eq!(fixture.pending_frame(cx), 1);
    cx.run_until_parked();
    assert_eq!(fixture.last().status(), MotionStatus::Running);
    assert!(
      fixture
        .samples
        .borrow()
        .iter()
        .all(|sample| !sample.is_finished()),
      "the first step's end must not read as the sequence finishing"
    );

    assert_sample(fixture.advance(cx, 100), 20.0, 1, MotionStatus::Finished);
    fixture.pending_frame(cx);
    cx.run_until_parked();
    assert_eq!(fixture.pending_frame(cx), 0);
    assert_sample(fixture.advance(cx, 1_000), 20.0, 1, MotionStatus::Finished);
  }

  #[gpui::test]
  fn reduced_motion_adopts_a_sequences_last_target_without_requesting_a_frame(
    cx: &mut TestAppContext,
  ) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
        SequenceStep::new(20.0, SequenceFixture::linear(100)),
      ],
    );
    assert_sample(fixture.last(), 20.0, 1, MotionStatus::Finished);
    assert_eq!(fixture.pending_frame(cx), 0);
  }

  #[gpui::test]
  fn a_changed_step_target_restarts_the_sequence_from_the_sampled_value(cx: &mut TestAppContext) {
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
        SequenceStep::new(20.0, SequenceFixture::linear(100)),
      ],
    );
    fixture.advance(cx, 150);
    assert_sample(fixture.last(), 15.0, 1, MotionStatus::Running);

    *fixture.steps.borrow_mut() = vec![
      SequenceStep::new(10.0, SequenceFixture::linear(100)),
      SequenceStep::new(35.0, SequenceFixture::linear(100)),
    ];
    assert_sample(fixture.render(cx), 15.0, 0, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 50), 12.5, 0, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 100), 22.5, 1, MotionStatus::Running);

    // Fewer steps than the one being played is a different sequence too.
    *fixture.steps.borrow_mut() = vec![SequenceStep::new(0.0, SequenceFixture::linear(100))];
    assert_sample(fixture.render(cx), 22.5, 0, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 100), 0.0, 0, MotionStatus::Finished);
  }

  #[gpui::test]
  fn a_change_to_a_step_not_being_played_does_not_restart_the_sequence(cx: &mut TestAppContext) {
    let fixture = SequenceFixture::open(
      cx,
      vec![
        SequenceStep::new(10.0, SequenceFixture::linear(100)),
        SequenceStep::new(20.0, SequenceFixture::linear(100)),
      ],
    );
    fixture.advance(cx, 50);
    *fixture.steps.borrow_mut() = vec![
      SequenceStep::new(10.0, SequenceFixture::linear(100)),
      SequenceStep::new(30.0, SequenceFixture::linear(100)),
    ];
    assert_sample(fixture.render(cx), 5.0, 0, MotionStatus::Running);
    assert_sample(fixture.advance(cx, 100), 20.0, 1, MotionStatus::Running);
  }

  struct SingleStepView {
    armed: Rc<Cell<bool>>,
    #[allow(clippy::type_complexity)]
    samples: Rc<RefCell<Vec<(SequenceSample<f32>, MotionValue<f32>)>>>,
  }

  impl Render for SingleStepView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      let policy = Transition::new(Duration::from_millis(100))
        .delay(Duration::from_millis(20))
        .easing(Easing::EaseInOut);
      // A plain transition adopts its first target where a sequence
      // plays from `from` at once, so the transition is primed at 0.0
      // and both leave for 10.0 on the frame that arms the view.
      if !self.armed.get() {
        transition_with_status("plain", 0.0, policy, window, cx);
        return Empty;
      }
      let sequence = Sequence::new("single-step", 0.0)
        .with_step(10.0, policy.clone())
        .sample(window, cx);
      let plain = transition_with_status("plain", 10.0, policy, window, cx);
      self.samples.borrow_mut().push((sequence, plain));
      Empty
    }
  }

  #[gpui::test]
  fn a_single_step_sequence_matches_a_plain_transition(cx: &mut TestAppContext) {
    let armed = Rc::new(Cell::new(false));
    let samples = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.), px(100.)), {
      let armed = armed.clone();
      let samples = samples.clone();
      move |_, _| SingleStepView { armed, samples }
    });
    cx.run_until_parked();
    armed.set(true);
    for millis in [0, 10, 30, 70, 120] {
      cx.executor().advance_clock(Duration::from_millis(millis));
      window.update(cx, |_, window, _| window.refresh()).unwrap();
      cx.run_until_parked();
      let (sequence, plain) = *samples.borrow().last().unwrap();
      assert_eq!(*sequence.value(), plain.value, "value after {millis} ms");
      assert_eq!(sequence.status(), plain.status, "status after {millis} ms");
      assert_eq!(sequence.step(), 0);
    }
  }

  #[gpui::test]
  fn an_empty_sequence_idles_at_its_starting_value(cx: &mut TestAppContext) {
    let fixture = SequenceFixture::open(cx, Vec::new());
    assert_sample(fixture.last(), 0.0, 0, MotionStatus::Idle);
    assert_eq!(fixture.pending_frame(cx), 0);
  }

  #[test]
  fn transition_ids_accept_element_like_scalars_and_named_channels() {
    assert_eq!(
      TransitionId::from("opacity"),
      TransitionId::from(ElementId::from("opacity"))
    );
    assert_ne!(
      TransitionId::from(("terms", "fill")),
      TransitionId::from(("terms", "mark-opacity"))
    );
    let _: TransitionId = 7usize.into();
    let _: TransitionId = 7i32.into();
  }

  struct TestView {
    target: Rc<Cell<f32>>,
    duration: Duration,
    samples: Rc<RefCell<Vec<f32>>>,
  }

  impl Render for TestView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(transition(
        ("test", "value"),
        self.target.get(),
        Transition::new(self.duration).ease(|t| t),
        window,
        cx,
      ));
      Empty
    }
  }

  struct DelayedView {
    target: Rc<Cell<f32>>,
    samples: Rc<RefCell<Vec<f32>>>,
  }

  impl Render for DelayedView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(transition(
        ("delayed-test", "value"),
        self.target.get(),
        Transition::new(Duration::from_millis(100))
          .delay(Duration::from_millis(50))
          .ease(|t| t),
        window,
        cx,
      ));
      Empty
    }
  }

  struct Fixture {
    window: WindowHandle<TestView>,
    target: Rc<Cell<f32>>,
    samples: Rc<RefCell<Vec<f32>>>,
  }

  impl Fixture {
    fn open(cx: &mut TestAppContext, duration: Duration) -> Self {
      let target = Rc::new(Cell::new(0.0));
      let samples = Rc::new(RefCell::new(Vec::new()));
      let window = cx.open_window(size(px(100.), px(100.)), {
        let target = target.clone();
        let samples = samples.clone();
        move |_, _| TestView {
          target,
          duration,
          samples,
        }
      });
      cx.run_until_parked();
      Self {
        window,
        target,
        samples,
      }
    }

    fn render(&self, cx: &mut TestAppContext, target: f32) -> f32 {
      self.target.set(target);
      self
        .window
        .update(cx, |_, window, _| window.refresh())
        .unwrap();
      cx.run_until_parked();
      *self.samples.borrow().last().unwrap()
    }

    fn pending_frame(&self, cx: &mut TestAppContext) -> usize {
      self
        .window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap()
    }
  }

  #[gpui::test]
  fn a_zero_duration_target_change_is_immediate(cx: &mut TestAppContext) {
    let fixture = Fixture::open(cx, Duration::ZERO);
    assert_eq!(fixture.render(cx, 1.0), 1.0);
  }

  #[gpui::test]
  fn a_changed_target_transitions_over_time(cx: &mut TestAppContext) {
    let duration = Duration::from_millis(100);
    let fixture = Fixture::open(cx, duration);
    assert_eq!(fixture.render(cx, 10.0), 0.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(fixture.render(cx, 10.0), 5.0);
  }

  #[gpui::test]
  fn requested_animation_frames_resample_without_manual_refresh(cx: &mut TestAppContext) {
    let duration = Duration::from_millis(100);
    let fixture = Fixture::open(cx, duration);
    assert_eq!(fixture.render(cx, 10.0), 0.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(fixture.pending_frame(cx), 1);
    cx.run_until_parked();

    assert_eq!(*fixture.samples.borrow().last().unwrap(), 5.0);
  }

  #[gpui::test]
  fn reversing_uses_the_current_sample_and_shortens_the_return(cx: &mut TestAppContext) {
    let duration = Duration::from_millis(100);
    let fixture = Fixture::open(cx, duration);
    assert_eq!(fixture.render(cx, 10.0), 0.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    assert_eq!(fixture.render(cx, 0.0), 5.0);
    cx.executor().advance_clock(Duration::from_millis(25));
    assert_eq!(fixture.render(cx, 0.0), 2.5);
  }

  #[gpui::test]
  fn delay_holds_the_previous_value_before_interpolation(cx: &mut TestAppContext) {
    let target = Rc::new(Cell::new(0.0));
    let samples = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.), px(100.)), {
      let target = target.clone();
      let samples = samples.clone();
      move |_, _| DelayedView { target, samples }
    });
    cx.run_until_parked();

    target.set(10.0);
    window.update(cx, |_, window, _| window.refresh()).unwrap();
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), 0.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    window.update(cx, |_, window, _| window.refresh()).unwrap();
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), 0.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    window.update(cx, |_, window, _| window.refresh()).unwrap();
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), 5.0);
  }

  #[gpui::test]
  fn a_completed_transition_stops_requesting_frames(cx: &mut TestAppContext) {
    let duration = Duration::from_millis(100);
    let fixture = Fixture::open(cx, duration);
    fixture.render(cx, 1.0);
    assert_eq!(fixture.pending_frame(cx), 1);

    cx.executor().advance_clock(duration);
    assert_eq!(fixture.render(cx, 1.0), 1.0);
    fixture.pending_frame(cx);
    cx.run_until_parked();
    assert_eq!(fixture.pending_frame(cx), 0);
  }

  #[gpui::test]
  fn reduced_motion_adopts_the_target_without_requesting_a_frame(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let duration = Duration::from_millis(100);
    let fixture = Fixture::open(cx, duration);
    assert_eq!(fixture.render(cx, 1.0), 1.0);
    assert_eq!(fixture.pending_frame(cx), 0);
  }

  struct SpringView {
    target: Rc<Cell<f32>>,
    policy: Rc<Cell<Spring>>,
    samples: Rc<RefCell<Vec<f32>>>,
  }

  impl Render for SpringView {
    fn render(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) -> impl IntoElement {
      self.samples.borrow_mut().push(spring(
        ("spring-test", "value"),
        self.target.get(),
        self.policy.get(),
        window,
        cx,
      ));
      Empty
    }
  }

  struct SpringFixture {
    window: WindowHandle<SpringView>,
    target: Rc<Cell<f32>>,
    policy: Rc<Cell<Spring>>,
    samples: Rc<RefCell<Vec<f32>>>,
  }

  impl SpringFixture {
    fn open(cx: &mut TestAppContext, policy: Spring) -> Self {
      let target = Rc::new(Cell::new(0.0));
      let policy = Rc::new(Cell::new(policy));
      let samples = Rc::new(RefCell::new(Vec::new()));
      let window = cx.open_window(size(px(100.), px(100.)), {
        let target = target.clone();
        let policy = policy.clone();
        let samples = samples.clone();
        move |_, _| SpringView {
          target,
          policy,
          samples,
        }
      });
      cx.run_until_parked();
      Self {
        window,
        target,
        policy,
        samples,
      }
    }

    fn render(&self, cx: &mut TestAppContext, target: f32) -> f32 {
      self.target.set(target);
      self
        .window
        .update(cx, |_, window, _| window.refresh())
        .unwrap();
      cx.run_until_parked();
      *self.samples.borrow().last().unwrap()
    }

    fn advance(&self, cx: &mut TestAppContext, millis: u64, target: f32) -> f32 {
      cx.executor().advance_clock(Duration::from_millis(millis));
      self.render(cx, target)
    }

    fn pending_frame(&self, cx: &mut TestAppContext) -> usize {
      self
        .window
        .update(cx, |_, window, cx| window.simulate_next_frame(cx))
        .unwrap()
    }
  }

  #[gpui::test]
  fn a_spring_adopts_its_first_target_immediately(cx: &mut TestAppContext) {
    let fixture = SpringFixture::open(cx, Spring::new(Duration::from_millis(300)));
    assert_eq!(*fixture.samples.borrow().first().unwrap(), 0.0);
  }

  #[gpui::test]
  fn a_spring_travels_toward_its_target_over_time(cx: &mut TestAppContext) {
    let fixture = SpringFixture::open(cx, Spring::new(Duration::from_millis(300)));
    assert_eq!(fixture.render(cx, 1.0), 0.0);

    let early = fixture.advance(cx, 50, 1.0);
    let late = fixture.advance(cx, 50, 1.0);
    assert!(
      0.0 < early && early < late && late < 1.0,
      "expected monotonic approach, got {early} then {late}"
    );
  }

  #[gpui::test]
  fn a_reversed_spring_keeps_its_momentum_before_turning_around(cx: &mut TestAppContext) {
    let fixture = SpringFixture::open(cx, Spring::new(Duration::from_millis(300)));
    fixture.render(cx, 1.0);
    let reversed_at = fixture.advance(cx, 100, 1.0);

    // Retarget mid-flight. A duration-based transition restarts its easing
    // here and moves away from 1.0 on the very next frame.
    assert_eq!(fixture.render(cx, 0.0), reversed_at);

    let next = fixture.advance(cx, 16, 0.0);
    assert!(
      next > reversed_at,
      "expected the spring to carry its velocity past {reversed_at}, got {next}"
    );

    assert_eq!(fixture.advance(cx, 1_000, 0.0), 0.0);
  }

  #[gpui::test]
  fn a_bouncy_spring_overshoots_its_target(cx: &mut TestAppContext) {
    let fixture = SpringFixture::open(
      cx,
      Spring::new(Duration::from_millis(350)).with_damping(0.7),
    );
    fixture.render(cx, 1.0);
    for _ in 0..30 {
      fixture.advance(cx, 16, 1.0);
    }

    let peak = fixture
      .samples
      .borrow()
      .iter()
      .copied()
      .fold(f32::MIN, f32::max);
    assert!(peak > 1.0, "expected an overshoot past 1.0, got {peak}");
  }

  #[gpui::test]
  fn a_settled_spring_stops_requesting_frames(cx: &mut TestAppContext) {
    let fixture = SpringFixture::open(cx, Spring::new(Duration::from_millis(300)));
    fixture.render(cx, 1.0);
    assert_eq!(fixture.pending_frame(cx), 1);

    assert_eq!(fixture.advance(cx, 2_000, 1.0), 1.0);
    fixture.pending_frame(cx);
    cx.run_until_parked();
    assert_eq!(fixture.pending_frame(cx), 0);
  }

  #[gpui::test]
  fn a_spring_that_is_not_travelling_adopts_its_target_on_the_spot(cx: &mut TestAppContext) {
    let travelling = Spring::new(Duration::from_millis(300));
    let fixture = SpringFixture::open(cx, travelling.with_travel(false));

    assert_eq!(fixture.render(cx, 1.0), 1.0);
    assert_eq!(fixture.pending_frame(cx), 0);
    assert_eq!(fixture.advance(cx, 100, 5.0), 5.0);

    // Travel resumes from the value the suspension left behind. A spring
    // that had kept the state it held beforehand would jump back to it here.
    fixture.policy.set(travelling);
    assert_eq!(fixture.render(cx, 6.0), 5.0);
    let next = fixture.advance(cx, 50, 6.0);
    assert!(
      5.0 < next && next < 6.0,
      "expected travel to resume from 5.0, got {next}"
    );
  }

  #[gpui::test]
  fn a_zero_response_spring_resolves_instead_of_dividing_by_its_period(cx: &mut TestAppContext) {
    let fixture = SpringFixture::open(cx, Spring::new(Duration::ZERO));
    assert_eq!(fixture.render(cx, 1.0), 1.0);
    assert_eq!(fixture.pending_frame(cx), 0);
  }

  #[test]
  fn spring_rejects_non_finite_or_negative_physical_parameters() {
    let spring = Spring::new(Duration::from_millis(300));

    assert_eq!(
      spring.try_with_damping(f32::NAN).unwrap_err(),
      SpringError::InvalidDamping
    );
    assert_eq!(
      spring.try_with_damping(-0.1).unwrap_err(),
      SpringError::InvalidDamping
    );
    assert_eq!(
      spring.try_with_epsilon(f32::INFINITY).unwrap_err(),
      SpringError::InvalidEpsilon
    );
    assert_eq!(
      spring.try_with_epsilon(-0.1).unwrap_err(),
      SpringError::InvalidEpsilon
    );
  }

  #[test]
  fn spring_reports_its_unit_specific_settling_tolerance() {
    let normalized = Spring::new(Duration::from_millis(180));
    let pixels = Spring::new(Duration::from_millis(180)).with_epsilon(0.1);

    assert!(normalized.epsilon() < 0.01);
    assert_eq!(pixels.epsilon(), 0.1);
  }

  #[gpui::test]
  fn reduced_motion_adopts_the_spring_target_without_requesting_a_frame(cx: &mut TestAppContext) {
    cx.update(|cx| cx.set_reduce_motion(true));
    let fixture = SpringFixture::open(
      cx,
      Spring::new(Duration::from_millis(350)).with_damping(0.7),
    );
    assert_eq!(fixture.render(cx, 1.0), 1.0);
    assert_eq!(fixture.pending_frame(cx), 0);
  }
}
