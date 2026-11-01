//! Software video output.
//!
//! mpv's render API is asked for RGBA frames instead of a GPU context, so video
//! output works on every platform gpui runs on, without an OpenGL/EGL/WGL
//! context of our own and without sharing textures across graphics APIs. Frames
//! are produced on a dedicated thread and handed to the UI through a bounded
//! channel owned by the [`Player`](super::player::Player): a slow or hidden
//! window drops frames instead of growing memory.
//!
//! This is one half of the player's two video transports. Its sibling
//! `gpu.rs` (Linux) renders mpv straight into a gpui composition surface
//! through GL, with no CPU copy; the player negotiates between the two and
//! falls back to this one wherever the GL path cannot run. See
//! `docs/media-gpu.md` for the full map.

use std::{
  ffi::{CString, c_char, c_void},
  sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, RecvTimeoutError},
  },
  thread,
  time::Duration,
};

use anyhow::{Result, anyhow};
use async_channel::Sender;
use libmpv2::Mpv;
use libmpv2_sys as sys;

use super::player::PlayerEvent;

/// Lower bound for one render axis: mpv's software renderer needs a span of at
/// least two pixels per axis to produce a meaningful frame. The view checks
/// against it too, so a collapsed stage sends no request at all instead of one
/// that the output would clamp back up to this minimum.
pub(crate) const MIN_RENDER_SIZE: u32 = 2;
/// Upper bound for one render axis: an 8192 x 8192 RGBA buffer is already
/// 256 MiB, which dwarfs any surface a design-system view is meant to fill.
const MAX_RENDER_SIZE: u32 = 8192;

/// One decoded frame, top-down, 8-bit RGBA.
#[derive(Clone)]
pub struct VideoFrame {
  /// Frame width in pixels.
  pub width: u32,
  /// Frame height in pixels.
  pub height: u32,
  /// `width * height * 4` bytes of RGBA.
  pub rgba: Vec<u8>,
}

impl std::fmt::Debug for VideoFrame {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("VideoFrame")
      .field("width", &self.width)
      .field("height", &self.height)
      .field("bytes", &self.rgba.len())
      .finish()
  }
}

/// A running software video output.
///
/// Dropping it stops the render thread; the frame channel lives on with the
/// player, which owns it, so a later output can publish to the same pump.
pub struct VideoOutput {
  sizes: mpsc::Sender<(u32, u32)>,
  shutdown: Arc<AtomicBool>,
  thread: Option<thread::JoinHandle<()>>,
}

impl VideoOutput {
  /// Starts producing frames for `mpv`, scaled to `width` x `height` and
  /// published on `frames` — the player's stable frame channel. A fatal
  /// failure on the render thread is reported once through `events`, the
  /// player's own [`PlayerEvent`] channel, so a dead output does not leave
  /// the stage silently empty.
  pub(crate) fn start(
    mpv: Arc<Mpv>, width: u32, height: u32, frames: Sender<Arc<VideoFrame>>,
    events: Sender<PlayerEvent>,
  ) -> Result<Self> {
    let width = width.clamp(MIN_RENDER_SIZE, MAX_RENDER_SIZE);
    let height = height.clamp(MIN_RENDER_SIZE, MAX_RENDER_SIZE);
    let (size_tx, size_rx) = mpsc::channel::<(u32, u32)>();
    let shutdown = Arc::new(AtomicBool::new(false));

    let thread_shutdown = shutdown.clone();
    let thread = thread::Builder::new()
      .name("woocraft-mpv-video".into())
      .spawn(move || {
        if let Err(error) = render_loop(mpv, width, height, size_rx, frames, thread_shutdown) {
          tracing::warn!(%error, "woocraft: mpv video output stopped");
          // The player keeps `video` pointing at this output and has no
          // window-less way to learn the thread died, so the stage would
          // stay on its placeholder forever with no report. Report once
          // through the same channel every other player error flows through.
          let _ = events.send_blocking(PlayerEvent::Error(format!(
            "the software video output stopped: {error:#}"
          )));
        }
      })
      .map_err(|error| anyhow!("failed to spawn the mpv video thread: {error}"))?;

    Ok(Self {
      sizes: size_tx,
      shutdown,
      thread: Some(thread),
    })
  }

  /// Requests a new render size, clamped per axis. Requests are coalesced on
  /// the render thread, so a burst of them allocates at most one buffer.
  ///
  /// A failed send means the render thread has already stopped and there is
  /// nobody left to resize for; the request is dropped rather than warned
  /// about on every paint of a player that is being torn down.
  pub(crate) fn request_size(&self, width: u32, height: u32) {
    let _ = self.sizes.send((
      width.clamp(MIN_RENDER_SIZE, MAX_RENDER_SIZE),
      height.clamp(MIN_RENDER_SIZE, MAX_RENDER_SIZE),
    ));
  }
}

impl Drop for VideoOutput {
  fn drop(&mut self) {
    self.shutdown.store(true, Ordering::SeqCst);
    if let Some(thread) = self.thread.take() {
      let _ = thread.join();
    }
  }
}

struct RenderContext(*mut sys::mpv_render_context);

impl Drop for RenderContext {
  fn drop(&mut self) {
    if !self.0.is_null() {
      unsafe { sys::mpv_render_context_free(self.0) };
    }
  }
}

fn render_loop(
  mpv: Arc<Mpv>, mut width: u32, mut height: u32, sizes: mpsc::Receiver<(u32, u32)>,
  frames: Sender<Arc<VideoFrame>>, shutdown: Arc<AtomicBool>,
) -> Result<()> {
  let api_type = CString::new("sw")?;
  let format = CString::new("rgb0")?;
  let mut stride_bytes = width as usize * 4;

  let context = create_render_context(mpv.ctx.as_ptr(), &api_type)?;

  // mpv calls this from its own thread whenever a new frame is ready, and the
  // callback must not call back into mpv: it only nudges this thread awake.
  // The channel is bounded (capacity 1) and sends use `try_send`: a render
  // thread stuck in a blocking step must not let mpv queue nudges without
  // bound, and one queued nudge is all it takes to wake it.
  let (wake_tx, wake_rx) = mpsc::sync_channel::<()>(1);
  let wake = Box::into_raw(Box::new(wake_tx));
  unsafe {
    sys::mpv_render_context_set_update_callback(
      context.0,
      Some(wake_callback),
      wake as *mut c_void,
    );
  }

  // The pixel buffer belongs to this thread alone: size requests only ever
  // change `width` and `height` here, between renders, so every render call
  // sees a stride and a pointer that match the buffer it writes into.
  let mut buffer = vec![0u8; stride_bytes * height as usize];
  // Set when the size changed: one render without a new decoded frame, so a
  // resize is visible even while playback is paused.
  let mut force_render = false;
  // Consecutive render failures since the last success (or since the
  // first one): a persistent failure logs once, not per frame, and is
  // summarised on recovery or when the output stops below.
  let mut render_failures: u64 = 0;
  while !shutdown.load(Ordering::SeqCst) {
    match wake_rx.recv_timeout(Duration::from_millis(50)) {
      Ok(()) | Err(RecvTimeoutError::Timeout) => {}
      Err(RecvTimeoutError::Disconnected) => break,
    }

    // Keep only the newest queued size: a resize that overtakes an older
    // request must not allocate a buffer it would immediately replace.
    let mut requested = None;
    while let Ok(size) = sizes.try_recv() {
      requested = Some(size);
    }
    if let Some((new_width, new_height)) = requested.filter(|size| *size != (width, height)) {
      (width, height) = (new_width, new_height);
      stride_bytes = width as usize * 4;
      buffer = vec![0u8; stride_bytes * height as usize];
      force_render = true;
    }

    let flags = unsafe { sys::mpv_render_context_update(context.0) };
    if flags & sys::mpv_render_update_flag_MPV_RENDER_UPDATE_FRAME as u64 == 0 && !force_render {
      continue;
    }
    force_render = false;

    // mpv documents this parameter as `int[2]`, not int64.
    let mut size: [i32; 2] = [width as i32, height as i32];
    let mut stride = stride_bytes as i64;
    let mut params = [
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_SIZE,
        size.as_mut_ptr().cast(),
      ),
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_FORMAT,
        format.as_ptr() as *mut c_char as *mut c_void,
      ),
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_STRIDE,
        (&mut stride as *mut i64).cast(),
      ),
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_SW_POINTER,
        buffer.as_mut_ptr().cast(),
      ),
      param(0, std::ptr::null_mut()),
    ];

    let status = unsafe { sys::mpv_render_context_render(context.0, params.as_mut_ptr()) };
    if status < 0 {
      if render_failures == 0 {
        tracing::warn!(status, "woocraft: mpv_render_context_render failed");
      }
      render_failures += 1;
      continue;
    }
    if render_failures > 0 {
      tracing::warn!(
        render_failures,
        "woocraft: software video rendering recovered after failed renders"
      );
      render_failures = 0;
    }
    // "rgb0" leaves the fourth byte of every pixel unused.
    for pixel in buffer.as_chunks_mut::<4>().0 {
      pixel[3] = 255;
    }

    // A window that is not drawing must not stall playback: drop the frame.
    let _ = frames.try_send(Arc::new(VideoFrame {
      width,
      height,
      rgba: buffer.clone(),
    }));
  }

  if render_failures > 0 {
    tracing::warn!(
      render_failures,
      "woocraft: software video rendering was still failing when the output stopped"
    );
  }
  drop(context);
  // mpv holds no reference to the callback context once the render context is
  // freed, so the allocation can be released here.
  unsafe { drop(Box::from_raw(wake)) };
  Ok(())
}

fn param(type_: sys::mpv_render_param_type, data: *mut c_void) -> sys::mpv_render_param {
  sys::mpv_render_param { type_, data }
}

fn create_render_context(mpv: *mut sys::mpv_handle, api_type: &CString) -> Result<RenderContext> {
  let mut params = [
    param(
      sys::mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
      api_type.as_ptr() as *mut c_char as *mut c_void,
    ),
    param(0, std::ptr::null_mut()),
  ];
  let mut context: *mut sys::mpv_render_context = std::ptr::null_mut();
  let status = unsafe { sys::mpv_render_context_create(&mut context, mpv, params.as_mut_ptr()) };
  if status < 0 {
    return Err(anyhow!("mpv_render_context_create failed: {status}"));
  }
  Ok(RenderContext(context))
}

unsafe extern "C" fn wake_callback(ctx: *mut c_void) {
  if ctx.is_null() {
    return;
  }
  let sender = unsafe { &*(ctx as *const mpsc::SyncSender<()>) };
  // A disconnected channel means the render thread has exited; a full one
  // means a nudge is already queued — dropping this one is correct.
  let _ = sender.try_send(());
}
