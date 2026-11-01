//! A general-purpose audio/video player on the optional `media` feature.
//!
//! ```text
//! cargo run -p woocraft --features media --example player -- \
//!   [--transport=gpu|software] [--decoder=no|auto|auto-safe|auto-copy|vaapi|vulkan|nvdec] \
//!   [--size=sm|md|lg] [--audio] [file-or-url]
//! ```
//!
//! The flags set the opening state; every one of them can be changed from
//! the window afterwards:
//!
//! - `--transport` requests the video transport of the first player (`gpu` is
//!   the default on Linux, `software` elsewhere). The transport is negotiated
//!   when a player is built — it is not a runtime property — so switching it
//!   rebuilds the player entity with the same source (see
//!   [`PlayerApp::rebuild_player`]). Negotiation and any fallback log at
//!   `RUST_LOG=woocraft=warn`.
//! - `--decoder` picks the opening hardware-decoding mode; later choices are
//!   applied live through the `hwdec` property, and the panel shows the value
//!   mpv actually reports. The typed equivalent is
//!   `Player::set_hardware_decoding` with [`HwDec`]; the example writes the
//!   property directly to double as a passthrough demonstration.
//! - `--size` sets the one size tier that drives the `PlayerView` and every
//!   control in the window (`Sizable`).
//! - `--audio` plays audio only (`PlayerOptions::audio_only`).
//! - The optional argument is a file path or URL; without it the header's
//!   source field loads one through `Player::load`.
//!
//! The window:
//!
//! - The header carries the source field, the title (`PlayerState::title`,
//!   falling back to the source string), and the latest error —
//!   `PlayerState::error` or a failed control call — at the same tier in the
//!   muted colour, never a bespoke banner.
//! - The left column is a `PlayerView` with its default stage and transport
//!   bar; the standalone `PlayerTrackMenu` and `PlayerPlaylist` are mounted
//!   through the `controls` slots. That is the whole pluggable-chrome point:
//!   nothing of this example lives inside the library.
//! - The right column is the control panel, one group per area of the `Player`
//!   API. Playback: seek steps, frame stepping, speed, the A-B loop, chapters.
//!   Audio: volume and mute (the bar carries its own pair), balance, delay,
//!   ReplayGain, pitch correction, the output device, the raw `af` chain with
//!   an equalizer preset, the volume ceiling. Subtitles: track pickers, delay,
//!   scale, position, visibility, ASS styling, external files. Video: decoder,
//!   transport, size tier, deinterlace, rotation, aspect ratio. Info:
//!   `PlayerState` fields, track and metadata summaries, and a handful of live
//!   mpv properties (`Player::get_property`) polled once a second — structure
//!   refreshes on player events, runtime stats on the timer, and neither runs
//!   on the per-render path.

use std::time::Duration;

use gpui::{
  App, AppContext as _, Bounds, ClickEvent, Context, Div, Entity, InteractiveElement as _,
  IntoElement, ParentElement as _, Render, Styled as _, Subscription, Task, Window, WindowBounds,
  WindowOptions, div, prelude::FluentBuilder as _, px,
};
use woocraft::{
  ActiveTheme as _, Button, Disableable, Divider, DropdownMenu, Icon, IconLabel, IconName, Input,
  InputEvent, InputState, Player, PlayerEvent, PlayerOptions, PlayerPlaylist, PlayerTrackMenu,
  PlayerView, PopupMenuItem, PropertyValue, ScrollableElement, ScrollbarShow, Sizable, Size,
  Slider, SliderEvent, SliderOrigin, SliderState, StyleSized, Switch, Theme, ThemeMode, TitleBar,
  Tooltip, Track, TrackKind, VideoTransport, h_flex, init, translate_woocraft, v_flex,
};

/// The decoder menu entries, as mpv `hwdec` values: the four generic modes
/// plus three concrete APIs. Any of them applies live through the `hwdec`
/// property (`Player::set_property`, the same write
/// `Player::set_hardware_decoding` performs — see the module docs for why
/// that method is not called by name here).
const DECODERS: [&str; 7] = [
  "no",
  "auto",
  "auto-safe",
  "auto-copy",
  "vaapi",
  "vulkan",
  "nvdec",
];

/// The playback speeds the speed picker offers.
const SPEEDS: [f64; 4] = [0.5, 1.0, 1.5, 2.0];

/// `(frequency Hz, gain dB)` bands of the equalizer presets, applied through
/// `Player::set_equalizer`.
/// Centre frequencies of the three equalizer bands, in Hz.
const EQ_BASS_HZ: f64 = 80.0;
const EQ_MID_HZ: f64 = 1_000.0;
const EQ_TREBLE_HZ: f64 = 10_000.0;

/// The opening state parsed from the CLI.
#[derive(Clone)]
struct Launch {
  /// A file path or URL to load, if any.
  source: Option<String>,
  /// Whether video output is enabled (`--audio` clears it).
  video: bool,
  /// The transport to request for the first player.
  transport: VideoTransport,
  /// The hardware-decoding mode to start with, as an mpv `hwdec` value.
  decoder: &'static str,
  /// The size tier the whole window starts at.
  size: Size,
}

/// Width of one panel slider, in ems of the panel's size tier.
const SLIDER_WIDTH_EM: f32 = 16.;

/// The mpv properties the info panel shows. Each is one `get_property` round
/// trip: far too cheap to justify per-frame reads, and (unlike structure)
/// live enough that player events alone would not refresh them — so they are
/// polled once a second by [`PlayerApp::spawn_live_poll`].
#[derive(Default)]
struct LiveInfo {
  /// The `hwdec` mode mpv reports, not the one that was requested.
  hwdec: Option<String>,
  video_format: Option<String>,
  audio_codec: Option<String>,
  /// `sub-visibility`.
  sub_visibility: Option<bool>,
  /// `sub-ass`.
  sub_ass: Option<bool>,
  /// `audio-pitch-correction`.
  pitch: Option<bool>,
  /// `replaygain`.
  replaygain: Option<String>,
  /// `deinterlace`.
  deinterlace: Option<String>,
  /// `video-rotate`.
  rotation: Option<i64>,
  /// `demuxer-cache-duration`, formatted.
  demuxer_cache: Option<String>,
  /// `cache-speed`, formatted.
  cache_speed: Option<String>,
}

/// A text field's Enter handler.
type EnterApply = fn(&mut PlayerApp, &mut Context<PlayerApp>);

/// The application: one player, its view, and the panel state around them.
struct PlayerApp {
  /// The player entity. Replaced only by [`Self::rebuild_player`].
  player: Entity<Player>,
  /// The player's view; replaced when the player is rebuilt or the size tier
  /// changes (its tier is set at construction through `Sizable`).
  view: Entity<PlayerView>,
  /// The transport the *next* player build will request.
  transport: VideoTransport,
  /// The decoder mode (an mpv `hwdec` value) new players start with;
  /// changes also apply live.
  hwdec: &'static str,
  /// The one size tier driving the view and every control.
  size: Size,
  /// Audio-only mode: no video output, nothing to transport.
  audio_only: bool,
  /// The source the player was last asked to load, so a rebuild can reissue
  /// it.
  source: Option<String>,
  /// The latest failure to surface in the header (a failed control call or
  /// a reported mpv error).
  notice: Option<String>,
  /// Live mpv properties, refreshed once a second.
  live: LiveInfo,
  /// Track counts, refreshed on structural player events only.
  track_summary: String,
  /// The selected subtitle track's label, same refresh rule as above.
  subtitle_label: String,
  /// The selected secondary subtitle track's label.
  secondary_label: String,
  /// The A-B loop this panel last set: `(start, end)`, unset legs `None`.
  ab_loop: (Option<f64>, Option<f64>),
  /// The audio delay this panel last applied, in seconds.
  audio_delay: f64,
  /// The subtitle delay this panel last applied, in seconds.
  subtitle_delay: f64,
  /// Volume-slider writes still to be swallowed as echoes of player-driven
  /// syncs (the same loop rule the transport bar uses).
  pending_volume_echoes: usize,
  // Text fields and sliders live in entities, so they keep their content,
  // caret and thumb across renders; the builders below only re-mount them.
  source_input: Entity<InputState>,
  audio_device: Entity<InputState>,
  audio_filters: Entity<InputState>,
  volume_max: Entity<InputState>,
  aspect: Entity<InputState>,
  subtitle_path: Entity<InputState>,
  volume: Entity<SliderState>,
  balance: Entity<SliderState>,
  eq_bass: Entity<SliderState>,
  eq_mid: Entity<SliderState>,
  eq_treble: Entity<SliderState>,
  sub_scale: Entity<SliderState>,
  sub_pos: Entity<SliderState>,
  _player_subscription: Subscription,
  _control_subscriptions: Vec<Subscription>,
  _live_poll: Task<()>,
}

impl PlayerApp {
  fn new(launch: Launch, cx: &mut Context<Self>) -> Self {
    let size = launch.size;
    let launch_source = launch.source.clone();

    let source_input = cx.new(|cx| {
      let state = InputState::new(cx).placeholder(t("media.source_placeholder"));
      match launch_source {
        Some(source) => state.default_value(source),
        None => state,
      }
    });
    let audio_device = cx.new(|cx| InputState::new(cx).placeholder("auto"));
    let audio_filters =
      cx.new(|cx| InputState::new(cx).placeholder(t("media.filters_placeholder")));
    let volume_max = cx.new(|cx| InputState::new(cx).default_value("130"));
    let aspect = cx.new(|cx| InputState::new(cx).placeholder(t("media.aspect_placeholder")));
    let subtitle_path =
      cx.new(|cx| InputState::new(cx).placeholder(t("media.subtitle_path_placeholder")));

    let player = cx.new(|cx| {
      Player::new(
        cx,
        options_for(launch.video, launch.transport, launch.decoder),
      )
    });
    let initial_volume = player.read(cx).state().volume as f32;
    let volume = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(1.0)
        .default_value(initial_volume)
    });
    let balance = cx.new(|_| {
      SliderState::new()
        .min(-1.0)
        .max(1.0)
        .step(0.05)
        .default_value(0.0)
    });
    // Equalizer bands: a gain in dB around zero, so the fill grows from the
    // middle, and ticks make the steps readable.
    let mut eq_band = || {
      cx.new(|_| {
        SliderState::new()
          .min(-12.0)
          .max(12.0)
          .step(1.0)
          .default_value(0.0)
          .with_origin(SliderOrigin::Middle)
          .with_ticks(true)
      })
    };
    let eq_bass = eq_band();
    let eq_mid = eq_band();
    let eq_treble = eq_band();
    let sub_scale = cx.new(|_| {
      SliderState::new()
        .min(0.5)
        .max(3.0)
        .step(0.05)
        .default_value(1.0)
    });
    let sub_pos = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(150.0)
        .step(5.0)
        .default_value(100.0)
    });
    let view = mount_player_view(&player, size, cx);
    let live_poll = Self::spawn_live_poll(cx);
    let player_subscription = cx.subscribe(&player, Self::on_player_event);
    let notice = player.read(cx).state().error.clone();

    let mut this = Self {
      player,
      view,
      transport: launch.transport,
      hwdec: launch.decoder,
      size,
      audio_only: !launch.video,
      source: launch.source.clone(),
      notice,
      live: LiveInfo::default(),
      track_summary: String::new(),
      subtitle_label: t("media.off"),
      secondary_label: t("media.off"),
      ab_loop: (None, None),
      audio_delay: 0.0,
      subtitle_delay: 0.0,
      pending_volume_echoes: 0,
      source_input,
      audio_device,
      audio_filters,
      volume_max,
      aspect,
      subtitle_path,
      volume,
      balance,
      eq_bass,
      eq_mid,
      eq_treble,
      sub_scale,
      sub_pos,
      _player_subscription: player_subscription,
      _control_subscriptions: Vec::new(),
      _live_poll: live_poll,
    };
    this.bind_controls(cx);
    this.refresh_structure(cx);
    this.refresh_live(cx);
    if let Some(source) = launch.source {
      let result = this.player.update(cx, |player, _| player.load(&source));
      this.note_result(cx, "load", result);
    }
    this
  }

  /// Subscribes the panel's own controls. They live across player rebuilds:
  /// every handler reaches the player through `this.player`, so switching
  /// transports never rebinds them.
  fn bind_controls(&mut self, cx: &mut Context<Self>) {
    let mut subscriptions = Vec::new();

    subscriptions.push(cx.subscribe(
      &self.source_input,
      |this: &mut Self, _, event: &InputEvent, cx| {
        if matches!(event, InputEvent::PressEnter { .. }) {
          this.open_source(cx);
        }
      },
    ));
    let text_fields: [(Entity<InputState>, EnterApply); 5] = [
      (self.audio_device.clone(), Self::apply_audio_device),
      (self.audio_filters.clone(), Self::apply_audio_filters),
      (self.volume_max.clone(), Self::apply_volume_max),
      (self.aspect.clone(), Self::apply_aspect),
      (self.subtitle_path.clone(), Self::add_external_subtitle),
    ];
    for (field, apply) in text_fields {
      subscriptions.push(cx.subscribe(
        &field,
        move |this: &mut Self, _, event: &InputEvent, cx| {
          if matches!(event, InputEvent::PressEnter { .. }) {
            apply(this, cx);
          }
        },
      ));
    }

    // Equalizer bands drive one filter chain: every band change re-applies the
    // three gains together, so they compose instead of replacing each other.
    for band in [
      self.eq_bass.clone(),
      self.eq_mid.clone(),
      self.eq_treble.clone(),
    ] {
      subscriptions.push(
        cx.subscribe(&band, |this: &mut Self, _, event: &SliderEvent, cx| {
          let value = match event {
            SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
            SliderEvent::Preview(_) => return,
          };
          // Range sliders are not used for the bands, so the committed single
          // value is what the filter chain takes.
          debug_assert!(!value.is_range());
          let bands = [
            (EQ_BASS_HZ, this.eq_bass.read(cx).value().start()),
            (EQ_MID_HZ, this.eq_mid.read(cx).value().start()),
            (EQ_TREBLE_HZ, this.eq_treble.read(cx).value().start()),
          ];
          let bands: Vec<(f64, f64)> = bands
            .into_iter()
            .map(|(hz, gain)| (hz, f64::from(gain)))
            .collect();
          this.call_player(cx, "equalizer", move |player| player.set_equalizer(&bands));
        }),
      );
    }

    subscriptions.push(cx.subscribe(
      &self.volume,
      |this: &mut Self, _, event: &SliderEvent, cx| {
        let value = match event {
          SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
          // A non-immediate drag preview is not applied by the host; the value
          // arrives again as `Commit` when the pointer is released.
          SliderEvent::Preview(_) => return,
        };
        // A player-driven sync echoes back as a user drag would; only genuine
        // drags may write the volume.
        if this.pending_volume_echoes > 0 {
          this.pending_volume_echoes -= 1;
        } else {
          let volume = f64::from(value.start());
          this.call_player(cx, "volume", move |player| player.set_volume(volume));
        }
      },
    ));
    subscriptions.push(cx.subscribe(
      &self.balance,
      |this: &mut Self, _, event: &SliderEvent, cx| {
        let value = match event {
          SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
          // A non-immediate drag preview is not applied by the host; the value
          // arrives again as `Commit` when the pointer is released.
          SliderEvent::Preview(_) => return,
        };
        let balance = f64::from(value.start()).clamp(-1.0, 1.0);
        this.call_player(cx, "balance", move |player| player.set_balance(balance));
      },
    ));
    subscriptions.push(cx.subscribe(
      &self.sub_scale,
      |this: &mut Self, _, event: &SliderEvent, cx| {
        let value = match event {
          SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
          // A non-immediate drag preview is not applied by the host; the value
          // arrives again as `Commit` when the pointer is released.
          SliderEvent::Preview(_) => return,
        };
        let scale = f64::from(value.start());
        this.call_player(cx, "sub-scale", move |player| {
          player.set_subtitle_scale(scale)
        });
      },
    ));
    subscriptions.push(cx.subscribe(
      &self.sub_pos,
      |this: &mut Self, _, event: &SliderEvent, cx| {
        let value = match event {
          SliderEvent::Change(value) | SliderEvent::Commit(value) => value,
          // A non-immediate drag preview is not applied by the host; the value
          // arrives again as `Commit` when the pointer is released.
          SliderEvent::Preview(_) => return,
        };
        let position = f64::from(value.start());
        this.call_player(cx, "sub-pos", move |player| {
          player.set_subtitle_position(position)
        });
      },
    ));

    self._control_subscriptions = subscriptions;
  }

  /// One player report. Structure changes refresh the cached summaries, the
  /// volume property keeps the panel slider in step (echo-guarded), and
  /// errors surface in the header.
  fn on_player_event(
    this: &mut Self, _player: Entity<Player>, event: &PlayerEvent, cx: &mut Context<Self>,
  ) {
    match event {
      PlayerEvent::Error(message) => this.notice = Some(message.clone()),
      PlayerEvent::EndFile { reason } if reason.contains("Error") => {
        this.notice = Some(reason.clone());
      }
      PlayerEvent::FileLoaded | PlayerEvent::VideoReconfig | PlayerEvent::AudioReconfig => {
        this.refresh_structure(cx);
        this.refresh_live(cx);
      }
      PlayerEvent::PropertyChanged {
        name,
        value: PropertyValue::Double(volume),
      } if name == "volume" => {
        this.pending_volume_echoes += 1;
        this
          .volume
          .update(cx, |slider, cx| slider.set_value(*volume as f32, cx));
      }
      _ => {}
    }
    cx.notify();
  }

  /// Polls the live mpv properties once a second: honest about cost, since
  /// each read is a synchronous `get_property` round trip to the core.
  fn spawn_live_poll(cx: &mut Context<Self>) -> Task<()> {
    cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor().timer(Duration::from_secs(1)).await;
        let alive = this.update(cx, |this, cx| {
          this.refresh_live(cx);
          cx.notify();
        });
        if alive.is_err() {
          break;
        }
      }
    })
  }

  /// Re-reads the track summary and the selected subtitle labels.
  /// `Player::tracks` is a burst of synchronous mpv property reads, so it
  /// runs on structural events only — never per render, and not on the poll.
  fn refresh_structure(&mut self, cx: &Context<Self>) {
    let player = self.player.read(cx);
    let tracks = player.tracks();
    let count = |kind: TrackKind| tracks.iter().filter(|track| track.kind == kind).count();
    self.track_summary = format!(
      "{} {} · {} {} · {} {}",
      count(TrackKind::Audio),
      t("media.audio"),
      count(TrackKind::Video),
      t("media.video"),
      count(TrackKind::Subtitle),
      t("media.subtitles"),
    );
    self.subtitle_label = tracks
      .iter()
      .find(|track| track.kind == TrackKind::Subtitle && track.selected)
      .map(track_label)
      .unwrap_or_else(|| t("media.off"));
    let secondary = player.get_property::<i64>("secondary-sid").ok();
    self.secondary_label = secondary
      .and_then(|id| {
        tracks
          .iter()
          .find(|track| track.kind == TrackKind::Subtitle && track.id == id)
      })
      .map(track_label)
      .unwrap_or_else(|| t("media.off"));
  }

  /// Re-reads the polled mpv properties.
  fn refresh_live(&mut self, cx: &Context<Self>) {
    let player = self.player.read(cx);
    let live = &mut self.live;
    live.hwdec = player.get_property::<String>("hwdec").ok();
    live.video_format = player.get_property::<String>("video-format").ok();
    live.audio_codec = player.get_property::<String>("audio-codec-name").ok();
    live.sub_visibility = player.get_property::<bool>("sub-visibility").ok();
    live.sub_ass = player.get_property::<bool>("sub-ass").ok();
    live.pitch = player.get_property::<bool>("audio-pitch-correction").ok();
    live.replaygain = player.get_property::<String>("replaygain").ok();
    live.deinterlace = player.get_property::<String>("deinterlace").ok();
    live.rotation = player.get_property::<i64>("video-rotate").ok();
    live.demuxer_cache = player
      .get_property::<f64>("demuxer-cache-duration")
      .ok()
      .map(|seconds| format!("{seconds:.1} s"));
    live.cache_speed = player
      .get_property::<f64>("cache-speed")
      .ok()
      .map(|bytes| format!("{:.0} kB/s", bytes / 1000.0));
  }

  /// Rebuilds the player entity. The transport is negotiated when a player
  /// is constructed — it is not a runtime property — so "switching
  /// transport" means dropping this player and its view and creating a new
  /// one with the same source and the current decoder choice. The new
  /// player starts fresh: paused=false, from the beginning.
  fn rebuild_player(&mut self, cx: &mut Context<Self>) {
    let options = options_for(!self.audio_only, self.transport, self.hwdec);
    let player = cx.new(|cx| Player::new(cx, options));
    let view = mount_player_view(&player, self.size, cx);
    // Replace the view first: releasing it detaches its video surface while
    // the old player is still there to own it, then the player goes.
    self.view = view;
    self.player = player;
    self._player_subscription = cx.subscribe(&self.player, Self::on_player_event);
    self.refresh_structure(cx);
    self.refresh_live(cx);
    if let Some(source) = self.source.clone() {
      let result = self.player.update(cx, |player, _| player.load(&source));
      self.note_result(cx, "load", result);
    }
    cx.notify();
  }

  /// Applies the size tier to the whole window: one tier drives the player
  /// view and every control. Only the view is rebuilt — its tier is set at
  /// construction through `Sizable` — the player keeps playing.
  fn set_size_tier(&mut self, size: Size, cx: &mut Context<Self>) {
    self.size = size;
    self.view = mount_player_view(&self.player, size, cx);
    cx.notify();
  }

  /// Loads whatever the source field holds (`Player::load`), remembering it
  /// as the player's source so a transport switch can rebuild around it.
  /// Opens the platform's file chooser and loads what it returns.
  ///
  /// The dialog is a blocking native call, so it runs on the background
  /// executor and only the result comes back to the UI thread; the source
  /// field stays editable for people who prefer pasting a path or URL.
  fn browse_source(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    cx.spawn_in(window, async move |this, cx| {
      let picked = cx
        .background_executor()
        .spawn(async move { rfd::FileDialog::new().pick_file() })
        .await;
      let Some(path) = picked else {
        return;
      };
      let source = path.display().to_string();
      let loaded = this.update_in(cx, |this, window, cx| {
        this
          .source_input
          .update(cx, |input, cx| input.set_value(source.clone(), window, cx));
        this.open_source(cx);
      });
      if loaded.is_err() {
        tracing::debug!("woocraft: player window closed before the dialog returned");
      }
    })
    .detach();
  }

  fn open_source(&mut self, cx: &mut Context<Self>) {
    let source = self.source_input.read(cx).value().trim().to_string();
    if source.is_empty() {
      self.notice = Some(t("media.source_placeholder"));
      cx.notify();
      return;
    }
    self.source = Some(source.clone());
    let result = self.player.update(cx, |player, _| player.load(&source));
    self.note_result(cx, "load", result);
  }

  /// Applies the audio output device from its field; empty or `auto` means
  /// mpv's default (`set_audio_device(None)`).
  fn apply_audio_device(&mut self, cx: &mut Context<Self>) {
    let device = self.audio_device.read(cx).value().trim().to_string();
    let device = if device.is_empty() || device == "auto" {
      None
    } else {
      Some(device)
    };
    self.call_player(cx, "audio-device", move |player| {
      player.set_audio_device(device.as_deref())
    });
  }

  /// Applies the raw `af` chain from its field.
  fn apply_audio_filters(&mut self, cx: &mut Context<Self>) {
    let filters = self.audio_filters.read(cx).value().trim().to_string();
    self.call_player(cx, "af", move |player| player.set_audio_filters(&filters));
  }

  /// Applies the volume ceiling from its field.
  fn apply_volume_max(&mut self, cx: &mut Context<Self>) {
    let text = self.volume_max.read(cx).value().trim().to_string();
    match text.parse::<f64>() {
      Ok(value) => self.call_player(cx, "volume-max", move |player| player.set_volume_max(value)),
      Err(_) => {
        self.notice = Some(format!("{}: {text}", t("media.volume_max")));
        cx.notify();
      }
    }
  }

  /// Applies the aspect-ratio override from its field.
  fn apply_aspect(&mut self, cx: &mut Context<Self>) {
    let aspect = self.aspect.read(cx).value().trim().to_string();
    if aspect.is_empty() {
      self.notice = Some(t("media.aspect_placeholder"));
      cx.notify();
      return;
    }
    self.call_player(cx, "aspect", move |player| player.set_aspect(&aspect));
  }

  /// Loads the external subtitle file from its field (`sub-add … select`).
  fn add_external_subtitle(&mut self, cx: &mut Context<Self>) {
    let path = self.subtitle_path.read(cx).value().trim().to_string();
    if path.is_empty() {
      self.notice = Some(t("media.subtitle_path_placeholder"));
      cx.notify();
      return;
    }
    self.call_player(cx, "sub-add", move |player| {
      player.add_subtitle(&path, "select")
    });
  }

  /// Runs one `&self`-taking player call and surfaces any failure in the
  /// header notice instead of panicking in a click handler.
  fn call_player<E: std::fmt::Display>(
    &mut self, cx: &mut Context<Self>, what: &str, call: impl FnOnce(&Player) -> Result<(), E>,
  ) {
    let result = call(self.player.read(cx));
    self.note_result(cx, what, result);
  }

  /// Reports a finished player call, success or failure.
  fn note_result<E: std::fmt::Display>(
    &mut self, cx: &mut Context<Self>, what: &str, result: Result<(), E>,
  ) {
    if let Err(error) = result {
      self.notice = Some(format!("{what}: {error}"));
    }
    cx.notify();
  }

  // ----- render -----------------------------------------------------------

  /// The header: the source field, the current title, and any error the
  /// player or a control reported — muted, at the same tier.
  fn header(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> Div {
    let title = self
      .player
      .read(cx)
      .state()
      .title
      .clone()
      .or_else(|| self.source.clone())
      .unwrap_or_else(|| t("media.nothing_playing"));
    let error = self
      .notice
      .clone()
      .or_else(|| self.player.read(cx).state().error.clone());

    v_flex()
      .w_full()
      .container_size(size)
      .container_gap_y(size)
      .child(
        h_flex()
          .w_full()
          .items_center()
          .container_gap(size)
          .child(icon_button(
            "open",
            IconName::FolderOpen,
            t("media.open"),
            size,
            cx.listener(|this, _, window, cx| this.browse_source(window, cx)),
          ))
          .child(
            div().min_w_0().flex_1().child(
              Input::new(&self.source_input)
                .with_size(size)
                .cleanable(true),
            ),
          )
          .child(
            div()
              .min_w_0()
              .flex_1()
              .child(IconLabel::new("header-title").with_size(size).label(title)),
          ),
      )
      .when_some(error, |this, error| {
        this.child(
          IconLabel::new("header-error")
            .with_size(size)
            .icon(Icon::new(IconName::Info))
            .label(error)
            .text_color(theme.muted_foreground),
        )
      })
  }

  /// The control panel: one group per area of the `Player` API.
  ///
  /// The chrome (fixed width, border, background) sits on an outer box that
  /// owns the row height; the scrolling content is a separate `Scrollable`
  /// whose own styles — applied *after* `overflow_y_scrollbar()` wraps it — are
  /// what give the scroll area a bounded height. Sizing the inner `v_flex`
  /// instead leaves the wrapper auto-height and the panel cannot scroll.
  fn control_panel(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> impl IntoElement {
    let content = v_flex()
      .id("control-panel-scroll")
      .w_full()
      .container_gap_y(size)
      .child(self.playback_group(size, theme, cx))
      .child(Divider::horizontal())
      .child(self.audio_group(size, theme, cx))
      .child(Divider::horizontal())
      .child(self.subtitle_group(size, theme, cx))
      .child(Divider::horizontal())
      .child(self.video_group(size, theme, cx))
      .child(Divider::horizontal())
      .child(
        v_flex()
          .w_full()
          .container_gap_y(size)
          .child(sub_header(t("media.tracks"), size, theme))
          .child(PlayerTrackMenu::new(&self.player).with_size(size)),
      )
      .child(Divider::horizontal())
      .child(
        v_flex()
          .w_full()
          .container_gap_y(size)
          .child(sub_header(t("media.playlist"), size, theme))
          .child(PlayerPlaylist::new(&self.player).with_size(size)),
      )
      .child(Divider::horizontal())
      .child(self.info_group(size, theme, cx))
      .overflow_y_scrollbar()
      // `Scrollable` renders its own wrapper and applies these to it, which is
      // what bounds the scroll area inside the row.
      .h_full()
      .min_h_0()
      .w_full();

    div()
      .w_80()
      .h_full()
      .min_h_0()
      .flex_none()
      .border_l_1()
      .border_color(theme.border)
      .bg(theme.card)
      .text_color(theme.card_foreground)
      .child(content)
  }

  /// Playback: the transport commands the bar does not carry — seek steps,
  /// frame stepping, speed, the A-B loop, chapters.
  fn playback_group(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> Div {
    let paused = self.player.read(cx).state().paused;
    let (loop_start, loop_end) = self.ab_loop;
    let loop_label = match (loop_start, loop_end) {
      (Some(start), Some(end)) => format!("A {} · B {}", format_time(start), format_time(end)),
      (Some(start), None) => format!("A {}", format_time(start)),
      (None, Some(end)) => format!("B {}", format_time(end)),
      (None, None) => "—".into(),
    };

    v_flex()
      .w_full()
      .container_gap_y(size)
      .child(group_header(
        "header-playback",
        IconName::Play,
        t("media.playback"),
        size,
        theme,
      ))
      .child(
        h_flex()
          .w_full()
          .component_gap(size)
          .child(icon_button(
            "play-toggle",
            if paused {
              IconName::Play
            } else {
              IconName::Pause
            },
            t(if paused { "media.play" } else { "media.pause" }),
            size,
            cx.listener(|this, _, _, cx| {
              // `toggle_pause` needs the player's own context.
              let result = this.player.update(cx, |player, cx| player.toggle_pause(cx));
              this.note_result(cx, "pause", result);
            }),
          ))
          .child(icon_button(
            "stop",
            IconName::Stop,
            t("media.stop"),
            size,
            cx.listener(|this, _, _, cx| this.call_player(cx, "stop", |player| player.stop())),
          ))
          .child(icon_button(
            "seek-back",
            IconName::Rewind,
            t("media.seek_back"),
            size,
            cx.listener(|this, _, _, cx| {
              this.call_player(cx, "seek", |player| player.seek_by(-10.0))
            }),
          ))
          .child(icon_button(
            "seek-forward",
            IconName::FastForward,
            t("media.seek_forward"),
            size,
            cx.listener(|this, _, _, cx| {
              this.call_player(cx, "seek", |player| player.seek_by(10.0))
            }),
          ))
          .child(icon_button(
            "frame-back",
            IconName::ArrowCounterclockwise,
            t("media.frame_back"),
            size,
            cx.listener(|this, _, _, cx| {
              this.call_player(cx, "frame-back-step", |player| player.frame_back_step())
            }),
          ))
          .child(icon_button(
            "frame-step",
            IconName::Frame,
            t("media.frame_step"),
            size,
            cx.listener(|this, _, _, cx| {
              this.call_player(cx, "frame-step", |player| player.frame_step())
            }),
          )),
      )
      .child(
        h_flex()
          .w_full()
          .component_gap(size)
          .child(with_tooltip(
            Button::new("ab-a")
              .with_size(size)
              .label("A")
              .on_click(cx.listener(|this, _, _, cx| {
                let position = this.player.read(cx).state().position;
                this.ab_loop.0 = Some(position);
                this.call_player(cx, "ab-loop-a", move |player| {
                  player.set_property("ab-loop-a", position)
                });
              })),
            t("media.loop_a"),
          ))
          .child(with_tooltip(
            Button::new("ab-b")
              .with_size(size)
              .label("B")
              .on_click(cx.listener(|this, _, _, cx| {
                let position = this.player.read(cx).state().position;
                this.ab_loop.1 = Some(position);
                this.call_player(cx, "ab-loop-b", move |player| {
                  player.set_property("ab-loop-b", position)
                });
              })),
            t("media.loop_b"),
          ))
          .child(icon_button(
            "ab-clear",
            IconName::Dismiss,
            t("media.loop_clear"),
            size,
            cx.listener(|this, _, _, cx| {
              this.ab_loop = (None, None);
              this.call_player(cx, "ab-loop", |player| player.set_ab_loop(None));
            }),
          ))
          .child(
            div()
              .min_w_0()
              .flex_1()
              .truncate()
              .text_color(theme.muted_foreground)
              .child(loop_label),
          ),
      )
      .child(
        h_flex()
          .w_full()
          .component_gap(size)
          .child(self.speed_menu(size, cx))
          .child(self.chapters_menu(size, cx)),
      )
  }

  /// Audio: volume and mute (the view's transport bar carries its own pair;
  /// the panel keeps one so the group is complete), balance, delay,
  /// ReplayGain, pitch correction, the output device, the `af` chain with an
  /// equalizer preset, and the volume ceiling.
  fn audio_group(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> Div {
    let muted = self.player.read(cx).state().muted;
    let volume = self.player.read(cx).state().volume;
    let balance = self.balance.read(cx).value().start();

    v_flex()
      .w_full()
      .container_gap_y(size)
      .child(group_header(
        "header-audio",
        IconName::Speaker2,
        t("media.audio"),
        size,
        theme,
      ))
      .child(
        v_flex()
          .w_full()
          .container_gap_y(size)
          .child(
            h_flex()
              .w_full()
              .list_size(size)
              .items_center()
              .component_gap(size)
              .child(
                div()
                  .min_w_0()
                  .flex_1()
                  .truncate()
                  .text_color(theme.muted_foreground)
                  .child(t("media.volume")),
              )
              .child(div().min_w_0().truncate().child(format!("{volume:.0}")))
              .child(icon_button(
                "panel-mute",
                if muted {
                  IconName::SpeakerOff
                } else {
                  IconName::Speaker2
                },
                t(if muted { "media.unmute" } else { "media.mute" }),
                size,
                cx.listener(|this, _, _, cx| {
                  let muted = this.player.read(cx).state().muted;
                  this.call_player(cx, "mute", move |player| player.set_muted(!muted));
                }),
              )),
          )
          .child(Slider::new("panel-volume", &self.volume).with_size(size)),
      )
      .child(slider_setting(
        "balance-slider",
        t("media.balance"),
        format!("{balance:+.2}"),
        &self.balance,
        size,
        theme,
      ))
      .child(setting_row(
        t("media.audio_delay"),
        h_flex()
          .component_gap(size)
          .child(
            Button::new("audio-delay-minus")
              .with_size(size)
              .label("-0.1")
              .on_click(cx.listener(|this, _, _, cx| {
                this.audio_delay -= 0.1;
                let delay = this.audio_delay;
                this.call_player(cx, "audio-delay", move |player| {
                  player.set_audio_delay(delay)
                });
              })),
          )
          .child(
            Button::new("audio-delay-plus")
              .with_size(size)
              .label("+0.1")
              .on_click(cx.listener(|this, _, _, cx| {
                this.audio_delay += 0.1;
                let delay = this.audio_delay;
                this.call_player(cx, "audio-delay", move |player| {
                  player.set_audio_delay(delay)
                });
              })),
          )
          .child(
            div()
              .min_w_0()
              .truncate()
              .text_color(theme.muted_foreground)
              .child(format!("{:+.1} s", self.audio_delay)),
          ),
        size,
        theme,
      ))
      .child(setting_row(
        t("media.replaygain"),
        self.replaygain_menu(size, cx),
        size,
        theme,
      ))
      .child(setting_row(
        t("media.pitch_correction"),
        Switch::new("switch-pitch")
          .with_size(size)
          .checked(self.live.pitch.unwrap_or(true))
          .on_click(cx.listener(|this, checked, _, cx| {
            let enabled = *checked;
            this.call_player(cx, "audio-pitch-correction", move |player| {
              player.set_pitch_correction(enabled)
            });
          })),
        size,
        theme,
      ))
      .child(text_setting(
        t("media.audio_device"),
        &self.audio_device,
        Button::new("device-apply")
          .with_size(size)
          .label(t("media.apply"))
          .on_click(cx.listener(|this, _, _, cx| this.apply_audio_device(cx))),
        size,
        theme,
      ))
      .child(text_setting(
        t("media.volume_max"),
        &self.volume_max,
        Button::new("volume-max-apply")
          .with_size(size)
          .label(t("media.apply"))
          .on_click(cx.listener(|this, _, _, cx| this.apply_volume_max(cx))),
        size,
        theme,
      ))
      .child(text_setting(
        t("media.filters"),
        &self.audio_filters,
        h_flex()
          .component_gap(size)
          .child(
            Button::new("filters-apply")
              .with_size(size)
              .label(t("media.apply"))
              .on_click(cx.listener(|this, _, _, cx| this.apply_audio_filters(cx))),
          )
          .child(icon_button(
            "filters-clear",
            IconName::Dismiss,
            t("media.clear"),
            size,
            cx.listener(|this, _, _, cx| {
              this.call_player(cx, "af", |player| player.clear_audio_filters())
            }),
          )),
        size,
        theme,
      ))
      .child(slider_setting(
        "eq-bass",
        t("media.eq_bass"),
        format!("{:+.0} dB", self.eq_bass.read(cx).value().start()),
        &self.eq_bass,
        size,
        theme,
      ))
      .child(slider_setting(
        "eq-mid",
        t("media.eq_mid"),
        format!("{:+.0} dB", self.eq_mid.read(cx).value().start()),
        &self.eq_mid,
        size,
        theme,
      ))
      .child(slider_setting(
        "eq-treble",
        t("media.eq_treble"),
        format!("{:+.0} dB", self.eq_treble.read(cx).value().start()),
        &self.eq_treble,
        size,
        theme,
      ))
      .child(setting_row(
        t("media.equalizer"),
        Button::new("eq-flat")
          .with_size(size)
          .label(t("media.clear"))
          .on_click(cx.listener(|this, _, _, cx| {
            for band in [
              this.eq_bass.clone(),
              this.eq_mid.clone(),
              this.eq_treble.clone(),
            ] {
              band.update(cx, |band, cx| band.set_value(0.0, cx));
            }
          })),
        size,
        theme,
      ))
  }

  /// Subtitles: track pickers, delay, scale, position, visibility, ASS
  /// styling and external files.
  fn subtitle_group(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> Div {
    let scale = self.sub_scale.read(cx).value().start();
    let position = self.sub_pos.read(cx).value().start();

    v_flex()
      .w_full()
      .container_gap_y(size)
      .child(group_header(
        "header-subtitles",
        IconName::ClosedCaption,
        t("media.subtitles"),
        size,
        theme,
      ))
      .child(self.subtitle_track_menu(size, cx))
      .child(self.secondary_subtitle_menu(size, cx))
      .child(setting_row(
        t("media.subtitle_delay"),
        h_flex()
          .component_gap(size)
          .child(
            Button::new("sub-delay-minus")
              .with_size(size)
              .label("-0.5")
              .on_click(cx.listener(|this, _, _, cx| {
                this.subtitle_delay -= 0.5;
                let delay = this.subtitle_delay;
                this.call_player(cx, "sub-delay", move |player| {
                  player.set_subtitle_delay(delay)
                });
              })),
          )
          .child(
            Button::new("sub-delay-plus")
              .with_size(size)
              .label("+0.5")
              .on_click(cx.listener(|this, _, _, cx| {
                this.subtitle_delay += 0.5;
                let delay = this.subtitle_delay;
                this.call_player(cx, "sub-delay", move |player| {
                  player.set_subtitle_delay(delay)
                });
              })),
          )
          .child(
            div()
              .min_w_0()
              .truncate()
              .text_color(theme.muted_foreground)
              .child(format!("{:+.1} s", self.subtitle_delay)),
          ),
        size,
        theme,
      ))
      .child(slider_setting(
        "sub-scale-slider",
        t("media.subtitle_scale"),
        format!("{scale:.2}×"),
        &self.sub_scale,
        size,
        theme,
      ))
      .child(slider_setting(
        "sub-pos-slider",
        t("media.subtitle_position"),
        format!("{position:.0}"),
        &self.sub_pos,
        size,
        theme,
      ))
      .child(setting_row(
        t("media.subtitle_visibility"),
        Switch::new("switch-sub-visible")
          .with_size(size)
          .checked(self.live.sub_visibility.unwrap_or(true))
          .on_click(cx.listener(|this, checked, _, cx| {
            let visible = *checked;
            this.call_player(cx, "sub-visibility", move |player| {
              player.set_subtitle_visibility(visible)
            });
          })),
        size,
        theme,
      ))
      .child(setting_row(
        t("media.subtitle_ass"),
        Switch::new("switch-sub-ass")
          .with_size(size)
          .checked(self.live.sub_ass.unwrap_or(true))
          .on_click(cx.listener(|this, checked, _, cx| {
            let ass = *checked;
            this.call_player(cx, "sub-ass", move |player| player.set_subtitle_ass(ass));
          })),
        size,
        theme,
      ))
      .child(text_setting(
        t("media.subtitle_add"),
        &self.subtitle_path,
        icon_button(
          "subtitle-add",
          IconName::Add,
          t("media.subtitle_add"),
          size,
          cx.listener(|this, _, _, cx| this.add_external_subtitle(cx)),
        ),
        size,
        theme,
      ))
  }

  /// Video: decoder, transport, size tier, deinterlace, rotation, aspect
  /// ratio — and the render resolution the player actually runs at.
  fn video_group(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> Div {
    let transport = self.player.read(cx).video_transport();
    let resolution = self
      .player
      .read(cx)
      .state()
      .video_size
      .map(|(width, height)| format!("{width}×{height}"))
      .unwrap_or_else(|| "—".into());

    v_flex()
      .w_full()
      .container_gap_y(size)
      .child(group_header(
        "header-video",
        IconName::Video,
        t("media.video"),
        size,
        theme,
      ))
      .child(self.decoder_menu(size, cx))
      .child(self.transport_menu(size, cx))
      .child(self.size_menu(size, cx))
      .child(self.deinterlace_menu(size, cx))
      .child(self.rotation_menu(size, cx))
      .child(text_setting(
        t("media.aspect"),
        &self.aspect,
        Button::new("aspect-apply")
          .with_size(size)
          .label(t("media.apply"))
          .on_click(cx.listener(|this, _, _, cx| this.apply_aspect(cx))),
        size,
        theme,
      ))
      .child(info_row(
        t("media.resolution"),
        format!("{} · {}", transport_label(transport), resolution),
        size,
        theme,
      ))
  }

  /// Info: the `PlayerState` fields, cached summaries, the polled mpv
  /// properties and the entry's metadata.
  fn info_group(&self, size: Size, theme: &Theme, cx: &Context<Self>) -> Div {
    let player = self.player.read(cx);
    let state = player.state();
    let cache = match (&self.live.demuxer_cache, &self.live.cache_speed) {
      (Some(buffered), Some(speed)) => format!("{buffered} · {speed}"),
      (Some(buffered), None) => buffered.clone(),
      (None, Some(speed)) => speed.clone(),
      (None, None) => "—".into(),
    };

    let mut group = v_flex()
      .w_full()
      .container_gap_y(size)
      .child(group_header(
        "header-info",
        IconName::Info,
        t("media.info"),
        size,
        theme,
      ))
      .child(info_row(
        t("media.state"),
        format!(
          "{} / {}",
          format_time(state.position),
          format_time(state.duration)
        ),
        size,
        theme,
      ))
      .child(info_row(
        "eof · idle".into(),
        format!("{} · {}", flag_text(state.eof), flag_text(state.idle)),
        size,
        theme,
      ))
      .child(info_row(
        t("media.tracks"),
        self.track_summary.clone(),
        size,
        theme,
      ))
      .child(info_row(
        t("media.decoder"),
        or_dash(&self.live.hwdec),
        size,
        theme,
      ))
      .child(info_row(
        t("media.video_format"),
        or_dash(&self.live.video_format),
        size,
        theme,
      ))
      .child(info_row(
        t("media.audio_codec"),
        or_dash(&self.live.audio_codec),
        size,
        theme,
      ))
      .child(info_row(t("media.cache"), cache, size, theme))
      .child(sub_header(t("media.metadata"), size, theme));
    for (key, value) in player.metadata() {
      group = group.child(info_row(key.clone(), value.clone(), size, theme));
    }
    group
  }

  // ----- pickers ----------------------------------------------------------

  /// The speed picker: mpv's `speed` property, live.
  fn speed_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let speed = self.player.read(cx).state().speed;
    Button::new("menu-speed")
      .with_size(size)
      .label(format!(
        "{}: {}×",
        t("media.speed"),
        (speed * 100.0).round() / 100.0
      ))
      .dropdown_menu(move |mut menu, _, cx| {
        let current = app.read(cx).player.read(cx).state().speed;
        for speed in SPEEDS {
          let app = app.clone();
          menu = menu.item(
            PopupMenuItem::new(format!("{speed}×"))
              .checked((current - speed).abs() < f64::EPSILON)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "speed", move |player| player.set_speed(speed));
                });
              }),
          );
        }
        menu
      })
  }

  /// The chapter list: `Player::chapters`, read when the menu opens, each
  /// entry seeking to its start.
  fn chapters_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let player = self.player.clone();
    Button::new("menu-chapters")
      .with_size(size)
      .label(t("media.chapters"))
      .dropdown_menu(move |mut menu, _, cx| {
        let chapters = player.read(cx).chapters();
        let position = player.read(cx).state().position;
        if chapters.is_empty() {
          return menu.label(t("media.no_chapters"));
        }
        // The chapter containing the playback position, for the check mark.
        let current = chapters
          .iter()
          .rposition(|chapter| chapter.time <= position);
        for (index, chapter) in chapters.iter().enumerate() {
          let app = app.clone();
          let time = chapter.time;
          let title = chapter
            .title
            .clone()
            .unwrap_or_else(|| format!("{} {}", t("media.chapter"), index + 1));
          menu = menu.item(
            PopupMenuItem::new(format!("{} · {}", title, format_time(time)))
              .checked(current == Some(index))
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "seek", move |player| player.seek_to(time));
                });
              }),
          );
        }
        menu
      })
  }

  /// The hardware-decoding picker. Any entry applies live through the
  /// `hwdec` property; the checked one is what mpv reports. This is the
  /// same write `Player::set_hardware_decoding` performs — the example
  /// cannot call that method by name because its `HwDec` argument type is
  /// not re-exported from the `media` module.
  fn decoder_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let label = format!(
      "{}: {}",
      t("media.decoder"),
      self.live.hwdec.clone().unwrap_or_else(|| self.hwdec.into()),
    );
    Button::new("menu-decoder")
      .with_size(size)
      .label(label)
      .dropdown_menu(move |mut menu, _, cx| {
        let active = app.read(cx).live.hwdec.clone();
        for mode in DECODERS {
          let app = app.clone();
          let checked = active.as_deref() == Some(mode);
          menu = menu.item(
            PopupMenuItem::new(mode)
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.hwdec = mode;
                  this.call_player(cx, "hwdec", move |player| {
                    player.set_property("hwdec", mode)
                  });
                  this.refresh_live(cx);
                });
              }),
          );
        }
        menu
      })
  }

  /// The transport picker. Switching rebuilds the player — see
  /// [`Self::rebuild_player`] — which is why it is disabled for audio-only
  /// players.
  fn transport_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    Button::new("menu-transport")
      .with_size(size)
      .disabled(self.audio_only)
      .label(format!(
        "{}: {}",
        t("media.transport"),
        transport_label(self.transport)
      ))
      .dropdown_menu(move |mut menu, _, cx| {
        let requested = app.read(cx).transport;
        for (transport, label) in [
          (VideoTransport::Composition, t("media.transport_gpu")),
          (VideoTransport::Software, t("media.transport_software")),
        ] {
          let app = app.clone();
          let checked = requested == transport;
          menu = menu.item(
            PopupMenuItem::new(label)
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.transport = transport;
                  this.rebuild_player(cx);
                });
              }),
          );
        }
        menu
      })
  }

  /// The size-tier picker: one tier for the `PlayerView` and every control.
  fn size_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    Button::new("menu-size")
      .with_size(size)
      .label(format!("{}: {}", t("media.size"), self.size.as_str()))
      .dropdown_menu(move |mut menu, _, cx| {
        let current = app.read(cx).size;
        for (tier, label) in [
          (Size::Small, t("media.size_small")),
          (Size::Medium, t("media.size_medium")),
          (Size::Large, t("media.size_large")),
        ] {
          let app = app.clone();
          let checked = current == tier;
          menu = menu.item(
            PopupMenuItem::new(label)
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| this.set_size_tier(tier, cx));
              }),
          );
        }
        menu
      })
  }

  /// The deinterlace picker (`deinterlace`).
  fn deinterlace_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let current = self.live.deinterlace.clone().unwrap_or_else(|| "no".into());
    Button::new("menu-deinterlace")
      .with_size(size)
      .label(format!("{}: {}", t("media.deinterlace"), current))
      .dropdown_menu(move |mut menu, _, cx| {
        let active = app.read(cx).live.deinterlace.clone();
        for mode in ["no", "auto", "yes"] {
          let app = app.clone();
          let checked = active.as_deref() == Some(mode);
          menu = menu.item(
            PopupMenuItem::new(mode)
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "deinterlace", move |player| {
                    player.set_deinterlace(mode)
                  });
                  this.refresh_live(cx);
                });
              }),
          );
        }
        menu
      })
  }

  /// The rotation picker (`video-rotate`).
  fn rotation_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let current = self.live.rotation.unwrap_or(0);
    Button::new("menu-rotation")
      .with_size(size)
      .label(format!("{}: {current}°", t("media.rotation")))
      .dropdown_menu(move |mut menu, _, cx| {
        let active = app.read(cx).live.rotation;
        for degrees in [0, 90, 180, 270] {
          let app = app.clone();
          let checked = active == Some(degrees);
          menu = menu.item(
            PopupMenuItem::new(format!("{degrees}°"))
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "video-rotate", move |player| {
                    player.set_rotation(degrees)
                  });
                  this.refresh_live(cx);
                });
              }),
          );
        }
        menu
      })
  }

  /// The ReplayGain picker (`replaygain`).
  fn replaygain_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let current = self.live.replaygain.clone().unwrap_or_else(|| "no".into());
    Button::new("menu-replaygain")
      .with_size(size)
      .label(format!("{}: {}", t("media.replaygain"), current))
      .dropdown_menu(move |mut menu, _, cx| {
        let active = app.read(cx).live.replaygain.clone();
        for mode in ["no", "track", "album"] {
          let app = app.clone();
          let checked = active.as_deref() == Some(mode);
          menu = menu.item(
            PopupMenuItem::new(mode)
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "replaygain", move |player| player.set_replaygain(mode));
                  this.refresh_live(cx);
                });
              }),
          );
        }
        menu
      })
  }

  /// The subtitle track picker: the entry's subtitle tracks plus Off, read
  /// from `Player::tracks` when the menu opens.
  fn subtitle_track_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let player = self.player.clone();
    Button::new("menu-sub-track")
      .with_size(size)
      .label(format!("{}: {}", t("media.subtitles"), self.subtitle_label))
      .dropdown_menu(move |mut menu, _, cx| {
        let (tracks, selected) = subtitle_tracks(&player, cx);
        menu = menu.item(
          PopupMenuItem::new(t("media.off"))
            .checked(selected.is_none())
            .on_click({
              let app = app.clone();
              move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "sid", |player| player.set_subtitle_track(None))
                });
              }
            }),
        );
        for track in tracks {
          let app = app.clone();
          let id = track.id;
          let checked = selected == Some(id);
          menu = menu.item(
            PopupMenuItem::new(track_label(&track))
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "sid", move |player| player.set_subtitle_track(Some(id)));
                });
              }),
          );
        }
        menu
      })
  }

  /// The secondary subtitle picker (`secondary-sid`).
  fn secondary_subtitle_menu(&self, size: Size, cx: &Context<Self>) -> impl IntoElement {
    let app = cx.entity();
    let player = self.player.clone();
    Button::new("menu-sub-secondary")
      .with_size(size)
      .label(format!(
        "{}: {}",
        t("media.subtitle_secondary"),
        self.secondary_label
      ))
      .dropdown_menu(move |mut menu, _, cx| {
        let (tracks, _) = subtitle_tracks(&player, cx);
        let selected = player.read(cx).get_property::<i64>("secondary-sid").ok();
        menu = menu.item(
          PopupMenuItem::new(t("media.off"))
            .checked(selected.is_none())
            .on_click({
              let app = app.clone();
              move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "secondary-sid", |player| {
                    player.set_secondary_subtitle(None)
                  })
                });
              }
            }),
        );
        for track in tracks {
          let app = app.clone();
          let id = track.id;
          let checked = selected == Some(id);
          menu = menu.item(
            PopupMenuItem::new(track_label(&track))
              .checked(checked)
              .on_click(move |_, _, cx| {
                app.update(cx, |this, cx| {
                  this.call_player(cx, "secondary-sid", move |player| {
                    player.set_secondary_subtitle(Some(id))
                  });
                });
              }),
          );
        }
        menu
      })
  }
}

impl Render for PlayerApp {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let size = self.size;

    v_flex()
      .size_full()
      .bg(theme.background)
      .child(
        TitleBar::new()
          .title(t("media.title"))
          .theme_button(true)
          .language_button(true),
      )
      .child(self.header(size, &theme, cx))
      .child(
        h_flex()
          .flex_1()
          .min_h_0()
          .w_full()
          .child(
            div()
              .flex_1()
              .min_w_0()
              .min_h_0()
              .h_full()
              .child(self.view.clone()),
          )
          .child(self.control_panel(size, &theme, cx)),
      )
  }
}

// ----- small element builders ---------------------------------------------

/// A panel group's header: an icon and the group name at the panel's tier,
/// muted like every other secondary label in the panel.
fn group_header(
  id: &'static str, icon: IconName, label: String, size: Size, theme: &Theme,
) -> IconLabel {
  IconLabel::new(id)
    .with_size(size)
    .icon(Icon::new(icon))
    .label(label)
    .text_color(theme.muted_foreground)
}

/// A muted sub-header inside a group (the metadata block).
fn sub_header(label: String, size: Size, theme: &Theme) -> Div {
  h_flex().w_full().list_size(size).items_center().child(
    div()
      .min_w_0()
      .flex_1()
      .truncate()
      .text_color(theme.muted_foreground)
      .child(label),
  )
}

/// One setting row: the setting's name on the left (muted, at the tier's
/// row metrics) and its control on the right.
fn setting_row(name: String, control: impl IntoElement, size: Size, theme: &Theme) -> Div {
  h_flex()
    .w_full()
    .list_size(size)
    .items_center()
    .component_gap(size)
    .child(
      div()
        .min_w_0()
        .flex_1()
        .truncate()
        .text_color(theme.muted_foreground)
        .child(name),
    )
    .child(control)
}

/// One read-only info row: the property's name muted, its value normal.
fn info_row(name: String, value: String, size: Size, theme: &Theme) -> Div {
  h_flex()
    .w_full()
    .list_size(size)
    .items_center()
    .component_gap(size)
    .child(
      div()
        .min_w_0()
        .flex_1()
        .truncate()
        .text_color(theme.muted_foreground)
        .child(name),
    )
    .child(div().min_w_0().truncate().child(value))
}

/// One slider setting: name and live value in a row, the slider full-width
/// under it.
fn slider_setting(
  id: &'static str, name: String, value: String, state: &Entity<SliderState>, size: Size,
  theme: &Theme,
) -> Div {
  v_flex()
    .w_full()
    .container_gap_y(size)
    .child(
      h_flex()
        .w_full()
        .list_size(size)
        .items_center()
        .component_gap(size)
        .child(
          div()
            .min_w_0()
            .flex_1()
            .truncate()
            .text_color(theme.muted_foreground)
            .child(name),
        )
        .child(div().min_w_0().truncate().child(value)),
    )
    // Bounded width: a full-bleed track reads as a stray coloured band rather
    // than as a control, and leaves no room for the row's own padding.
    .child(
      div()
        .w(size.em(SLIDER_WIDTH_EM))
        .child(Slider::new(id, state).with_size(size)),
    )
}

/// One text setting that needs the full row width: its name above, an input
/// and its action below.
fn text_setting(
  name: String, state: &Entity<InputState>, action: impl IntoElement, size: Size, theme: &Theme,
) -> Div {
  v_flex()
    .w_full()
    .container_gap_y(size)
    .child(sub_header(name, size, theme))
    .child(
      h_flex()
        .w_full()
        .component_gap(size)
        .child(
          div()
            .min_w_0()
            .flex_1()
            .child(Input::new(state).with_size(size).cleanable(true)),
        )
        .child(action),
    )
}

/// An icon-only button with a translated tooltip — an icon wherever the
/// meaning is unambiguous.
fn icon_button(
  id: &'static str, icon: IconName, tooltip: String, size: Size,
  on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
  with_tooltip(
    Button::new(id).with_size(size).icon(Icon::new(icon)),
    tooltip,
  )
  .on_click(on_click)
}

/// Attaches a plain-text tooltip to a button.
fn with_tooltip(button: Button, tooltip: String) -> Button {
  button.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
}

// ----- small data helpers -------------------------------------------------

/// Translates one `media.*` string of this example.
fn t(key: &str) -> String {
  translate_woocraft(key)
}

/// A transport's display name.
fn transport_label(transport: VideoTransport) -> String {
  match transport {
    VideoTransport::Composition => t("media.transport_gpu"),
    VideoTransport::Software => t("media.transport_software"),
  }
}

/// The label a track entry shows: its title, else its language, else its
/// position among tracks (`PlayerTrackMenu` uses the same rule).
fn track_label(track: &Track) -> String {
  track
    .title
    .clone()
    .or_else(|| track.lang.clone())
    .unwrap_or_else(|| format!("{} {}", t("media.track"), track.id))
}

/// Formats a position as `m:ss` / `h:mm:ss`; the library's own formatter is
/// private to the `media` module, and the info rows need one.
fn format_time(seconds: f64) -> String {
  let total = seconds.max(0.0) as u64;
  let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
  if hours > 0 {
    format!("{hours}:{minutes:02}:{seconds:02}")
  } else {
    format!("{minutes}:{seconds:02}")
  }
}

/// Localized yes/no for the state flags.
fn flag_text(value: bool) -> String {
  t(if value { "media.yes" } else { "media.no" })
}

/// A live property's value, or an em dash while mpv reports none.
fn or_dash(value: &Option<String>) -> String {
  value.clone().unwrap_or_else(|| "—".into())
}

/// The entry's subtitle tracks and the selected one, read from mpv when a
/// menu opens.
fn subtitle_tracks(player: &Entity<Player>, cx: &App) -> (Vec<Track>, Option<i64>) {
  let tracks = player
    .read(cx)
    .tracks()
    .into_iter()
    .filter(|track| track.kind == TrackKind::Subtitle)
    .collect::<Vec<_>>();
  let selected = tracks
    .iter()
    .find(|track| track.selected)
    .map(|track| track.id);
  (tracks, selected)
}

// ----- player plumbing ----------------------------------------------------

/// The options a new player is created with.
fn options_for(video: bool, transport: VideoTransport, decoder: &str) -> PlayerOptions {
  if video {
    PlayerOptions::default()
      .video_transport(transport)
      .option("hwdec", decoder)
  } else {
    PlayerOptions::audio_only().option("hwdec", decoder)
  }
}

/// Builds the player's view with its pluggable chrome: the default stage and
/// transport stay in place, and the standalone track menu and playlist are
/// mounted through the `controls` slots — the view never grows this UI
/// itself.
fn mount_player_view(
  player: &Entity<Player>, size: Size, cx: &mut Context<PlayerApp>,
) -> Entity<PlayerView> {
  let slot_player = player.clone();
  cx.new(|cx| {
    PlayerView::new(player, cx)
      .with_size(size)
      // The slot stays available for host chrome; the standard track menu and
      // playlist live in the control panel instead, where they scroll with it.
      .controls(move |_, _, _| {
        // `controls` takes an `Fn`, so the entity handles are cloned per call.
        let back = slot_player.clone();
        let forward = slot_player.clone();
        h_flex()
          .w_full()
          .container_gap(size)
          .child(icon_button(
            "slot-back",
            IconName::ArrowCircleLeft,
            t("media.seek_back"),
            size,
            move |_, _, cx| {
              let _ = back.update(cx, |player, _| player.seek_by(-10.0));
            },
          ))
          .child(icon_button(
            "slot-forward",
            IconName::ArrowCircleRight,
            t("media.seek_forward"),
            size,
            move |_, _, cx| {
              let _ = forward.update(cx, |player, _| player.seek_by(10.0));
            },
          ))
          .into_any_element()
      })
  })
}

// ----- CLI ----------------------------------------------------------------

fn usage() -> ! {
  eprintln!(
    "usage: player [--transport=gpu|software] \
     [--decoder=no|auto|auto-safe|auto-copy|vaapi|vulkan|nvdec] \
     [--size=sm|md|lg] [--audio] [file-or-url]"
  );
  std::process::exit(2);
}

fn parse_decoder(value: &str) -> &'static str {
  match value {
    // Every accepted value is one of the static names above, so the parsed
    // mode can stay a `&'static str` without leaking anything.
    "no" => "no",
    "auto" => "auto",
    "auto-safe" => "auto-safe",
    "auto-copy" => "auto-copy",
    "vaapi" => "vaapi",
    "vulkan" => "vulkan",
    "nvdec" => "nvdec",
    _ => usage(),
  }
}

fn parse_size(value: &str) -> Size {
  match value {
    "sm" | "small" => Size::Small,
    "md" | "medium" => Size::Medium,
    "lg" | "large" => Size::Large,
    _ => usage(),
  }
}

fn main() {
  // Make the library's `tracing` lines — transport negotiation and fallback
  // among them — visible on stderr through RUST_LOG.
  tracing_subscriber::fmt()
    .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
    .with_target(false)
    .init();

  let mut launch = Launch {
    source: None,
    video: true,
    transport: VideoTransport::default(),
    decoder: "no",
    size: Size::default(),
  };
  for arg in std::env::args().skip(1) {
    if let Some(value) = arg.strip_prefix("--transport=") {
      launch.transport = match value {
        "gpu" => VideoTransport::Composition,
        "software" => VideoTransport::Software,
        _ => usage(),
      };
    } else if let Some(value) = arg.strip_prefix("--decoder=") {
      launch.decoder = parse_decoder(value);
    } else if let Some(value) = arg.strip_prefix("--size=") {
      launch.size = parse_size(value);
    } else if arg == "--audio" {
      launch.video = false;
    } else if arg.starts_with("--") {
      usage();
    } else {
      launch.source = Some(arg);
    }
  }
  eprintln!(
    "player: requested video transport {:?}; negotiation and fallback log at RUST_LOG=woocraft=warn",
    launch.transport
  );

  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(move |cx: &mut App| {
      init(cx);
      cx.activate(true);
      Theme::set_mode(ThemeMode::Dark, cx);
      // The control panel is always taller than the window, so keep its
      // scrollbar visible instead of fading it out: without it the clipped
      // bottom edge reads as broken layout rather than as scrollable content.
      Theme::global_mut(cx).scrollbar_show = ScrollbarShow::Always;

      let bounds = Bounds::centered(None, gpui::Size::new(px(960.0), px(640.0)), cx);
      let launch = launch.clone();
      cx.open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..TitleBar::window_options()
        },
        move |_window, cx| {
          let launch = launch.clone();
          cx.new(|cx| PlayerApp::new(launch, cx))
        },
      )
      .expect("open player example window failed");
    });
}
