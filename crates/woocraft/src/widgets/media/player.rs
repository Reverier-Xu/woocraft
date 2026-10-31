//! The mpv-backed player: one core, one entity, video as a mode of it.

use std::{
  collections::BTreeMap,
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
  },
  thread::{self, JoinHandle},
  time::Duration,
};

use anyhow::{Result, anyhow};
use gpui::{Context, EventEmitter, RenderImage, Task};
use libmpv2::{Format, Mpv};

use super::video::{VideoFrame, VideoOutput};

/// What a player reports to its views.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerEvent {
  /// A file was loaded and playback may start.
  FileLoaded,
  /// Playback of the current entry ended (or failed).
  EndFile {
    /// The reason mpv reported, for example `eof` or `error`.
    reason: String,
  },
  /// An observed property changed.
  PropertyChanged {
    /// The mpv property name, for example `time-pos`.
    name: String,
    /// Its new value.
    value: PropertyValue,
  },
  /// The video track was (re)configured.
  VideoReconfig,
  /// The audio track was (re)configured.
  AudioReconfig,
  /// A seek finished.
  Seek,
  /// The player returned to a playable state after a stall.
  PlaybackRestart,
  /// mpv logged an error.
  Error(String),
}

/// A scalar mpv property value.
#[derive(Clone, Debug, PartialEq)]
pub enum PropertyValue {
  /// A string property.
  String(String),
  /// A boolean property.
  Flag(bool),
  /// A 64-bit integer property.
  Int64(i64),
  /// A floating point property.
  Double(f64),
}

/// Options a player is created with. They map onto mpv options one to one.
#[derive(Clone, Debug)]
pub struct PlayerOptions {
  /// Produce video frames. When false the core never opens a video output and
  /// plays the audio track only, which is what a music player wants.
  pub video: bool,
  /// Resolution the software video output renders at, in pixels.
  pub video_size: (u32, u32),
  /// Initial volume, 0..=100.
  pub volume: f64,
  /// Whether to start muted.
  pub muted: bool,
  /// Initial playback speed.
  pub speed: f64,
  /// Hardware decoding mode. The default is [`HwDec::No`]: the software video
  /// output downloads every frame anyway, and hardware paths are where decoder
  /// drivers are least uniform, so a design-system component opts in.
  pub hwdec: HwDec,
  /// Any further mpv option, applied before the core initializes. This is how
  /// hardware decoding, `--ytdl`, network caches, audio filters and the rest of
  /// mpv's configuration stay reachable without a wrapper API per feature.
  pub options: Vec<(String, String)>,
}

impl Default for PlayerOptions {
  fn default() -> Self {
    Self {
      video: true,
      video_size: (1280, 720),
      volume: 100.0,
      muted: false,
      speed: 1.0,
      hwdec: HwDec::No,
      options: Vec::new(),
    }
  }
}

impl PlayerOptions {
  /// Audio-only player: no video output, no video frames.
  pub fn audio_only() -> Self {
    Self {
      video: false,
      ..Default::default()
    }
  }

  /// Sets one mpv option, for example `"hwdec", "auto"`.
  pub fn option(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
    self.options.push((name.into(), value.into()));
    self
  }

  /// Enables or disables video output.
  pub fn video(mut self, video: bool) -> Self {
    self.video = video;
    self
  }
}

/// Everything the views render from.
#[derive(Clone, Debug, Default)]
pub struct PlayerState {
  /// Whether playback is paused.
  pub paused: bool,
  /// Current position, in seconds.
  pub position: f64,
  /// Duration of the current entry, in seconds; zero while unknown.
  pub duration: f64,
  /// Volume, 0..=100.
  pub volume: f64,
  /// Whether audio is muted.
  pub muted: bool,
  /// Playback speed multiplier.
  pub speed: f64,
  /// `media-title`, which mpv derives from tags when the file has them.
  pub title: Option<String>,
  /// `Artist` tag of the current entry.
  pub artist: Option<String>,
  /// `Album` tag of the current entry.
  pub album: Option<String>,
  /// Source video size in pixels, once a video track is configured.
  pub video_size: Option<(u32, u32)>,
  /// Whether the current entry has a video track.
  pub has_video: bool,
  /// Whether playback reached the end of the current entry.
  pub eof: bool,
  /// Whether the core is idle (no file loaded).
  pub idle: bool,
  /// The last error the player reported.
  pub error: Option<String>,
}

/// A track of the current entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Track {
  /// mpv track id.
  pub id: i64,
  /// Audio, video or subtitle.
  pub kind: TrackKind,
  /// Track title, when the container carries one.
  pub title: Option<String>,
  /// Track language code.
  pub lang: Option<String>,
  /// Codec name.
  pub codec: Option<String>,
  /// Whether the track is selected.
  pub selected: bool,
  /// Whether the track comes from an external file.
  pub external: bool,
}

/// The kind of a [`Track`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackKind {
  /// An audio track.
  Audio,
  /// A video track.
  Video,
  /// A subtitle track.
  Subtitle,
}

impl TrackKind {
  fn parse(value: &str) -> Option<Self> {
    match value {
      "audio" => Some(Self::Audio),
      "video" => Some(Self::Video),
      "sub" => Some(Self::Subtitle),
      _ => None,
    }
  }
}

/// One entry of the playlist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaylistEntry {
  /// Position in the playlist.
  pub index: usize,
  /// The file or URL the entry points at.
  pub filename: String,
  /// Its title, when mpv can resolve one.
  pub title: Option<String>,
  /// Whether this is the entry being played.
  pub current: bool,
}

/// One chapter of the current entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Chapter {
  /// Chapter title.
  pub title: Option<String>,
  /// Start time in seconds.
  pub time: f64,
}

/// One mpv core plus the state its views render.
///
/// The entity owns a control handle and a dedicated event thread; all methods
/// take `&self` and can be called from anywhere on the UI thread. Dropping the
/// entity terminates the core.
pub struct Player {
  mpv: Arc<Mpv>,
  options: PlayerOptions,
  state: PlayerState,
  metadata: BTreeMap<String, String>,
  video: Option<VideoOutput>,
  video_image: Option<Arc<RenderImage>>,
  shutdown: Arc<AtomicBool>,
  event_thread: Option<JoinHandle<()>>,
  event_task: Option<Task<()>>,
  frame_task: Option<Task<()>>,
}

impl EventEmitter<PlayerEvent> for Player {}

impl Player {
  /// Creates a player. mpv is initialized on the calling thread; a failure is
  /// reported through [`PlayerState::error`] instead of failing construction,
  /// so views can be built unconditionally.
  pub fn new(cx: &mut Context<Self>, options: PlayerOptions) -> Self {
    let mut player = Self {
      // Replaced below; mpv handles are not nullable in the public type.
      mpv: Arc::new(Mpv::new().expect("mpv_create")),
      options: options.clone(),
      state: PlayerState {
        volume: options.volume,
        muted: options.muted,
        speed: options.speed,
        ..Default::default()
      },
      metadata: BTreeMap::new(),
      video: None,
      video_image: None,
      shutdown: Arc::new(AtomicBool::new(false)),
      event_thread: None,
      event_task: None,
      frame_task: None,
    };

    match create_mpv(&options) {
      Ok(mpv) => {
        player.mpv = Arc::new(mpv);
        player.observe_default_properties();
        player.start_event_pump(cx);
        if options.video {
          player.start_video(cx);
        }
      }
      Err(error) => {
        player.state.error = Some(error.to_string());
      }
    }

    player
  }

  /// The current state, for views.
  pub fn state(&self) -> &PlayerState {
    &self.state
  }

  /// The raw mpv handle, for anything this wrapper does not cover.
  pub fn mpv(&self) -> &Mpv {
    &self.mpv
  }

  /// The latest video frame as a gpui image, or `None` when the player is
  /// audio-only or nothing has been rendered yet.
  pub fn video_image(&self) -> Option<Arc<RenderImage>> {
    self.video_image.clone()
  }

  /// Whether video output is enabled for this player.
  pub fn video_enabled(&self) -> bool {
    self.options.video
  }

  /// Loads `uri` (a path or URL) and starts playing it.
  pub fn load(&self, uri: &str) -> Result<()> {
    self.command("loadfile", &[uri, "replace"])
  }

  /// Appends `uri` to the playlist.
  pub fn enqueue(&self, uri: &str) -> Result<()> {
    self.command("loadfile", &[uri, "append"])
  }

  /// Starts or resumes playback.
  pub fn play(&mut self, cx: &mut Context<Self>) -> Result<()> {
    self.set_property("pause", false)?;
    self.sync_state(cx);
    Ok(())
  }

  /// Pauses playback.
  pub fn pause(&mut self, cx: &mut Context<Self>) -> Result<()> {
    self.set_property("pause", true)?;
    self.sync_state(cx);
    Ok(())
  }

  /// Toggles between playing and paused.
  pub fn toggle_pause(&mut self, cx: &mut Context<Self>) -> Result<()> {
    if self.state.paused {
      self.play(cx)
    } else {
      self.pause(cx)
    }
  }

  /// Stops playback and unloads the current entry.
  pub fn stop(&self) -> Result<()> {
    self.command("stop", &[])
  }

  /// Seeks to an absolute position, in seconds.
  pub fn seek_to(&self, position: f64) -> Result<()> {
    self.command("seek", &[&format!("{position:.3}"), "absolute+exact"])
  }

  /// Seeks by a relative offset, in seconds.
  pub fn seek_by(&self, offset: f64) -> Result<()> {
    self.command("seek", &[&format!("{offset:.3}"), "relative+exact"])
  }

  /// Sets the volume, 0..=100.
  pub fn set_volume(&self, volume: f64) -> Result<()> {
    self.set_property("volume", volume.clamp(0.0, 100.0))
  }

  /// Sets the playback speed multiplier.
  pub fn set_speed(&self, speed: f64) -> Result<()> {
    self.set_property("speed", speed.clamp(0.01, 100.0))
  }

  /// Mutes or unmutes.
  pub fn set_muted(&self, muted: bool) -> Result<()> {
    self.set_property("mute", muted)
  }

  /// Runs any mpv command by name, with its arguments.
  pub fn command(&self, name: &str, args: &[&str]) -> Result<()> {
    self
      .mpv
      .command(name, args)
      .map_err(|error| anyhow!("mpv command `{name}` failed: {error}"))
  }

  /// Runs a command from a single string, exactly as mpv's input configuration
  /// would (`"seek 10 absolute+exact"`). This is the escape hatch for commands
  /// whose arguments do not fit the `&[&str]` form.
  pub fn command_string(&self, command: &str) -> Result<()> {
    let command = std::ffi::CString::new(command)?;
    let status =
      unsafe { libmpv2_sys::mpv_command_string(self.mpv.ctx.as_ptr(), command.as_ptr()) };
    if status < 0 {
      return Err(anyhow!(
        "mpv command `{}` failed: {}",
        command.to_string_lossy(),
        libmpv2_sys::mpv_error_str(status)
      ));
    }
    Ok(())
  }

  /// Sets any mpv property.
  pub fn set_property(&self, name: &str, value: impl Into<PropertyValue>) -> Result<()> {
    match value.into() {
      PropertyValue::String(value) => self.mpv.set_property(name, value.as_str()),
      PropertyValue::Flag(value) => self.mpv.set_property(name, value),
      PropertyValue::Int64(value) => self.mpv.set_property(name, value),
      PropertyValue::Double(value) => self.mpv.set_property(name, value),
    }
    .map_err(|error| anyhow!("failed to set `{name}`: {error}"))
  }

  /// Reads any mpv property, formatted the way you ask for it.
  pub fn get_property<T: libmpv2::GetData>(&self, name: &str) -> Result<T> {
    self
      .mpv
      .get_property(name)
      .map_err(|error| anyhow!("failed to read `{name}`: {error}"))
  }

  /// Watches a property; every change arrives as
  /// [`PlayerEvent::PropertyChanged`].
  pub fn observe(&self, name: &str) -> Result<()> {
    let id = self.mpv.observe_property(name, Format::Double, 0);
    id.map_err(|error| anyhow!("failed to observe `{name}`: {error}"))
  }

  /// The tracks of the current entry.
  pub fn tracks(&self) -> Vec<Track> {
    let count = self.get_property::<i64>("track-list/count").unwrap_or(0);
    (0..count)
      .filter_map(|index| {
        let kind = TrackKind::parse(
          &self
            .get_property::<String>(&format!("track-list/{index}/type"))
            .ok()?,
        );
        Some(Track {
          id: self
            .get_property::<i64>(&format!("track-list/{index}/id"))
            .ok()?,
          kind: kind?,
          title: self
            .get_property::<String>(&format!("track-list/{index}/title"))
            .ok(),
          lang: self
            .get_property::<String>(&format!("track-list/{index}/lang"))
            .ok(),
          codec: self
            .get_property::<String>(&format!("track-list/{index}/codec"))
            .ok(),
          selected: self
            .get_property::<bool>(&format!("track-list/{index}/selected"))
            .unwrap_or(false),
          external: self
            .get_property::<bool>(&format!("track-list/{index}/external"))
            .unwrap_or(false),
        })
      })
      .collect()
  }

  /// The playlist, as mpv sees it.
  pub fn playlist(&self) -> Vec<PlaylistEntry> {
    let count = self.get_property::<i64>("playlist/count").unwrap_or(0);
    let current = self.get_property::<i64>("playlist-pos").unwrap_or(-1);
    (0..count)
      .filter_map(|index| {
        let filename = self
          .get_property::<String>(&format!("playlist/{index}/filename"))
          .ok()?;
        Some(PlaylistEntry {
          index: index as usize,
          filename,
          title: self
            .get_property::<String>(&format!("playlist/{index}/title"))
            .ok(),
          current: index == current,
        })
      })
      .collect()
  }

  /// Jumps to a playlist position.
  pub fn set_playlist_position(&self, index: usize) -> Result<()> {
    self.set_property("playlist-pos", index as i64)
  }

  /// The chapters of the current entry.
  pub fn chapters(&self) -> Vec<Chapter> {
    let count = self.get_property::<i64>("chapter-list/count").unwrap_or(0);
    (0..count)
      .filter_map(|index| {
        Some(Chapter {
          title: self
            .get_property::<String>(&format!("chapter-list/{index}/title"))
            .ok(),
          time: self
            .get_property::<f64>(&format!("chapter-list/{index}/time"))
            .ok()?,
        })
      })
      .collect()
  }

  /// Reloads [`PlayerState`] from mpv. Views call this after a command instead
  /// of waiting for the matching property event.
  pub fn sync_state(&mut self, cx: &mut Context<Self>) {
    self.state.paused = self.get_property::<bool>("pause").unwrap_or(false);
    self.state.volume = self
      .get_property::<f64>("volume")
      .unwrap_or(self.state.volume);
    self.state.muted = self.get_property::<bool>("mute").unwrap_or(false);
    self.state.speed = self
      .get_property::<f64>("speed")
      .unwrap_or(self.state.speed);
    self.refresh_media_info();
    cx.notify();
  }

  fn observe_default_properties(&self) {
    for (name, format) in [
      ("pause", Format::Flag),
      ("time-pos", Format::Double),
      ("duration", Format::Double),
      ("volume", Format::Double),
      ("mute", Format::Flag),
      ("speed", Format::Double),
      ("width", Format::Int64),
      ("height", Format::Int64),
      ("eof-reached", Format::Flag),
      ("idle-active", Format::Flag),
    ] {
      let _ = self.mpv.observe_property(name, format, 0);
    }
  }

  fn start_event_pump(&mut self, cx: &mut Context<Self>) {
    let (tx, rx) = async_channel::bounded::<PlayerEvent>(256);
    let mpv = self.mpv.clone();
    let shutdown = self.shutdown.clone();

    let thread = thread::Builder::new()
      .name("woocraft-mpv-events".into())
      .spawn(move || {
        while !shutdown.load(Ordering::SeqCst) {
          let Some(event) = mpv.wait_event(0.05) else {
            continue;
          };
          let event = match event {
            Ok(event) => translate_event(&event),
            Err(error) => PlayerEvent::Error(error.to_string()),
          };
          if tx.send_blocking(event).is_err() {
            break;
          }
        }
      });

    match thread {
      Ok(thread) => self.event_thread = Some(thread),
      Err(error) => {
        self.state.error = Some(format!("failed to spawn the mpv event thread: {error}"));
      }
    }

    let task = cx.spawn(async move |this, cx| {
      while let Ok(event) = rx.recv().await {
        let alive = this.update(cx, |player, cx| player.apply(event, cx));
        if alive.is_err() {
          break;
        }
      }
    });
    self.event_task = Some(task);
  }

  fn start_video(&mut self, cx: &mut Context<Self>) {
    let (width, height) = self.options.video_size;
    match VideoOutput::start(self.mpv.clone(), width, height) {
      Ok(video) => {
        let frames = video.frames().clone();
        self.video = Some(video);
        let task = cx.spawn(async move |this, cx| {
          while let Ok(frame) = frames.recv().await {
            let image = render_image(&frame);
            let updated = this.update(cx, |player, cx| {
              player.video_image = Some(image);
              cx.notify();
            });
            if updated.is_err() {
              break;
            }
          }
        });
        self.frame_task = Some(task);
      }
      Err(error) => self.state.error = Some(error.to_string()),
    }
  }

  fn apply(&mut self, event: PlayerEvent, cx: &mut Context<Self>) {
    match &event {
      PlayerEvent::PropertyChanged { name, value } => match (name.as_str(), value) {
        ("pause", PropertyValue::Flag(paused)) => self.state.paused = *paused,
        ("time-pos", PropertyValue::Double(position)) => self.state.position = *position,
        ("duration", PropertyValue::Double(duration)) => self.state.duration = *duration,
        ("volume", PropertyValue::Double(volume)) => self.state.volume = *volume,
        ("mute", PropertyValue::Flag(muted)) => self.state.muted = *muted,
        ("speed", PropertyValue::Double(speed)) => self.state.speed = *speed,
        ("width", PropertyValue::Int64(width)) => {
          let height = self.state.video_size.map(|(_, height)| height).unwrap_or(0);
          self.state.video_size = Some((*width as u32, height));
          self.state.has_video = true;
        }
        ("height", PropertyValue::Int64(height)) => {
          let width = self.state.video_size.map(|(width, _)| width).unwrap_or(0);
          self.state.video_size = Some((width, *height as u32));
          self.state.has_video = true;
        }
        ("eof-reached", PropertyValue::Flag(eof)) => self.state.eof = *eof,
        ("idle-active", PropertyValue::Flag(idle)) => self.state.idle = *idle,
        _ => {}
      },
      PlayerEvent::FileLoaded => {
        self.state.eof = false;
        self.refresh_media_info();
        self.sync_state_now();
      }
      PlayerEvent::EndFile { .. } => {
        self.state.position = 0.0;
      }
      _ => {}
    }

    cx.emit(event);
    cx.notify();
  }

  fn sync_state_now(&mut self) {
    self.state.paused = self.get_property::<bool>("pause").unwrap_or(false);
    self.state.duration = self.get_property::<f64>("duration").unwrap_or(0.0);
    self.state.position = self.get_property::<f64>("time-pos").unwrap_or(0.0);
  }

  fn refresh_media_info(&mut self) {
    self.state.title = self.get_property::<String>("media-title").ok();
    self.state.artist = self
      .get_property::<String>("metadata/by-key/Artist")
      .ok()
      .filter(|value| !value.is_empty());
    self.state.album = self
      .get_property::<String>("metadata/by-key/Album")
      .ok()
      .filter(|value| !value.is_empty());
    self.metadata = self.metadata_tags();
    self.state.has_video = self
      .get_property::<i64>("vid")
      .map(|id| id != 0)
      .unwrap_or(false);
    let width = self.get_property::<i64>("width").unwrap_or(0);
    let height = self.get_property::<i64>("height").unwrap_or(0);
    if width > 0 && height > 0 {
      self.state.video_size = Some((width as u32, height as u32));
    }
  }

  fn metadata_tags(&self) -> BTreeMap<String, String> {
    let count = self.get_property::<i64>("metadata/list/count").unwrap_or(0);
    (0..count)
      .filter_map(|index| {
        let key = self
          .get_property::<String>(&format!("metadata/list/{index}/key"))
          .ok()?;
        let value = self
          .get_property::<String>(&format!("metadata/list/{index}/value"))
          .ok()?;
        Some((key, value))
      })
      .collect()
  }

  /// All tags of the current entry.
  pub fn metadata(&self) -> &BTreeMap<String, String> {
    &self.metadata
  }

  // ----- audio -------------------------------------------------------------

  /// Selects an audio track by mpv track id, or disables audio with `None`.
  pub fn set_audio_track(&self, id: Option<i64>) -> Result<()> {
    self.set_property(
      "aid",
      id.map(PropertyValue::Int64)
        .unwrap_or(PropertyValue::Flag(false)),
    )
  }

  /// Replaces the audio filter chain, using mpv's format, for example
  /// `"lavfi=[equalizer=f=1000:t=q:w=1:g=4]"`.
  pub fn set_audio_filters(&self, filters: &str) -> Result<()> {
    self.set_property("af", filters)
  }

  /// Removes every audio filter.
  pub fn clear_audio_filters(&self) -> Result<()> {
    self.set_property("af", "")
  }

  /// Sets a graphic equalizer from `(frequency_hz, gain_db)` bands.
  ///
  /// The chain is a single `lavfi` graph of peaking `equalizer` filters, so it
  /// composes with [`Self::set_audio_filters`] only if the caller concatenates
  /// it themselves; use this method for the common case.
  pub fn set_equalizer(&self, bands: &[(f64, f64)]) -> Result<()> {
    if bands.is_empty() {
      return self.clear_audio_filters();
    }
    let chain = bands
      .iter()
      .map(|(frequency, gain)| format!("equalizer=f={frequency}:t=q:w=1:g={gain}"))
      .collect::<Vec<_>>()
      .join(",");
    self.set_audio_filters(&format!("lavfi=[{chain}]"))
  }

  /// Selects an output device by name (`ao` device list), or the default.
  pub fn set_audio_device(&self, device: Option<&str>) -> Result<()> {
    self.set_property("audio-device", device.unwrap_or("auto"))
  }

  /// Sets left/right balance, -1 (left) ..= 1 (right).
  pub fn set_balance(&self, balance: f64) -> Result<()> {
    self.set_property("balance", balance.clamp(-1.0, 1.0))
  }

  /// Delays or advances audio by `seconds` relative to video.
  pub fn set_audio_delay(&self, seconds: f64) -> Result<()> {
    self.set_property("audio-delay", seconds)
  }

  /// Scales the audio volume ceiling above 100 for quiet material.
  pub fn set_volume_max(&self, volume: f64) -> Result<()> {
    self.set_property("volume-max", volume.clamp(100.0, 1000.0))
  }

  /// Enables ReplayGain (`"no"`, `"track"` or `"album"`).
  pub fn set_replaygain(&self, mode: &str) -> Result<()> {
    self.set_property("replaygain", mode)
  }

  /// Keeps the current audio pitch when the speed changes.
  pub fn set_pitch_correction(&self, enabled: bool) -> Result<()> {
    self.set_property("audio-pitch-correction", enabled)
  }

  // ----- subtitles ---------------------------------------------------------

  /// Selects a subtitle track by mpv track id, or hides subtitles with `None`.
  pub fn set_subtitle_track(&self, id: Option<i64>) -> Result<()> {
    self.set_property(
      "sid",
      id.map(PropertyValue::Int64)
        .unwrap_or(PropertyValue::Flag(false)),
    )
  }

  /// Loads an external subtitle file (`select`, `auto` or `cached`).
  pub fn add_subtitle(&self, path: &str, flag: &str) -> Result<()> {
    self.command("sub-add", &[path, flag])
  }

  /// Shows or hides subtitles without changing the selected track.
  pub fn set_subtitle_visibility(&self, visible: bool) -> Result<()> {
    self.set_property("sub-visibility", visible)
  }

  /// Delays or advances subtitles by `seconds`.
  pub fn set_subtitle_delay(&self, seconds: f64) -> Result<()> {
    self.set_property("sub-delay", seconds)
  }

  /// Scales subtitle text, 1.0 being the style's own size.
  pub fn set_subtitle_scale(&self, scale: f64) -> Result<()> {
    self.set_property("sub-scale", scale.max(0.1))
  }

  /// Positions subtitles vertically, 0 (top) ..= 150 (below the bottom edge).
  pub fn set_subtitle_position(&self, position: f64) -> Result<()> {
    self.set_property("sub-pos", position.clamp(0.0, 150.0))
  }

  /// Applies a plain-text subtitle style. Styled ASS subtitles keep their own
  /// styling unless `ass` is turned off through [`Self::set_subtitle_ass`].
  pub fn set_subtitle_style(&self, style: &SubtitleStyle) -> Result<()> {
    self.set_property("sub-font", style.font.as_str())?;
    self.set_property("sub-font-size", style.font_size)?;
    self.set_property("sub-color", style.color.as_str())?;
    self.set_property("sub-border-color", style.border_color.as_str())?;
    self.set_property("sub-border-size", style.border_size)?;
    self.set_property("sub-shadow-offset", style.shadow_offset)?;
    self.set_property("sub-bold", style.bold)?;
    self.set_property("sub-italic", style.italic)?;
    Ok(())
  }

  /// Lets mpv render ASS styling itself (`true`) or forces plain text.
  pub fn set_subtitle_ass(&self, ass: bool) -> Result<()> {
    self.set_property("sub-ass", ass)
  }

  /// Selects a second subtitle track, shown alongside the first.
  pub fn set_secondary_subtitle(&self, id: Option<i64>) -> Result<()> {
    self.set_property(
      "secondary-sid",
      id.map(PropertyValue::Int64)
        .unwrap_or(PropertyValue::Flag(false)),
    )
  }

  // ----- video -------------------------------------------------------------

  /// Sets how video is decoded. [`HwDec::AutoSafe`] is the library default: it
  /// uses hardware decoding when the driver is known to be correct and falls
  /// back to software otherwise.
  pub fn set_hardware_decoding(&self, mode: HwDec) -> Result<()> {
    self.set_property("hwdec", mode.as_str())
  }

  /// Restricts hardware decoding to a codec list, for example `"h264,hevc"`.
  pub fn set_hardware_decoding_codecs(&self, codecs: &str) -> Result<()> {
    self.set_property("hwdec-codecs", codecs)
  }

  /// Sets deinterlacing (`"no"`, `"auto"` or `"yes"`).
  pub fn set_deinterlace(&self, mode: &str) -> Result<()> {
    self.set_property("deinterlace", mode)
  }

  /// Overrides the display aspect ratio, for example `"16:9"`.
  pub fn set_aspect(&self, aspect: &str) -> Result<()> {
    self.set_property("video-aspect-override", aspect)
  }

  /// Rotates the video by 0, 90, 180 or 270 degrees.
  pub fn set_rotation(&self, degrees: i64) -> Result<()> {
    self.set_property("video-rotate", degrees.rem_euclid(360))
  }

  /// Sets an A-B loop, or clears it with `None`.
  pub fn set_ab_loop(&self, range: Option<(f64, f64)>) -> Result<()> {
    match range {
      Some((start, end)) => {
        self.set_property("ab-loop-a", start)?;
        self.set_property("ab-loop-b", end)
      }
      None => {
        self.set_property("ab-loop-a", "no")?;
        self.set_property("ab-loop-b", "no")
      }
    }
  }

  /// Steps one frame forward while paused.
  pub fn frame_step(&self) -> Result<()> {
    self.command("frame-step", &[])
  }

  /// Steps one frame backward while paused.
  pub fn frame_back_step(&self) -> Result<()> {
    self.command("frame-back-step", &[])
  }
}

impl Drop for Player {
  fn drop(&mut self) {
    self.shutdown.store(true, Ordering::SeqCst);
    if let Some(thread) = self.event_thread.take() {
      let _ = thread.join();
    }
    self.video = None;
  }
}

fn create_mpv(options: &PlayerOptions) -> Result<Mpv> {
  let options = options.clone();
  let mpv = Mpv::with_initializer(|init| {
    // Everything is driven from Rust: no OSD, no input bindings, no terminal
    // chatter. Applications that want mpv's own configuration can pass
    // `PlayerOptions::option("config", "yes")`.
    init.set_option("idle", "yes")?;
    init.set_option("terminal", "no")?;
    init.set_option("input-default-bindings", "no")?;
    init.set_option("input-vo-keyboard", "no")?;
    init.set_option("osc", "no")?;
    init.set_option("keep-open", "yes")?;
    init.set_option("vo", if options.video { "libmpv" } else { "null" })?;
    if !options.video {
      init.set_option("vid", "no")?;
    }
    init.set_option("volume", options.volume)?;
    init.set_option("mute", options.muted)?;
    init.set_option("speed", options.speed)?;
    init.set_option("hwdec", options.hwdec.as_str())?;
    for (name, value) in &options.options {
      init.set_option(name.as_str(), value.as_str())?;
    }
    Ok(())
  })
  .map_err(|error| anyhow!("failed to initialize libmpv: {error}"))?;
  Ok(mpv)
}

fn translate_event(event: &libmpv2::events::Event<'_>) -> PlayerEvent {
  use libmpv2::events::Event;

  match event {
    Event::FileLoaded => PlayerEvent::FileLoaded,
    Event::EndFile(reason) => PlayerEvent::EndFile {
      reason: format!("{reason:?}"),
    },
    Event::VideoReconfig => PlayerEvent::VideoReconfig,
    Event::AudioReconfig => PlayerEvent::AudioReconfig,
    Event::Seek => PlayerEvent::Seek,
    Event::PlaybackRestart => PlayerEvent::PlaybackRestart,
    Event::PropertyChange { name, change, .. } => PlayerEvent::PropertyChanged {
      name: (*name).to_string(),
      value: match change {
        libmpv2::events::PropertyData::Str(value)
        | libmpv2::events::PropertyData::OsdStr(value) => PropertyValue::String((*value).into()),
        libmpv2::events::PropertyData::Flag(value) => PropertyValue::Flag(*value),
        libmpv2::events::PropertyData::Int64(value) => PropertyValue::Int64(*value),
        libmpv2::events::PropertyData::Double(value) => PropertyValue::Double(*value),
        libmpv2::events::PropertyData::None => PropertyValue::Flag(false),
      },
    },
    Event::Shutdown => PlayerEvent::Error("mpv shut down".into()),
    Event::LogMessage { text, .. } => PlayerEvent::Error((*text).into()),
    other => PlayerEvent::Error(format!("{other:?}")),
  }
}

fn render_image(frame: &VideoFrame) -> Arc<RenderImage> {
  let buffer = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
    .unwrap_or_else(|| image::RgbaImage::new(1, 1));
  Arc::new(RenderImage::new([image::Frame::new(buffer)]))
}

/// Exposed for views: `Duration` formatting that never shows a negative time.
pub(crate) fn format_time(position: Duration) -> String {
  let total = position.as_secs();
  let (hours, minutes, seconds) = (total / 3600, (total % 3600) / 60, total % 60);
  if hours > 0 {
    format!("{hours}:{minutes:02}:{seconds:02}")
  } else {
    format!("{minutes}:{seconds:02}")
  }
}

impl From<String> for PropertyValue {
  fn from(value: String) -> Self {
    Self::String(value)
  }
}

impl From<&str> for PropertyValue {
  fn from(value: &str) -> Self {
    Self::String(value.to_string())
  }
}

impl From<bool> for PropertyValue {
  fn from(value: bool) -> Self {
    Self::Flag(value)
  }
}

impl From<i64> for PropertyValue {
  fn from(value: i64) -> Self {
    Self::Int64(value)
  }
}

impl From<f64> for PropertyValue {
  fn from(value: f64) -> Self {
    Self::Double(value)
  }
}

/// mpv hardware decoding modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum HwDec {
  /// Decode in software only.
  #[default]
  No,
  /// Use hardware decoding and fall back to software on failure.
  Auto,
  /// Like [`Self::Auto`] but restricted to drivers mpv considers safe.
  AutoSafe,
  /// Hardware decode into system memory, then upload again.
  AutoCopy,
  /// Force a specific API, for example `vaapi` or `d3d11va`.
  Api(&'static str),
}

impl HwDec {
  fn as_str(self) -> &'static str {
    match self {
      Self::No => "no",
      Self::Auto => "auto",
      Self::AutoSafe => "auto-safe",
      Self::AutoCopy => "auto-copy",
      Self::Api(api) => api,
    }
  }
}

/// Plain-text subtitle styling.
#[derive(Clone, Debug)]
pub struct SubtitleStyle {
  /// Font family used for plain-text subtitles.
  pub font: String,
  /// Font size in scaled pixels.
  pub font_size: f64,
  /// Text colour in `#RRGGBB` or `#AARRGGBB`.
  pub color: String,
  /// Outline colour.
  pub border_color: String,
  /// Outline width in pixels.
  pub border_size: f64,
  /// Shadow offset in pixels.
  pub shadow_offset: f64,
  /// Whether plain-text subtitles are bold.
  pub bold: bool,
  /// Whether plain-text subtitles are italic.
  pub italic: bool,
}

impl Default for SubtitleStyle {
  fn default() -> Self {
    Self {
      font: crate::DEFAULT_FONT_FAMILY.to_string(),
      font_size: 48.0,
      color: "#FFFFFFFF".to_string(),
      border_color: "#FF000000".to_string(),
      border_size: 2.0,
      shadow_offset: 1.0,
      bold: false,
      italic: false,
    }
  }
}

impl PlayerOptions {
  /// Selects the hardware decoding mode used from the first frame on.
  pub fn hardware_decoding(mut self, mode: HwDec) -> Self {
    self.hwdec = mode;
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn formats_short_and_long_positions() {
    assert_eq!(format_time(Duration::from_secs(0)), "0:00");
    assert_eq!(format_time(Duration::from_secs(5)), "0:05");
    assert_eq!(format_time(Duration::from_secs(605)), "10:05");
    assert_eq!(format_time(Duration::from_secs(3661)), "1:01:01");
  }

  #[test]
  fn hardware_decoding_modes_match_mpv_names() {
    assert_eq!(HwDec::No.as_str(), "no");
    assert_eq!(HwDec::Auto.as_str(), "auto");
    assert_eq!(HwDec::AutoSafe.as_str(), "auto-safe");
    assert_eq!(HwDec::AutoCopy.as_str(), "auto-copy");
    assert_eq!(HwDec::Api("vaapi").as_str(), "vaapi");
    assert_eq!(HwDec::default(), HwDec::No);
  }

  #[test]
  fn audio_only_disables_video_and_defaults_to_software_decoding() {
    let options = PlayerOptions::audio_only();
    assert!(!options.video);
    assert_eq!(options.hwdec, HwDec::No);
    let options = PlayerOptions::default().hardware_decoding(HwDec::AutoSafe);
    assert_eq!(options.hwdec, HwDec::AutoSafe);
  }

  #[test]
  fn subtitle_style_defaults_to_the_embedded_family() {
    let style = SubtitleStyle::default();
    assert_eq!(style.font, crate::DEFAULT_FONT_FAMILY);
    assert!(style.color.starts_with('#') && style.color.len() == 9);
  }
}
