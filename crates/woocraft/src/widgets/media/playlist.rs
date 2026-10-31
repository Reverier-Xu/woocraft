//! The player's playlist as a scrollable list.

use gpui::{
  App, Div, ElementId, Entity, InteractiveElement as _, IntoElement, ParentElement as _,
  RenderOnce, SharedString, Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled,
  Window, div, prelude::FluentBuilder as _,
};

use super::player::{Player, PlaylistEntry};
use crate::{
  ActiveTheme as _, Icon, IconName, Label, ScrollableElement, Sizable, Size, StyleSized, StyledExt,
  TableThemeExt, Theme, h_flex, v_flex,
};

/// Width of the reserved leading marker slot of each row, in ems of the
/// playlist's size tier: the footprint of one tier-sized icon, matching the
/// check slot `ListItem` reserves so row text never shifts on playback
/// changes.
const NOW_PLAYING_MARKER_EM: f32 = 1.25;

/// The player's playlist, read live from [`Player::playlist`].
///
/// A standalone control: render it on its own, or in a
/// [`PlayerView`](super::PlayerView) through
/// [`PlayerView::controls`](super::PlayerView::controls); the view never
/// renders it on its own. Rows use the shared list-row metrics and
/// hover/selected treatment; clicking a row jumps the player to it
/// (`set_playlist_position`).
///
/// The list fills the remaining space of a flex parent so the built-in
/// scrollbar has a bounded viewport; hosts override height through the usual
/// `Styled` refinements.
#[derive(IntoElement)]
pub struct PlayerPlaylist {
  id: ElementId,
  player: Entity<Player>,
  size: Size,
  style: StyleRefinement,
}

impl PlayerPlaylist {
  /// Builds a playlist for `player`.
  pub fn new(player: &Entity<Player>) -> Self {
    Self {
      id: ("player-playlist", player.entity_id().as_u64()).into(),
      player: player.clone(),
      size: Size::default(),
      style: StyleRefinement::default(),
    }
  }
}

impl_sizable!(PlayerPlaylist);
impl_styled!(PlayerPlaylist);

impl RenderOnce for PlayerPlaylist {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let size = self.size;
    let entries = self.player.read(cx).playlist();
    let player = self.player;
    let player_id = player.entity_id().as_u64();

    let rows = v_flex().w_full().children(
      entries
        .iter()
        .map(|entry| playlist_row(entry, &player, player_id, size, &theme)),
    );

    v_flex()
      .id(self.id)
      .w_full()
      .min_h_0()
      .flex_1()
      .refine_style(&self.style)
      .child(rows.overflow_y_scrollbar())
  }
}

/// One playlist row: `list_size` metrics, the hover/selected treatment shared
/// with `ListItem` and table rows, and a reserved leading slot for the
/// now-playing marker so the label never shifts.
fn playlist_row(
  entry: &PlaylistEntry, player: &Entity<Player>, player_id: u64, size: Size, theme: &Theme,
) -> Stateful<Div> {
  // Same player-scoped row key packing `List` uses, so two playlists in one
  // window never share element ids.
  let row_id = (player_id << 32) | (entry.index as u64 & 0xFFFF_FFFF);
  let index = entry.index;
  // The title leads when mpv resolved one; the filename then reads as
  // secondary information — muted colour at the row's own tier.
  let label: SharedString = entry
    .title
    .clone()
    .map(SharedString::from)
    .unwrap_or_else(|| entry.filename.clone().into());
  let detail = entry
    .title
    .as_ref()
    .map(|_| SharedString::from(entry.filename.clone()));
  let player = player.clone();

  div()
    .id(("player-playlist-entry", row_id))
    .w_full()
    .list_size(size)
    .items_center()
    .component_gap(size)
    .component_rounded(size)
    .cursor_pointer()
    .when(entry.current, |this| this.bg(theme.table_active()))
    .when(!entry.current, |this| {
      this
        .hover(|row| row.bg(theme.table_hover()))
        .active(|row| row.bg(theme.table_active()))
    })
    .on_click(move |_, _, cx| {
      let _ = player.read(cx).set_playlist_position(index);
    })
    .child(
      div()
        .flex_none()
        .w(size.em(NOW_PLAYING_MARKER_EM))
        .items_center()
        .justify_center()
        .when(entry.current, |this| {
          this.child(
            Icon::new(IconName::Play)
              .with_size(size)
              .text_color(theme.primary),
          )
        }),
    )
    .child(
      h_flex()
        .min_w_0()
        .flex_1()
        .items_center()
        .component_gap(size)
        .child(div().min_w_0().flex_1().truncate().child(Label::new(label)))
        .when_some(detail, |this, detail| {
          this.child(
            div()
              .min_w_0()
              .truncate()
              .text_color(theme.muted_foreground)
              .child(Label::new(detail)),
          )
        }),
    )
}
