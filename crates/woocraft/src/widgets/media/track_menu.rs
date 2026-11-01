//! Audio and subtitle track selection for a [`Player`].

use gpui::{
  App, ClickEvent, Div, ElementId, Entity, InteractiveElement as _, IntoElement,
  ParentElement as _, RenderOnce, SharedString, Stateful, StatefulInteractiveElement as _,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use super::player::{Player, Track, TrackKind};
use crate::{
  ActiveTheme as _, Icon, IconLabel, IconName, Label, Sizable, Size, StyleSized, StyledExt,
  TableThemeExt, Theme, h_flex, translate_woocraft, v_flex,
};

/// Width of the reserved leading marker slot of each row, in ems of the
/// menu's size tier: the footprint of one tier-sized icon, matching the check
/// slot `ListItem` reserves so row text never shifts on selection.
const SELECTED_MARKER_EM: f32 = 1.25;

/// Audio and subtitle track selection, read live from [`Player::tracks`].
///
/// A standalone control: render it anywhere — on its own, or in a
/// [`PlayerView`](super::PlayerView) through
/// [`PlayerView::controls`](super::PlayerView::controls); the view never
/// renders it on its own. It carries no surface of its own (no background,
/// no border), so it fits whatever panel or popover the host puts it in.
///
/// Each section lists the entry's tracks of that kind plus an "Off" entry
/// (`set_audio_track(None)` / `set_subtitle_track(None)`); the selected track
/// is highlighted with the shared list-row treatment. One [`Size`] tier
/// threads through headers, rows and icons; the menu sets no font size
/// itself.
#[derive(IntoElement)]
pub struct PlayerTrackMenu {
  id: ElementId,
  player: Entity<Player>,
  size: Size,
  style: StyleRefinement,
}

impl PlayerTrackMenu {
  /// Builds a track menu for `player`.
  pub fn new(player: &Entity<Player>) -> Self {
    Self {
      id: ("player-track-menu", player.entity_id().as_u64()).into(),
      player: player.clone(),
      size: Size::default(),
      style: StyleRefinement::default(),
    }
  }
}

impl_sizable!(PlayerTrackMenu);
impl_styled!(PlayerTrackMenu);

impl RenderOnce for PlayerTrackMenu {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme().clone();
    let size = self.size;
    let tracks = self.player.read(cx).tracks();

    let audio: Vec<&Track> = tracks
      .iter()
      .filter(|t| t.kind == TrackKind::Audio)
      .collect();
    let subtitles: Vec<&Track> = tracks
      .iter()
      .filter(|t| t.kind == TrackKind::Subtitle)
      .collect();
    let audio_selected = audio.iter().copied().find(|t| t.selected).map(|t| t.id);
    let subtitle_selected = subtitles.iter().copied().find(|t| t.selected).map(|t| t.id);

    let player = self.player;
    let id = player.entity_id().as_u64();

    v_flex()
      .id(self.id)
      .w_full()
      .container_gap_y(size)
      .refine_style(&self.style)
      .child(
        v_flex()
          .w_full()
          .container_gap_y(size)
          .child(section_header(
            ("player-tracks-audio-header", id),
            IconName::Speaker2,
            translate_woocraft("media.audio").into(),
            size,
            &theme,
          ))
          .child(track_row(
            ("player-tracks-audio-off", id),
            size,
            &theme,
            audio_selected.is_none(),
            translate_woocraft("media.off").into(),
            None,
            {
              let player = player.clone();
              move |_, _, cx| {
                let _ = player.read(cx).set_audio_track(None);
              }
            },
          ))
          .children(audio.iter().map(|track| {
            track_row(
              ("player-tracks-audio", row_key(id, track.id)),
              size,
              &theme,
              audio_selected == Some(track.id),
              track_label(track),
              track_detail(track),
              {
                let player = player.clone();
                let track_id = track.id;
                move |_, _, cx| {
                  let _ = player.read(cx).set_audio_track(Some(track_id));
                }
              },
            )
          })),
      )
      .child(
        v_flex()
          .w_full()
          .container_gap_y(size)
          .child(section_header(
            ("player-tracks-subtitle-header", id),
            IconName::ClosedCaption,
            translate_woocraft("media.subtitles").into(),
            size,
            &theme,
          ))
          .child(track_row(
            ("player-tracks-subtitle-off", id),
            size,
            &theme,
            subtitle_selected.is_none(),
            translate_woocraft("media.off").into(),
            None,
            {
              let player = player.clone();
              move |_, _, cx| {
                let _ = player.read(cx).set_subtitle_track(None);
              }
            },
          ))
          .children(subtitles.iter().map(|track| {
            track_row(
              ("player-tracks-subtitle", row_key(id, track.id)),
              size,
              &theme,
              subtitle_selected == Some(track.id),
              track_label(track),
              track_detail(track),
              {
                let player = player.clone();
                let track_id = track.id;
                move |_, _, cx| {
                  let _ = player.read(cx).set_subtitle_track(Some(track_id));
                }
              },
            )
          })),
      )
  }
}

/// Packs a player-scoped row key, so two menus in one window never share
/// element ids (same packing `List` uses for its rows).
fn row_key(player_id: u64, track_id: i64) -> u64 {
  (player_id << 32) | (track_id as u64 & 0xFFFF_FFFF)
}

/// The label a track row shows: its title, else its language, else its
/// position among tracks of its kind.
fn track_label(track: &Track) -> SharedString {
  track
    .title
    .clone()
    .or_else(|| track.lang.clone())
    .map(SharedString::from)
    .unwrap_or_else(|| format!("{} {}", translate_woocraft("media.track"), track.id).into())
}

/// The secondary text a track row shows: the language when the title is the
/// label, else the codec.
fn track_detail(track: &Track) -> Option<SharedString> {
  if track.title.is_some() {
    track.lang.clone().map(SharedString::from)
  } else {
    track.codec.clone().map(SharedString::from)
  }
}

/// A section header: secondary information, so muted colour at the menu's
/// own tier.
fn section_header(
  element_id: impl Into<ElementId>, icon: IconName, label: SharedString, size: Size, theme: &Theme,
) -> IconLabel {
  IconLabel::new(element_id)
    .with_size(size)
    .icon(Icon::new(icon))
    .label(label)
    .text_color(theme.muted_foreground)
}

/// One selectable row: `list_size` metrics, the hover/selected treatment
/// shared with `ListItem` and table rows (`table_hover`/`table_active`), and
/// a reserved leading slot for the selected marker so the label never shifts.
fn track_row(
  element_id: impl Into<ElementId>, size: Size, theme: &Theme, selected: bool, label: SharedString,
  detail: Option<SharedString>, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
  div()
    .id(element_id)
    .w_full()
    .list_size(size)
    .list_px(size)
    .flex()
    .items_center()
    .component_gap(size)
    .component_rounded(size)
    .cursor_pointer()
    .when(selected, |this| this.bg(theme.table_active()))
    .when(!selected, |this| {
      this
        .hover(|row| row.bg(theme.table_hover()))
        .active(|row| row.bg(theme.table_active()))
    })
    .on_click(on_click)
    .child(
      div()
        .flex_none()
        .w(size.em(SELECTED_MARKER_EM))
        .items_center()
        .justify_center()
        .when(selected, |this| {
          this.child(
            Icon::new(IconName::Checkmark)
              .with_size(size)
              .text_color(theme.muted_foreground),
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
