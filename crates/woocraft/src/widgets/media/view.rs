//! One player view for both audio and video.

use std::sync::Arc;

use gpui::{
  App, AppContext as _, Bounds, Context, Corners, Element, ElementId, Entity, GlobalElementId,
  InspectorElementId, IntoElement, LayoutId, ParentElement as _, Pixels, Render, RenderImage,
  Style, Styled as _, Subscription, Window, div, prelude::FluentBuilder as _, px, relative,
};

use super::{
  player::{Player, PlayerEvent, format_time},
  video::MIN_RENDER_SIZE,
};
use crate::{
  ActiveTheme as _, Button, Icon, IconName, Label, Slider, SliderEvent, SliderState, h_flex, v_flex,
};

const SPEEDS: [f64; 5] = [0.5, 1.0, 1.25, 1.5, 2.0];

/// Seek-slider changes whose target is closer than this to the current
/// position are ignored: a video frame lasts roughly 16-40 ms, so a seek below
/// this bound cannot reach a perceivably different position and only disrupts
/// decoding. This is also what stops a percentage rounded to the slider step
/// from seeking playback backwards by a frame.
const SEEK_EPSILON_SECS: f64 = 0.1;

/// Rendering of a [`Player`].
///
/// The same view serves an audio player and a video player: the stage shows the
/// player's video frames when it produces them and falls back to the track's
/// title and artist when it does not. `PlayerOptions::video(false)` therefore
/// only changes what the player decodes, not which component renders it.
pub struct PlayerView {
  player: Entity<Player>,
  seek: Entity<SliderState>,
  volume: Entity<SliderState>,
  /// Seek-slider changes still to be swallowed as echoes of player-driven
  /// writes; see [`Self::sync_seek`].
  pending_seek_echoes: usize,
  _subscriptions: Vec<Subscription>,
  show_stage: bool,
}

impl PlayerView {
  /// Builds a view bound to `player`.
  pub fn new(player: &Entity<Player>, cx: &mut Context<Self>) -> Self {
    let seek = cx.new(|_| SliderState::new().min(0.0).max(100.0).step(0.1));
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
        let SliderEvent::Change(value) = event;
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
        let SliderEvent::Change(value) = event;
        let volume = value.start() as f64;
        let player = this.player.clone();
        let _ = player.update(cx, |player, _| player.set_volume(volume));
      }),
    );

    let show_stage = player.read(cx).video_enabled();
    let volume_now = player.read(cx).state().volume as f32;
    let view = Self {
      player: player.clone(),
      seek,
      volume,
      pending_seek_echoes: 0,
      _subscriptions: subscriptions,
      show_stage,
    };
    view.volume.update(cx, |slider, cx| {
      slider.set_value(volume_now, cx);
    });
    view
  }

  /// Hides the stage, for a compact transport-only bar.
  pub fn compact(mut self) -> Self {
    self.show_stage = false;
    self
  }

  /// The player this view renders.
  pub fn player(&self) -> &Entity<Player> {
    &self.player
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

impl Render for PlayerView {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let (paused, muted, speed, position, duration, title, artist, has_video, image) = {
      let player = self.player.read(cx);
      let state = player.state();
      (
        state.paused,
        state.muted,
        state.speed,
        state.position,
        state.duration,
        state.title.clone(),
        state.artist.clone(),
        state.has_video,
        player.video_image(),
      )
    };
    let theme = cx.theme().clone();
    let wide = self.show_stage && image.is_some();

    let stage = wide.then(|| {
      div()
        .w_full()
        .flex_1()
        .relative()
        .overflow_hidden()
        .bg(theme.muted)
        .when_some(image.clone(), |this, image| {
          this.child(VideoStage::new(
            ("video-stage", self.player.entity_id().as_u64()),
            self.player.clone(),
            image,
          ))
        })
    });

    let placeholder = (!wide).then(|| {
      h_flex()
        .w_full()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .child(Icon::new(if has_video {
          IconName::Filmstrip
        } else {
          IconName::MusicNote2
        }))
        .child(
          v_flex()
            .flex_1()
            .child(Label::new(
              title.clone().unwrap_or_else(|| "Nothing playing".into()),
            ))
            .children(artist.clone().map(Label::new)),
        )
    });

    let play_button = {
      let player = self.player.clone();
      Button::new("player-play")
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
      Button::new("player-mute")
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
      Button::new("player-speed")
        .label(format!("{}x", (speed * 100.0).round() / 100.0))
        .on_click(move |_, _, cx| {
          let _ = player.update(cx, |player, _| player.set_speed(next_speed));
        })
    };

    let controls = h_flex()
      .w_full()
      .items_center()
      .gap_2()
      .px_3()
      .py_2()
      .bg(theme.card)
      .child(play_button)
      .child(Label::new(format_time(std::time::Duration::from_secs_f64(
        position.max(0.0),
      ))))
      .child(div().flex_1().child(Slider::new("player-seek", &self.seek)))
      .child(Label::new(format_time(std::time::Duration::from_secs_f64(
        duration.max(0.0),
      ))))
      .child(mute_button)
      .child(
        div()
          .w(px(96.0))
          .child(Slider::new("player-volume", &self.volume)),
      )
      .child(speed_button);

    v_flex()
      .size_full()
      .bg(theme.background)
      .children(stage)
      .children(placeholder)
      .child(controls)
  }
}

/// The video surface: the player's current frame, letterboxed into bounds.
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
  image: Arc<RenderImage>,
}

impl VideoStage {
  fn new(id: impl Into<ElementId>, player: Entity<Player>, image: Arc<RenderImage>) -> Self {
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
          let replaced = state
            .painted
            .take_if(|painted| !Arc::ptr_eq(painted, &self.image));
          state.painted = Some(self.image.clone());
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

    style.paint(bounds, window, cx, |window, _| {
      let image_size = self.image.size(0);
      let fit = contain(
        bounds,
        i32::from(image_size.width) as f32,
        i32::from(image_size.height) as f32,
      );
      if let Err(error) =
        window.paint_image(fit, fit, Corners::default(), self.image.clone(), 0, false)
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
