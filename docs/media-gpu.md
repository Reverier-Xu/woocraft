# GPU video transport (`VideoTransport::Composition`)

How mpv video reaches the screen without a CPU copy, on Linux. This document
covers the frame path, the ownership and thread model, the Wayland
`SwapInterval` requirement, the fallback triggers, and the platform matrix.
The software transport — the fallback and the default elsewhere — is described
in [`media.md`](media.md).

## Frame path

```
mpv core (decode, filters, OSD/subtitles composited by mpv's renderer)
  │  mpv render API (opengl), fbo 0
  ▼
EGL window surface  ── created over the gpui composition child window:
  │                    X11:   an X11 child window (`RawWindowHandle::Xcb`)
  │                    Wayland: a desynchronized wl_surface subsurface
  ▼
eglSwapBuffers → compositor
```

The composition surface is created by gpui
(`Window::enable_window_composition()` → `create_native_surface()`) and
stacked above gpui's base scene, so gpui keeps drawing its own chrome on the
main window surface while the child window carries the video. The player owns
the EGL context that renders into it; mpv letterboxes the video inside the
surface, so the stage shows black bars rather than distorted video.

The GL viewport is never a literal: it is the stage's bounds, converted to
device pixels with the window's scale factor. Resizes call
`set_bounds` on the platform attachment and `Surface::resize` on the EGL
window, and the render thread re-renders the current frame once so a paused
video resizes too.

## Ownership and thread model

Three threads, each with exclusive phases of every object:

| Object | Created on | Used on | Freed on |
| --- | --- | --- | --- |
| composition surface (platform child window / subsurface) | UI thread | platform | UI thread, on detach / window close |
| EGL display, config, context, surface | UI thread | render thread (context bound there) | render thread (context/surface), UI thread (display, after the join) |
| mpv render context | UI thread (context current there) | render thread | render thread, before the GL context is released |
| mpv core | UI thread | mpv's own + event thread | UI thread, after every render context is freed |

The sequence on attach (`Player::attach_video_surface`, first call):

1. UI thread: `enable_window_composition`, `create_native_surface`, `set_bounds`,
   `set_visible`.
2. UI thread: glutin EGL display (from the child surface handle plus, on
   Wayland, the `wl_display` of gpui's own connection via
   `window.display_handle()`), config, context, window surface, `make_current`,
   and — on Wayland only — `set_swap_interval(DontWait)` (below).
3. UI thread: `mpv_render_context_create` with the GL context current (mpv
   resolves its GL entry points through the glutin display here, and never
   later), mpv's update callback registered to nudge the render thread.
4. UI thread: `make_not_current`, then hand the now-`Send` context/surface
   pair and the render context to a new thread.
5. Render thread: `make_current`, then a blocking loop — mpv's update callback
   wakes it per frame, the UI thread nudges it on resize and shutdown; no
   polling — `mpv_render_context_update` / `_render` / `_report_swap` /
   `swap_buffers` per frame.
6. On detach or drop: shutdown flag + nudge, join. Inside the thread, in
   order: `mpv_render_context_free`, reclaim the callback context,
   `make_not_current`. Then the UI thread removes the composition surface and
   drops the EGL display.

Drop order is enforced by types, not comments: `RenderContextGuard` frees the
mpv render context on every setup error path (while the GL context that
created it is still current), and ownership passes to the render thread only
via `into_raw`, which frees it after its last render. mpv aborts a process
whose core is destroyed while a render context exists, so `Player::drop`
detaches — joining the render thread — before the core's `Arc` can drop.

## The Wayland `SwapInterval::DontWait` requirement

On Wayland the composition child is a **desynchronized subsurface**, and
Mesa's `eglSwapBuffers` at swap interval 1 blocks until a frame callback
arrives *on that subsurface*. The first present happens while gpui's paint
loop is blocked waiting for the main surface's own frame cycle, so the
callback never arrives and the very first `eglSwapBuffers` deadlocks the
render thread. `set_swap_interval(SwapInterval::DontWait)` queues the commit
instead of waiting, which unblocks it. X11 does not have this coupling and
keeps the default (vsync) interval.

This was verified empirically on both backends during the spike that this
module productionises: X11 and Wayland each sustained ~28-29 fps of a 30 fps
file with no GL errors (`gl_error=none`), and the deadlock without
`DontWait` on Wayland was reproduced and fixed before the spike was accepted.

## Fallback triggers

Every one of these logs once (`tracing::warn!`, visible at
`RUST_LOG=woocraft=warn`) with the failed call and the platform error, switches
`Player::video_transport()` to `Software`, starts the software output if it
was not running, and returns `Ok(())` from `attach_video_surface`:

| Trigger | Where it fires |
| --- | --- |
| Composition requested off Linux | at `Player::new` (no window exists yet) |
| `enable_window_composition` (no SHAPE extension on X11, no `wl_subcompositor` on Wayland) | first attach |
| `create_native_surface` / `platform_handle` / `set_bounds` / `set_visible` | first attach |
| EGL display/config/context/surface creation, `make_current`, `SwapInterval` | first attach |
| `mpv_render_context_create` | first attach |
| render-thread spawn, or `set_bounds` on a later bounds update | first attach / resize |
| `attach_video_surface` on a player with video disabled | returns `Err` — the only hard error (programming misuse) |

A fallback is terminal for the player: the view stops calling attach and
paints software frames from then on. The counterpart success line —
`woocraft: video transport: composition (...)` — is logged once when the
first attach succeeds, so GUI runs are attributable at
`RUST_LOG=woocraft=warn`.

A runtime failure *after* a successful attach (a lost compositor, a swap
error) is logged once by the render thread, which then stops; the last
presented frame stays on screen and `Player::detach_video_surface` /
re-attach remain available. It does not silently switch transports, because
the failure belongs to a surface that the host may be about to discard.

## Detach without a window, and leaks

`Player::detach_video_surface(&mut self)` has no window, and gpui's
composition tree needs one to unregister a surface (`remove_surface`). The
detached surface is therefore hidden (`set_visible(false)`, which detaches the
subsurface / unmaps the child window) and parked as *stale*; the next attach
removes it from the tree, and a window's own destruction frees it regardless.
At most one stale surface exists per player at any time, so repeated
attach/detach cycles do not accumulate. The view detaches through the
window-holding variant while it still has the window (a stage that disappears
— a compacted view, a hidden stage — detaches on its next render).

How this was verified: repeated attach/detach cycling driven from the view
layer (toggling the stage ~40 times over 30 s) on X11, counting the player
window's children with `xwininfo -tree` after each cycle — the count returns
to its pre-attach value, i.e. every child window is destroyed on detach; and
the same cycling on Wayland observing the subsurface attach/detach logs.
Dropping a player mid-playback joins the render thread (no hang, no mpv
abort) and leaves at most the single hidden stale surface described above.

## Platform matrix

| Platform | Composition transport | Notes |
| --- | --- | --- |
| Linux / X11, XWayland | supported, default | child X11 window; EGL via `EGL_DEFAULT_DISPLAY`; vsync swap |
| Linux / Wayland | supported, default | desynchronized subsurface; `wl_display` from gpui's connection; `SwapInterval::DontWait` required |
| macOS | not implemented | negotiation downgrades to software at `Player::new`; `attach_video_surface` logs once and stays on software. No untested platform code is shipped. |
| Windows | not implemented | as macOS |

The GL path is deliberately Linux-only: no other backend's composition
surface, EGL/WGL/CGL interop or mpv render-context path has been written or
tested here, and the library does not claim support it cannot verify.
