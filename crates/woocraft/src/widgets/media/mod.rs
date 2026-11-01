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
//! [`PlayerView`] is a container with two defaults — the stage and the
//! [`PlayerTransport`] bar — plus slots ([`PlayerView::stage`],
//! [`PlayerView::transport`], [`PlayerView::controls`]) for everything else.
//! The standard controls also exist standalone and are never wired in unless
//! the host asks for them: [`PlayerTransport`], [`PlayerTrackMenu`] and
//! [`PlayerPlaylist`].
//!
//! Video has two transports, negotiated by the player and reported through
//! [`Player::video_transport`]: [`VideoTransport::Software`] paints mpv's RGBA
//! frames as gpui images everywhere, and [`VideoTransport::Composition`]
//! (Linux default) renders mpv straight into a gpui composition surface
//! through GL with no CPU copy — see `docs/media-gpu.md`.
//!
//! The optional `media` feature is off by default because libmpv is a system
//! library: enabling it links `libmpv` and requires it (and its development
//! package) at build time.

#[cfg(target_os = "linux")]
mod gpu;
mod player;
mod playlist;
mod snapshot;
mod track_menu;
mod transport;
mod video;
mod view;

pub use player::{
  Chapter, Player, PlayerEvent, PlayerOptions, PlayerState, PlaylistEntry, PropertyValue, Track,
  TrackKind, VideoTransport,
};
pub use playlist::PlayerPlaylist;
pub use snapshot::PlayerSnapshot;
pub use track_menu::PlayerTrackMenu;
pub use transport::PlayerTransport;
pub use video::{VideoFrame, VideoOutput};
pub use view::PlayerView;
