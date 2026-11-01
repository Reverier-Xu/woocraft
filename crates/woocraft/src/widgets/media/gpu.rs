//! GPU video transport (Linux, X11 and Wayland): mpv renders into a gpui
//! composition surface through an EGL/OpenGL context this module owns.
//!
//! Frame path: mpv decodes and composites with its own OpenGL renderer into
//! the default framebuffer (fbo 0) of an EGL window surface created over the
//! composition child window — an X11 child window on X11/XWayland, a
//! desynchronized `wl_surface` subsurface on Wayland. `eglSwapBuffers`
//! presents each frame straight to the compositor; a video pixel never
//! round-trips through the CPU. gpui keeps drawing its scene on the main
//! window surface, with the child surface stacked above gpui's base surface.
//!
//! Ownership and thread model (full map in `docs/media-gpu.md`):
//!
//! * The **UI thread** runs [`CompositionVideo::attach`]: glutin's EGL
//!   `Context`/`Surface` are `!Send`, so the display, config, context and
//!   surface are created there, as is the mpv render context (which requires
//!   the GL context to be current on the creating thread). The UI thread then
//!   releases the context with `make_not_current` and moves the now-`Send`
//!   context/surface pair plus the render context to the render thread.
//! * The **render thread** re-binds the context, renders whenever mpv's update
//!   callback nudges it awake, swaps, and — on shutdown — frees the mpv render
//!   context and releases the GL context before returning. Joining it therefore
//!   enforces "the render context dies before the mpv core" by ordering, not by
//!   convention.
//! * [`Player`](super::player::Player) joins the render thread on detach and on
//!   drop, before the mpv core can be destroyed; [`RenderContextGuard`]
//!   enforces the same rule on the setup error paths — including a failed
//!   render-thread spawn — where no thread exists yet.
//!
//! Platform handles: on X11 the composition surface hands out an `Xcb` handle
//! for the child window, which carries no Xlib `Display*`, so the EGL display
//! side uses `EGL_DEFAULT_DISPLAY` (same server; XIDs are server-global). On
//! Wayland the surface hands out only the child `wl_surface`; the missing
//! `wl_display` comes from gpui's own window through
//! [`raw_window_handle::HasDisplayHandle`], and EGL requires exactly that
//! connection to accept the child surface as a native window.
//!
//! This module is Linux-only: the composition transport is implemented and
//! verified on X11 and Wayland, and no other platform carries this code.

use std::{
  ffi::{CStr, CString, c_char, c_int, c_void},
  num::NonZeroU32,
  rc::Rc,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
  },
  thread::JoinHandle,
};

use anyhow::{Context as _, Result, anyhow, bail};
use glutin::{
  config::{Api as GlApi, ConfigSurfaceTypes, ConfigTemplate},
  context::{
    ContextApi, ContextAttributesBuilder, NotCurrentContext, NotCurrentGlContext as _,
    PossiblyCurrentGlContext as _,
  },
  display::{Display as GlutinDisplay, DisplayApiPreference},
  prelude::{GlConfig as _, GlDisplay as _, GlSurface as _},
  surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface},
};
use gpui::{Bounds, DevicePixels, Window, WindowCompositionSurface};
use libmpv2::Mpv;
use libmpv2_sys as sys;
use raw_window_handle::{HasDisplayHandle as _, RawDisplayHandle, RawWindowHandle};

/// Owns the mpv render context — and, once its update callback is
/// registered, the callback's boxed wake sender — until both are handed to
/// the render thread.
///
/// mpv aborts the process if a core is destroyed while one of its render
/// contexts still exists, and the update callback must never run against a
/// freed box, so every failure between creation and the thread handoff has
/// to free the context first and reclaim the box second. This type makes
/// the Rust drop order do that: any failure — including a failed
/// render-thread spawn — drops the guard, and [`RenderContextGuard::hand_off`]
/// transfers ownership to the render thread only after that thread exists.
struct RenderContextGuard {
  /// The render context; never null, because the guard is built only after
  /// `mpv_render_context_create` succeeded.
  rc: *mut sys::mpv_render_context,
  /// The leaked `Wake` box, null until the update callback is registered —
  /// before that, nothing can reach the box through mpv.
  wake_box: *mut Wake,
}

impl RenderContextGuard {
  /// Relinquishes ownership to the render thread, which frees the render
  /// context and reclaims the wake box after its last render. Only call
  /// this once that thread exists.
  fn hand_off(self) {
    std::mem::forget(self);
  }
}

impl Drop for RenderContextGuard {
  fn drop(&mut self) {
    // SAFETY: the pointer came from `mpv_render_context_create`, is freed
    // exactly once, and no mpv core can be destroyed under it — the guard
    // lives only inside `CompositionVideo::build`, on the UI thread, while
    // the player still owns the core. Whether the GL context that created
    // it is still current depends on which step failed: it is on every
    // step up to the `make_not_current` handoff, and is not after it (a
    // failed spawn). Freeing without a current context is the least-bad
    // option there; `render_thread` explains why.
    unsafe { sys::mpv_render_context_free(self.rc) };
    // SAFETY: the free above permanently stops the update callback, so
    // nothing can reach the box anymore.
    if !self.wake_box.is_null() {
      unsafe { drop(Box::from_raw(self.wake_box)) };
    }
  }
}

/// The raw mpv render context as a `Send` token for the thread handoff.
struct RenderContextPtr(*mut sys::mpv_render_context);
// SAFETY: between the UI thread's `make_not_current` and the render thread's
// `make_current`, nothing uses the render context; from then on only the
// render thread touches it, and only until it is joined. The pointer carries
// no thread affinity of its own.
unsafe impl Send for RenderContextPtr {}

/// The update-callback context: a clone of the render thread's wake sender,
/// boxed so mpv can hold it as a raw pointer. Leaked for as long as the
/// render context exists (mpv calls the callback from its own threads) and
/// reclaimed once `mpv_render_context_free` has stopped the callback — by the
/// render thread on a normal stop, or by [`RenderContextGuard`] on a setup
/// failure.
///
/// The channel is bounded (capacity 1) and every send uses `try_send`: a
/// render thread blocked in a swap must not let the callback queue nudges
/// without bound, and one queued nudge is all it takes to wake it.
struct Wake(mpsc::SyncSender<()>);

/// A `*mut Wake` that may cross into the render thread with the handoff.
struct WakePtr(*mut Wake);
// SAFETY: the pointer is only dereferenced and only reclaimed (once) on the
// render thread, after the UI thread has stopped touching it and after the
// render context is freed.
unsafe impl Send for WakePtr {}

/// `MPV_RENDER_PARAM_OPENGL_INIT_PARAMS.get_proc_address` shim over the
/// glutin display. `ctx` is a leaked `Box<Rc<GlutinDisplay>>` reclaimed right
/// after `mpv_render_context_create` returns: mpv resolves every GL entry
/// point while creating the render context and never calls this afterwards.
/// The glutin display itself is `Send + Sync`, so a call from any of mpv's
/// threads would be sound too.
unsafe extern "C" fn gl_get_proc_address(ctx: *mut c_void, name: *const c_char) -> *mut c_void {
  if ctx.is_null() || name.is_null() {
    return std::ptr::null_mut();
  }
  // SAFETY: `ctx` is the leaked `Box<Rc<GlutinDisplay>>` passed at create
  // time, alive for the whole `mpv_render_context_create` call that can reach
  // this shim.
  let display = unsafe { &*(ctx as *const Rc<GlutinDisplay>) };
  // SAFETY: `name` is a NUL-terminated C string from mpv, and resolving a
  // function pointer by name has no preconditions beyond an initialized
  // display.
  let ptr = unsafe { display.get_proc_address(CStr::from_ptr(name)) };
  ptr as *mut c_void
}

/// mpv calls this from its own threads whenever a new frame should be
/// rendered. The render API forbids calling back into mpv from the callback,
/// so it only nudges the render thread awake.
unsafe extern "C" fn wake_callback(ctx: *mut c_void) {
  if ctx.is_null() {
    return;
  }
  // SAFETY: `ctx` is the leaked `Box<Wake>` passed at
  // `mpv_render_context_set_update_callback` time, alive until the render
  // thread reclaims it after freeing the render context.
  let wake = unsafe { &*(ctx as *const Wake) };
  // A send failure means the render thread has already exited; there is
  // nobody left to wake. A full channel drops the nudge the same way: one
  // queued wake is enough, so extra ones are worthless by design (see
  // [`Wake`]).
  let _ = wake.0.try_send(());
}

/// One `mpv_render_param` entry, like the helper in [`super::video`].
fn param(type_: sys::mpv_render_param_type, data: *mut c_void) -> sys::mpv_render_param {
  sys::mpv_render_param { type_, data }
}

/// The target surface size in device pixels, shared between the threads: the
/// UI thread replaces it on resize and nudges `wake`; the render thread reads
/// it between frames.
///
/// A poisoned lock can only come from a panic while holding it for one
/// statement, and the value stays readable afterwards; recovering it beats
/// freezing video over a bug elsewhere.
fn shared_size(size: &Mutex<(u32, u32)>) -> (u32, u32) {
  *size.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Everything the render thread owns exclusively once started.
struct RenderThreadArgs {
  context: NotCurrentContext,
  surface: Surface<WindowSurface>,
  rc: RenderContextPtr,
  /// See [`shared_size`]; the size the EGL surface is resized to.
  size: Arc<Mutex<(u32, u32)>>,
  wake: mpsc::Receiver<()>,
  wake_box: WakePtr,
  shutdown: Arc<AtomicBool>,
}

/// The render thread: re-binds the EGL context, renders an mpv frame into the
/// composition child surface every time mpv's update callback wakes it, and
/// on exit frees the mpv render context and releases the GL context.
///
/// Waking is event-driven — mpv's update callback, a resize nudge from the UI
/// thread, or the shutdown nudge — so the loop blocks on the channel and never
/// polls.
fn render_thread(args: RenderThreadArgs) {
  // SAFETY: the context was made not-current on the UI thread and this thread
  // owns it exclusively from here until it releases it below.
  let context = match args.context.make_current(&args.surface) {
    Ok(context) => context,
    Err(error) => {
      tracing::warn!(
        %error,
        "woocraft: making the video GL context current on the render thread failed"
      );
      // The GL context never became current on this thread, so the free
      // below runs without one. mpv does not document that case; its
      // GL-side cleanup cannot happen and driver resources may leak, but
      // the render context must still be freed — a live one at mpv core
      // destruction aborts the process — and the EGL context is destroyed
      // right after, taking the leaked objects with it. That trade is the
      // least-bad option here.
      free_render_context(&args.rc, &args.wake_box);
      return;
    }
  };

  let mut rendered: u64 = 0;
  let mut size = shared_size(&args.size);
  // Set when the surface was resized: one render without a new decoded frame,
  // so a resize is visible even while playback is paused.
  let mut force_render = false;

  while let Ok(()) = args.wake.recv() {
    if args.shutdown.load(Ordering::SeqCst) {
      break;
    }

    let target = shared_size(&args.size);
    if target != size {
      size = target;
      // SAFETY: the context is current on this thread, which is what
      // `Surface::resize` requires. On Wayland this moves the `wl_egl_window`
      // to the new size; on X11 the child window's geometry is the platform
      // attachment's business (`set_bounds`), and this is a no-op.
      args.surface.resize(
        &context,
        NonZeroU32::new(size.0.max(1)).unwrap_or(NonZeroU32::MIN),
        NonZeroU32::new(size.1.max(1)).unwrap_or(NonZeroU32::MIN),
      );
      force_render = true;
    }

    // SAFETY: the GL context that created the render context is current on
    // this thread — the render API requires exactly that same context to be
    // current for every render call, on whatever thread makes it (the
    // context was created on the UI thread and moved here with its GL
    // context; the pre-0.30 same-thread restriction no longer applies).
    let flags = unsafe { sys::mpv_render_context_update(args.rc.0) };
    let frame_flag = sys::mpv_render_update_flag_MPV_RENDER_UPDATE_FRAME as u64;
    if flags & frame_flag == 0 && !force_render {
      continue;
    }
    force_render = false;

    // fbo = 0 is the default framebuffer — this EGL window surface's
    // drawable; FLIP_Y = 1 because EGL window surfaces use GL bottom-up
    // coordinates.
    let mut fbo = sys::mpv_opengl_fbo {
      fbo: 0,
      w: size.0 as c_int,
      h: size.1 as c_int,
      internal_format: 0,
    };
    let mut flip: c_int = 1;
    let mut params = [
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_FBO,
        (&mut fbo as *mut sys::mpv_opengl_fbo).cast(),
      ),
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_FLIP_Y,
        (&mut flip as *mut c_int).cast(),
      ),
      param(0, std::ptr::null_mut()),
    ];
    // SAFETY: as above — the context is current here, and the params
    // reference the stack structures above, which outlive this call.
    let status = unsafe { sys::mpv_render_context_render(args.rc.0, params.as_mut_ptr()) };
    if status < 0 {
      tracing::warn!(status, "woocraft: mpv_render_context_render failed");
      break;
    }
    // SAFETY: as above; `swap_buffers` only needs the current context.
    if let Err(error) = args.surface.swap_buffers(&context) {
      tracing::warn!(%error, "woocraft: presenting the video frame failed");
      break;
    }
    // SAFETY: as above — current context, render thread. Reported after the
    // swap succeeded: mpv documents this as "a frame was flipped".
    unsafe { sys::mpv_render_context_report_swap(args.rc.0) };
    rendered += 1;
    tracing::trace!(rendered, "woocraft: presented a video frame through GL");
  }

  free_render_context(&args.rc, &args.wake_box);
  // SAFETY: this thread is the context's only user and stops using GL above;
  // releasing it here lets the EGL objects drop safely afterwards.
  if let Err(error) = context.make_not_current() {
    tracing::warn!(%error, "woocraft: releasing the video GL context failed");
  }
  tracing::debug!(rendered, "woocraft: video render thread stopped");
}

/// Frees the mpv render context and reclaims the wake box, in the order mpv
/// requires: the context first — after which mpv never calls the update
/// callback again — then the box, so the callback can no longer run against
/// freed memory.
fn free_render_context(rc: &RenderContextPtr, wake_box: &WakePtr) {
  // SAFETY: the pointer came from `mpv_render_context_create`, is freed
  // exactly once (the caller hands it to nobody else), and the GL context it
  // rendered with is either current on this thread or already unusable.
  unsafe { sys::mpv_render_context_free(rc.0) };
  // SAFETY: the box was leaked at setup, reached only by the update callback,
  // which `mpv_render_context_free` above has permanently stopped.
  unsafe { drop(Box::from_raw(wake_box.0)) };
}

/// A running GPU video pipeline for one window: the composition surface plus
/// the render thread that presents into it.
pub(crate) struct CompositionVideo {
  /// The composition-tree identity of the surface, for `remove_surface`.
  id: gpui::CompositionSurfaceId,
  /// Keeps the platform attachment — the X11 child window or the Wayland
  /// subsurface — alive. Presenting must stop (the render thread must be
  /// joined) before this drops; the platform docs for the attachment require
  /// exactly that.
  surface: Option<WindowCompositionSurface>,
  /// The EGL display, held for lifetime only. glutin requires it to outlive
  /// every context and surface created from it — the render thread owns those
  /// — so this clone stays until [`Self::stop`] has joined that thread.
  _display: Rc<GlutinDisplay>,
  size: Arc<Mutex<(u32, u32)>>,
  wake_tx: mpsc::SyncSender<()>,
  shutdown: Arc<AtomicBool>,
  thread: Option<JoinHandle<()>>,
  /// The last bounds applied to the composition surface, in device pixels.
  bounds: Bounds<DevicePixels>,
}

impl CompositionVideo {
  /// Builds the whole pipeline for `window` and starts the render thread:
  /// composition surface, EGL display/config/context/surface, mpv render
  /// context, then the `make_not_current` handoff.
  ///
  /// `bounds` is the surface's placement in device pixels (the caller has the
  /// window's scale factor; every later update arrives pre-converted too).
  ///
  /// On failure every object created so far is released, and the composition
  /// surface is removed from the window by the caller
  /// ([`Player::attach_video_surface`](super::player::Player::attach_video_surface)),
  /// which owns the fallback.
  pub(crate) fn attach(
    mpv: &Arc<Mpv>, window: &mut Window, bounds: Bounds<DevicePixels>,
  ) -> Result<Self> {
    // --- composition surface ------------------------------------------------
    // `enable_window_composition` is a capability check plus a controller;
    // calling it again later (detach, re-attach) is by design.
    let native = {
      let composition = window
        .enable_window_composition()
        .context("enable_window_composition failed")?;
      composition
        .create_native_surface()
        .context("create_native_surface failed")?
    };
    match Self::build(mpv, window, &native, bounds) {
      Ok(video) => Ok(video),
      Err(error) => {
        // The tree keeps the platform attachment alive as long as it is
        // registered, so remove the surface we just created before failing.
        if let Ok(composition) = window.enable_window_composition()
          && let Err(remove_error) = composition.remove_surface(native.id())
        {
          tracing::warn!(
            %remove_error,
            "woocraft: removing the composition surface of a failed video setup failed"
          );
        }
        drop(native);
        Err(error)
      }
    }
  }

  /// The second half of [`Self::attach`], run with the native surface created
  /// and the window free for its own handles.
  fn build(
    mpv: &Arc<Mpv>, window: &mut Window, native: &WindowCompositionSurface,
    bounds: Bounds<DevicePixels>,
  ) -> Result<Self> {
    // Clamp every axis to one device pixel: an EGL window surface needs a
    // nonzero drawable, and a collapsed stage is invisible either way.
    let bounds = Bounds {
      origin: bounds.origin,
      size: gpui::Size::new(
        DevicePixels(bounds.size.width.0.max(1)),
        DevicePixels(bounds.size.height.0.max(1)),
      ),
    };
    let attachment = native
      .platform_surface()
      .context("the composition surface has no platform attachment")?;
    attachment
      .set_bounds(bounds)
      .context("set_bounds on the composition surface failed")?;
    attachment
      .set_visible(true)
      .context("set_visible on the composition surface failed")?;

    let (width, height) = (bounds.size.width.0 as u32, bounds.size.height.0 as u32);

    let handle = attachment
      .platform_handle()
      .context("platform_handle on the composition surface failed")?;
    let raw = handle
      .downcast::<RawWindowHandle>()
      .map_err(|_| anyhow!("platform_handle did not return a RawWindowHandle"))?;

    // `unthrottled_swap` marks the EGL surface that must not wait for
    // compositor frame callbacks on swap. On Wayland, Mesa's `eglSwapBuffers`
    // blocks at interval 1 until a frame callback arrives on the subsurface —
    // which never comes while gpui's paint loop is stalled, because the
    // desynchronized subsurface's first present happens exactly then. Interval
    // 0 queues the commit instead; see `docs/media-gpu.md`.
    let (window_handle, display_handle, unthrottled_swap) = match *raw {
      RawWindowHandle::Xcb(handle) => {
        // The Xcb window handle carries no Xlib `Display*`; use the default
        // X11 display (same server, and XIDs are server-global).
        (
          RawWindowHandle::Xcb(handle),
          RawDisplayHandle::Xlib(raw_window_handle::XlibDisplayHandle::new(None, 0)),
          false,
        )
      }
      RawWindowHandle::Wayland(surface_handle) => {
        // The child surface handle carries no `wl_display`; gpui's own window
        // hands out the `wl_display` of its connection, and EGL needs exactly
        // that display to accept the child surface as a native window.
        let raw_display = window
          .display_handle()
          .context("window.display_handle failed")?
          .as_raw();
        let RawDisplayHandle::Wayland(display_handle) = raw_display else {
          bail!("Wayland window handle but a non-Wayland display handle");
        };
        (
          RawWindowHandle::Wayland(surface_handle),
          RawDisplayHandle::Wayland(display_handle),
          true,
        )
      }
      other => bail!("unsupported platform handle for the composition surface: {other:?}"),
    };

    // --- EGL display / config / context / surface ----------------------------
    // SAFETY: on X11 the display handle is `EGL_DEFAULT_DISPLAY`, which EGL
    // resolves itself. On Wayland both handles point into live gpui objects
    // — the `wl_display` of gpui's still-open connection, which is
    // connection-global and stays valid for any surface on it, and the
    // gpui-owned child `wl_surface`, which the player keeps alive by
    // holding the composition surface handle until after the render thread
    // is joined (see [`CompositionVideo::stop`] and the drop order below).
    let display = Rc::new(
      unsafe { GlutinDisplay::new(display_handle, DisplayApiPreference::Egl) }
        .context("glutin EGL display creation failed")?,
    );

    let template = ConfigTemplate::default();
    // SAFETY: display creation succeeded, so EGL is initialized for it.
    let config = unsafe { display.find_configs(template) }
      .context("no EGL configs found")?
      .find(|config| {
        config.api().contains(GlApi::OPENGL)
          && config
            .config_surface_types()
            .contains(ConfigSurfaceTypes::WINDOW)
      })
      .ok_or_else(|| anyhow!("no EGL config with OpenGL + window support"))?;

    let context_attributes = ContextAttributesBuilder::new()
      .with_context_api(ContextApi::OpenGl(None))
      .build(Some(window_handle));
    // SAFETY: the config matches the display and the window handle is the
    // composition child window created above.
    let context = unsafe { display.create_context(&config, &context_attributes) }
      .context("EGL context creation failed")?;

    let surface_attributes = SurfaceAttributesBuilder::<WindowSurface>::new().build(
      window_handle,
      NonZeroU32::new(width).context("video surface width is zero")?,
      NonZeroU32::new(height).context("video surface height is zero")?,
    );
    // SAFETY: as above — config and window handle are valid.
    let surface = unsafe { display.create_window_surface(&config, &surface_attributes) }
      .context("EGL window surface creation failed")?;

    let context = context
      .make_current(&surface)
      .context("making the EGL context current failed")?;
    if unthrottled_swap {
      surface
        .set_swap_interval(&context, SwapInterval::DontWait)
        .context("set_swap_interval(DontWait) failed")?;
    }

    // --- mpv render context
    // ---------------------------------------------------
    let (wake_tx, wake_rx) = mpsc::sync_channel::<()>(1);
    let wake_box = Box::into_raw(Box::new(Wake(wake_tx.clone())));

    let api_type = CString::new("opengl").context("CString::new(\"opengl\")")?;
    let mut advanced_control: c_int = 0; // MPV_RENDER_PARAM_ADVANCED_CONTROL off
    let mut init_params = sys::mpv_opengl_init_params {
      get_proc_address: Some(gl_get_proc_address),
      get_proc_address_ctx: Box::into_raw(Box::new(display.clone())) as *mut c_void,
    };
    let mut render_context: *mut sys::mpv_render_context = std::ptr::null_mut();
    let mut params = [
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_API_TYPE,
        api_type.as_ptr() as *const c_char as *mut c_void,
      ),
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_OPENGL_INIT_PARAMS,
        (&mut init_params as *mut sys::mpv_opengl_init_params).cast(),
      ),
      param(
        sys::mpv_render_param_type_MPV_RENDER_PARAM_ADVANCED_CONTROL,
        (&mut advanced_control as *mut c_int).cast(),
      ),
      param(0, std::ptr::null_mut()),
    ];
    // SAFETY: the params reference the stack structures above, which outlive
    // this call, and the GL context is current on this thread, as the render
    // API requires while creating the render context.
    let create_status = unsafe {
      sys::mpv_render_context_create(&mut render_context, mpv.ctx.as_ptr(), params.as_mut_ptr())
    };
    // mpv resolves all GL entry points during create and never calls
    // `get_proc_address` afterwards, so reclaim the ctx box here.
    // SAFETY: the box was leaked two statements above for exactly this call.
    drop(unsafe { Box::from_raw(init_params.get_proc_address_ctx as *mut Rc<GlutinDisplay>) });
    if create_status < 0 {
      // SAFETY: the wake box was leaked for the render context this call
      // failed to create, so nothing can reach it anymore.
      drop(unsafe { Box::from_raw(wake_box) });
      bail!("mpv_render_context_create failed: {create_status}");
    }
    let mut render_context = RenderContextGuard {
      rc: render_context,
      wake_box: std::ptr::null_mut(),
    };

    // SAFETY: the render context was just created and is valid; the callback
    // only sends on the channel inside the leaked `Wake` box.
    unsafe {
      sys::mpv_render_context_set_update_callback(
        render_context.rc,
        Some(wake_callback),
        wake_box as *mut c_void,
      );
    };
    // From here mpv can reach the box, so the guard owns it too: every
    // failure below frees the render context first (stopping the callback)
    // and reclaims the box second.
    render_context.wake_box = wake_box;

    // --- hand the GL context to the render thread
    // ----------------------------- `make_not_current` releases the context on
    // this thread and returns a `NotCurrentContext`, which is `Send`. Nothing
    // here uses GL afterwards.
    let context = context
      .make_not_current()
      .context("releasing the GL context on the UI thread failed")?;

    let size = Arc::new(Mutex::new((width, height)));
    let shutdown = Arc::new(AtomicBool::new(false));
    let args = RenderThreadArgs {
      context,
      surface,
      // A copy of the pointer, not a transfer of ownership: until the spawn
      // below succeeds the guard still owns the render context, so a failed
      // spawn frees it and reclaims the wake box instead of leaking both —
      // and the fallback can then safely create a second render context on
      // the same core.
      rc: RenderContextPtr(render_context.rc),
      size: size.clone(),
      wake: wake_rx,
      wake_box: WakePtr(wake_box),
      shutdown: shutdown.clone(),
    };
    let thread = std::thread::Builder::new()
      .name("woocraft-mpv-video-gl".into())
      .spawn(move || render_thread(args))
      .context("failed to spawn the video render thread")?;
    // The spawn succeeded: the render thread now owns the render context and
    // the wake box and frees both on its exit. Forgetting the guard here —
    // and only here — is the handoff; dropping it instead would double-free.
    render_context.hand_off();
    // A frame may already be waiting: mpv renders the current frame as soon
    // as the render context exists, which can precede the thread's first
    // `recv`.
    let _ = wake_tx.try_send(());

    Ok(Self {
      id: native.id(),
      surface: Some(native.clone()),
      _display: display,
      size,
      wake_tx,
      shutdown,
      thread: Some(thread),
      bounds,
    })
  }

  /// The composition-tree identity of this surface.
  pub(crate) fn id(&self) -> gpui::CompositionSurfaceId {
    self.id
  }

  /// Takes the surface handle out, for a caller that will unregister the
  /// surface from the composition tree itself. Presenting has stopped by then
  /// (see [`Self::stop`]), which the platform attachment requires.
  pub(crate) fn take_surface(&mut self) -> Option<WindowCompositionSurface> {
    self.surface.take()
  }

  /// Moves the composition surface to new device-pixel `bounds` and asks the
  /// render thread to resize the EGL window with it. A no-op for unchanged
  /// bounds, so steady-state paints cost nothing.
  pub(crate) fn update_bounds(&mut self, bounds: Bounds<DevicePixels>) -> Result<()> {
    // Keep every axis at one device pixel: an EGL window surface needs a
    // nonzero drawable, and a collapsed stage is invisible either way.
    let bounds = Bounds {
      origin: bounds.origin,
      size: gpui::Size::new(
        DevicePixels(bounds.size.width.0.max(1)),
        DevicePixels(bounds.size.height.0.max(1)),
      ),
    };
    if bounds == self.bounds {
      return Ok(());
    }
    self.bounds = bounds;
    self
      .surface
      .as_ref()
      .context("the composition surface was already taken")?
      .platform_surface()
      .context("the composition surface has no platform attachment")?
      .set_bounds(bounds)
      .context("set_bounds on the composition surface failed")?;
    *self
      .size
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner()) =
      (bounds.size.width.0 as u32, bounds.size.height.0 as u32);
    // Wake the render thread so the EGL surface resize and the paused-frame
    // re-render happen promptly rather than at the next mpv callback.
    let _ = self.wake_tx.try_send(());
    Ok(())
  }

  /// Stops the render thread. This joins it, which frees the mpv render
  /// context and releases the GL context inside the thread — after which the
  /// EGL surface, context and display can be destroyed safely. Idempotent.
  pub(crate) fn stop(&mut self) {
    self.shutdown.store(true, Ordering::SeqCst);
    // Release a `recv` blocked on the next mpv frame. The channel holds at
    // most one nudge, and any earlier one serves the same purpose.
    let _ = self.wake_tx.try_send(());
    if let Some(thread) = self.thread.take()
      && thread.join().is_err()
    {
      tracing::warn!("woocraft: the video render thread panicked");
    }
  }

  /// Hides the platform surface without destroying it, for the detach path
  /// that has no window to unregister it with. Harmless if already hidden.
  pub(crate) fn hide(&self) {
    if let Some(surface) = self.surface.as_ref()
      && let Ok(attachment) = surface.platform_surface()
      && let Err(error) = attachment.set_visible(false)
    {
      tracing::warn!(%error, "woocraft: hiding the video composition surface failed");
    }
  }
}

impl Drop for CompositionVideo {
  fn drop(&mut self) {
    // Join the render thread before the fields below drop: the EGL surface
    // and context live inside that thread, the EGL display is declared after
    // `surface` and drops after it, and presenting must stop before the
    // platform attachment (the child window / subsurface) can go away.
    self.stop();
    self.hide();
  }
}
