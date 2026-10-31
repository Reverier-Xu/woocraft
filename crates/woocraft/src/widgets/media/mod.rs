//! Audio/video playback on top of libmpv.
//!
//! One [`Player`] instance drives one mpv core: video is a mode of that player
//! ([`PlayerOptions::video`]), not a different component, so an application
//! that plays both audio and video owns exactly one player and renders it with
//! [`PlayerView`]. Everything mpv can do stays reachable through
//! [`Player::command`], [`Player::command_string`], [`Player::set_property`],
//! [`Player::get_property`] and the playlist/track/chapter readers that sit on
//! top of them.
//!
//! The optional `media` feature is off by default because libmpv is a system
//! library: enabling it links `libmpv` and requires it (and its development
//! package) at build time.

mod player;
mod video;
mod view;

pub use player::{
  Chapter, Player, PlayerEvent, PlayerOptions, PlayerState, PlaylistEntry, PropertyValue, Track,
  TrackKind,
};
pub use video::{VideoFrame, VideoOutput};
pub use view::PlayerView;
