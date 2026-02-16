use gpui::{AnyElement, App, Context, IntoElement, ParentElement as _, Styled as _, Task, Window};

use super::{ListState, loading::Loading};
use crate::{ActiveTheme as _, Icon, IconName, IndexPath, Selectable, Sizable, h_flex};

/// A delegate for the List.
#[allow(unused)]
pub trait ListDelegate: Sized + 'static {
  type Item: Selectable + IntoElement;

  /// Called when query input changes.
  fn perform_search(
    &mut self, _query: &str, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) -> Task<()> {
    Task::ready(())
  }

  /// Return the number of sections in the list, default is 1.
  fn sections_count(&self, _cx: &App) -> usize {
    1
  }

  /// Return the number of items in the section at the given index.
  fn items_count(&self, section: usize, cx: &App) -> usize;

  /// Render the item at the given index.
  ///
  /// Return `None` to skip the item.
  fn render_item(
    &mut self, ix: IndexPath, window: &mut Window, cx: &mut Context<ListState<Self>>,
  ) -> Option<Self::Item>;

  /// Render the section header at the given index.
  fn render_section_header(
    &mut self, _section: usize, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) -> Option<impl IntoElement> {
    None::<AnyElement>
  }

  /// Render the section footer at the given index.
  fn render_section_footer(
    &mut self, _section: usize, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) -> Option<impl IntoElement> {
    None::<AnyElement>
  }

  /// Return an element to show when list is empty.
  fn render_empty(
    &mut self, _window: &mut Window, cx: &mut Context<ListState<Self>>,
  ) -> impl IntoElement {
    h_flex()
      .size_full()
      .justify_center()
      .text_color(cx.theme().muted_foreground)
      .child(Icon::new(IconName::Search).small())
      .into_any_element()
  }

  /// Returns Some(AnyElement) to render initial state before user interaction.
  fn render_initial(
    &mut self, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) -> Option<AnyElement> {
    None
  }

  /// Returns loading state.
  fn loading(&self, _cx: &App) -> bool {
    false
  }

  /// Returns loading element.
  fn render_loading(
    &mut self, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) -> impl IntoElement {
    Loading
  }

  /// Set selected index (without confirming).
  fn set_selected_index(
    &mut self, ix: Option<IndexPath>, window: &mut Window, cx: &mut Context<ListState<Self>>,
  );

  /// Set the index of the item that has been right clicked.
  fn set_right_clicked_index(
    &mut self, _ix: Option<IndexPath>, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) {
  }

  /// Called when user confirms current selection.
  fn confirm(
    &mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) {
  }

  /// Called when user cancels selection.
  fn cancel(&mut self, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {}

  /// Return true to enable loading more when near bottom.
  fn has_more(&self, _cx: &App) -> bool {
    false
  }

  /// Threshold (entity count) to trigger `load_more`.
  fn load_more_threshold(&self) -> usize {
    20
  }

  /// Load more rows.
  fn load_more(&mut self, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {}
}
