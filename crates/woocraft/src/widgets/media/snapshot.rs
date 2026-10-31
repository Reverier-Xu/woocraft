//! The owned read-model `PlayerView` hands to its slot closures.

use std::{ops::Deref, sync::Arc};

use gpui::{App, Entity, RenderImage};

use super::player::{Player, PlayerState, PlaylistEntry, Track};

/// A cheap, owned snapshot of what a [`Player`] is showing, taken once per
/// `PlayerView` render.
///
/// Why this exists: `PlayerView`'s slots ([`PlayerView::stage`],
/// [`PlayerView::transport`], [`PlayerView::controls`]) are host closures
/// called as `f(&PlayerSnapshot, &mut Window, &mut App)`. The snapshot owns
/// every render input — the whole `PlayerState` and the current video frame —
/// so a closure never has to capture the `Entity<Player>` it would otherwise
/// need to read through `cx`, and never borrows the entity across the context
/// either.
///
/// Keeping the snapshot cheap rules out eagerly copying `tracks()` or
/// `playlist()`: each is a burst of synchronous mpv property reads, and the
/// view re-renders on every position tick. They stay one call away through
/// the player handle the snapshot carries ([`PlayerSnapshot::tracks`],
/// [`PlayerSnapshot::playlist`]), read on demand so track menus highlight
/// from fresh data without taxing every frame.
pub struct PlayerSnapshot {
  player: Entity<Player>,
  state: PlayerState,
  frame: Option<Arc<RenderImage>>,
  video_enabled: bool,
}

impl PlayerSnapshot {
  /// Captures the player's state, latest video frame and video mode.
  pub fn new(player: &Entity<Player>, cx: &App) -> Self {
    let player_state = player.read(cx);
    Self {
      player: player.clone(),
      state: player_state.state().clone(),
      frame: player_state.video_image(),
      video_enabled: player_state.video_enabled(),
    }
  }

  /// The player this snapshot was taken from.
  pub fn player(&self) -> &Entity<Player> {
    &self.player
  }

  /// The state at snapshot time.
  pub fn state(&self) -> &PlayerState {
    &self.state
  }

  /// The latest decoded video frame, when the player produces them.
  pub fn frame(&self) -> Option<&Arc<RenderImage>> {
    self.frame.as_ref()
  }

  /// Whether video output is enabled (see [`super::PlayerOptions::video`]).
  pub fn video_enabled(&self) -> bool {
    self.video_enabled
  }

  /// The current entry's tracks, read from mpv on call. Track lists change
  /// rarely but go stale cheaply (selecting a track only flips a flag), so
  /// they are queried rather than cached in the snapshot.
  pub fn tracks(&self, cx: &App) -> Vec<Track> {
    self.player.read(cx).tracks()
  }

  /// The playlist, read from mpv on call; see [`Self::tracks`].
  pub fn playlist(&self, cx: &App) -> Vec<PlaylistEntry> {
    self.player.read(cx).playlist()
  }
}

/// Read the state fields directly off the snapshot (`snapshot.position`,
/// `snapshot.title`, …) — the common case for slot closures.
impl Deref for PlayerSnapshot {
  type Target = PlayerState;

  fn deref(&self) -> &Self::Target {
    &self.state
  }
}
