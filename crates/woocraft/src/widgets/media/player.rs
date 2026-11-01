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
use async_channel::Sender;
use gpui::{Bounds, Context, EventEmitter, Pixels, RenderImage, Task, Window};
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

/// How video frames reach the screen.
///
/// [`PlayerOptions::video_transport`] requests one; [`Player::video_transport`]
/// reports what actually runs after the platform had its say.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VideoTransport {
  /// mpv renders RGBA frames into system memory on a dedicated thread and the
  /// view paints them as gpui images. Works everywhere gpui does; costs a
  /// decode download and a texture upload per frame.
  Software,
  /// mpv renders through the player's own OpenGL context straight into a gpui
  /// composition surface: no CPU copy, no texture upload, the compositor
  /// shows the video. Implemented and verified on Linux (X11 and Wayland)
  /// only; requests that cannot run fall back to [`Self::Software`] and log
  /// the reason once. See `docs/media-gpu.md` for the frame path and thread
  /// model.
  Composition,
}

impl Default for VideoTransport {
  /// [`Self::Composition`] on Linux, where the GL path is implemented;
  /// [`Self::Software`] everywhere else.
  fn default() -> Self {
    if composition_supported() {
      Self::Composition
    } else {
      Self::Software
    }
  }
}

/// Whether this build carries the GPU transport at all.
pub(crate) const fn composition_supported() -> bool {
  cfg!(target_os = "linux")
}

/// Why a requested [`VideoTransport::Composition`] was refused without a
/// window. Attach-time failures carry their platform error in the log line
/// instead; this type covers the decisions made up front.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TransportFallback {
  /// The platform has no GPU transport implementation. Linux only, on
  /// purpose: no other platform's path has been written or tested.
  UnsupportedPlatform,
}

/// Decides which transport a player runs: what was requested, whether video
/// exists at all, and what the platform can do — before any window is needed.
pub(crate) fn negotiate(
  requested: VideoTransport, video_enabled: bool,
) -> (VideoTransport, Option<TransportFallback>) {
  match (requested, video_enabled) {
    // Nothing to transport without video, whatever was requested.
    (_, false) => (VideoTransport::Software, None),
    (VideoTransport::Software, _) => (VideoTransport::Software, None),
    (VideoTransport::Composition, _) if !composition_supported() => (
      VideoTransport::Software,
      Some(TransportFallback::UnsupportedPlatform),
    ),
    (VideoTransport::Composition, _) => (VideoTransport::Composition, None),
  }
}

/// Options a player is created with. They map onto mpv options one to one.
#[derive(Clone, Debug)]
pub struct PlayerOptions {
  /// Produce video frames. When false the core never opens a video output and
  /// plays the audio track only, which is what a music player wants.
  pub video: bool,
  /// Resolution the software video output renders at, in pixels.
  pub video_size: (u32, u32),
  /// The video transport to request. See [`VideoTransport`]; the player
  /// negotiates what actually runs against the platform and reports it
  /// through [`Player::video_transport`].
  pub video_transport: VideoTransport,
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
      video_transport: VideoTransport::default(),
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

  /// Requests a video transport. This is what the player tries; what runs is
  /// negotiated against the platform (and against the first surface attach)
  /// and reported through [`Player::video_transport`].
  pub fn video_transport(mut self, transport: VideoTransport) -> Self {
    self.video_transport = transport;
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
/// One `loadfile` the host asked for before the composition surface existed.
#[derive(Clone, Debug)]
struct PendingLoad {
  uri: String,
  /// `append` when the host called [`Player::enqueue`], `replace` for
  /// [`Player::load`].
  append: bool,
}

pub struct Player {
  mpv: Arc<Mpv>,
  options: PlayerOptions,
  state: PlayerState,
  metadata: BTreeMap<String, String>,
  video: Option<VideoOutput>,
  video_image: Option<Arc<RenderImage>>,
  /// The frame channel every software output of this player publishes to. It
  /// outlives any single [`VideoOutput`], so a transport fallback can start a
  /// new one without re-spawning the UI-side frame pump.
  frame_tx: Sender<Arc<VideoFrame>>,
  /// The event channel the player reports through: mpv events flow to the
  /// entity through it, and threads the player cannot reach synchronously —
  /// the software render thread above all — report failures as
  /// [`PlayerEvent::Error`] on it too.
  event_tx: Sender<PlayerEvent>,
  /// The transport this player runs: what was negotiated at construction and,
  /// on Linux, confirmed or abandoned by the first surface attach.
  transport: VideoTransport,
  /// Whether the transport fallback has been logged; a player falls back at
  /// most once (composition → software is terminal).
  fallback_logged: bool,
  /// Whether the successful composition negotiation has been logged.
  negotiated_logged: bool,
  /// The attached GL pipeline, on platforms that carry it (Linux). `None`
  /// while unattached.
  #[cfg(target_os = "linux")]
  composition: Option<super::gpu::CompositionVideo>,
  /// Media the host asked to load while the composition transport was
  /// negotiated but had no surface yet. mpv requires the render context to
  /// exist before a `vo=libmpv` output is created, so loading first would
  /// leave the VO uninitialised and the render thread waiting for frames that
  /// never come; these are replayed on the first successful attach.
  pending_loads: Vec<PendingLoad>,
  /// A surface detached without its window, waiting to be removed from the
  /// composition tree at the next window-having call — or by the window's own
  /// destruction, which frees it regardless. At most one entry; see
  /// `docs/media-gpu.md`.
  stale_surface: Option<(gpui::CompositionSurfaceId, gpui::WindowCompositionSurface)>,
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
  ///
  /// The transport is negotiated here: a composition request that this
  /// platform cannot run is downgraded to software and logged once. A
  /// composition request that Linux *can* run stays pending until the first
  /// [`Self::attach_video_surface`]; only that call proves the window's
  /// compositor, EGL and the mpv render context can actually work together.
  pub fn new(cx: &mut Context<Self>, options: PlayerOptions) -> Self {
    let (transport, fallback) = negotiate(options.video_transport, options.video);
    let (frame_tx, frame_rx) = async_channel::bounded::<Arc<VideoFrame>>(2);
    let (event_tx, event_rx) = async_channel::bounded::<PlayerEvent>(256);

    // The mpv core first, so an mpv failure stays a report in
    // [`PlayerState::error`] instead of a constructor panic. On success this
    // is the one and only core; on failure an idle placeholder keeps the
    // public `mpv()` non-nullable while nothing ever plays. The `expect` on
    // the placeholder cannot fire in practice: a bare `Mpv::new()` — create
    // and initialize, no options — fails only on OOM or a client-API version
    // mismatch, and both would have failed `create_mpv` above; an options
    // problem, the realistic failure, reaches the `Err` arm instead.
    let (mpv, error) = match create_mpv(&options) {
      Ok(mpv) => (Arc::new(mpv), None),
      Err(error) => (
        Arc::new(Mpv::new().expect("creating an idle mpv placeholder failed")),
        Some(error.to_string()),
      ),
    };

    let mut player = Self {
      mpv,
      options: options.clone(),
      state: PlayerState {
        volume: options.volume,
        muted: options.muted,
        speed: options.speed,
        error,
        ..Default::default()
      },
      metadata: BTreeMap::new(),
      video: None,
      video_image: None,
      frame_tx,
      event_tx,
      transport,
      fallback_logged: false,
      negotiated_logged: false,
      #[cfg(target_os = "linux")]
      composition: None,
      pending_loads: Vec::new(),
      stale_surface: None,
      shutdown: Arc::new(AtomicBool::new(false)),
      event_thread: None,
      event_task: None,
      frame_task: None,
    };

    if player.state.error.is_some() {
      return player;
    }
    player.observe_default_properties();
    player.start_event_pump(cx, event_rx);
    if options.video {
      player.start_frame_pump(cx, frame_rx);
      if let Some(reason) = fallback {
        player.log_transport_fallback(reason);
      }
      // The composition transport starts its output at the first surface
      // attach instead; software starts here.
      if player.transport == VideoTransport::Software {
        player.start_video();
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

  /// The transport video actually runs. [`VideoTransport::Composition`]
  /// means the player will render through GL into a gpui composition surface
  /// once [`Self::attach_video_surface`] succeeds (before that attach it is
  /// the pending plan); [`VideoTransport::Software`] means frames arrive as
  /// gpui images. Every fallback from composition to software is logged once
  /// through `tracing` at `warn` level, as is the successful negotiation.
  pub fn video_transport(&self) -> VideoTransport {
    self.transport
  }

  /// Whether a composition surface is currently attached. The view uses this
  /// to detach when its video stage disappears.
  pub(crate) fn composition_attached(&self) -> bool {
    #[cfg(target_os = "linux")]
    {
      self.composition.is_some()
    }
    #[cfg(not(target_os = "linux"))]
    {
      false
    }
  }

  /// Requests a new render resolution for the video output, in pixels,
  /// clamped to a sane range per axis. The render thread reallocates its buffer
  /// on its own thread, so an in-flight frame is never invalidated, and the
  /// frame stream switches to the new size; while playback is paused the
  /// current frame is re-rendered at it. A no-op for audio-only players.
  pub fn set_video_size(&self, size: (u32, u32)) {
    if let Some(video) = &self.video {
      video.request_size(size.0, size.1);
    }
  }

  /// Loads `uri` (a path or URL) and starts playing it.
  pub fn load(&mut self, uri: &str) -> Result<()> {
    if self.defer_until_attach() {
      self.pending_loads.push(PendingLoad {
        uri: uri.to_string(),
        append: false,
      });
      return Ok(());
    }
    self.command("loadfile", &[uri, "replace"])
  }

  /// Appends `uri` to the playlist.
  pub fn enqueue(&mut self, uri: &str) -> Result<()> {
    if self.defer_until_attach() {
      self.pending_loads.push(PendingLoad {
        uri: uri.to_string(),
        append: true,
      });
      return Ok(());
    }
    self.command("loadfile", &[uri, "append"])
  }

  /// Whether a `loadfile` must wait for the composition surface.
  ///
  /// Only the composition transport needs this: the software output creates its
  /// own render context as soon as it starts, while the GL context can only
  /// exist once a window has handed the player a surface.
  fn defer_until_attach(&self) -> bool {
    self.options.video
      && self.transport == VideoTransport::Composition
      && !self.composition_attached()
  }

  /// Replays the loads that were deferred while no output existed, and — if
  /// mpv already holds an entry — re-loads that entry at the position
  /// playback reached. A successful (re)attach and the fallback to software
  /// both end here, because mpv creates a `vo=libmpv` output only on a load:
  /// without a (re)load the fresh output would wait for frames that never
  /// come. See [`restart_loads`] for the decision and its rules.
  fn resume_after_attach(&mut self) {
    // mpv's live `path`, never a URI remembered when the host last loaded:
    // a playlist can advance past that (an `enqueue` never updates a
    // remembered URI, and `playlist-pos` moves on its own), and reloading a
    // stale URI would jump the wrong file and destroy the playlist.
    let path = self.get_property::<String>("path").ok();
    let commands = restart_loads(
      std::mem::take(&mut self.pending_loads),
      path.as_deref(),
      self.state.position,
    );
    for (uri, mode, option) in commands {
      let result = match option.as_deref() {
        Some(option) => self.command("loadfile", &[uri.as_str(), mode, option]),
        None => self.command("loadfile", &[uri.as_str(), mode]),
      };
      if let Err(error) = result {
        tracing::warn!(
          %error, uri,
          "woocraft: re-issuing a load after the video output changed failed"
        );
      }
    }
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

  fn start_event_pump(
    &mut self, cx: &mut Context<Self>, events: async_channel::Receiver<PlayerEvent>,
  ) {
    let tx = self.event_tx.clone();
    let mpv = self.mpv.clone();
    let shutdown = self.shutdown.clone();

    let thread = thread::Builder::new()
      .name("woocraft-mpv-events".into())
      .spawn(move || {
        while !shutdown.load(Ordering::SeqCst) {
          let Some(event) = mpv.wait_event(0.05) else {
            continue;
          };
          // A failed wait stays an error, but routine events translate to
          // `None` and are dropped here rather than waking the UI thread just
          // to ignore them.
          let event = match event {
            Ok(event) => translate_event(&event),
            Err(error) => Some(PlayerEvent::Error(error.to_string())),
          };
          if let Some(event) = event
            && tx.send_blocking(event).is_err()
          {
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
      while let Ok(event) = events.recv().await {
        let alive = this.update(cx, |player, cx| player.apply(event, cx));
        if alive.is_err() {
          break;
        }
      }
    });
    self.event_task = Some(task);
  }

  /// Spawns the one UI-side frame pump, which stays alive for the whole
  /// player: every software output — the initial one and any started by a
  /// transport fallback — publishes to the player's stable frame channel, so
  /// switching transports never re-spawns UI tasks.
  fn start_frame_pump(
    &mut self, cx: &mut Context<Self>, frames: async_channel::Receiver<Arc<VideoFrame>>,
  ) {
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

  /// Starts the software video output, if the player wants one and does not
  /// already run one. Deliberately context-free: the fallback from the
  /// composition transport needs it from `attach_video_surface`, which has no
  /// entity context.
  fn start_video(&mut self) {
    if self.video.is_some() || !self.options.video {
      return;
    }
    let (width, height) = self.options.video_size;
    match VideoOutput::start(
      self.mpv.clone(),
      width,
      height,
      self.frame_tx.clone(),
      self.event_tx.clone(),
    ) {
      Ok(video) => self.video = Some(video),
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

  // ----- video surface (composition transport) ------------------------------

  /// Attaches the player's video output to a composition surface of `window`,
  /// placed and sized at `bounds` (window coordinates, converted to device
  /// pixels with the window's scale factor).
  ///
  /// The first call performs the whole setup — window composition, the native
  /// surface, the EGL context and the mpv render context — and starts the
  /// render thread; later calls only move and resize. [`PlayerView`] drives
  /// this from its stage's paint.
  ///
  /// A machine that cannot run the GL path is not an error for the host: any
  /// failing step is logged once through `tracing` with the failed call and
  /// the platform error, the player switches its negotiated transport to
  /// [`VideoTransport::Software`] — starting that output if it was not
  /// running — and this returns `Ok(())`, so playback simply continues. `Err`
  /// is reserved for programming misuse, such as calling this on a player
  /// built with [`PlayerOptions::video`](false).
  ///
  /// No-op for the software transport, on either side of a fallback.
  pub fn attach_video_surface(
    &mut self, window: &mut Window, bounds: Bounds<Pixels>,
  ) -> Result<()> {
    if !self.options.video {
      return Err(anyhow!(
        "attach_video_surface needs a player with video enabled"
      ));
    }
    if self.transport == VideoTransport::Software {
      return Ok(());
    }

    #[cfg(not(target_os = "linux"))]
    {
      // Unreachable in practice: negotiation already downgraded the
      // transport to software at `Player::new` — logging once there — so the
      // software return above fires first. The composition path is
      // Linux-only by design (see the platform matrix in `docs/media-gpu.md`);
      // this stub only keeps the parameters used.
      let _ = (window, bounds);
      return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
      let device = bounds.to_device_pixels(window.scale_factor());

      // Attached already: this call only moves and resizes. A failure there
      // (the surface's platform window refusing the new geometry) is a
      // fallback like any other.
      let update_failed = match self.composition.as_mut() {
        Some(video) => video.update_bounds(device).err(),
        None => None,
      };
      if let Some(error) = update_failed {
        self.fall_back_to_software(Some(window), format!("{error:#}"));
        return Ok(());
      }
      if self.composition.is_some() {
        return Ok(());
      }

      // First attach: unregister any surface a window-less detach left
      // behind, then build the pipeline. `enable_window_composition` is a
      // capability check plus a controller and safe to take again.
      if let Some((stale_id, stale_handle)) = self.stale_surface.take() {
        if let Ok(composition) = window.enable_window_composition()
          && let Err(error) = composition.remove_surface(stale_id)
        {
          tracing::warn!(%error, "woocraft: removing a detached video surface failed");
        }
        // Dropping the handle now that the tree no longer references it
        // destroys the hidden platform surface for good.
        drop(stale_handle);
      }

      match super::gpu::CompositionVideo::attach(&self.mpv, window, device) {
        Ok(video) => {
          self.composition = Some(video);
          self.resume_after_attach();
          if !self.negotiated_logged {
            // The working path is not a warning: composition is the default
            // everywhere it can run, and only the fallback below deserves one.
            tracing::debug!(
              "woocraft: video transport: composition \
               (mpv renders into a gpui composition surface through GL, no CPU copy)"
            );
            self.negotiated_logged = true;
          }
        }
        Err(error) => {
          self.fall_back_to_software(Some(window), format!("{error:#}"));
        }
      }
      Ok(())
    }
  }

  /// Detaches the player's video surface: stops the render thread (which
  /// frees the mpv render context and releases the GL context inside it) and
  /// hides the composition surface. Idempotent, and a no-op for the software
  /// transport or when nothing is attached.
  ///
  /// Without a window the surface's composition-tree entry cannot be removed
  /// yet; it is kept hidden and unregistered at the next attach, or freed by
  /// the window's own destruction — at most one such surface exists per
  /// player (see `docs/media-gpu.md`). The view calls the window-having
  /// [`Self::detach_video_surface_in_window`] variant while it still has one.
  pub fn detach_video_surface(&mut self) {
    self.detach_surface(None);
  }

  /// [`Self::detach_video_surface`], removing the composition surface from
  /// `window`'s composition tree immediately. Used by the view when its video
  /// stage disappears while the window is at hand.
  pub(crate) fn detach_video_surface_in_window(&mut self, window: &mut Window) {
    self.detach_surface(Some(window));
  }

  /// Tears down the composition pipeline, removing its surface when a window
  /// is available and parking it as stale otherwise.
  fn detach_surface(&mut self, window: Option<&mut Window>) {
    #[cfg(target_os = "linux")]
    if let Some(mut video) = self.composition.take() {
      video.stop();
      video.hide();
      match window {
        Some(window) => {
          if let Ok(composition) = window.enable_window_composition()
            && let Err(error) = composition.remove_surface(video.id())
          {
            tracing::warn!(
              %error,
              "woocraft: removing the video composition surface failed"
            );
          }
        }
        None => {
          self.stale_surface = video.take_surface().map(|surface| (video.id(), surface));
        }
      }
    }
  }

  /// The terminal switch to the software transport: tears down any attached
  /// composition pipeline, starts the software output if it is not running,
  /// replays whatever load the switch interrupted (see
  /// [`Self::resume_after_attach`]), and logs `reason` once. Called from the
  /// attach path, where the platform has just refused a step.
  fn fall_back_to_software(&mut self, window: Option<&mut Window>, reason: String) {
    if !self.fallback_logged {
      tracing::warn!("woocraft: video transport fell back to software: {reason}");
      self.fallback_logged = true;
    }
    self.transport = VideoTransport::Software;
    // Detach before starting the software output: mpv allows one render
    // context per core, so the GL one must be freed before the software one
    // is created.
    self.detach_surface(window);
    self.start_video();
    // The new output starts as blank as a first attach: whatever was
    // deferred while unattached, or already playing before the fallback,
    // must be (re)loaded into it — without this, a machine whose first
    // attach fails would never play anything at all.
    self.resume_after_attach();
  }

  /// Logs why a requested composition transport was refused up front. The
  /// counterpart of the attach-time fallback log, and like it, once.
  fn log_transport_fallback(&mut self, fallback: TransportFallback) {
    let reason = match fallback {
      TransportFallback::UnsupportedPlatform => {
        "the composition transport is implemented on Linux (X11/Wayland) only"
      }
    };
    tracing::warn!("woocraft: video transport fell back to software: {reason}");
    self.fallback_logged = true;
  }
}

impl Drop for Player {
  fn drop(&mut self) {
    // The GL render thread first: joining it frees the mpv render context,
    // and the core below must never see a render context at its own
    // destruction.
    self.detach_video_surface();
    self.shutdown.store(true, Ordering::SeqCst);
    if let Some(thread) = self.event_thread.take() {
      let _ = thread.join();
    }
    self.video = None;
  }
}

/// The `loadfile` commands a video output owes mpv when it starts after
/// playback may already have been requested: every deferred load, in order
/// and exactly once — or, when nothing was deferred but mpv already holds an
/// entry, a single reload of that entry at the position playback reached.
///
/// A pure decision, so the fallback behaviour is testable without a window.
/// The reload target is mpv's live `path`, never a URI remembered when the
/// host last loaded: a playlist can advance past such a URI, and reloading
/// it would jump the wrong file and destroy the playlist. Deferred loads win
/// over the reload — they carry the host's newest intent. Each command is
/// `(uri, mode, option)` with at most one `start=` option.
fn restart_loads(
  pending: Vec<PendingLoad>, path: Option<&str>, position: f64,
) -> Vec<(String, &'static str, Option<String>)> {
  if !pending.is_empty() {
    return pending
      .into_iter()
      .map(|load| {
        let mode = if load.append { "append" } else { "replace" };
        (load.uri, mode, None)
      })
      .collect();
  }
  match path.filter(|path| !path.is_empty()) {
    Some(path) => vec![(
      path.to_string(),
      "replace",
      Some(format!("start={:.3}", position.max(0.0))),
    )],
    None => Vec::new(),
  }
}

fn create_mpv(options: &PlayerOptions) -> Result<Mpv> {
  let options = options.clone();
  let mpv = Mpv::with_initializer(|init| {
    // Everything is driven from Rust: no OSD, no input bindings, no terminal
    // chatter. Applications that want mpv's own configuration can pass
    // `PlayerOptions::option("config", "yes")`.
    init.set_option("idle", "yes")?;
    // Diagnostics: `WOOCRAFT_MPV_LOG=1` lets mpv write its own log (decoder,
    // VO, colour decisions) to the terminal, which is the only way to compare
    // a player against a standalone mpv run on the same file.
    if std::env::var_os("WOOCRAFT_MPV_LOG").is_some() {
      init.set_option("terminal", "yes")?;
      init.set_option("msg-level", "all=info")?;
    } else {
      init.set_option("terminal", "no")?;
    }
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

/// Maps one observed property value, or `None` when mpv reported that the
/// property currently has no value (`MPV_FORMAT_NONE`).
///
/// mpv sends the no-value form while a property is unavailable — `time-pos`
/// before the first frame, `width`/`height` before a video track exists — and
/// again once the current entry ends. It must not become
/// [`PropertyValue::Flag(false)`] or any other invented value: the state would
/// briefly claim "paused == false" or "width == 0" from data mpv never sent.
fn property_value(change: &libmpv2::events::PropertyData<'_>) -> Option<PropertyValue> {
  use libmpv2::events::PropertyData;

  match change {
    PropertyData::None => None,
    PropertyData::Str(value) | PropertyData::OsdStr(value) => {
      Some(PropertyValue::String((*value).into()))
    }
    PropertyData::Flag(value) => Some(PropertyValue::Flag(*value)),
    PropertyData::Int64(value) => Some(PropertyValue::Int64(*value)),
    PropertyData::Double(value) => Some(PropertyValue::Double(*value)),
  }
}

/// Translates one mpv event into a player event, or `None` for a routine one.
///
/// Most of mpv's stream is bookkeeping: `Seek` and `PlaybackRestart` bracket
/// every position change, deprecated events (`Tick` among them) arrive as
/// [`Event::Deprecated`], and the reply events answer async requests this
/// wrapper never makes. None of them is a failure, so none may surface as
/// [`PlayerEvent::Error`] — which views render from [`PlayerState::error`] —
/// and none carries state a view acts on, because the observed properties
/// already deliver it. `Error` stays reserved for genuine failures.
fn translate_event(event: &libmpv2::events::Event<'_>) -> Option<PlayerEvent> {
  use libmpv2::events::Event;

  match event {
    Event::FileLoaded => Some(PlayerEvent::FileLoaded),
    Event::EndFile(reason) => Some(PlayerEvent::EndFile {
      reason: format!("{reason:?}"),
    }),
    Event::VideoReconfig => Some(PlayerEvent::VideoReconfig),
    Event::AudioReconfig => Some(PlayerEvent::AudioReconfig),
    Event::PropertyChange { name, change, .. } => {
      property_value(change).map(|value| PlayerEvent::PropertyChanged {
        name: (*name).to_string(),
        value,
      })
    }
    // A core that shut down can no longer play anything; that is a failure
    // from this wrapper's point of view.
    Event::Shutdown => Some(PlayerEvent::Error("mpv shut down".into())),
    // Log messages only arrive when requested; of those, only error and
    // fatal level are failures worth reporting — the rest is chatter.
    Event::LogMessage {
      text, log_level, ..
    } => {
      let failure = *log_level == libmpv2_sys::mpv_log_level_MPV_LOG_LEVEL_ERROR
        || *log_level == libmpv2_sys::mpv_log_level_MPV_LOG_LEVEL_FATAL;
      failure.then(|| PlayerEvent::Error((*text).into()))
    }
    _ => None,
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
  use libmpv2::events::{Event, PropertyData};

  use super::*;

  #[test]
  fn a_late_video_output_replays_deferred_loads_once_and_in_order() {
    // The first-attach and fallback decision: deferred requests drain
    // exactly once, in order, each with its own mode — and never stack the
    // position reload on top, even when something is already playing.
    let pending = vec![
      PendingLoad {
        uri: "a.mp4".into(),
        append: false,
      },
      PendingLoad {
        uri: "b.mp4".into(),
        append: true,
      },
    ];
    assert_eq!(
      restart_loads(pending, Some("c.mp4"), 42.0),
      vec![
        ("a.mp4".into(), "replace", None),
        ("b.mp4".into(), "append", None),
      ]
    );
    // Drained: a second decision with nothing deferred reloads instead of
    // replaying, and it reloads mpv's live `path`, never a remembered URI.
    assert_eq!(
      restart_loads(Vec::new(), Some("c.mp4"), 42.0),
      vec![("c.mp4".into(), "replace", Some("start=42.000".into()))]
    );
  }

  #[test]
  fn a_late_video_output_reloads_the_live_path_only_when_loaded() {
    // Nothing playing (a failed first attach, a stopped player): no reload
    // at all — the fallback must not resurrect a file mpv never loaded.
    assert!(restart_loads(Vec::new(), Some(""), 42.0).is_empty());
    assert!(restart_loads(Vec::new(), None, 42.0).is_empty());
    // A position mpv reports before the first frame must not leak a negative
    // value into the loadfile options.
    assert_eq!(
      restart_loads(Vec::new(), Some("c.mp4"), -1.0),
      vec![("c.mp4".into(), "replace", Some("start=0.000".into()))]
    );
  }

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
  fn transport_negotiation_maps_requested_to_negotiated_with_a_reason() {
    // Software requested: nothing to negotiate, no fallback.
    assert_eq!(
      negotiate(VideoTransport::Software, true),
      (VideoTransport::Software, None)
    );
    // Without video there is nothing to transport, whatever was requested.
    assert_eq!(
      negotiate(VideoTransport::Composition, false),
      (VideoTransport::Software, None)
    );
    // Composition on a platform that carries the GL path stays composition;
    // elsewhere it is refused with exactly one reason. Attach-time failures
    // are decided by the window, not here, and log their platform error.
    if composition_supported() {
      assert_eq!(
        negotiate(VideoTransport::Composition, true),
        (VideoTransport::Composition, None)
      );
    } else {
      assert_eq!(
        negotiate(VideoTransport::Composition, true),
        (
          VideoTransport::Software,
          Some(TransportFallback::UnsupportedPlatform)
        )
      );
    }
  }

  #[test]
  fn transport_default_and_builder_follow_the_platform() {
    // The default request follows what this build can actually run.
    if composition_supported() {
      assert_eq!(VideoTransport::default(), VideoTransport::Composition);
      assert_eq!(
        PlayerOptions::default().video_transport,
        VideoTransport::Composition
      );
    } else {
      assert_eq!(VideoTransport::default(), VideoTransport::Software);
      assert_eq!(
        PlayerOptions::default().video_transport,
        VideoTransport::Software
      );
    }
    // And the builder overrides it.
    assert_eq!(
      PlayerOptions::default()
        .video_transport(VideoTransport::Software)
        .video_transport,
      VideoTransport::Software
    );
    assert_eq!(
      PlayerOptions::audio_only()
        .video_transport(VideoTransport::Composition)
        .video_transport,
      VideoTransport::Composition
    );
  }

  #[test]
  fn subtitle_style_defaults_to_the_embedded_family() {
    let style = SubtitleStyle::default();
    assert_eq!(style.font, crate::DEFAULT_FONT_FAMILY);
    assert!(style.color.starts_with('#') && style.color.len() == 9);
  }

  /// A deprecated event, which is how mpv's `Tick` and other removed events
  /// reach the wrapper, with no payload attached.
  fn deprecated_event() -> Event<'static> {
    Event::Deprecated(libmpv2_sys::mpv_event {
      event_id: 0,
      error: 0,
      reply_userdata: 0,
      data: std::ptr::null_mut(),
    })
  }

  #[test]
  fn routine_mpv_events_are_dropped_not_reported_as_errors() {
    let routine = [
      Event::StartFile,
      Event::Seek,
      Event::PlaybackRestart,
      Event::QueueOverflow,
      Event::ClientMessage(vec!["key-binding"]),
      Event::GetPropertyReply {
        name: "time-pos",
        result: PropertyData::Double(1.0),
        reply_userdata: 0,
      },
      Event::SetPropertyReply(0),
      Event::CommandReply(0),
      deprecated_event(),
    ];
    for event in &routine {
      assert_eq!(translate_event(event), None, "event: {event:?}");
    }
  }

  #[test]
  fn log_messages_are_errors_only_at_error_or_fatal_level() {
    let message = |log_level| Event::LogMessage {
      prefix: "cplayer",
      level: "error",
      text: "failed to initialize the audio output",
      log_level,
    };
    let fatal = message(libmpv2_sys::mpv_log_level_MPV_LOG_LEVEL_FATAL);
    let error = message(libmpv2_sys::mpv_log_level_MPV_LOG_LEVEL_ERROR);
    let warn = message(libmpv2_sys::mpv_log_level_MPV_LOG_LEVEL_WARN);
    let info = message(libmpv2_sys::mpv_log_level_MPV_LOG_LEVEL_INFO);
    assert!(matches!(
      translate_event(&fatal),
      Some(PlayerEvent::Error(_))
    ));
    assert!(matches!(
      translate_event(&error),
      Some(PlayerEvent::Error(_))
    ));
    assert_eq!(translate_event(&warn), None);
    assert_eq!(translate_event(&info), None);
  }

  #[test]
  fn properties_without_a_value_produce_no_change() {
    // mpv reports MPV_FORMAT_NONE for unavailable properties; the state must
    // never take a value mpv did not send, not even a false flag.
    assert_eq!(property_value(&PropertyData::None), None);
    let event = Event::PropertyChange {
      name: "time-pos",
      change: PropertyData::None,
      reply_userdata: 0,
    };
    assert_eq!(translate_event(&event), None);
  }

  #[test]
  fn property_changes_carry_the_name_and_value() {
    let event = Event::PropertyChange {
      name: "time-pos",
      change: PropertyData::Double(12.5),
      reply_userdata: 0,
    };
    assert_eq!(
      translate_event(&event),
      Some(PlayerEvent::PropertyChanged {
        name: "time-pos".into(),
        value: PropertyValue::Double(12.5),
      })
    );
    assert_eq!(
      property_value(&PropertyData::Flag(true)),
      Some(PropertyValue::Flag(true))
    );
    assert_eq!(
      property_value(&PropertyData::Int64(1920)),
      Some(PropertyValue::Int64(1920))
    );
    assert_eq!(
      property_value(&PropertyData::Str("album")),
      Some(PropertyValue::String("album".into()))
    );
    assert_eq!(
      property_value(&PropertyData::OsdStr("0:12")),
      Some(PropertyValue::String("0:12".into()))
    );
  }
}
