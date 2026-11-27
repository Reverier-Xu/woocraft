//! The standard transport bar for a [`Player`].
//!
//! [`PlayerView`] renders one by default; a host composing its own chrome
//! creates one standalone — it is usable without the view:
//!
//! ```rust,ignore
//! let transport = cx.new(|cx| {
//!   PlayerTransport::new(&player, cx)
//!     .hide_control(TransportControl::Settings)
//!     .on_control(TransportControl::Fullscreen, |_, _, cx| { /* ... */ })
//! });
//! ```
//!
//! # Layout
//!
//! Two rows: the timeline — position, seek slider, duration — above the
//! controls. The second row is split into a leading group (previous, back
//! ten seconds, play/pause, forward ten seconds, next) and a trailing group
//! (mute, volume, fullscreen, settings). Both groups are replaceable wholesale
//! through [`PlayerTransport::leading`] and [`PlayerTransport::trailing`], and
//! each built-in control can be hidden ([`PlayerTransport::control`]) or
//! rewired to a host action ([`PlayerTransport::on_control`]).
//!
//! # Floating chrome
//!
//! [`PlayerTransport::floating`] turns the bar into an elevated surface — a
//! border and a drop shadow instead of a flush card — for bars laid over the
//! picture rather than docked under it. The bar itself never positions or
//! hides itself: an overlay is the host's layout, since only the host knows
//! when the pointer is idle (see the `player` example).
//!
//! Sizing: a single [`Size`] tier threads through every part — the container
//! metrics, the buttons, the sliders, the icons and the labels — so
//! `.small()` yields an entirely small bar. Text scale rides on the sized
//! controls (`Button`, the time labels); the bar itself never sets a font
//! size. Secondary information (the ticking times) reads as secondary through
//! `theme.muted_foreground` at the same tier, never a smaller font.
//!
//! Every built-in button is flat ([`crate::ButtonVariant::Flat`]): the bar is
//! chrome over content, so the controls carry no resting surface of their own
//! and only show one on hover, active or focus.
//!
//! # Seek preview
//!
//! [`PlayerTransport::seek_preview`] hangs a host-built element off the seek
//! slider — a frame thumbnail, a timecode card — the same way `leading` and
//! `trailing` replace a control group. The host is handed the position the
//! pointer or the drag names; a picture for that position is its to provide.

use std::{rc::Rc, time::Duration};

use gpui::{
  Anchor, AnyElement, App, AppContext as _, ClickEvent, Context, ElementId, Entity, IntoElement,
  ParentElement as _, Render, SharedString, Styled as _, Subscription, Window, div,
  prelude::FluentBuilder as _,
};

use super::{
  event::PlayerEvent,
  player::{Player, format_time},
  snapshot::PlayerSnapshot,
};
use crate::{
  ActiveTheme as _, Button, ButtonVariants as _, Icon, IconLabel, IconName, PopupMenu,
  PopupMenuItem, Sizable, Size, Slider, SliderEvent, SliderState, StyleSized, Theme, Tooltip,
  h_flex, translate, v_flex,
};

/// Playback speeds the default settings menu offers, ascending.
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

/// The seek step of the two skip controls, in seconds, when the host does not
/// set one through [`PlayerTransport::seek_step`].
///
/// Ten seconds is also what the `media.seek_back` / `media.seek_forward`
/// tooltips say, so a host changing the step should keep its own labels in
/// step.
const DEFAULT_SEEK_STEP_SECS: f64 = 10.;

/// The built-in controls of the bar's second row, in bar order.
///
/// Every control is shown by default; [`PlayerTransport::control`] hides or
/// re-shows one, and [`PlayerTransport::on_control`] replaces what clicking it
/// does. The two groups the controls are laid out in — leading (`Previous`,
/// `SeekBackward`, `PlayPause`, `SeekForward`, `Next`) and trailing (`Mute`,
/// `Volume`, `Fullscreen`, `Settings`) — can each be replaced wholesale by a
/// host element through [`PlayerTransport::leading`] and
/// [`PlayerTransport::trailing`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TransportControl {
  /// Previous playlist entry (`playlist-prev`).
  Previous,
  /// Skip backwards by [`PlayerTransport::seek_step`] seconds.
  SeekBackward,
  /// Play/pause toggle, with the icon following the player's state.
  PlayPause,
  /// Skip forwards by [`PlayerTransport::seek_step`] seconds.
  SeekForward,
  /// Next playlist entry (`playlist-next`).
  Next,
  /// Mute/unmute, with the icon following the player's state.
  Mute,
  /// The volume slider.
  Volume,
  /// Window fullscreen toggle, with the icon following the window's state.
  Fullscreen,
  /// Settings; by default it opens a playback-speed menu.
  Settings,
}

impl TransportControl {
  /// Every control, in bar order.
  pub const ALL: [TransportControl; 9] = [
    TransportControl::Previous,
    TransportControl::SeekBackward,
    TransportControl::PlayPause,
    TransportControl::SeekForward,
    TransportControl::Next,
    TransportControl::Mute,
    TransportControl::Volume,
    TransportControl::Fullscreen,
    TransportControl::Settings,
  ];

  /// The controls of the second row's leading group.
  const LEADING: [TransportControl; 5] = [
    TransportControl::Previous,
    TransportControl::SeekBackward,
    TransportControl::PlayPause,
    TransportControl::SeekForward,
    TransportControl::Next,
  ];

  /// The controls of the second row's trailing group.
  const TRAILING: [TransportControl; 4] = [
    TransportControl::Mute,
    TransportControl::Volume,
    TransportControl::Fullscreen,
    TransportControl::Settings,
  ];

  /// This control's bit in [`PlayerTransport`]'s shown/hidden set.
  const fn index(self) -> usize {
    self as usize
  }
}

/// A host action attached to one built-in control.
type TransportHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// A host-built replacement for one side of the second row.
type TransportSlot = Rc<dyn Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement>;

/// A host-built preview hung off the seek slider.
type SeekPreviewBuilder = Rc<dyn Fn(SeekPreview, &mut Window, &mut App) -> AnyElement>;

/// A host-built settings menu, opened by the settings button.
type SettingsMenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// What a seek preview asks the host to draw.
///
/// The transport knows where the pointer or the drag stands in the entry; a
/// picture for that position — a decoded frame, a cached thumbnail — is the
/// host's to provide, since only the host has a thumbnail source. The timecode
/// is measured in seconds of the current entry, so a preview can format it with
/// [`format_time`](super::format_time) like the bar's own labels do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeekPreview {
  /// The position the preview stands for, in seconds.
  pub position: f64,
  /// The duration of the current entry, in seconds; zero with nothing loaded.
  pub duration: f64,
  /// Whether a drag rather than a hover is showing the preview.
  pub dragging: bool,
}

/// The player and window state one render pass draws its controls from.
#[derive(Clone, Copy)]
struct ControlState {
  paused: bool,
  muted: bool,
  fullscreen: bool,
}

/// The transport bar: a timeline row above a control row, bound to one
/// [`Player`].
///
/// The bar owns the seek/volume slider states and the loop guard that keeps
/// the seek slider in sync with the player's position reports (see
/// `PlayerTransport::sync_seek`). Element ids are scoped to the player, so
/// several transports can live in one window.
///
/// [`PlayerView`]: super::PlayerView
pub struct PlayerTransport {
  player: Entity<Player>,
  seek: Entity<SliderState>,
  volume: Entity<SliderState>,
  /// Seek-slider changes still to be swallowed as echoes of player-driven
  /// writes; see [`Self::sync_seek`].
  pending_seek_echoes: usize,
  size: Size,
  /// The skip step of [`TransportControl::SeekBackward`] and
  /// [`TransportControl::SeekForward`], in seconds.
  seek_step: f64,
  /// Whether the bar draws itself as an elevated surface for use over the
  /// picture; see [`Self::floating`].
  floating: bool,
  /// One bit per hidden [`TransportControl`]; zero shows everything. `u16`
  /// so the set has room for the controls to grow past eight entries, which
  /// a `u8` bit set would not.
  hidden: u16,
  /// Host actions overriding the built-in behavior, per control.
  handlers: [Option<TransportHandler>; TransportControl::ALL.len()],
  /// Host-built settings menu; `None` uses the default speed menu.
  settings_menu: Option<SettingsMenuBuilder>,
  /// Host-built replacement for the second row's leading group.
  leading: Option<TransportSlot>,
  /// Host-built replacement for the second row's trailing group.
  trailing: Option<TransportSlot>,
  /// Host-built preview on the seek slider; `None` leaves the slider plain.
  seek_preview: Option<SeekPreviewBuilder>,
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
      seek_step: DEFAULT_SEEK_STEP_SECS,
      floating: false,
      hidden: 0,
      handlers: std::array::from_fn(|_| None),
      settings_menu: None,
      leading: None,
      trailing: None,
      seek_preview: None,
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

  /// Sets how far the two skip controls move playback, in seconds: that much
  /// back for [`TransportControl::SeekBackward`], that much forward for
  /// [`TransportControl::SeekForward`].
  ///
  /// Defaults to ten seconds. The built-in tooltips name ten seconds (the
  /// `media.seek_back` and `media.seek_forward` translations), so a host
  /// choosing another step should supply its own labels — rewiring the two
  /// controls through [`Self::on_control`] keeps the icons and tooltip slot
  /// available for that.
  pub fn seek_step(mut self, seconds: f64) -> Self {
    self.set_seek_step(seconds);
    self
  }

  /// Runtime form of [`Self::seek_step`]; a non-finite or non-positive step
  /// is ignored rather than turning the skip controls into no-ops.
  pub fn set_seek_step(&mut self, seconds: f64) {
    if seconds.is_finite() && seconds > 0.0 {
      self.seek_step = seconds;
    }
  }

  /// The skip step of the two seek controls, in seconds; see
  /// [`Self::seek_step`].
  pub fn seek_step_secs(&self) -> f64 {
    self.seek_step
  }

  /// Draws the bar as a floating surface: a hairline border and a drop shadow
  /// around the same card, for a bar laid over the picture instead of docked
  /// beneath it.
  ///
  /// The flag only changes the surface. Positioning, showing and hiding the
  /// bar over the stage stay the host's business, since only the host knows
  /// when the pointer is idle — the `player` example does both.
  pub fn floating(mut self, floating: bool) -> Self {
    self.floating = floating;
    self
  }

  /// Shows or hides one built-in control. Every control is shown by default.
  ///
  /// Hiding a control removes it from its group; the group itself stays, so
  /// the remaining controls keep their alignment. A group whose controls are
  /// all hidden disappears (and takes the second row with it when both are).
  pub fn control(mut self, control: TransportControl, shown: bool) -> Self {
    self.set_control(control, shown);
    self
  }

  /// Shows one built-in control; see [`Self::control`].
  pub fn show_control(self, control: TransportControl) -> Self {
    self.control(control, true)
  }

  /// Hides one built-in control; see [`Self::control`].
  pub fn hide_control(self, control: TransportControl) -> Self {
    self.control(control, false)
  }

  /// Runtime form of [`Self::control`], for a bar that already exists.
  pub fn set_control(&mut self, control: TransportControl, shown: bool) {
    let bit = 1u16 << control.index();
    if shown {
      self.hidden &= !bit;
    } else {
      self.hidden |= bit;
    }
  }

  /// Whether one built-in control is currently shown.
  pub fn is_control_shown(&self, control: TransportControl) -> bool {
    self.hidden & (1u16 << control.index()) == 0
  }

  /// Replaces what clicking one built-in control does, keeping its icon,
  /// tooltip and position.
  ///
  /// The icons still follow the player and window state (play/pause, mute,
  /// fullscreen), so a host action reads as the same control with different
  /// behavior. Overriding `Settings` also takes precedence over
  /// [`Self::settings_menu`]: the button stops opening a menu and runs the
  /// action instead.
  pub fn on_control(
    mut self, control: TransportControl,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.set_control_action(control, handler);
    self
  }

  /// Runtime form of [`Self::on_control`].
  pub fn set_control_action(
    &mut self, control: TransportControl,
    handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) {
    self.handlers[control.index()] = Some(Rc::new(handler));
  }

  /// Drops a host action and restores the built-in behavior of one control.
  pub fn clear_control_action(&mut self, control: TransportControl) {
    self.handlers[control.index()] = None;
  }

  /// Replaces the menu the settings button opens.
  ///
  /// Without it, settings opens the playback-speed menu. With
  /// [`Self::on_control`]`(TransportControl::Settings, ..)` the action wins
  /// and no menu opens.
  pub fn settings_menu(
    mut self,
    builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    self.settings_menu = Some(Rc::new(builder));
    self
  }

  /// Replaces the second row's leading group (previous, play/pause, next)
  /// with a host-built element.
  ///
  /// The closure receives a fresh [`PlayerSnapshot`] every render, like the
  /// [`PlayerView`](super::PlayerView) slots. When set, none of the built-in
  /// leading controls render — hide or re-mount them through the usual element
  /// builders, or keep them and skip this slot.
  pub fn leading(
    mut self, build: impl Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.leading = Some(Rc::new(build));
    self
  }

  /// Replaces the second row's trailing group (mute, volume, fullscreen,
  /// settings) with a host-built element; see [`Self::leading`].
  pub fn trailing(
    mut self, build: impl Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.trailing = Some(Rc::new(build));
    self
  }

  /// Hangs a host-built preview off the seek slider.
  ///
  /// The content is any element, not only a text tooltip: a seek bar preview is
  /// typically a frame thumbnail with the timecode beside it (see the `player`
  /// example). The closure is asked on every render while the preview is
  /// visible — so it follows a drag's live position — and receives the position
  /// it stands for, the entry's duration and whether a drag is showing it.
  ///
  /// It shows on hover anywhere along the rail and during a drag; see
  /// [`Slider::preview`] for the lower-level trigger.
  pub fn seek_preview(
    mut self, build: impl Fn(SeekPreview, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.seek_preview = Some(Rc::new(build));
    self
  }

  /// Sets the size tier without a rebuild. `PlayerView` threads its own tier
  /// into its default transport through this on every render, so resizing the
  /// view resizes the bar with it.
  pub(crate) fn set_size(&mut self, size: Size) {
    self.size = size;
  }

  /// The action a control runs: the host override when there is one, else the
  /// built-in behavior.
  fn handler(
    &self, control: TransportControl,
    built_in: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> TransportHandler {
    self.handlers[control.index()]
      .clone()
      .unwrap_or_else(|| Rc::new(built_in))
  }

  /// Builds one built-in control, or `None` when the host hid it.
  ///
  /// Controls are built lazily per render (rather than cached) because every
  /// one of them reads live state — the play/pause icon follows `paused`, the
  /// mute icon `muted`, the fullscreen icon the window.
  fn built_in(
    &self, control: TransportControl, state: ControlState, size: Size,
  ) -> Option<AnyElement> {
    if !self.is_control_shown(control) {
      return None;
    }
    let id = self.player.entity_id().as_u64();

    Some(match control {
      TransportControl::Previous => {
        let player = self.player.clone();
        let handler = self.handler(control, move |_, _, cx| {
          let _ = player.read(cx).playlist_prev();
        });
        control_button(
          ("player-previous", id),
          IconName::Previous,
          tooltip("media.previous"),
          size,
        )
        .on_click(move |event, window, cx| handler(event, window, cx))
        .into_any_element()
      }
      TransportControl::PlayPause => {
        let player = self.player.clone();
        let handler = self.handler(control, move |_, _, cx| {
          let _ = player.update(cx, |player, cx| player.toggle_pause(cx));
        });
        control_button(
          ("player-play", id),
          if state.paused {
            IconName::Play
          } else {
            IconName::Pause
          },
          tooltip(if state.paused {
            "media.play"
          } else {
            "media.pause"
          }),
          size,
        )
        .on_click(move |event, window, cx| handler(event, window, cx))
        .into_any_element()
      }
      TransportControl::Next => {
        let player = self.player.clone();
        let handler = self.handler(control, move |_, _, cx| {
          let _ = player.read(cx).playlist_next();
        });
        control_button(
          ("player-next", id),
          IconName::Next,
          tooltip("media.next"),
          size,
        )
        .on_click(move |event, window, cx| handler(event, window, cx))
        .into_any_element()
      }
      TransportControl::SeekBackward | TransportControl::SeekForward => {
        let step = if matches!(control, TransportControl::SeekBackward) {
          -self.seek_step
        } else {
          self.seek_step
        };
        let player = self.player.clone();
        let handler = self.handler(control, move |_, _, cx| {
          let _ = player.read(cx).seek_by(step);
        });
        let (element_id, icon, label) = match control {
          TransportControl::SeekBackward => (
            ("player-seek-back", id),
            IconName::Rewind,
            "media.seek_back",
          ),
          _ => (
            ("player-seek-forward", id),
            IconName::FastForward,
            "media.seek_forward",
          ),
        };
        control_button(element_id, icon, tooltip(label), size)
          .on_click(move |event, window, cx| handler(event, window, cx))
          .into_any_element()
      }
      TransportControl::Mute => {
        let player = self.player.clone();
        let handler = self.handler(control, move |_, _, cx| {
          let _ = player.update(cx, |player, _| {
            let muted = player.state().muted;
            player.set_muted(!muted)
          });
        });
        control_button(
          ("player-mute", id),
          if state.muted {
            IconName::SpeakerOff
          } else {
            IconName::Speaker2
          },
          tooltip(if state.muted {
            "media.unmute"
          } else {
            "media.mute"
          }),
          size,
        )
        .on_click(move |event, window, cx| handler(event, window, cx))
        .into_any_element()
      }
      TransportControl::Volume => div()
        .flex_none()
        .w(size.em(VOLUME_SLIDER_WIDTH_EM))
        .child(Slider::new(format!("player-volume-{id}"), &self.volume).with_size(size))
        .into_any_element(),
      TransportControl::Fullscreen => {
        let handler = self.handler(control, move |_, window, _| window.toggle_fullscreen());
        control_button(
          ("player-fullscreen", id),
          if state.fullscreen {
            IconName::FullScreenMinimize
          } else {
            IconName::FullScreenMaximize
          },
          tooltip(if state.fullscreen {
            "media.exit_fullscreen"
          } else {
            "media.fullscreen"
          }),
          size,
        )
        .on_click(move |event, window, cx| handler(event, window, cx))
        .into_any_element()
      }
      TransportControl::Settings => {
        let button = control_button(
          ("player-settings", id),
          IconName::Settings,
          tooltip("media.settings"),
          size,
        );
        match self.handlers[control.index()].clone() {
          // An action replaces the menu outright: a button that both ran an
          // action and opened a menu would do two things on one click.
          Some(handler) => button
            .on_click(move |event, window, cx| handler(event, window, cx))
            .into_any_element(),
          None => {
            let player = self.player.clone();
            let builder = self.settings_menu.clone();
            button
              // Above the trigger: the bar sits at the bottom of the player,
              // so a menu opening downwards would leave the window.
              .menu(move |menu, window, cx| match builder.as_ref() {
                Some(build) => build(menu, window, cx),
                None => speed_menu(menu, &player, cx),
              })
              .anchor(Anchor::BottomLeft)
              .into_any_element()
          }
        }
      }
    })
  }

  /// The built-in controls of one group, skipping hidden ones.
  fn group(
    &self, controls: &[TransportControl], state: ControlState, size: Size,
  ) -> Vec<AnyElement> {
    controls
      .iter()
      .filter_map(|control| self.built_in(*control, state, size))
      .collect()
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
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let size = self.size;
    let theme = cx.theme().clone();
    let id = self.player.entity_id().as_u64();
    let (paused, muted, position, duration) = {
      let state = self.player.read(cx).state();
      (state.paused, state.muted, state.position, state.duration)
    };
    let state = ControlState {
      paused,
      muted,
      fullscreen: window.is_fullscreen(),
    };

    // Row one: the timeline. The slider takes whatever the two ticking
    // labels leave, so the bar fills its container at any width.
    let mut seek = Slider::new(format!("player-seek-{id}"), &self.seek).with_size(size);
    if let Some(build) = self.seek_preview.clone() {
      // The slider speaks percentages of the entry; a preview speaks seconds,
      // so the conversion lives here instead of in every host.
      seek = seek.preview(move |preview, window, cx| {
        build(
          SeekPreview {
            position: seek_target(duration, preview.value),
            duration,
            dragging: preview.dragging,
          },
          window,
          cx,
        )
      });
    }

    let timeline = h_flex()
      .w_full()
      .items_center()
      .container_gap(size)
      .child(time_label(("player-position", id), size, &theme, position))
      .child(div().min_w_0().flex_1().child(seek))
      .child(time_label(("player-duration", id), size, &theme, duration));

    let mut rows = vec![timeline.into_any_element()];

    // Row two: the control groups. A custom slot replaces its group entirely;
    // a group with nothing left to show is dropped, and when both are gone
    // the row goes with them instead of leaving a gap under the timeline.
    let snapshot = (self.leading.is_some() || self.trailing.is_some())
      .then(|| PlayerSnapshot::new(&self.player, cx));

    let leading = if let Some(build) = self.leading.as_ref() {
      snapshot
        .as_ref()
        .map(|snapshot| build(snapshot, window, cx))
    } else {
      let controls = self.group(&TransportControl::LEADING, state, size);
      (!controls.is_empty()).then(|| {
        h_flex()
          .items_center()
          .container_gap(size)
          .children(controls)
          .into_any_element()
      })
    };

    let trailing = if let Some(build) = self.trailing.as_ref() {
      snapshot
        .as_ref()
        .map(|snapshot| build(snapshot, window, cx))
    } else {
      let controls = self.group(&TransportControl::TRAILING, state, size);
      (!controls.is_empty()).then(|| {
        h_flex()
          .items_center()
          .container_gap(size)
          .children(controls)
          .into_any_element()
      })
    };

    if leading.is_some() || trailing.is_some() {
      rows.push(
        h_flex()
          .w_full()
          .items_center()
          .container_gap(size)
          // The leading side grows so a host slot can stretch (a title, a
          // badge row); the trailing side keeps its natural width.
          .child(div().min_w_0().flex_1().children(leading))
          .children(trailing)
          .into_any_element(),
      );
    }

    v_flex()
      .w_full()
      .container_px(size)
      .container_py(size)
      .container_gap_y(size)
      .container_rounded(size)
      .bg(theme.card)
      .text_color(theme.card_foreground)
      // A floating bar reads as an elevated surface: the hairline keeps it
      // apart from a bright frame behind it, the shadow lifts it off the
      // picture. A docked bar needs neither.
      .when(self.floating, |this| {
        this.border_1().border_color(theme.border).shadow_md()
      })
      .children(rows)
  }
}

/// A built-in control button: flat, tier-sized, icon-only, with a tooltip.
fn control_button(
  id: impl Into<ElementId>, icon: IconName, tooltip: SharedString, size: Size,
) -> Button {
  Button::new(id)
    .with_size(size)
    .flat()
    .icon(Icon::new(icon))
    .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
}

/// A translated tooltip label.
fn tooltip(key: &str) -> SharedString {
  translate(key).into()
}

/// The default settings menu: playback speed, checked against the live
/// `speed` property so a menu opened after a speed change shows the truth.
fn speed_menu(mut menu: PopupMenu, player: &Entity<Player>, cx: &mut App) -> PopupMenu {
  let current = player.read(cx).state().speed;
  for speed in SPEEDS {
    let player = player.clone();
    menu = menu.item(
      PopupMenuItem::new(format!("{speed}x"))
        .checked((current - speed).abs() < f64::EPSILON)
        .on_click(move |_, _, cx| {
          let _ = player.update(cx, |player, _| player.set_speed(speed));
        }),
    );
  }
  menu
}

/// One ticking time label (position or duration): secondary information, so
/// muted colour at the bar's own tier — never a smaller font.
///
/// An [`IconLabel`] takes the label's metrics, colour and insets from the bar's
/// tier instead of hardcoding a text size, so the times line up with every
/// other label and scale with the window font like the buttons beside them.
///
/// The label is still sized by its text: `format_time` switches from `M:SS` to
/// `H:MM:SS` only once the value passes an hour, and every glyph of the UI
/// family is a single cell wide, so the box changes width at minute and hour
/// boundaries — not on every tick. A reserved width would leave slack that
/// reads as a gap between the time and the slider; the seek slider's `flex_1`
/// absorbs the shift instead.
fn time_label(
  element_id: impl Into<ElementId>, size: Size, theme: &Theme, seconds: f64,
) -> IconLabel {
  IconLabel::new(element_id)
    .with_size(size)
    .label(format_time(Duration::from_secs_f64(seconds.max(0.0))))
    .text_color(theme.muted_foreground)
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
  (seek_target(duration, slider_percentage) - position).abs() >= SEEK_EPSILON_SECS
}

/// The position a percentage of the entry stands for: the seek slider speaks
/// percentages, the player and its previews speak seconds.
fn seek_target(duration: f64, percentage: f32) -> f64 {
  duration * f64::from(percentage) / 100.0
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

  #[test]
  fn every_control_owns_one_bit_and_one_group_slot() {
    // The shown/hidden set is a bit per control, so the indices must be
    // distinct; and every control must be reachable from one of the two
    // groups, or hiding it could never hide anything.
    let mut seen = 0u32;
    for control in TransportControl::ALL {
      let bit = 1u32 << control.index();
      assert_eq!(seen & bit, 0, "{control:?} reuses another control's bit");
      seen |= bit;
    }
    // The bar's own set is a `u16`: one more control than it has bits would
    // silently alias two controls into one hidden flag.
    assert!(
      TransportControl::ALL.len() <= u16::BITS as usize,
      "the shown/hidden set cannot hold {} controls",
      TransportControl::ALL.len()
    );
    // `index()` is the discriminant, so `ALL` has to stay in declaration order
    // or a control's bit would drift away from its position.
    for (position, control) in TransportControl::ALL.into_iter().enumerate() {
      assert_eq!(control.index(), position, "{control:?} is out of order");
    }
    let mut grouped = Vec::new();
    grouped.extend(TransportControl::LEADING);
    grouped.extend(TransportControl::TRAILING);
    for control in TransportControl::ALL {
      assert!(grouped.contains(&control), "{control:?} is in no group");
    }
  }

  #[test]
  fn hiding_controls_is_a_per_control_switch() {
    // The bit set is a plain per-control flag: hiding one leaves every other
    // control alone, and showing it again restores exactly that one.
    let mut hidden = 0u16;
    let hide = |hidden: &mut u16, control: TransportControl| *hidden |= 1 << control.index();
    let show = |hidden: &mut u16, control: TransportControl| *hidden &= !(1 << control.index());
    hide(&mut hidden, TransportControl::Settings);
    assert_eq!(hidden, 1 << TransportControl::Settings.index());
    hide(&mut hidden, TransportControl::Volume);
    assert_eq!(
      hidden,
      (1 << TransportControl::Settings.index()) | (1 << TransportControl::Volume.index())
    );
    show(&mut hidden, TransportControl::Settings);
    assert_eq!(hidden, 1 << TransportControl::Volume.index());
  }
}
