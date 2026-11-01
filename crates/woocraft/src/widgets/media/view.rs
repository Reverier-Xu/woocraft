//! One player view for both audio and video: a container with slots.
//!
//! The view owns exactly two defaults — the stage (the player's video frames,
//! or the track title/artist placeholder) and the standard transport bar
//! ([`PlayerTransport`]) — and leaves everything else to the host through
//! [`PlayerView::stage`], [`PlayerView::transport`] and
//! [`PlayerView::controls`]. The standard controls ([`PlayerTransport`],
//! [`PlayerTrackMenu`], [`PlayerPlaylist`]) render without this view at all.

use std::sync::Arc;

use gpui::{
  AnyElement, App, AppContext as _, Bounds, Context, Corners, Element, ElementId, Entity,
  GlobalElementId, InspectorElementId, IntoElement, LayoutId, ParentElement as _, Pixels, Render,
  RenderImage, Style, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _,
  relative,
};

use super::{
  player::{Player, PlayerEvent, VideoTransport},
  snapshot::PlayerSnapshot,
  transport::PlayerTransport,
  video::MIN_RENDER_SIZE,
};
use crate::{
  ActiveTheme as _, Icon, IconLabel, IconName, Sizable, Size, StyleSized, Theme, h_flex, v_flex,
};

/// What a [`PlayerView`] slot builds: an element from a fresh
/// [`PlayerSnapshot`]. The snapshot owns the player's render inputs, so slot
/// closures never have to capture or borrow the player entity across `cx`.
type PlayerSlot = Box<dyn Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement>;

/// Rendering of a [`Player`]: the stage plus the transport bar, both
/// replaceable.
///
/// The same view serves an audio player and a video player: the stage shows the
/// player's video frames when it produces them and falls back to the track's
/// title and artist when it does not. `PlayerOptions::video(false)` therefore
/// only changes what the player decodes, not which component renders it.
///
/// One [`Size`] tier threads through everything the view renders itself and
/// into the default transport, so `.small()` yields a fully small player.
pub struct PlayerView {
  player: Entity<Player>,
  transport: Entity<PlayerTransport>,
  stage_slot: Option<PlayerSlot>,
  transport_slot: Option<PlayerSlot>,
  controls_slots: Vec<PlayerSlot>,
  show_stage: bool,
  size: Size,
  _subscriptions: Vec<Subscription>,
}

impl PlayerView {
  /// Builds a view bound to `player`: the default stage and the standard
  /// [`PlayerTransport`] bar, nothing else.
  pub fn new(player: &Entity<Player>, cx: &mut Context<Self>) -> Self {
    let transport = cx.new(|cx| PlayerTransport::new(player, cx));
    // Any player report re-renders the view, so slot content (a custom stage,
    // custom controls) follows position ticks and track changes without each
    // host wiring its own subscription.
    let subscriptions = vec![
      cx.subscribe(player, |_: &mut Self, _, _: &PlayerEvent, cx| {
        cx.notify();
      }),
    ];
    // A view the host removes must not leave its composition surface mapped
    // over whatever replaces it: detach on this entity's release, which is
    // the only guaranteed call after the last render. No window exists
    // there, so the detach parks the surface as stale (see
    // `docs/media-gpu.md`); the player is still alive at release — this
    // view's own handle keeps it so.
    cx.on_release(|this: &mut Self, cx: &mut App| {
      this
        .player
        .update(cx, |player, _| player.detach_video_surface());
    })
    .detach();

    Self {
      player: player.clone(),
      transport,
      stage_slot: None,
      transport_slot: None,
      controls_slots: Vec::new(),
      show_stage: player.read(cx).video_enabled(),
      size: Size::default(),
      _subscriptions: subscriptions,
    }
  }

  /// Hides the stage — the default one or a custom one — for a compact
  /// transport-only bar.
  pub fn compact(mut self) -> Self {
    self.show_stage = false;
    self
  }

  /// Replaces the stage — the area above the transport that shows the video
  /// frames, or the track placeholder when there are none — with a
  /// caller-built element. The closure receives a fresh [`PlayerSnapshot`]
  /// every render. [`Self::compact`] still hides the stage, custom or not.
  pub fn stage(
    mut self, f: impl Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.stage_slot = Some(Box::new(f));
    self
  }

  /// Replaces the built-in transport bar entirely; none of the default bar
  /// remains. Hosts composing their own bar usually start from a standalone
  /// [`PlayerTransport`] plus their own controls.
  pub fn transport(
    mut self, f: impl Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.transport_slot = Some(Box::new(f));
    self
  }

  /// Appends caller-provided controls below the transport, keeping the
  /// transport itself. Repeated calls append in order. This is the slot the
  /// standalone [`PlayerTrackMenu`] or [`PlayerPlaylist`] go into; the view
  /// itself never renders those on its own.
  pub fn controls(
    mut self, f: impl Fn(&PlayerSnapshot, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.controls_slots.push(Box::new(f));
    self
  }

  /// The player this view renders.
  pub fn player(&self) -> &Entity<Player> {
    &self.player
  }
}

impl_sizable!(PlayerView);

impl Render for PlayerView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let snapshot = PlayerSnapshot::new(&self.player, cx);
    let theme = cx.theme().clone();
    let size = self.size;
    let transport = self.player.read(cx).video_transport();

    let stage = self.show_stage.then(|| {
      if let Some(build) = self.stage_slot.as_ref() {
        build(&snapshot, window, cx)
      } else {
        default_stage(&snapshot, &self.player, &theme, size, transport)
      }
    });
    // A stage that is not rendered must release its composition surface while
    // the window is still at hand: `compact()` views and stages hidden behind
    // a custom slot detach instead of leaving a surface pinned on screen.
    if stage.is_none() && self.player.read(cx).composition_attached() {
      self.player.update(cx, |player, _| {
        player.detach_video_surface_in_window(window)
      });
    }

    let transport = if let Some(build) = self.transport_slot.as_ref() {
      build(&snapshot, window, cx)
    } else {
      // Thread the view's tier into the default bar every render, so a host
      // resizing the view resizes the bar with it.
      self
        .transport
        .update(cx, |transport, _| transport.set_size(size));
      self.transport.clone().into_any_element()
    };

    let controls = self
      .controls_slots
      .iter()
      .map(|build| build(&snapshot, window, cx))
      .collect::<Vec<_>>();

    v_flex()
      .size_full()
      .bg(theme.background)
      .children(stage)
      .child(transport)
      .children(controls)
  }
}

/// The default stage: the player's current video frame when it produces one,
/// else the track's title and artist. [`PlayerView::stage`] replaces this.
///
/// The placeholder threads the view's tier through its container metrics,
/// icon and labels; the video surface itself is unsized — it letterboxes
/// whatever the player renders into the space it is given.
fn default_stage(
  snapshot: &PlayerSnapshot, player: &Entity<Player>, theme: &Theme, size: Size,
  transport: VideoTransport,
) -> AnyElement {
  match transport {
    // Composition: the stage only hosts the surface; the composited child
    // window carries the picture, so there is no frame to paint here — not
    // even before the first attach, which `VideoStage` performs itself.
    VideoTransport::Composition => video_stage_container(theme)
      .child(VideoStage::new(
        ("video-stage", player.entity_id().as_u64()),
        player.clone(),
        None,
      ))
      .into_any_element(),
    VideoTransport::Software => {
      if let Some(image) = snapshot.frame() {
        video_stage_container(theme)
          .child(VideoStage::new(
            ("video-stage", player.entity_id().as_u64()),
            player.clone(),
            Some(image.clone()),
          ))
          .into_any_element()
      } else {
        placeholder_stage(snapshot, player, theme, size)
      }
    }
  }
}

/// The box the video surface lives in, shared by both transports.
fn video_stage_container(theme: &Theme) -> gpui::Div {
  div()
    .w_full()
    .flex_1()
    .relative()
    .overflow_hidden()
    .bg(theme.muted)
}

/// The track placeholder: icon plus title and artist, for players without a
/// picture to show.
fn placeholder_stage(
  snapshot: &PlayerSnapshot, player: &Entity<Player>, theme: &Theme, size: Size,
) -> AnyElement {
  let id = player.entity_id().as_u64();
  h_flex()
    .w_full()
    .items_center()
    .container_size(size)
    .container_gap(size)
    .child(
      Icon::new(if snapshot.has_video {
        IconName::Filmstrip
      } else {
        IconName::MusicNote2
      })
      .with_size(size),
    )
    .child(
      v_flex()
        .min_w_0()
        .flex_1()
        .container_gap_y(size)
        .child(
          IconLabel::new(("player-stage-title", id))
            .with_size(size)
            .label(
              snapshot
                .title
                .clone()
                .unwrap_or_else(|| "Nothing playing".into()),
            ),
        )
        // The artist reads as secondary information: muted colour at the
        // same tier, never a smaller font.
        .when_some(snapshot.artist.clone(), |this, artist| {
          this.child(
            IconLabel::new(("player-stage-artist", id))
              .with_size(size)
              .label(artist)
              .text_color(theme.muted_foreground),
          )
        }),
    )
    .into_any_element()
}

/// The video surface: the player's current frame letterboxed into bounds, or
/// — for the composition transport — the host that keeps a GL surface
/// attached at this element's bounds and paints nothing.
///
/// This is an element rather than a plain `canvas` because painting video
/// needs bookkeeping that survives a frame: gpui keeps every painted image in
/// the window's sprite atlas until it is removed explicitly, and every decoded
/// frame is a new [`RenderImage`] with a fresh id. The element state therefore
/// remembers the image it painted last and frees it from the atlas once a
/// newer frame replaces it, so playing video grows the atlas by at most the
/// frame currently on screen.
struct VideoStage {
  id: ElementId,
  player: Entity<Player>,
  /// The frame to paint on the software transport; `None` for the
  /// composition host, which paints nothing.
  image: Option<Arc<RenderImage>>,
}

impl VideoStage {
  fn new(
    id: impl Into<ElementId>, player: Entity<Player>, image: Option<Arc<RenderImage>>,
  ) -> Self {
    Self {
      id: id.into(),
      player,
      image,
    }
  }
}

/// Per-stage bookkeeping that outlives a single frame.
#[derive(Default)]
struct VideoStageState {
  /// The image the sprite atlas currently holds for this stage.
  painted: Option<Arc<RenderImage>>,
  /// The last render size requested from the player, in device pixels, or
  /// `None` while the stage has never been large enough to render at.
  requested_size: Option<(u32, u32)>,
  /// The last element bounds a composition surface was attached at, or
  /// `None` while the stage has never attached one.
  attached_bounds: Option<Bounds<Pixels>>,
}

impl IntoElement for VideoStage {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for VideoStage {
  type RequestLayoutState = Style;
  type PrepaintState = ();

  fn id(&self) -> Option<ElementId> {
    Some(self.id.clone())
  }

  fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window,
    cx: &mut App,
  ) -> (LayoutId, Style) {
    let mut style = Style::default();
    style.size.width = relative(1.).into();
    style.size.height = relative(1.).into();
    (window.request_layout(style.clone(), [], cx), style)
  }

  fn prepaint(
    &mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>,
    _: &mut Style, _: &mut Window, _: &mut App,
  ) {
  }

  fn paint(
    &mut self, global_id: Option<&GlobalElementId>, _: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>, style: &mut Style, _: &mut (), window: &mut Window, cx: &mut App,
  ) {
    // The transport decides what this stage is: a passive host for the GL
    // surface, or the gpui-image painter of the software frames.
    if self.player.read(cx).video_transport() == VideoTransport::Composition {
      self.paint_surface_host(global_id, bounds, window, cx);
      return;
    }
    self.paint_image(global_id, bounds, style, window, cx);
  }
}

impl VideoStage {
  /// The composition stage: keeps the player's video surface attached at this
  /// element's bounds and paints nothing — the composition child surface
  /// shows the video above gpui's scene, so anything painted here would be
  /// hidden behind it.
  fn paint_surface_host(
    &mut self, global_id: Option<&GlobalElementId>, bounds: Bounds<Pixels>, window: &mut Window,
    cx: &mut App,
  ) {
    let attach = match global_id {
      Some(global_id) => {
        window.with_element_state(global_id, |state: Option<VideoStageState>, _| {
          let mut state = state.unwrap_or_default();
          // Attach once, then only when the stage moved or resized; every other
          // paint leaves the pipeline exactly as it is. A stage below one pixel
          // per axis still attaches — the player clamps the surface to a
          // nonzero drawable — because a collapsed stage re-expanding must find
          // the pipeline alive.
          let changed = state.attached_bounds.as_ref() != Some(&bounds);
          if changed {
            state.attached_bounds = Some(bounds);
          }
          (changed, state)
        })
      }
      // Without an id there is no state slot to remember bounds by: attach
      // every paint rather than track them. `VideoStage::new` always assigns
      // an id, so this only guards foreign embedding.
      None => true,
    };
    if attach {
      self.player.update(cx, |player, _| {
        // Every failure inside — including the fallback to the software
        // transport — is logged once there and returns `Ok`; the only `Err`
        // is programming misuse, which the view surfaces as a warning
        // instead of a panic in paint.
        if let Err(error) = player.attach_video_surface(window, bounds) {
          tracing::warn!(%error, "woocraft: attaching the video surface failed");
        }
      });
    }
  }

  /// The software stage: paints the player's current frame, letterboxed into
  /// bounds, and keeps the atlas and render size in step with it.
  fn paint_image(
    &mut self, global_id: Option<&GlobalElementId>, bounds: Bounds<Pixels>, style: &mut Style,
    window: &mut Window, cx: &mut App,
  ) {
    let stage_size = device_size(bounds, window.scale_factor());
    // `None` while the stage is below the minimum render span: a collapsed
    // stage must not have its request clamped up to a 2x2 render.
    let request = render_size(stage_size);
    let (replaced, resized) = match global_id {
      Some(global_id) => {
        window.with_element_state(global_id, |state: Option<VideoStageState>, _| {
          let mut state = state.unwrap_or_default();
          // `Arc::ptr_eq` leaves the painted image alone while the player has
          // not produced a newer frame, so repaints resize, hover or otherwise
          // free nothing.
          let replaced = state.painted.take_if(|painted| {
            !self
              .image
              .as_ref()
              .is_some_and(|image| Arc::ptr_eq(painted, image))
          });
          state.painted = self.image.clone();
          // Requests only exist for renderable sizes, and the last one is
          // kept otherwise, so a stage that collapses and re-expands to the
          // same size does not re-request it.
          let resized = request.is_some() && state.requested_size != request;
          if resized {
            state.requested_size = request;
          }
          ((replaced, resized), state)
        })
      }
      // Without an id there is no state slot for the bookkeeping: paint the
      // frame as-is and skip the atlas free and the resize request rather
      // than panicking mid-paint. `VideoStage::new` always assigns an id, so
      // this arm only guards foreign embedding.
      None => (None, false),
    };

    // Free the previous frame before painting the next one: during an upscale
    // this releases the atlas space the new frame is about to need.
    if let Some(previous) = replaced
      && let Err(error) = window.drop_image(previous)
    {
      tracing::warn!(%error, "woocraft: failed to free the previous video frame");
    }
    // The render size follows the stage, so decoded frames match the view
    // instead of staying at `PlayerOptions::video_size`. Sending only on
    // change keeps the request off the steady-state paint path, and the send
    // merely queues a channel write — it cannot invalidate this frame.
    if let Some(size) = request.filter(|_| resized) {
      self.player.read(cx).set_video_size(size);
    }

    // No image to paint (the stage was built as a composition host and the
    // transport fell back between render and paint): leave the stage empty;
    // the next render builds it from the frames that follow.
    let Some(image) = self.image.clone() else {
      return;
    };
    style.paint(bounds, window, cx, |window, _| {
      let image_size = image.size(0);
      let fit = contain(
        bounds,
        i32::from(image_size.width) as f32,
        i32::from(image_size.height) as f32,
      );
      if let Err(error) = window.paint_image(fit, fit, Corners::default(), image.clone(), 0, false)
      {
        tracing::warn!(%error, "woocraft: failed to paint the video frame");
      }
    });
  }
}

/// The stage bounds in device pixels: the resolution frames are rendered at.
fn device_size(bounds: Bounds<Pixels>, scale_factor: f32) -> (u32, u32) {
  let width: f32 = bounds.size.width.into();
  let height: f32 = bounds.size.height.into();
  (
    (width * scale_factor).round().max(0.0) as u32,
    (height * scale_factor).round().max(0.0) as u32,
  )
}

/// The render size to request for a stage of `size` device pixels.
///
/// `None` when either axis is below the video output's minimum render span:
/// the output clamps requests up to that minimum, so a collapsed stage asking
/// for (0, 0) would force a 2x2 render. Such a stage sends no request at all
/// and the last valid size stays in effect on the render thread.
fn render_size(size: (u32, u32)) -> Option<(u32, u32)> {
  (size.0 >= MIN_RENDER_SIZE && size.1 >= MIN_RENDER_SIZE).then_some(size)
}

/// Largest rect with the image's aspect ratio that fits inside `bounds`.
fn contain(bounds: Bounds<Pixels>, image_width: f32, image_height: f32) -> Bounds<Pixels> {
  let aspect = image_width / image_height.max(1.0);
  let available_width: f32 = bounds.size.width.into();
  let available_height: f32 = bounds.size.height.into();
  let (width, height) = if available_width / available_height.max(1.0) > aspect {
    (available_height * aspect, available_height)
  } else {
    (available_width, available_width / aspect)
  };
  let origin_x: f32 = bounds.origin.x.into();
  let origin_y: f32 = bounds.origin.y.into();
  Bounds::new(
    gpui::point(
      gpui::px(origin_x + (available_width - width) / 2.0),
      gpui::px(origin_y + (available_height - height) / 2.0),
    ),
    gpui::size(gpui::px(width), gpui::px(height)),
  )
}

#[cfg(test)]
mod tests {
  use gpui::px;

  use super::*;

  #[test]
  fn device_size_scales_and_rounds_the_stage_bounds() {
    let bounds = Bounds::new(
      gpui::point(px(0.0), px(0.0)),
      gpui::size(px(100.5), px(50.4)),
    );
    assert_eq!(device_size(bounds, 1.0), (101, 50));
    assert_eq!(device_size(bounds, 2.0), (201, 101));
    // A collapsed stage measures zero device pixels rather than a negative
    // size, and asks for nothing (see `render_size`).
    let collapsed = Bounds::new(gpui::point(px(0.0), px(0.0)), gpui::size(px(0.0), px(0.0)));
    assert_eq!(device_size(collapsed, 1.0), (0, 0));
  }

  #[test]
  fn stages_below_the_minimum_render_span_send_no_size_request() {
    assert_eq!(render_size((0, 0)), None);
    assert_eq!(render_size((1280, 0)), None);
    assert_eq!(render_size((1, 720)), None);
    assert_eq!(render_size((2, 2)), Some((2, 2)));
    assert_eq!(render_size((1280, 720)), Some((1280, 720)));
  }
}
