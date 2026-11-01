# Media playback (`media` feature)

`woocraft` ships an optional player component on top of libmpv. The feature is
**off by default** because libmpv is a system library: enabling `media` links
`libmpv` and needs its development package at build time.

```toml
woocraft = { version = "0.6", features = ["media"] }
```

```sh
cargo run -p woocraft --features media --example player -- /path/to/file.mp4
cargo run -p woocraft --features media --example player -- --transport=software /path/to/file.mp4
cargo run -p woocraft --features media --example player -- --audio /path/to/song.flac
```

## One player, video as a mode

There is exactly one entity, [`Player`], and one view, [`PlayerView`]. Video is
a mode of the player (`PlayerOptions::video`), matching how mpv itself models
it (`vid`, `vo`), so an application that plays songs and films owns one player,
not one per format. `PlayerOptions::audio_only()` disables the video output and
the video track; `PlayerView` still renders, showing the track's title/artist
instead of a picture.

```rust
let player = cx.new(|cx| Player::new(cx, PlayerOptions::default()));
let view = cx.new(|cx| PlayerView::new(&player, cx));
player.update(cx, |player, _| player.load("https://example.com/clip.webm"))?;
```

## Capability coverage

Everything below is a typed method on `Player`; everything *not* below is still
reachable through the pass-through methods at the end of the table, which is
what keeps "mpv native features" complete instead of best-effort.

| Area | Covered by | mpv mechanism |
| --- | --- | --- |
| Load, enqueue, playlist navigation | `load`, `enqueue`, `playlist`, `set_playlist_position` | `loadfile`, `playlist-*` |
| Transport | `play`, `pause`, `toggle_pause`, `stop`, `seek_to`, `seek_by`, `frame_step`, `frame_back_step`, `set_ab_loop` | `seek`, `frame-step`, `ab-loop-*` |
| Volume / mute / speed | `set_volume`, `set_muted`, `set_speed`, `set_volume_max`, `set_pitch_correction` | `volume`, `mute`, `speed`, `volume-max`, `audio-pitch-correction` |
| Audio output | `set_audio_device`, `set_balance`, `set_audio_delay`, `set_replaygain`, `set_audio_track` | `audio-device`, `balance`, `audio-delay`, `replaygain`, `aid` |
| **Audio filters / equalizer** | `set_audio_filters`, `set_equalizer`, `clear_audio_filters` | `af` (`lavfi` filter graph) |
| **Subtitles** | `set_subtitle_track`, `add_subtitle`, `set_subtitle_visibility`, `set_subtitle_delay`, `set_subtitle_scale`, `set_subtitle_position`, `set_subtitle_style`, `set_subtitle_ass`, `set_secondary_subtitle` | `sid`, `sub-add`, `sub-*` |
| **Hardware decoding** | `set_hardware_decoding`, `set_hardware_decoding_codecs`, `PlayerOptions::hardware_decoding` | `hwdec`, `hwdec-codecs` |
| Video geometry | `set_aspect`, `set_rotation`, `set_deinterlace` | `video-aspect-override`, `video-rotate`, `deinterlace` |
| Tracks / chapters / tags | `tracks`, `chapters`, `metadata`, `PlayerState::{title, artist, album, video_size}` | `track-list/*`, `chapter-list/*`, `metadata/*` |
| State stream | `PlayerEvent`, `PlayerState`, observed properties | `observe_property`, event queue |
| Anything else | `command`, `command_string`, `set_property`, `get_property`, `mpv()` | `mpv_command*`, `mpv_set/get_property`, raw handle |

Subtitles and OSD are composited by mpv's own renderer into the frames we
receive, so ASS styling, secondary subtitles and styled positioning all work
without a separate text renderer in `woocraft`.

### Defaults chosen for a component library

- `hwdec=no`. The software video output downloads every frame anyway, and
  hardware paths are where decoder drivers differ most; opt in with
  `PlayerOptions::hardware_decoding(HwDec::AutoSafe)` when the target machines
  are known. `HwDec::AutoSafe` is the mode to reach for: it keeps mpv's
  known-bad driver list in play.
- `keep-open=yes`, `idle=yes`: the player outlives the file and reports EOF
  instead of resetting.
- `terminal=no`, `input-default-bindings=no`, `osc=no`: mpv does not read the
  terminal, grab keys or draw its own UI; input and chrome belong to the host.
- Any remaining option is reachable through `PlayerOptions::option` before the
  core initializes, and through `set_property` afterwards.

## Video transports: software and composition

Video reaches the screen through one of two transports. A player *requests*
the one it wants with `PlayerOptions::video_transport(..)` and reports what
actually runs through `Player::video_transport()`:

- **`VideoTransport::Software`** (the default outside Linux) asks mpv's
  software render API (`MPV_RENDER_API_TYPE_SW`, `rgb0`) for RGBA frames on a
  dedicated thread and paints them as gpui images. It works on every platform
  gpui supports, needs no GL/EGL/WGL context of our own and shares no
  textures across graphics APIs — but it costs a decode download and a texture
  upload per frame, so it suits embedded players, previews and audio-first
  applications rather than 4K playback. Frames render at the stage's size (the
  view requests resizes), starting from `PlayerOptions::video_size`.
- **`VideoTransport::Composition`** (the default on Linux) renders mpv
  straight into a gpui **composition surface** through the player's own
  OpenGL context: no CPU copy, no texture upload, the compositor shows the
  video. `PlayerView`'s stage attaches the surface at its bounds
  (`Player::attach_video_surface`) and paints nothing — the composited child
  window carries the picture, letterboxed by mpv — and resizes move and resize
  the GL viewport with the stage. See [`docs/media-gpu.md`](media-gpu.md) for
  the frame path, the ownership/thread model, the Wayland `SwapInterval`
  requirement, the fallback triggers and the platform matrix.

Failure is a first-class path, not a host concern: a composition request that
this platform refuses up front, or any step of the first attach that fails —
from a compositor without the needed extensions to an EGL context that will
not bind — is logged once through `tracing` at `warn` level with the failed
call and the platform error, the negotiated transport switches to software,
that output starts if it was not running, and playback continues.
`Player::attach_video_surface` therefore returns `Ok(())` on every such
fallback; `Err` is reserved for programming misuse (attaching a player with
video disabled). Both `attach` and `Player::detach_video_surface` are no-ops
on the software transport, and `detach` is idempotent.

The software path keeps the bounds it always had: every frame is a new
`RenderImage` pinned in the window's sprite atlas until the stage frees the
previous one (the view's element state does), so a long-running software
player holds at most the frame currently on screen.

## Threading

One thread pumps mpv's event queue, and it talks to the entity over
`async-channel` (no external async runtime, per the workspace rules); the
software transport adds one frame-rendering thread and the composition
transport one GL-presenting thread, each joined before the mpv core can be
destroyed. Dropping the `Player` stops all of them and terminates the core;
see `docs/media-gpu.md` for the full map.
