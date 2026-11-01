//! The standard transport bar for a [`Player`].
//!
//! [`PlayerView`] renders one by default; a host composing its own chrome
//! creates one standalone — it is usable without the view:
//!
//! ```rust,ignore
//! let transport = cx.new(|cx| PlayerTransport::new(&player, cx).small());
//! ```
//!
//! Sizing: a single [`Size`] tier threads through every part — the container
//! metrics, the buttons, the sliders, the icons and the labels — so `.small()`
//! yields an entirely small bar. Text scale rides on the sized controls
//! (`Button`, `IconLabel`); the bar itself never sets a font size. Secondary
//! information (the ticking times) reads as secondary through
//! `theme.muted_foreground` at the same tier, never a smaller font.

use std::time::Duration;

use gpui::{
  AppContext as _, Context, ElementId, Entity, IntoElement, ParentElement as _, Render,
  Styled as _, Subscription, Window, div,
};

use super::player::{Player, PlayerEvent, format_time};
use crate::{
  ActiveTheme as _, Button, Icon, IconLabel, IconName, Sizable, Size, Slider, SliderEvent,
  SliderState, StyleSized, Theme, h_flex,
};

/// Playback speeds the speed button cycles through, ascending: the first one
/// above the current speed is next, wrapping around.
const SPEEDS: [f64; 5] = [0.5, 1.0, 1.25, 1.5, 2.0];

/// Seek-slider changes whose target is closer than this to the current
/// position are ignored: a video frame lasts roughly 16-40 ms, so a seek below
/// this bound cannot reach a perceivably different position and only disrupts
/// decoding. This is also what stops a percentage rounded to the slider step
/// from seeking playback backwards by a frame.
const SEEK_EPSILON_SECS: f64 = 0.1;

/// Width of the volume slider, in ems of the bar's size tier.
///
/// `6em` reproduces the 96 px this bar historically hardcoded at the default
/// root size, while scaling with the tier and the window font size like every
/// other metric here.
const VOLUME_SLIDER_WIDTH_EM: f32 = 6.;

/// Reserved minimum width of the position and duration labels, in ems of the
/// bar's size tier.
///
/// Both labels tick at least once a second; if their boxes followed the text,
/// the whole bar would re-layout on every tick. `4.5em` holds the widest time
/// `format_time` renders for sane inputs (`H:MM:SS` — seven cells in the
/// monospaced UI family, where one cell is exactly half an em) plus the
/// label's own horizontal insets, so the digits change inside a stable box.
const TIME_LABEL_MIN_EM: f32 = 4.5;

/// The transport bar: play/pause, position, seek, duration, mute, volume and
/// speed, bound to one [`Player`].
///
/// The bar owns the seek/volume slider states and the loop guard that keeps
/// the seek slider in sync with the player's position reports (see
/// [`PlayerTransport::sync_seek`]). Element ids are scoped to the player, so
/// several transports can live in one window.
pub struct PlayerTransport {
  player: Entity<Player>,
  seek: Entity<SliderState>,
  volume: Entity<SliderState>,
  /// Seek-slider changes still to be swallowed as echoes of player-driven
  /// writes; see [`Self::sync_seek`].
  pending_seek_echoes: usize,
  size: Size,
  _subscriptions: Vec<Subscription>,
}

impl PlayerTransport {
  /// Builds a transport bar bound to `player`.
  pub fn new(player: &Entity<Player>, cx: &mut Context<Self>) -> Self {
    // The progress bar is not immediate: dragging previews a target while
    // playback keeps moving the real thumb, and the seek is issued once, on
    // release. Applying every pointer move would fight the position reports.
    let seek = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(0.1)
        .with_immediate(false)
    });
    let volume = cx.new(|_| SliderState::new().min(0.0).max(100.0).step(1.0));

    let mut subscriptions = Vec::new();

    subscriptions.push(
      cx.subscribe(player, |this: &mut Self, _, event: &PlayerEvent, cx| {
        let (position, duration) = {
          let state = this.player.read(cx).state();
          (state.position, state.duration)
        };
        if duration > 0.0 {
          this.sync_seek((position / duration * 100.0) as f32, cx);
        }
        if let PlayerEvent::FileLoaded = event {
          this.sync_seek(0.0, cx);
        }
      }),
    );

    subscriptions.push(
      cx.subscribe(&seek, |this: &mut Self, _, event: &SliderEvent, cx| {
        let value = match event {
          SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
          // A non-immediate drag preview is not applied by the host; the value
          // arrives again as `Commit` when the pointer is released.
          SliderEvent::Preview(_) => return,
        };
        // A player-driven write echoes back through the same event a user
        // drag uses; consume the echo instead of seeking on the player's own
        // position report.
        let player_driven = this.pending_seek_echoes > 0;
        if player_driven {
          this.pending_seek_echoes -= 1;
        }
        let (position, duration) = {
          let state = this.player.read(cx).state();
          (state.position, state.duration)
        };
        if should_seek(position, duration, value.start(), player_driven) {
          let target = duration * f64::from(value.start()) / 100.0;
          let _ = this.player.update(cx, |player, _| player.seek_to(target));
        }
      }),
    );

    subscriptions.push(
      cx.subscribe(&volume, |this: &mut Self, _, event: &SliderEvent, cx| {
        let value = match event {
          SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
          // A non-immediate drag preview is not applied by the host; the value
          // arrives again as `Commit` when the pointer is released.
          SliderEvent::Preview(_) => return,
        };
        let volume = value.start() as f64;
        let _ = this
          .player
          .update(cx, |player, _| player.set_volume(volume));
      }),
    );

    let volume_now = player.read(cx).state().volume as f32;
    let transport = Self {
      player: player.clone(),
      seek,
      volume,
      pending_seek_echoes: 0,
      size: Size::default(),
      _subscriptions: subscriptions,
    };
    transport.volume.update(cx, |slider, cx| {
      slider.set_value(volume_now, cx);
    });
    transport
  }

  /// The player this bar transports.
  pub fn player(&self) -> &Entity<Player> {
    &self.player
  }

  /// Sets the size tier without a rebuild. `PlayerView` threads its own tier
  /// into its default transport through this on every render, so resizing the
  /// view resizes the bar with it.
  pub(crate) fn set_size(&mut self, size: Size) {
    self.size = size;
  }

  /// Mirrors a player-reported position into the seek slider.
  ///
  /// `SliderState::set_value` emits the very `SliderEvent::Change` a user drag
  /// emits, so without a guard every position report would come back as user
  /// input and order a seek to the position playback is already at. Each seek
  /// triggers a new report, which is the feedback loop that froze the progress
  /// bar. Every write is therefore paired with exactly one swallowed echo,
  /// counted in [`Self::pending_seek_echoes`]. The pairing cannot drift: gpui
  /// flushes the emitted change before any user input can be dispatched, so a
  /// write's echo is always consumed before the next user-driven change.
  fn sync_seek(&mut self, percentage: f32, cx: &mut Context<Self>) {
    self.pending_seek_echoes += 1;
    self
      .seek
      .update(cx, |slider, cx| slider.set_value(percentage, cx));
  }
}

impl_sizable!(PlayerTransport);

impl Render for PlayerTransport {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let size = self.size;
    let theme = cx.theme().clone();
    let id = self.player.entity_id().as_u64();
    let (paused, muted, speed, position, duration) = {
      let state = self.player.read(cx).state();
      (
        state.paused,
        state.muted,
        state.speed,
        state.position,
        state.duration,
      )
    };

    let play_button = {
      let player = self.player.clone();
      Button::new(("player-play", id))
        .with_size(size)
        .icon(Icon::new(if paused {
          IconName::Play
        } else {
          IconName::Pause
        }))
        .on_click(move |_, _, cx| {
          let _ = player.update(cx, |player, cx| player.toggle_pause(cx));
        })
    };

    let mute_button = {
      let player = self.player.clone();
      Button::new(("player-mute", id))
        .with_size(size)
        .icon(Icon::new(if muted {
          IconName::SpeakerOff
        } else {
          IconName::Speaker2
        }))
        .on_click(move |_, _, cx| {
          let _ = player.update(cx, |player, _| player.set_muted(!muted));
        })
    };

    let next_speed = SPEEDS
      .iter()
      .copied()
      .find(|candidate| *candidate > speed + f64::EPSILON)
      .unwrap_or(SPEEDS[0]);
    let speed_button = {
      let player = self.player.clone();
      Button::new(("player-speed", id))
        .with_size(size)
        .label(format!("{}x", (speed * 100.0).round() / 100.0))
        .on_click(move |_, _, cx| {
          let _ = player.update(cx, |player, _| player.set_speed(next_speed));
        })
    };

    h_flex()
      .w_full()
      .items_center()
      // A container row, aligned with the `Slider` it hosts: same height
      // tier (container around component), same horizontal padding, same
      // gap, same radius step. The `Slider` itself is never restyled.
      .container_size(size)
      .container_gap(size)
      .container_rounded(size)
      .bg(theme.card)
      .text_color(theme.card_foreground)
      .child(play_button)
      .child(time_label(("player-position", id), size, &theme, position))
      .child(
        div()
          .flex_1()
          .child(Slider::new(format!("player-seek-{id}"), &self.seek).with_size(size)),
      )
      .child(time_label(("player-duration", id), size, &theme, duration))
      .child(mute_button)
      .child(
        div()
          .flex_none()
          .w(size.em(VOLUME_SLIDER_WIDTH_EM))
          .child(Slider::new(format!("player-volume-{id}"), &self.volume).with_size(size)),
      )
      .child(speed_button)
  }
}

/// One ticking time label (position or duration): secondary information, so
/// muted colour at the bar's own tier — never a smaller font — inside a
/// reserved minimum width (see [`TIME_LABEL_MIN_EM`]) so the digits tick
/// without resizing the bar.
fn time_label(
  element_id: impl Into<ElementId>, size: Size, theme: &Theme, seconds: f64,
) -> IconLabel {
  IconLabel::new(element_id)
    .with_size(size)
    .label(format_time(Duration::from_secs_f64(seconds.max(0.0))))
    .text_color(theme.muted_foreground)
    .min_w(size.em(TIME_LABEL_MIN_EM))
}

/// Whether a seek-slider change should seek the player.
///
/// Extracted as a pure decision so the loop-prevention rules are testable
/// without a window. Player-driven echoes never seek, or the player's own
/// position reports would order seeks back to the position playback is
/// already at. A user change only seeks when it moves the target more than
/// [`SEEK_EPSILON_SECS`] away from the current position, so a rounded
/// percentage cannot drag playback backwards by a frame.
fn should_seek(position: f64, duration: f64, slider_percentage: f32, player_driven: bool) -> bool {
  // A non-positive duration means nothing is loaded; a NaN one (never sent
  // by mpv, but never turned into a seek either) is rejected the same way.
  if player_driven || duration <= 0.0 || duration.is_nan() {
    return false;
  }
  let target = duration * f64::from(slider_percentage) / 100.0;
  (target - position).abs() >= SEEK_EPSILON_SECS
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn player_position_reports_never_seek() {
    // The seek loop that froze the progress bar, replayed as the pure
    // decision it reduces to: a 3-second clip reports a steadily advancing
    // position, the view mirrors it into the slider, and the write echoes
    // back as a change. Zero of those reports may seek, or `time-pos` would
    // oscillate instead of advancing.
    let duration = 3.0;
    let mut pending_echoes = 0;
    let mut seeks = 0;
    let mut position = 0.0;
    while position < duration {
      pending_echoes += 1;
      let percentage = (position / duration * 100.0) as f32;
      if should_seek(position, duration, percentage, pending_echoes > 0) {
        seeks += 1;
      }
      pending_echoes -= 1;
      position += 0.04;
    }
    assert_eq!(seeks, 0);
  }

  #[test]
  fn user_seek_requests_only_fire_beyond_the_epsilon() {
    let duration = 3.0;
    let position = 1.5;
    // Parked at half of a 3-second clip: targets within the epsilon are
    // no-ops, so a rounded percentage cannot seek playback back a frame.
    assert!(!should_seek(position, duration, 50.0, false));
    assert!(!should_seek(position, duration, 51.0, false));
    assert!(!should_seek(position, duration, 49.0, false));
    assert!(should_seek(position, duration, 60.0, false));
    assert!(should_seek(position, duration, 40.0, false));
    // Nothing loaded yet: an unknown duration must never seek.
    assert!(!should_seek(position, 0.0, 50.0, false));
    // An echo of a player-driven write never seeks, however far it reads.
    assert!(!should_seek(0.0, duration, 99.0, true));
  }
}
