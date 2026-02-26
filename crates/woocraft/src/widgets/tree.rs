use std::{ops::Range, rc::Rc};

use gpui::{
  AnyElement, App, AppContext, ClickEvent, Context, ElementId, Entity, EntityId, EventEmitter,
  FocusHandle, Focusable, InteractiveElement as _, IntoElement, KeyBinding, ListSizingBehavior,
  MouseButton, MouseDownEvent, ParentElement, Render, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, UniformListScrollHandle, Window, div,
  prelude::FluentBuilder as _, px, uniform_list,
};

use crate::{
  ActiveTheme, ContextMenuExt as _, Icon, IconName, ListItem, PopupMenu, ScrollableElement,
  StyledExt, TreeEntry, TreeItem, TreeModel,
  actions::{Confirm, SelectDown, SelectLeft, SelectRight, SelectUp},
  h_flex,
};

const CONTEXT: &str = "Tree";

type TreeRenderItem = Rc<dyn Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> AnyElement>;
type TreeContextMenuBuilder =
  Rc<dyn Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// Events emitted by the tree component.
#[derive(Clone, Debug)]
pub enum TreeEvent {
  /// A row was selected (single click).
  Select(usize),
  /// A row was double-clicked.
  DoubleClicked(usize),
  /// A row was right-clicked.
  RightClicked(usize),
  /// Selection was cleared.
  ClearSelection,
  /// A row was toggled in multi-select (via Ctrl/Cmd+Click).
  ToggleSelect(usize),
  /// A range selection was made (via Shift+Click).
  RangeSelect(usize),
  /// An item was dragged from one index to another.
  MoveItem(usize, usize),
}

/// Drag payload for tree item reordering.
#[derive(Clone)]
pub(crate) struct DragTreeItem {
  pub(crate) entity_id: EntityId,
  pub(crate) ix: usize,
  pub(crate) label: SharedString,
}

impl Render for DragTreeItem {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .px_4()
      .py_1()
      .bg(cx.theme().card)
      .text_color(cx.theme().foreground)
      .opacity(0.9)
      .border_1()
      .border_color(cx.theme().border)
      .shadow_md()
      .max_w(px(300.))
      .overflow_hidden()
      .text_ellipsis()
      .child(self.label.clone())
  }
}

pub(crate) fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("up", SelectUp, Some(CONTEXT)),
    KeyBinding::new("down", SelectDown, Some(CONTEXT)),
    KeyBinding::new("left", SelectLeft, Some(CONTEXT)),
    KeyBinding::new("right", SelectRight, Some(CONTEXT)),
    KeyBinding::new("enter", Confirm { secondary: false }, Some(CONTEXT)),
  ]);
}

/// Create a tree with built-in rendering:
/// - item icon on the left
/// - label in the middle (fills width)
/// - expand/collapse icon on the right
/// - vertical guide line for expanded folders
pub fn tree(state: &Entity<TreeState>) -> Tree {
  Tree::new(state)
}

/// Create a tree and override center content rendering while keeping built-in
/// affordances.
pub fn tree_with<R, E>(state: &Entity<TreeState>, render_item: R) -> Tree
where
  R: Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> E + 'static,
  E: IntoElement, {
  Tree::new(state).render_item(render_item)
}

pub struct TreeState {
  focus_handle: FocusHandle,
  model: TreeModel,
  scroll_handle: UniformListScrollHandle,
  render_item: TreeRenderItem,
  context_menu_builder: Option<TreeContextMenuBuilder>,
  /// The index of the row that was right-clicked (used by context menu).
  right_clicked_ix: Option<usize>,
  /// Whether multi-selection is enabled.
  pub multi_selectable: bool,
  /// Whether drag-to-reorder is enabled.
  pub draggable: bool,
}

impl Focusable for TreeState {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl EventEmitter<TreeEvent> for TreeState {}

impl TreeState {
  pub fn new(cx: &mut App) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      model: TreeModel::new(),
      scroll_handle: UniformListScrollHandle::default(),
      render_item: Rc::new(|_, entry, _, _, _| {
        div()
          .w_full()
          .min_w_0()
          .overflow_hidden()
          .text_ellipsis()
          .child(entry.item().label.clone())
          .into_any_element()
      }),
      context_menu_builder: None,
      right_clicked_ix: None,
      multi_selectable: false,
      draggable: false,
    }
  }

  /// Enable or disable multi-selection mode (builder pattern).
  pub fn multi_selectable(mut self, enabled: bool) -> Self {
    self.multi_selectable = enabled;
    self.model.set_multi_selectable(enabled);
    self
  }

  /// Enable or disable drag-to-reorder (builder pattern).
  pub fn draggable(mut self, enabled: bool) -> Self {
    self.draggable = enabled;
    self
  }

  pub fn items(mut self, items: impl Into<Vec<TreeItem>>) -> Self {
    self.model = self.model.items(items);
    self
  }

  pub fn set_items(&mut self, items: impl Into<Vec<TreeItem>>, cx: &mut Context<Self>) {
    self.model.set_items(items);
    cx.notify();
  }

  pub fn entries(&self) -> &[TreeEntry] {
    self.model.entries()
  }

  pub fn selected_index(&self) -> Option<usize> {
    self.model.selected_index()
  }

  pub fn set_selected_index(&mut self, ix: Option<usize>, cx: &mut Context<Self>) {
    self.model.set_selected_index(ix);
    cx.notify();
  }

  pub fn set_selected_item(&mut self, item: Option<&TreeItem>, cx: &mut Context<Self>) {
    self.model.set_selected_item(item);
    cx.notify();
  }

  pub fn selected_item(&self) -> Option<&TreeItem> {
    self.model.selected_item()
  }

  pub fn selected_entry(&self) -> Option<&TreeEntry> {
    self.model.selected_entry()
  }

  pub fn scroll_to_item(&mut self, ix: usize, strategy: gpui::ScrollStrategy) {
    self.scroll_handle.scroll_to_item(ix, strategy);
  }

  pub fn focus(&mut self, window: &mut Window, _: &mut App) {
    self.focus_handle.focus(window);
  }

  /// Returns the underlying tree model.
  pub fn model(&self) -> &TreeModel {
    &self.model
  }

  /// Returns a mutable reference to the underlying tree model.
  pub fn model_mut(&mut self) -> &mut TreeModel {
    &mut self.model
  }

  /// Whether the given index is selected (works for both single and multi mode).
  pub fn is_selected(&self, ix: usize) -> bool {
    self.model.is_selected(ix)
  }

  /// Clear all selections and emit [`TreeEvent::ClearSelection`].
  pub fn clear_selection(&mut self, cx: &mut Context<Self>) {
    self.model.clear_selection();
    cx.emit(TreeEvent::ClearSelection);
    cx.notify();
  }

  /// Returns all selected items in multi-select mode.
  pub fn selected_items(&self) -> Vec<&TreeItem> {
    self.model.selected_items()
  }

  fn row_id(entry: &TreeEntry) -> ElementId {
    ElementId::Name(format!("tree-row-{}", entry.item().id.as_ref()).into())
  }

  fn item_id(entry: &TreeEntry) -> ElementId {
    ElementId::Name(format!("tree-item-{}", entry.item().id.as_ref()).into())
  }

  fn render_list_item(
    &self, ix: usize, entry: &TreeEntry, content: AnyElement, cx: &mut Context<Self>,
  ) -> ListItem {
    let expand_icon = if entry.is_expanded() {
      IconName::ChevronDown
    } else {
      IconName::ChevronRight
    };

    let _ = ix;
    ListItem::new(Self::item_id(entry)).child(
      h_flex()
        .w_full()
        .min_w_0()
        .relative()
        .items_center()
        .gap_x(px(6.))
        .child(div().flex_none().w(px(14.) * entry.depth()))
        .child(Icon::new(entry.icon_or_default()).text_color(cx.theme().muted_foreground))
        .child(div().flex_1().min_w_0().child(content))
        .child(
          div()
            .w_4()
            .items_center()
            .justify_center()
            .when(entry.is_folder(), |this| {
              this.child(Icon::new(expand_icon).text_color(cx.theme().muted_foreground))
            }),
        ),
    )
  }

  fn render_guide_layers(
    &self, ix: usize, entry: &TreeEntry, cx: &mut Context<Self>,
  ) -> Vec<AnyElement> {
    const ROW_CENTER_Y: f32 = 12.0;
    const BRANCH_LEN: f32 = 10.0;

    let mut layers = Vec::new();
    let entries = self.model.entries();
    let depth = entry.depth();
    let has_children = ix + 1 < entries.len() && entries[ix + 1].depth() > depth;
    let color = cx.theme().foreground.opacity(0.2);
    let immediate_parent_ix = Self::parent_index(entries, ix);

    // Ancestor continuation lines (VS Code style).
    let mut cursor_ix = ix;
    while let Some(parent_ix) = Self::parent_index(entries, cursor_ix) {
      let parent_depth = entries[parent_ix].depth();
      let is_immediate_parent = Some(parent_ix) == immediate_parent_ix;
      if !is_immediate_parent && Self::has_next_sibling(entries, parent_ix) {
        layers.push(
          div()
            .absolute()
            .left(Self::guide_x(parent_depth))
            .top_0()
            .bottom_0()
            .w(px(1.))
            .bg(color)
            .into_any_element(),
        );
      }
      cursor_ix = parent_ix;
    }

    // Branch segment for current row (├ / └).
    if depth > 0
      && let Some(parent_ix) = immediate_parent_ix
    {
      let branch_depth = entries[parent_ix].depth();
      let x = Self::guide_x(branch_depth);
      let current_has_next_sibling = Self::has_next_sibling(entries, ix);
      let parent_has_next_sibling = Self::has_next_sibling(entries, parent_ix);
      let should_full_height = current_has_next_sibling || parent_has_next_sibling;

      layers.push(
        div()
          .absolute()
          .left(x)
          .top_0()
          .w(px(1.))
          .when(!should_full_height, |this| this.h(px(ROW_CENTER_Y)))
          .when(should_full_height, |this| this.bottom_0())
          .bg(color)
          .into_any_element(),
      );
      layers.push(
        div()
          .absolute()
          .left(x)
          .top(px(ROW_CENTER_Y))
          .w(px(BRANCH_LEN))
          .h(px(1.))
          .bg(color)
          .into_any_element(),
      );
    }

    // Expanded folder stem to descendants.
    if entry.is_folder() && entry.is_expanded() && has_children {
      layers.push(
        div()
          .absolute()
          .left(Self::guide_x(depth))
          .top(px(ROW_CENTER_Y))
          .bottom_0()
          .w(px(1.))
          .bg(color)
          .into_any_element(),
      );
    }

    layers
  }

  fn subtree_end(entries: &[TreeEntry], root_ix: usize) -> usize {
    let root_depth = entries[root_ix].depth();
    let mut end = root_ix + 1;
    while end < entries.len() && entries[end].depth() > root_depth {
      end += 1;
    }
    end
  }

  fn parent_index(entries: &[TreeEntry], ix: usize) -> Option<usize> {
    let depth = entries.get(ix)?.depth();
    if depth == 0 {
      return None;
    }

    let target_depth = depth - 1;
    (0..ix).rev().find(|&candidate_ix| {
      entries[candidate_ix].depth() == target_depth && Self::subtree_end(entries, candidate_ix) > ix
    })
  }

  fn has_next_sibling(entries: &[TreeEntry], ix: usize) -> bool {
    let depth = entries[ix].depth();
    let end = Self::subtree_end(entries, ix);
    end < entries.len() && entries[end].depth() == depth
  }

  fn guide_x(depth: usize) -> gpui::Pixels {
    px(2.) + px(14.) * depth as f32
  }

  fn on_action_confirm(&mut self, _: &Confirm, _: &mut Window, cx: &mut Context<Self>) {
    let Some(selected_ix) = self.model.selected_index() else {
      return;
    };
    let Some(entry) = self.model.entry(selected_ix) else {
      return;
    };
    if !entry.is_folder() {
      return;
    }

    self.model.toggle_expand(selected_ix);
    cx.notify();
  }

  fn on_action_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
    let Some(selected_ix) = self.model.selected_index() else {
      return;
    };
    let Some(entry) = self.model.entry(selected_ix) else {
      return;
    };
    if !entry.is_folder() || !entry.is_expanded() {
      return;
    }

    self.model.toggle_expand(selected_ix);
    cx.notify();
  }

  fn on_action_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
    let Some(selected_ix) = self.model.selected_index() else {
      return;
    };
    let Some(entry) = self.model.entry(selected_ix) else {
      return;
    };
    if !entry.is_folder() || entry.is_expanded() {
      return;
    }

    self.model.toggle_expand(selected_ix);
    cx.notify();
  }

  fn on_action_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
    if self.model.is_empty() {
      return;
    }

    let selected_ix = self.model.selected_index().unwrap_or(0);
    let selected_ix = if selected_ix > 0 {
      selected_ix - 1
    } else {
      self.model.len() - 1
    };

    self.model.set_selected_index(Some(selected_ix));
    self
      .scroll_handle
      .scroll_to_item(selected_ix, gpui::ScrollStrategy::Top);
    cx.notify();
  }

  fn on_action_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
    if self.model.is_empty() {
      return;
    }

    let selected_ix = self.model.selected_index().unwrap_or(0);
    let selected_ix = if selected_ix + 1 < self.model.len() {
      selected_ix + 1
    } else {
      0
    };

    self.model.set_selected_index(Some(selected_ix));
    self
      .scroll_handle
      .scroll_to_item(selected_ix, gpui::ScrollStrategy::Bottom);
    cx.notify();
  }

  fn on_entry_click(
    &mut self, ix: usize, ev: &ClickEvent, _: &mut Window, cx: &mut Context<Self>,
  ) {
    // Double-click: emit event
    if ev.click_count() == 2 {
      cx.emit(TreeEvent::DoubleClicked(ix));
      return;
    }

    // Multi-selection modifiers
    if self.multi_selectable {
      if ev.modifiers().secondary() {
        // Ctrl/Cmd+Click: toggle individual item
        self.model.toggle_selected(ix);
        cx.emit(TreeEvent::ToggleSelect(ix));
        cx.notify();
        return;
      }
      if ev.modifiers().shift {
        // Shift+Click: range select
        self.model.select_range_to(ix);
        cx.emit(TreeEvent::RangeSelect(ix));
        cx.notify();
        return;
      }
      // Plain click in multi-mode: clear multi-selection, select single
      self.model.clear_selection();
    }

    self.model.set_selected_index(Some(ix));
    if self.multi_selectable {
      self.model.toggle_selected(ix);
    }
    self.model.toggle_expand(ix);
    cx.emit(TreeEvent::Select(ix));
    cx.notify();
  }

  fn on_entry_right_click(
    &mut self, ix: usize, _: &MouseDownEvent, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.right_clicked_ix = Some(ix);
    cx.emit(TreeEvent::RightClicked(ix));
    cx.notify();
  }

  fn on_drop_item(&mut self, from_ix: usize, to_ix: usize, _: &mut Window, cx: &mut Context<Self>) {
    if from_ix == to_ix {
      return;
    }
    cx.emit(TreeEvent::MoveItem(from_ix, to_ix));
    cx.notify();
  }
}

impl Render for TreeState {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let render_item = self.render_item.clone();
    let entries_len = self.model.len();
    let _multi_selectable = self.multi_selectable;
    let draggable = self.draggable;
    let entity_id = cx.entity_id();

    div().id("tree-state").size_full().relative().child(
      uniform_list("entries", entries_len, {
        cx.processor(move |state, visible_range: Range<usize>, window, cx| {
          let mut items = Vec::with_capacity(visible_range.len());
          for ix in visible_range {
            let Some(entry) = state.model.entry(ix) else {
              continue;
            };

            let selected = state.model.is_selected(ix);
            let disabled = entry.is_disabled();
            let content = (render_item)(ix, entry, selected, window, cx);
            let list_item = state.render_list_item(ix, entry, content, cx);
            let guides = state.render_guide_layers(ix, entry, cx);
            let label = entry.item().label.clone();

            let mut row = div()
              .id(Self::row_id(entry))
              .relative()
              .children(guides)
              .child(list_item.disabled(disabled).selected(selected));

            if !disabled {
              row = row
                .on_click(cx.listener(move |this, ev, window, cx| {
                  this.on_entry_click(ix, ev, window, cx);
                }))
                .on_mouse_down(
                  MouseButton::Right,
                  cx.listener(move |this, ev, window, cx| {
                    this.on_entry_right_click(ix, ev, window, cx);
                  }),
                );
            }

            // Drag-to-reorder support
            if draggable && !disabled {
              let drag_label = label.clone();
              row = row
                .on_drag(
                  DragTreeItem {
                    entity_id,
                    ix,
                    label: drag_label,
                  },
                  |drag, _, _, cx| {
                    cx.stop_propagation();
                    cx.new(|_| drag.clone())
                  },
                )
                .drag_over::<DragTreeItem>(|this, _, _, cx| {
                  this
                    .border_t_2()
                    .border_color(cx.theme().drag_border)
                })
                .on_drop(cx.listener(move |this, drag: &DragTreeItem, window, cx| {
                  if drag.entity_id != cx.entity_id() {
                    return;
                  }
                  this.on_drop_item(drag.ix, ix, window, cx);
                }));
            }

            items.push(row);
          }

          items
        })
      })
      .flex_grow()
      .size_full()
      .track_scroll(self.scroll_handle.clone())
      .with_sizing_behavior(ListSizingBehavior::Auto)
      .into_any_element(),
    )
  }
}

#[derive(IntoElement)]
pub struct Tree {
  id: ElementId,
  state: Entity<TreeState>,
  style: StyleRefinement,
  render_item: TreeRenderItem,
  context_menu_builder: Option<TreeContextMenuBuilder>,
}

impl Focusable for Tree {
  fn focus_handle(&self, cx: &App) -> FocusHandle {
    self.state.read(cx).focus_handle.clone()
  }
}

impl Tree {
  pub fn new(state: &Entity<TreeState>) -> Self {
    Self {
      id: ("tree", state.entity_id()).into(),
      state: state.clone(),
      style: StyleRefinement::default(),
      render_item: Rc::new(|_, entry, _, _, _| {
        div()
          .w_full()
          .min_w_0()
          .overflow_hidden()
          .text_ellipsis()
          .child(entry.item().label.clone())
          .into_any_element()
      }),
      context_menu_builder: None,
    }
  }

  pub fn render_item<R, E>(mut self, render_item: R) -> Self
  where
    R: Fn(usize, &TreeEntry, bool, &mut Window, &mut App) -> E + 'static,
    E: IntoElement, {
    self.render_item = Rc::new(move |ix, entry, selected, window, cx| {
      render_item(ix, entry, selected, window, cx).into_any_element()
    });
    self
  }

  /// Set a context menu builder for right-click menus.
  ///
  /// The callback receives `(ix, entry, menu, window, cx)` and should return
  /// a configured `PopupMenu`.
  pub fn context_menu<F>(mut self, builder: F) -> Self
  where
    F: Fn(usize, &TreeEntry, PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu
      + 'static,
  {
    self.context_menu_builder = Some(Rc::new(builder));
    self
  }
}

impl_styled!(Tree);

impl RenderOnce for Tree {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let focus_handle = self.state.read(cx).focus_handle.clone();
    let scroll_handle = self.state.read(cx).scroll_handle.clone();

    self.state.update(cx, |state, _| {
      state.render_item = self.render_item;
      state.context_menu_builder = self.context_menu_builder;
    });

    let tree_view = self.state.clone();

    div()
      .id(self.id)
      .key_context(CONTEXT)
      .track_focus(&focus_handle)
      .on_action(window.listener_for(&self.state, TreeState::on_action_confirm))
      .on_action(window.listener_for(&self.state, TreeState::on_action_left))
      .on_action(window.listener_for(&self.state, TreeState::on_action_right))
      .on_action(window.listener_for(&self.state, TreeState::on_action_up))
      .on_action(window.listener_for(&self.state, TreeState::on_action_down))
      .size_full()
      .child(self.state)
      .refine_style(&self.style)
      .vertical_scrollbar(&scroll_handle)
      .context_menu(move |menu, window, cx| {
        let state = tree_view.read(cx);
        let Some(ix) = state.right_clicked_ix else {
          return menu;
        };
        let Some(entry) = state.model.entry(ix) else {
          return menu;
        };
        let entry = entry.clone();
        let builder = state.context_menu_builder.clone();
        let _ = state;

        if let Some(builder) = builder {
          builder(ix, &entry, menu, window, cx)
        } else {
          menu
        }
      })
  }
}
