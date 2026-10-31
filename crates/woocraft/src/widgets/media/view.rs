//! One player view for both audio and video.

use std::sync::Arc;

use gpui::{
  AppContext as _, Bounds, Context, Corners, Entity, IntoElement, ParentElement as _, Pixels,
  Render, RenderImage, Styled as _, Subscription, Window, canvas, div, prelude::FluentBuilder as _,
  px,
};

use super::player::{Player, PlayerEvent, format_time};
use crate::{
  ActiveTheme as _, Button, Icon, IconName, Label, Slider, SliderEvent, SliderState, h_flex, v_flex,
};

const SPEEDS: [f64; 5] = [0.5, 1.0, 1.25, 1.5, 2.0];

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
          let percentage = (position / duration * 100.0) as f32;
          this.seek.update(cx, |slider, cx| {
            slider.set_value(percentage, cx);
          });
        }
        if let PlayerEvent::FileLoaded = event {
          this.seek.update(cx, |slider, cx| slider.set_value(0.0, cx));
        }
      }),
    );

    subscriptions.push(
      cx.subscribe(&seek, |this: &mut Self, _, event: &SliderEvent, cx| {
        let SliderEvent::Change(value) = event;
        let duration = this.player.read(cx).state().duration;
        if duration > 0.0 {
          let target = duration * (value.start() as f64) / 100.0;
          let player = this.player.clone();
          let _ = player.update(cx, |player, _| player.seek_to(target));
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
        .when_some(image.clone(), |this, image| this.child(video_stage(image)))
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

fn video_stage(image: Arc<RenderImage>) -> impl IntoElement {
  div()
    .absolute()
    .inset_0()
    .flex()
    .items_center()
    .justify_center()
    .child(
      canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
          let image_size = image.size(0);
          let width = i32::from(image_size.width) as f32;
          let height = i32::from(image_size.height) as f32;
          let fit = contain(bounds, width, height);
          let _ = window.paint_image(fit, fit, Corners::default(), image.clone(), 0, false);
        },
      )
      .size_full(),
    )
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
