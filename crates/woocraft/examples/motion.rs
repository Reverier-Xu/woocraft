//! Motion system example: transitions, springs, keyframes, stagger, presence,
//! sequence, and reveal.
//!
//! Each tab demonstrates one capability of `woocraft::motion`:
//!
//! - Sliding time — `transition`: rolling clock digits with targets that change
//!   faster than the transition settles.
//! - Spring — `spring`: a segmented-control indicator that keeps its velocity
//!   when rapidly retargeted.
//! - Keyframes — `Keyframes` + `animate_keyframes`: a repeating multi-stop
//!   signal with offset timing.
//! - Presence — `Presence`: an exit animation that keeps content mounted until
//!   it becomes absent.
//! - Stagger — `Stagger`: allocation-free timing offsets across a list.
//! - Sequence — `Sequence`: three chained steps that hand over at their
//!   boundaries.
//! - Reveal — `MotionReveal`: a measured, clipped height reveal.

use std::time::Duration;

use gpui::{
  AnyElement, App, AppContext as _, Bounds, Context, IntoElement, ParentElement, Render, Styled,
  Task, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, relative,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Easing, IterationCount, Keyframe, Keyframes,
  MotionReveal, MotionStatus, Presence, Sequence, Sizable, Spring, Stagger, StaggerOrigin, Theme,
  ThemeMode, Timing, TitleBar, Transition, animate_keyframes, h_flex, init, spring, transition,
  v_flex,
};

mod common;

const START_MINUTES: u32 = 8 * 60;
const END_MINUTES: u32 = 20 * 60;
const DIGIT_HEIGHT: f32 = 38.0;

#[derive(Clone, Copy, PartialEq)]
enum Demo {
  SlidingTime,
  Spring,
  Keyframes,
  Presence,
  Stagger,
  Sequence,
  Reveal,
}

impl Demo {
  const ALL: [Self; 7] = [
    Self::SlidingTime,
    Self::Spring,
    Self::Keyframes,
    Self::Presence,
    Self::Stagger,
    Self::Sequence,
    Self::Reveal,
  ];

  fn label(self) -> &'static str {
    match self {
      Self::SlidingTime => "Transition",
      Self::Spring => "Spring",
      Self::Keyframes => "Keyframes",
      Self::Presence => "Presence",
      Self::Stagger => "Stagger",
      Self::Sequence => "Sequence",
      Self::Reveal => "Reveal",
    }
  }

  fn description(self) -> &'static str {
    match self {
      Self::SlidingTime => "Interruptible transitions roll the clock from morning to evening.",
      Self::Spring => "A retargeted spring keeps its velocity instead of restarting.",
      Self::Keyframes => "Seven values follow one keyframe track with offset timing.",
      Self::Presence => "A surface stays mounted until its exit transition completes.",
      Self::Stagger => "List rows enter in order from one allocation-free delay policy.",
      Self::Sequence => "Three steps play in turn: slide in, fill, then rest and fade out.",
      Self::Reveal => "A measured panel reveals its height through clipped content.",
    }
  }
}

struct MotionExample {
  demo: Demo,
  minutes: u32,
  digit_targets: [f32; 4],
  playback: Option<Task<()>>,
  spring_selected: bool,
  present: bool,
  stagger_generation: usize,
  sequence_generation: usize,
  reveal_open: bool,
}

impl MotionExample {
  fn new() -> Self {
    Self {
      demo: Demo::SlidingTime,
      minutes: START_MINUTES,
      digit_targets: [0.0, 8.0, 0.0, 0.0],
      playback: None,
      spring_selected: false,
      present: true,
      stagger_generation: 0,
      sequence_generation: 0,
      reveal_open: true,
    }
  }

  fn play_time(&mut self, cx: &mut Context<Self>) {
    if self.playback.take().is_some() {
      cx.notify();
      return;
    }
    if self.minutes == END_MINUTES {
      self.minutes = START_MINUTES;
      self.digit_targets = [0.0, 8.0, 0.0, 0.0];
    }
    self.playback = Some(cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(500))
          .await;
        let Ok(finished) = this.update(cx, |this, cx| {
          this.minutes = (this.minutes + 30).min(END_MINUTES);
          for (target, digit) in this.digit_targets.iter_mut().zip(time_digits(this.minutes)) {
            *target = advance_digit(*target, digit);
          }
          cx.notify();
          this.minutes == END_MINUTES
        }) else {
          break;
        };
        if finished {
          _ = this.update(cx, |this, cx| {
            this.playback = None;
            cx.notify();
          });
          break;
        }
      }
    }));
    cx.notify();
  }

  fn rolling_digit(&self, ix: usize, target: f32, window: &mut Window, cx: &mut App) -> AnyElement {
    let value = transition(
      (ix, "clock-digit"),
      target,
      Transition::new(Duration::from_millis(620)).easing(Easing::EaseInOut),
      window,
      cx,
    );
    let digit = value.floor() as i32;
    let offset = value.fract() * DIGIT_HEIGHT;
    div()
      .relative()
      .w(px(25.0))
      .h(px(DIGIT_HEIGHT))
      .overflow_hidden()
      .child(clock_digit(digit, -offset))
      .child(clock_digit(digit + 1, DIGIT_HEIGHT - offset))
      .into_any_element()
  }

  fn sliding_time(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    let playing = self.playback.is_some();
    h_flex()
      .gap_6()
      .child(
        h_flex()
          .text_size(px(30.0))
          .font_weight(gpui::FontWeight::MEDIUM)
          .children(
            self
              .digit_targets
              .iter()
              .enumerate()
              .map(|(ix, target)| self.rolling_digit(ix, *target, window, cx)),
          ),
      )
      .child(
        Button::new("play-time")
          .label(if playing {
            "Stop"
          } else if self.minutes == END_MINUTES {
            "Replay"
          } else {
            "Play"
          })
          .on_click(cx.listener(|this, _, _, cx| this.play_time(cx))),
      )
      .into_any_element()
  }

  fn spring_demo(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    let (border, primary, primary_foreground) = {
      let theme = cx.theme();
      (theme.border, theme.primary, theme.primary_foreground)
    };
    let x = spring(
      "selector-indicator",
      if self.spring_selected { 120.0 } else { 0.0 },
      Spring::new(Duration::from_millis(420)).with_damping(0.68),
      window,
      cx,
    );
    div()
      .relative()
      .w(px(240.0))
      .h_10()
      .rounded_sm()
      .border_1()
      .border_color(border)
      .child(
        div()
          .absolute()
          .top_0()
          .left(px(x))
          .w(px(119.0))
          .h_full()
          .rounded_sm()
          .bg(primary),
      )
      .child(
        h_flex().relative().h_full().children(
          [("Focus", false), ("Flow", true)]
            .into_iter()
            .enumerate()
            .map(|(ix, (label, selected))| {
              Button::new(("spring-option", ix))
                .label(label)
                .flat()
                .w(px(119.0))
                .h_full()
                .when(self.spring_selected == selected, |this| {
                  this.text_color(primary_foreground)
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                  this.spring_selected = selected;
                  cx.notify();
                }))
            }),
        ),
      )
      .into_any_element()
  }

  fn keyframes_demo(&self, window: &mut Window, cx: &mut App) -> AnyElement {
    let primary = cx.theme().primary;
    let frames = Keyframes::try_new([
      Keyframe::new(0.0, 0.0),
      Keyframe::new(0.35, 1.0).ease(Easing::EaseOut),
      Keyframe::new(0.7, 0.0),
      Keyframe::new(1.0, 0.0),
    ])
    .expect("static keyframes are valid");
    v_flex()
      .w(px(320.0))
      .gap_4()
      .child(
        h_flex()
          .justify_between()
          .text_xs()
          .child("Playback")
          .child("Infinite · 1200ms"),
      )
      .child(
        h_flex()
          .h(px(64.0))
          .items_end()
          .justify_center()
          .gap_2()
          .children((0..7).map(|ix| {
            let value = animate_keyframes(
              (ix, "keyframe-bar"),
              &frames,
              Timing::new(Duration::from_millis(1200))
                .delay(Duration::from_millis(ix as u64 * 80).into())
                .iterations(IterationCount::Infinite),
              window,
              cx,
            )
            .value;
            div()
              .w_5()
              .h(px(18.0 + 38.0 * value))
              .rounded_full()
              .bg(primary)
              .opacity(0.35 + 0.65 * value)
          })),
      )
      .into_any_element()
  }

  fn presence_demo(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    let (border, card) = {
      let theme = cx.theme();
      (theme.border, theme.card)
    };
    let sample = Presence::new("presence-notice", self.present)
      .transition(Transition::new(Duration::from_millis(360)).easing(Easing::EaseInOut))
      .sample(window, cx);
    h_flex()
      .h(px(120.0))
      .gap_4()
      .child(div().w(px(320.0)).child(if sample.should_render() {
        div()
          .w_full()
          .p_3()
          .rounded_sm()
          .border_1()
          .border_color(border)
          .bg(card)
          .opacity(sample.progress)
          .child(
            v_flex()
              .gap_1()
              .child(
                h_flex()
                  .justify_between()
                  .child("Background task")
                  .child("Complete"),
              )
              .child(
                div()
                  .text_xs()
                  .opacity(0.6)
                  .child("Mounted through the exit phase."),
              ),
          )
          .into_any_element()
      } else {
        div().into_any_element()
      }))
      .child(
        Button::new("toggle-presence")
          .label(if self.present { "Remove" } else { "Insert" })
          .on_click(cx.listener(|this, _, _, cx| {
            this.present = !this.present;
            cx.notify();
          })),
      )
      .into_any_element()
  }

  fn stagger_demo(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    let border = cx.theme().border;
    let stagger = Stagger::new(Duration::from_millis(90), StaggerOrigin::First);
    let generation = self.stagger_generation;
    let frames = Keyframes::try_new([Keyframe::new(0.0, 0.0), Keyframe::new(1.0, 1.0)])
      .expect("static keyframes are valid");
    let rows: Vec<AnyElement> = (0..3)
      .map(|ix| {
        let value = animate_keyframes(
          ("stagger-item", format!("{generation}-{ix}")),
          &frames,
          Timing::new(Duration::from_millis(360))
            .delay(stagger.delay(ix, 3).into())
            .ease(Easing::EaseOut),
          window,
          cx,
        )
        .value;
        let title = ["Transition", "Spring", "Keyframes"][ix];
        h_flex()
          .ml(px((1.0 - value) * 24.0))
          .w_full()
          .h_10()
          .px_3()
          .when(ix < 2, |this| this.border_b_1())
          .opacity(value)
          .gap_3()
          .child(
            div()
              .w_5()
              .text_xs()
              .opacity(0.6)
              .child(format!("0{}", ix + 1)),
          )
          .child(title)
          .into_any_element()
      })
      .collect();
    v_flex()
      .w(px(380.0))
      .gap_3()
      .child(
        div()
          .rounded_sm()
          .border_1()
          .border_color(border)
          .children(rows),
      )
      .child(
        Button::new("replay-stagger")
          .label("Replay")
          .self_end()
          .on_click(cx.listener(|this, _, _, cx| {
            this.stagger_generation += 1;
            cx.notify();
          })),
      )
      .into_any_element()
  }

  /// One `Sequence<f32>` runs 0 → 1 → 2 → 3 with a different transition per
  /// step, and the demo reads each property off the segment the value is in:
  /// step 0 slides the card in, step 1 fills its bar, step 2 rests and fades.
  fn sequence_demo(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    const STEPS: [&str; 3] = ["Slide in", "Fill", "Rest, then fade out"];
    let (border, card, primary) = {
      let theme = cx.theme();
      (theme.border, theme.card, theme.primary)
    };
    let sample = Sequence::new((self.sequence_generation, "sequence-card"), 0.0)
      .with_step(
        1.0,
        Transition::new(Duration::from_millis(360)).easing(Easing::EaseOut),
      )
      .with_step(
        2.0,
        Transition::new(Duration::from_millis(900)).easing(Easing::EaseInOut),
      )
      .with_step(
        3.0,
        Transition::new(Duration::from_millis(420))
          .delay(Duration::from_millis(600))
          .easing(Easing::EaseIn),
      )
      .sample(window, cx);
    let value = *sample.value();
    let slide = value.min(1.0);
    let fill = (value - 1.0).clamp(0.0, 1.0);
    let fade = (value - 2.0).clamp(0.0, 1.0);
    let status = match sample.status() {
      MotionStatus::Idle => "idle",
      MotionStatus::Delayed => "delayed",
      MotionStatus::Running => "running",
      MotionStatus::Finished => "finished",
    };
    v_flex()
      .w(px(380.0))
      .gap_3()
      .child(
        div().h(px(96.0)).flex().items_center().child(
          v_flex()
            .w_full()
            .ml(px((1.0 - slide) * 48.0))
            .opacity(slide * (1.0 - fade))
            .p_3()
            .gap_2()
            .rounded_sm()
            .border_1()
            .border_color(border)
            .bg(card)
            .child(
              h_flex()
                .justify_between()
                .child("Uploading report.pdf")
                .child(format!("{}%", (fill * 100.0).round())),
            )
            .child(
              div()
                .h(px(6.0))
                .w_full()
                .rounded_full()
                .bg(border)
                .child(div().h_full().w(relative(fill)).rounded_full().bg(primary)),
            ),
        ),
      )
      .child(
        h_flex()
          .justify_between()
          .child(div().text_xs().opacity(0.6).child(format!(
            "step {} of {} · {} · {status}",
            sample.step() + 1,
            STEPS.len(),
            STEPS[sample.step()],
          )))
          .child(
            Button::new("replay-sequence")
              .label("Replay")
              .on_click(cx.listener(|this, _, _, cx| {
                this.sequence_generation += 1;
                cx.notify();
              })),
          ),
      )
      .into_any_element()
  }

  fn reveal_demo(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
    let (border, card) = {
      let theme = cx.theme();
      (theme.border, theme.card)
    };
    let progress = transition(
      "reveal-progress",
      if self.reveal_open { 1.0 } else { 0.0 },
      Transition::new(Duration::from_millis(320)).easing(Easing::EaseInOut),
      window,
      cx,
    );
    v_flex()
      .w(px(380.0))
      .gap_3()
      .child(
        div()
          .rounded_sm()
          .border_1()
          .border_color(border)
          .bg(card)
          .child(
            Button::new("toggle-reveal")
              .label(if self.reveal_open {
                "Collapse"
              } else {
                "Expand"
              })
              .flat()
              .w_full()
              .justify_start()
              .on_click(cx.listener(|this, _, _, cx| {
                this.reveal_open = !this.reveal_open;
                cx.notify();
              })),
          )
          .child(MotionReveal::new(
            "reveal-panel",
            progress,
            v_flex()
              .px_3()
              .py_2()
              .gap_1()
              .border_t_1()
              .border_color(border)
              .child("The panel measures its content once")
              .child("and clips it to the sampled progress.")
              .into_any_element(),
          )),
      )
      .into_any_element()
  }
}

impl Render for MotionExample {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let content = match self.demo {
      Demo::SlidingTime => self.sliding_time(window, cx),
      Demo::Spring => self.spring_demo(window, cx),
      Demo::Keyframes => self.keyframes_demo(window, cx),
      Demo::Presence => self.presence_demo(window, cx),
      Demo::Stagger => self.stagger_demo(window, cx),
      Demo::Sequence => self.sequence_demo(window, cx),
      Demo::Reveal => self.reveal_demo(window, cx),
    };
    let theme = cx.theme();
    v_flex()
      .size_full()
      .bg(theme.background)
      .text_color(theme.foreground)
      .p_6()
      .gap_4()
      .child(
        div()
          .text_lg()
          .font_weight(gpui::FontWeight::SEMIBOLD)
          .child("Motion examples"),
      )
      .child(
        h_flex()
          .flex_wrap()
          .gap_1()
          .children(Demo::ALL.into_iter().map(|demo| {
            Button::new(demo.label())
              .label(demo.label())
              .small()
              .when(self.demo == demo, |this| this.primary())
              .on_click(cx.listener(move |this, _, _, cx| {
                this.demo = demo;
                cx.notify();
              }))
          })),
      )
      .child(
        v_flex()
          .flex_1()
          .pt_4()
          .rounded_sm()
          .border_1()
          .border_color(theme.border)
          .gap_2()
          .child(
            div()
              .px_4()
              .text_sm()
              .font_weight(gpui::FontWeight::SEMIBOLD)
              .child(self.demo.label()),
          )
          .child(
            div()
              .px_4()
              .text_sm()
              .opacity(0.6)
              .child(self.demo.description()),
          )
          .child(
            div()
              .flex_1()
              .flex()
              .items_center()
              .justify_center()
              .child(content),
          ),
      )
      .child(common::rem_size_control(window, cx))
  }
}

fn clock_digit(value: i32, top: f32) -> gpui::Div {
  div()
    .absolute()
    .top(px(top))
    .w_full()
    .h(px(DIGIT_HEIGHT))
    .flex()
    .items_center()
    .justify_center()
    .child(value.rem_euclid(10).to_string())
}

fn time_digits(minutes: u32) -> [u8; 4] {
  let hour = minutes / 60;
  let minute = minutes % 60;
  [
    (hour / 10) as u8,
    (hour % 10) as u8,
    (minute / 10) as u8,
    (minute % 10) as u8,
  ]
}

fn advance_digit(current: f32, digit: u8) -> f32 {
  let visible = current.floor() as i32 % 10;
  current + (i32::from(digit) - visible).rem_euclid(10) as f32
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      init(cx);
      cx.activate(true);
      Theme::set_mode(ThemeMode::Dark, cx);

      let bounds = Bounds::centered(None, gpui::Size::new(px(860.0), px(620.0)), cx);
      let window = cx
        .open_window(
          WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            ..Default::default()
          },
          |_window, cx| cx.new(|_| MotionExample::new()),
        )
        .expect("open motion example window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Motion Example");
        })
        .expect("update motion example window failed");
    });
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn derives_time_digits() {
    assert_eq!(time_digits(START_MINUTES), [0, 8, 0, 0]);
    assert_eq!(time_digits(END_MINUTES), [2, 0, 0, 0]);
  }

  #[test]
  fn rolls_forward_across_zero() {
    assert_eq!(advance_digit(8.0, 0), 10.0);
    assert_eq!(advance_digit(19.0, 2), 22.0);
  }
}
