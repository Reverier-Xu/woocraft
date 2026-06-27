#[allow(clippy::module_inception)]
mod dock;
mod invalid_panel;
mod panel;
mod stack_panel;
mod state;
mod tab_panel;
mod tiles;

use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
pub use dock::*;
use gpui::{
  AnyElement, AnyView, App, AppContext, Axis, Bounds, Context, Edges, Entity, EntityId,
  EventEmitter, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels,
  Render, SharedString, Styled, Subscription, WeakEntity, Window, actions, div,
  prelude::FluentBuilder,
};
pub use panel::*;
pub use stack_panel::*;
pub use state::*;
pub use tab_panel::*;
pub use tiles::{AnyDrag, DragDrop, DragMoving, DragResizing, TileItem, Tiles};

use crate::{DockPlacement, ElementExt, TabBarDirection};

pub(crate) fn init(cx: &mut App) {
  PanelRegistry::init(cx);
}

actions!(dock, [ToggleZoom, ClosePanel]);

pub enum DockEvent {
  /// The layout of the dock has changed, subscribers this to save the layout.
  ///
  /// This event is emitted when every time the layout of the dock has changed,
  /// So it emits may be too frequently, you may want to debounce the event.
  LayoutChanged,

  /// The drag item drop event.
  DragDrop(AnyDrag),
}

/// The main area of the dock.
pub struct DockArea {
  id: SharedString,
  /// The version is used to special the default layout, this is like the
  /// `panel_version` in [`Panel`](Panel).
  version: Option<usize>,
  pub(crate) bounds: Bounds<Pixels>,

  /// The center view of the dock_area.
  center: DockItem,
  /// Whether the center area is enabled (visible).
  center_enabled: bool,
  /// The left dock of the dock_area (always present).
  left_dock: Entity<Dock>,
  /// The bottom dock of the dock_area (always present).
  bottom_dock: Entity<Dock>,
  /// The right dock of the dock_area (always present).
  right_dock: Entity<Dock>,

  /// The entity_id of the [`TabPanel`](TabPanel) where each toggle button
  /// should be displayed,
  toggle_button_panels: Edges<Option<EntityId>>,

  /// Whether to show the toggle button.
  toggle_button_visible: bool,
  /// The top zoom view of the dock_area, if any.
  zoom_view: Option<AnyView>,

  /// Lock panels layout, but allow to resize.
  locked: bool,

  /// The panel style, default is [`PanelStyle::Default`](PanelStyle::Default).
  pub(crate) panel_style: PanelStyle,

  /// The tab bar direction, default is
  /// [`TabBarDirection::Top`](TabBarDirection::Top).
  pub(crate) tab_bar_direction: TabBarDirection,

  /// The custom placeholder content for the center area when it has no panels.
  pub(crate) center_placeholder: Option<AnyView>,

  _subscriptions: Vec<Subscription>,
  subscribed_panel_ids: HashSet<EntityId>,
  subscribed_tile_drop_ids: HashSet<EntityId>,
  pending_layout_change: bool,

  /// Tracks which [`TabPanel`] drop zone the mouse was over during the last
  /// `DragPanel` drag move so that the single global listener can efficiently
  /// clear stale previews when the pointer leaves a zone.
  last_drag_hover: Option<(WeakEntity<TabPanel>, TabPanelDropZone)>,
}

/// DockItem is a tree structure that represents the layout of the dock.
#[derive(Clone)]
pub enum DockItem {
  /// Split layout
  Split {
    axis: Axis,
    /// Self size, only used for build split panels
    size: Option<Pixels>,
    items: Vec<DockItem>,
    /// Items sizes
    sizes: Vec<Option<Pixels>>,
    view: Entity<StackPanel>,
  },
  /// Tab layout
  Tabs {
    /// Self size, only used for build split panels
    size: Option<Pixels>,
    items: Vec<Arc<dyn PanelView>>,
    active_ix: usize,
    view: Entity<TabPanel>,
  },
  /// Panel layout
  Panel {
    /// Self size, only used for build split panels
    size: Option<Pixels>,
    view: Arc<dyn PanelView>,
  },
  /// Tiles layout
  Tiles {
    /// Self size, only used for build split panels
    size: Option<Pixels>,
    items: Vec<TileItem>,
    view: Entity<Tiles>,
  },
}

impl std::fmt::Debug for DockItem {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      DockItem::Split {
        axis, items, sizes, ..
      } => f
        .debug_struct("Split")
        .field("axis", axis)
        .field("items", &items.len())
        .field("sizes", sizes)
        .finish(),
      DockItem::Tabs {
        items, active_ix, ..
      } => f
        .debug_struct("Tabs")
        .field("items", &items.len())
        .field("active_ix", active_ix)
        .finish(),
      DockItem::Panel { .. } => f.debug_struct("Panel").finish(),
      DockItem::Tiles { .. } => f.debug_struct("Tiles").finish(),
    }
  }
}

impl DockItem {
  /// Return true if this dock item tree contains any real (user) panels.
  pub fn has_real_panels(&self, cx: &App) -> bool {
    match self {
      Self::Tabs { view, .. } => !view.read(cx).panels.is_empty(),
      Self::Split { items, .. } => items.iter().any(|item| item.has_real_panels(cx)),
      Self::Panel { .. } => true,
      Self::Tiles { view, .. } => !view.read(cx).panels().is_empty(),
    }
  }

  /// Get the size of the DockItem.
  fn get_size(&self) -> Option<Pixels> {
    match self {
      Self::Split { size, .. } => *size,
      Self::Tabs { size, .. } => *size,
      Self::Panel { size, .. } => *size,
      Self::Tiles { size, .. } => *size,
    }
  }

  /// Set size for the DockItem.
  pub fn size(mut self, new_size: impl Into<Pixels>) -> Self {
    let new_size: Option<Pixels> = Some(new_size.into());
    match self {
      Self::Split { ref mut size, .. } => *size = new_size,
      Self::Tabs { ref mut size, .. } => *size = new_size,
      Self::Tiles { ref mut size, .. } => *size = new_size,
      Self::Panel { ref mut size, .. } => *size = new_size,
    }
    self
  }

  /// Set active index for the DockItem, only valid for [`DockItem::Tabs`].
  pub fn active_index(mut self, new_active_ix: usize, cx: &mut App) -> Self {
    debug_assert!(
      matches!(self, Self::Tabs { .. }),
      "active_ix can only be set for DockItem::Tabs"
    );

    if let Self::Tabs {
      ref mut active_ix,
      ref mut view,
      ..
    } = self
    {
      *active_ix = new_active_ix;
      view.update(cx, |tab_panel, _| {
        tab_panel.active_ix = new_active_ix;
      });
    }
    self
  }

  /// Create DockItem::Split with given split layout.
  pub fn split(
    axis: Axis, items: Vec<DockItem>, dock_area: &WeakEntity<DockArea>, window: &mut Window,
    cx: &mut App,
  ) -> Self {
    let sizes = items.iter().map(|item| item.get_size()).collect();
    Self::split_with_sizes(axis, items, sizes, dock_area, window, cx)
  }

  /// Create DockItem with vertical split layout.
  pub fn v_split(
    items: Vec<DockItem>, dock_area: &WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
  ) -> Self {
    Self::split(Axis::Vertical, items, dock_area, window, cx)
  }

  /// Create DockItem with horizontal split layout.
  pub fn h_split(
    items: Vec<DockItem>, dock_area: &WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
  ) -> Self {
    Self::split(Axis::Horizontal, items, dock_area, window, cx)
  }

  /// Create DockItem with split layout, each item of panel have specified size.
  ///
  /// Please note that the `items` and `sizes` must have the same length.
  /// Set `None` in `sizes` to make the index of panel have auto size.
  pub fn split_with_sizes(
    axis: Axis, items: Vec<DockItem>, sizes: Vec<Option<Pixels>>, dock_area: &WeakEntity<DockArea>,
    window: &mut Window, cx: &mut App,
  ) -> Self {
    let mut items = items;
    let stack_panel = cx.new(|cx| {
      let mut stack_panel = StackPanel::new(axis, window, cx);
      stack_panel.set_dock_area(dock_area.clone());
      for (i, item) in items.iter_mut().enumerate() {
        let view = item.view();
        let size = sizes.get(i).copied().flatten();
        stack_panel.add_panel(view.clone(), size, dock_area.clone(), window, cx)
      }

      stack_panel
    });

    window.defer(cx, {
      let stack_panel = stack_panel.clone();
      let dock_area = dock_area.clone();
      move |window, cx| {
        _ = dock_area.update(cx, |this, cx| {
          this.subscribe_panel(&stack_panel, window, cx);
        });
      }
    });

    Self::Split {
      axis,
      size: None,
      items,
      sizes,
      view: stack_panel,
    }
  }

  /// Create DockItem with panel layout
  pub fn panel(panel: Arc<dyn PanelView>) -> Self {
    Self::Panel {
      size: None,
      view: panel,
    }
  }

  /// Create DockItem with tiles layout
  ///
  /// This items and metas should have the same length.
  pub fn tiles(
    items: Vec<DockItem>, metas: Vec<impl Into<TileMeta> + Copy>, dock_area: &WeakEntity<DockArea>,
    window: &mut Window, cx: &mut App,
  ) -> Self {
    assert!(items.len() == metas.len());

    let tile_panel = cx.new(|cx| {
      let mut tiles = Tiles::new(window, cx);
      for (ix, item) in items.clone().into_iter().enumerate() {
        match item {
          DockItem::Tabs { view, .. } => {
            let meta: TileMeta = metas[ix].into();
            let tile_item = TileItem::new(Arc::new(view), meta.bounds).z_index(meta.z_index);
            tiles.add_item(tile_item, dock_area, window, cx);
          }
          DockItem::Panel { view, .. } => {
            let meta: TileMeta = metas[ix].into();
            let tile_item = TileItem::new(view.clone(), meta.bounds).z_index(meta.z_index);
            tiles.add_item(tile_item, dock_area, window, cx);
          }
          _ => {
            // Ignore non-tabs items
          }
        }
      }
      tiles
    });

    window.defer(cx, {
      let tile_panel = tile_panel.clone();
      let dock_area = dock_area.clone();
      move |window, cx| {
        _ = dock_area.update(cx, |this, cx| {
          this.subscribe_panel(&tile_panel, window, cx);
          this.subscribe_tiles_item_drop(&tile_panel, window, cx);
        });
      }
    });

    Self::Tiles {
      size: None,
      items: tile_panel.read(cx).panels.clone(),
      view: tile_panel,
    }
  }

  /// Create DockItem with tabs layout, items are displayed as tabs.
  ///
  /// The `active_ix` is the index of the active tab, if `None` the first tab is
  /// active.
  pub fn tabs(
    items: Vec<Arc<dyn PanelView>>, dock_area: &WeakEntity<DockArea>, window: &mut Window,
    cx: &mut App,
  ) -> Self {
    let mut new_items: Vec<Arc<dyn PanelView>> = vec![];
    for item in items.into_iter() {
      new_items.push(item)
    }
    Self::new_tabs(new_items, None, dock_area, window, cx)
  }

  pub fn tab<P: Panel>(
    item: Entity<P>, dock_area: &WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
  ) -> Self {
    Self::new_tabs(vec![Arc::new(item.clone())], None, dock_area, window, cx)
  }

  fn new_tabs(
    items: Vec<Arc<dyn PanelView>>, active_ix: Option<usize>, dock_area: &WeakEntity<DockArea>,
    window: &mut Window, cx: &mut App,
  ) -> Self {
    let active_ix = active_ix.unwrap_or(0);
    let tab_panel = cx.new(|cx| {
      let mut tab_panel = TabPanel::new(None, dock_area.clone(), window, cx);
      for item in items.iter() {
        tab_panel.add_panel(item.clone(), window, cx)
      }
      tab_panel.active_ix = active_ix;
      tab_panel
    });

    Self::Tabs {
      size: None,
      items,
      active_ix,
      view: tab_panel,
    }
  }

  /// Returns the views of the dock item.
  pub fn view(&self) -> Arc<dyn PanelView> {
    match self {
      Self::Split { view, .. } => Arc::new(view.clone()),
      Self::Tabs { view, .. } => Arc::new(view.clone()),
      Self::Tiles { view, .. } => Arc::new(view.clone()),
      Self::Panel { view, .. } => view.clone(),
    }
  }

  /// Find existing panel in the dock item.
  pub fn find_panel(&self, panel: Arc<dyn PanelView>) -> Option<Arc<dyn PanelView>> {
    match self {
      Self::Split { items, .. } => items.iter().find_map(|item| item.find_panel(panel.clone())),
      Self::Tabs { items, .. } => items.iter().find(|item| *item == &panel).cloned(),
      Self::Panel { view, .. } => Some(view.clone()),
      Self::Tiles { items, .. } => items.iter().find_map(|item| {
        if item.panel == panel.clone() {
          Some(item.panel.clone())
        } else {
          None
        }
      }),
    }
  }

  /// Add a panel to the dock item.
  pub fn add_panel(
    &mut self, panel: Arc<dyn PanelView>, dock_area: &WeakEntity<DockArea>,
    bounds: Option<Bounds<Pixels>>, window: &mut Window, cx: &mut App,
  ) {
    match self {
      Self::Tabs { view, items, .. } => {
        items.push(panel.clone());
        view.update(cx, |tab_panel, cx| {
          tab_panel.add_panel(panel, window, cx);
        });
      }
      Self::Split { view, items, .. } => {
        // Iter items to add panel to the first tabs
        for item in items.iter_mut() {
          if let DockItem::Tabs { view, .. } = item {
            view.update(cx, |tab_panel, cx| {
              tab_panel.add_panel(panel.clone(), window, cx);
            });
            return;
          }
        }

        // Unable to find tabs, create new tabs
        let new_item = Self::tabs(vec![panel.clone()], dock_area, window, cx);
        items.push(new_item.clone());
        view.update(cx, |stack_panel, cx| {
          stack_panel.add_panel(new_item.view(), None, dock_area.clone(), window, cx);
        });
      }
      Self::Tiles { view, items, .. } => {
        let tile_item = TileItem::new(
          Arc::new(cx.new(|cx| {
            let mut tab_panel = TabPanel::new(None, dock_area.clone(), window, cx);
            tab_panel.add_panel(panel.clone(), window, cx);
            tab_panel
          })),
          bounds.unwrap_or_else(|| TileMeta::default().bounds),
        );

        items.push(tile_item.clone());
        view.update(cx, |tiles, cx| {
          tiles.add_item(tile_item, dock_area, window, cx);
        });
      }
      Self::Panel { .. } => {}
    }
  }

  /// Remove a panel from the dock item.
  pub fn remove_panel(&self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut App) {
    match self {
      DockItem::Tabs { view, .. } => {
        view.update(cx, |tab_panel, cx| {
          tab_panel.remove_panel(panel, window, cx);
        });
      }
      DockItem::Split { items, view, .. } => {
        // For each child item, set collapsed state
        for item in items {
          item.remove_panel(panel.clone(), window, cx);
        }
        view.update(cx, |split, cx| {
          split.remove_panel(panel, window, cx);
        });
      }
      DockItem::Tiles { view, .. } => {
        view.update(cx, |tiles, cx| {
          tiles.remove(panel, window, cx);
        });
      }
      DockItem::Panel { .. } => {}
    }
  }

  pub fn set_collapsed(&self, collapsed: bool, window: &mut Window, cx: &mut App) {
    match self {
      DockItem::Tabs { view, .. } => {
        view.update(cx, |tab_panel, cx| {
          tab_panel.set_collapsed(collapsed, window, cx);
        });
      }
      DockItem::Split { items, .. } => {
        // For each child item, set collapsed state
        for item in items {
          item.set_collapsed(collapsed, window, cx);
        }
      }
      DockItem::Tiles { .. } => {}
      DockItem::Panel { view, .. } => view.set_active(!collapsed, window, cx),
    }
  }

  /// Recursively traverses to find the left-most and top-most TabPanel.
  pub(crate) fn left_top_tab_panel(&self, cx: &App) -> Option<Entity<TabPanel>> {
    match self {
      DockItem::Tabs { view, .. } => Some(view.clone()),
      DockItem::Split { view, .. } => view.read(cx).left_top_tab_panel(true, cx),
      DockItem::Tiles { .. } => None,
      DockItem::Panel { .. } => None,
    }
  }
}

impl DockArea {
  pub fn new(
    id: impl Into<SharedString>, version: Option<usize>, window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let weak_self = cx.entity().downgrade();

    // Create center as a split with one empty TabPanel placeholder
    let stack_panel = cx.new(|cx| {
      let mut sp = StackPanel::new(Axis::Horizontal, window, cx);
      sp.set_dock_area(weak_self.clone());
      sp
    });

    let center_tab =
      cx.new(|cx| TabPanel::new(Some(stack_panel.downgrade()), weak_self.clone(), window, cx));

    stack_panel.update(cx, |sp, cx| {
      sp.add_panel(
        Arc::new(center_tab.clone()),
        None,
        weak_self.clone(),
        window,
        cx,
      );
    });

    let center = DockItem::Split {
      axis: Axis::Horizontal,
      size: None,
      items: vec![DockItem::Tabs {
        size: None,
        items: vec![],
        active_ix: 0,
        view: center_tab,
      }],
      sizes: vec![None],
      view: stack_panel.clone(),
    };

    // Create side docks (always present, start empty and collapsed)
    let left_dock = cx.new(|cx| {
      let mut d = Dock::left(weak_self.clone(), window, cx);
      d.set_collapsed(true, window, cx);
      d
    });
    let bottom_dock = cx.new(|cx| {
      let mut d = Dock::bottom(weak_self.clone(), window, cx);
      d.set_collapsed(true, window, cx);
      d
    });
    let right_dock = cx.new(|cx| {
      let mut d = Dock::right(weak_self.clone(), window, cx);
      d.set_collapsed(true, window, cx);
      d
    });

    let mut this = Self {
      id: id.into(),
      version,
      bounds: Bounds::default(),
      center,
      center_enabled: true,
      left_dock,
      bottom_dock,
      right_dock,
      zoom_view: None,
      toggle_button_panels: Edges::default(),
      toggle_button_visible: true,
      locked: false,
      panel_style: PanelStyle::default(),
      tab_bar_direction: TabBarDirection::default(),
      center_placeholder: None,
      _subscriptions: vec![],
      subscribed_panel_ids: HashSet::new(),
      subscribed_tile_drop_ids: HashSet::new(),
      pending_layout_change: false,
      last_drag_hover: None,
    };

    this.subscribe_panel(&stack_panel, window, cx);
    this.update_toggle_button_tab_panels(window, cx);

    this
  }

  /// Return the bounds of the dock area.
  pub fn bounds(&self) -> Bounds<Pixels> {
    self.bounds
  }

  /// Subscribe to the tiles item drag item drop event
  fn subscribe_tiles_item_drop(
    &mut self, tile_panel: &Entity<Tiles>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.subscribed_tile_drop_ids.insert(tile_panel.entity_id()) {
      return;
    }

    self
      ._subscriptions
      .push(cx.subscribe(tile_panel, move |_, _, evt: &DragDrop, cx| {
        let item = evt.0.clone();
        cx.emit(DockEvent::DragDrop(item));
      }));
  }

  /// Set the panel style of the dock area.
  pub fn panel_style(mut self, style: PanelStyle) -> Self {
    self.panel_style = style;
    self
  }

  /// Set the tab bar direction of the dock area.
  pub fn tab_bar_direction(mut self, direction: TabBarDirection) -> Self {
    self.tab_bar_direction = direction;
    self
  }

  /// Set the tab bar direction of the dock area.
  pub fn set_tab_bar_direction(
    &mut self, direction: TabBarDirection, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.tab_bar_direction = direction;
    cx.notify();
  }

  /// Set version of the dock area.
  pub fn set_version(&mut self, version: usize, _: &mut Window, cx: &mut Context<Self>) {
    self.version = Some(version);
    cx.notify();
  }

  /// Set a custom placeholder view for the center area when it has no panels.
  ///
  /// This view is displayed inside the empty center drop zone.
  pub fn set_center_placeholder(
    &mut self, view: impl Into<AnyView>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.center_placeholder = Some(view.into());
    cx.notify();
  }

  /// Clear the custom center placeholder.
  pub fn clear_center_placeholder(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    self.center_placeholder = None;
    cx.notify();
  }

  /// Return the center placeholder view, if any.
  pub fn center_placeholder(&self) -> Option<&AnyView> {
    self.center_placeholder.as_ref()
  }

  /// Return the center dock item.
  pub fn center(&self) -> &DockItem {
    &self.center
  }

  /// Return the left dock.
  pub fn left_dock(&self) -> &Entity<Dock> {
    &self.left_dock
  }

  /// Return the bottom dock.
  pub fn bottom_dock(&self) -> &Entity<Dock> {
    &self.bottom_dock
  }

  /// Return the right dock.
  pub fn right_dock(&self) -> &Entity<Dock> {
    &self.right_dock
  }

  /// Add a panel to the center area.
  pub fn add_to_center(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let weak_self = cx.entity().downgrade();
    self.center.add_panel(panel, &weak_self, None, window, cx);
    cx.notify();
  }

  /// Add a panel to the left dock.
  pub fn add_to_left_dock(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.left_dock.update(cx, |dock, cx| {
      dock.add_panel(panel, window, cx);
    });
  }

  /// Add a panel to the right dock.
  pub fn add_to_right_dock(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.right_dock.update(cx, |dock, cx| {
      dock.add_panel(panel, window, cx);
    });
  }

  /// Add a panel to the bottom dock.
  pub fn add_to_bottom_dock(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.bottom_dock.update(cx, |dock, cx| {
      dock.add_panel(panel, window, cx);
    });
  }

  /// Enable the center area.
  pub fn enable_center(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    self.center_enabled = true;
    cx.notify();
  }

  /// Disable the center area.
  pub fn disable_center(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    self.center_enabled = false;
    cx.notify();
  }

  /// Set whether the center area is enabled.
  pub fn set_center_enabled(&mut self, enabled: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.center_enabled = enabled;
    cx.notify();
  }

  /// Returns whether the center area is enabled.
  pub fn is_center_enabled(&self) -> bool {
    self.center_enabled
  }

  /// Set the size of a dock at the given placement.
  pub fn set_dock_size(
    &mut self, placement: DockPlacement, size: Pixels, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let dock = match placement {
      DockPlacement::Left => &self.left_dock,
      DockPlacement::Right => &self.right_dock,
      DockPlacement::Bottom => &self.bottom_dock,
      DockPlacement::Center => return,
    };
    dock.update(cx, |dock, cx| {
      dock.set_size(size, window, cx);
    });
  }

  /// Set the collapsed state of a dock at the given placement.
  pub fn set_dock_collapsed(
    &mut self, placement: DockPlacement, collapsed: bool, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let dock = match placement {
      DockPlacement::Left => &self.left_dock,
      DockPlacement::Right => &self.right_dock,
      DockPlacement::Bottom => &self.bottom_dock,
      DockPlacement::Center => return,
    };
    dock.update(cx, |dock, cx| {
      dock.set_collapsed(collapsed, window, cx);
    });
  }

  /// Set locked state of the dock area, if locked, the dock area cannot be
  /// split or move, but allows to resize panels.
  pub fn set_locked(&mut self, locked: bool, _window: &mut Window, _cx: &mut App) {
    self.locked = locked;
  }

  /// Determine if the dock area is locked.
  #[inline]
  pub fn is_locked(&self) -> bool {
    self.locked
  }

  /// Determine if the dock area has a dock at the given placement.
  ///
  /// Always returns true since all docks are permanently present.
  pub fn has_dock(&self, _placement: DockPlacement) -> bool {
    true
  }

  /// Determine if the dock at the given placement is collapsed.
  pub fn is_dock_collapsed(&self, placement: DockPlacement, cx: &App) -> bool {
    match placement {
      DockPlacement::Left => self.left_dock.read(cx).is_collapsed(),
      DockPlacement::Bottom => self.bottom_dock.read(cx).is_collapsed(),
      DockPlacement::Right => self.right_dock.read(cx).is_collapsed(),
      DockPlacement::Center => false,
    }
  }

  /// Set whether each dock edge is collapsible.
  ///
  /// Only the left, bottom, right dock can be configured.
  ///
  /// DEPRECATED: Docks are now always collapsible. This method is a no-op.
  #[deprecated(note = "Docks are now always collapsible. This method is a no-op.")]
  pub fn set_dock_collapsible(
    &mut self, _collapsible_edges: Edges<bool>, _window: &mut Window, _cx: &mut Context<Self>,
  ) {
  }

  /// Determine if the dock at the given placement is collapsible.
  ///
  /// DEPRECATED: Docks are now always collapsible. Always returns true.
  #[deprecated(note = "Docks are now always collapsible. Always returns true.")]
  pub fn is_dock_collapsible(&self, _placement: DockPlacement, _cx: &App) -> bool {
    true
  }

  /// Toggle the dock at the given placement.
  pub fn toggle_dock(&self, placement: DockPlacement, window: &mut Window, cx: &mut Context<Self>) {
    let dock = match placement {
      DockPlacement::Left => &self.left_dock,
      DockPlacement::Bottom => &self.bottom_dock,
      DockPlacement::Right => &self.right_dock,
      DockPlacement::Center => return,
    };
    dock.update(cx, |view, cx| {
      view.toggle_collapsed(window, cx);
    });
  }

  /// Set the visibility of the toggle button.
  pub fn set_toggle_button_visible(&mut self, visible: bool, _: &mut Context<Self>) {
    self.toggle_button_visible = visible;
  }

  /// Add a panel item to the dock area at the given placement.
  pub fn add_panel(
    &mut self, panel: Arc<dyn PanelView>, placement: DockPlacement, bounds: Option<Bounds<Pixels>>,
    window: &mut Window, cx: &mut Context<Self>,
  ) {
    match placement {
      DockPlacement::Left => {
        self
          .left_dock
          .update(cx, |dock, cx| dock.add_panel(panel, window, cx));
      }
      DockPlacement::Bottom => {
        self
          .bottom_dock
          .update(cx, |dock, cx| dock.add_panel(panel, window, cx));
      }
      DockPlacement::Right => {
        self
          .right_dock
          .update(cx, |dock, cx| dock.add_panel(panel, window, cx));
      }
      DockPlacement::Center => {
        self
          .center
          .add_panel(panel, &cx.entity().downgrade(), bounds, window, cx);
      }
    }
  }

  /// Remove panel from the DockArea at the given placement.
  pub fn remove_panel(
    &mut self, panel: Arc<dyn PanelView>, placement: DockPlacement, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    match placement {
      DockPlacement::Left => {
        self.left_dock.update(cx, |dock, cx| {
          dock.remove_panel(panel, window, cx);
        });
      }
      DockPlacement::Right => {
        self.right_dock.update(cx, |dock, cx| {
          dock.remove_panel(panel, window, cx);
        });
      }
      DockPlacement::Bottom => {
        self.bottom_dock.update(cx, |dock, cx| {
          dock.remove_panel(panel, window, cx);
        });
      }
      DockPlacement::Center => {
        self.center.remove_panel(panel, window, cx);
      }
    }
    cx.notify();
  }

  /// Remove a panel from all docks.
  pub fn remove_panel_from_all_docks(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.remove_panel(panel.clone(), DockPlacement::Center, window, cx);
    self.remove_panel(panel.clone(), DockPlacement::Left, window, cx);
    self.remove_panel(panel.clone(), DockPlacement::Right, window, cx);
    self.remove_panel(panel.clone(), DockPlacement::Bottom, window, cx);
  }

  /// Single global handler for `DragPanel` drag-move events.
  ///
  /// Previously each [`TabPanel`] registered three `on_drag_move` listeners,
  /// giving 3N capture-phase callbacks for N panels. This method replaces them
  /// with one listener on the [`DockArea`] root that performs a cheap bounds
  /// test against the drop zones recorded during each panel's last prepaint.
  fn handle_drag_move(
    &mut self, drag: &gpui::DragMoveEvent<DragPanel>, _window: &mut Window, cx: &mut Context<Self>,
  ) {
    let position = drag.event.position;
    let dock_area_locked = self.is_locked();
    let mut hovered: Option<(WeakEntity<TabPanel>, TabPanelDropZone)> = None;

    for tab_panel in self.all_tab_panels(cx) {
      let state = tab_panel.read(cx);
      let droppable = !state.is_locked_with_dock_area(dock_area_locked);

      if droppable
        && let Some(bounds) = state.tab_bar_bounds
        && bounds.contains(&position)
      {
        hovered = Some((tab_panel.downgrade(), TabPanelDropZone::TabBar));
        break;
      }
      if droppable
        && let Some(bounds) = state.vertical_tab_bar_bounds
        && bounds.contains(&position)
      {
        hovered = Some((tab_panel.downgrade(), TabPanelDropZone::VerticalTabBar));
        break;
      }
      if droppable
        && let Some(bounds) = state.panel_content_bounds
        && bounds.contains(&position)
      {
        hovered = Some((tab_panel.downgrade(), TabPanelDropZone::PanelContent));
        break;
      }
    }

    if self.last_drag_hover != hovered {
      if let Some((last_panel, last_zone)) = self.last_drag_hover.take()
        && let Some(last_panel) = last_panel.upgrade()
      {
        last_panel.update(cx, |panel, cx| match last_zone {
          TabPanelDropZone::TabBar | TabPanelDropZone::VerticalTabBar => {
            panel.clear_tab_drop_preview();
          }
          TabPanelDropZone::PanelContent => panel.clear_split_preview(cx),
        });
      }
      self.last_drag_hover = hovered.clone();
    }

    if let Some((panel, zone)) = hovered
      && let Some(panel) = panel.upgrade()
    {
      panel.update(cx, |panel, cx| {
        let bounds = match zone {
          TabPanelDropZone::TabBar => panel.tab_bar_bounds.unwrap(),
          TabPanelDropZone::VerticalTabBar => panel.vertical_tab_bar_bounds.unwrap(),
          TabPanelDropZone::PanelContent => panel.panel_content_bounds.unwrap(),
        };
        match zone {
          TabPanelDropZone::TabBar => panel.on_tab_bar_drag_move(position, bounds, cx),
          TabPanelDropZone::VerticalTabBar => {
            panel.on_vertical_tab_bar_drag_move(position, bounds, cx)
          }
          TabPanelDropZone::PanelContent => {
            if panel.allows_split_drop() {
              panel.on_panel_drag_move(position, bounds, cx);
            } else {
              panel.set_center_drop_active(true, cx);
            }
          }
        }
      });
    }
  }

  fn collect_tab_panels_from_panel_view(
    panel: Arc<dyn PanelView>, out: &mut Vec<Entity<TabPanel>>, cx: &App,
  ) {
    if let Ok(tab_panel) = panel.view().downcast::<TabPanel>() {
      out.push(tab_panel);
    } else if let Ok(stack_panel) = panel.view().downcast::<StackPanel>() {
      stack_panel.read(cx).collect_tab_panels(out, cx);
    }
  }

  fn all_tab_panels(&self, cx: &App) -> Vec<Entity<TabPanel>> {
    let mut panels = Vec::new();
    Self::collect_tab_panels_from_panel_view(self.center.view(), &mut panels, cx);
    Self::collect_tab_panels_from_panel_view(self.left_dock.read(cx).panel.view(), &mut panels, cx);
    Self::collect_tab_panels_from_panel_view(
      self.right_dock.read(cx).panel.view(),
      &mut panels,
      cx,
    );
    Self::collect_tab_panels_from_panel_view(
      self.bottom_dock.read(cx).panel.view(),
      &mut panels,
      cx,
    );
    panels
  }

  /// Find a panel by user-defined panel id.
  pub fn panel_by_id(&self, panel_id: &str, cx: &App) -> Option<Arc<dyn PanelView>> {
    for tab_panel in self.all_tab_panels(cx) {
      if let Some(panel) = tab_panel.read(cx).panel_by_id(panel_id, cx) {
        return Some(panel);
      }
    }

    None
  }

  /// Alias of [`DockArea::panel_by_id`].
  pub fn get_panel_by_id(&self, panel_id: &str, cx: &App) -> Option<Arc<dyn PanelView>> {
    self.panel_by_id(panel_id, cx)
  }

  /// Activate a panel by user-defined panel id.
  ///
  /// Returns `true` if a panel is found and activated.
  pub fn activate_panel_by_id(
    &mut self, panel_id: &str, window: &mut Window, cx: &mut Context<Self>,
  ) -> bool {
    for tab_panel in self.all_tab_panels(cx) {
      let mut activated = false;
      tab_panel.update(cx, |tab_panel, cx| {
        activated = tab_panel.activate_panel_by_id(panel_id, window, cx);
      });

      if activated {
        return true;
      }
    }

    false
  }

  /// Highlight a panel by user-defined panel id.
  ///
  /// This currently activates and focuses the panel.
  pub fn highlight_panel_by_id(
    &mut self, panel_id: &str, window: &mut Window, cx: &mut Context<Self>,
  ) -> bool {
    if !self.activate_panel_by_id(panel_id, window, cx) {
      return false;
    }

    if let Some(panel) = self.panel_by_id(panel_id, cx) {
      panel.focus_handle(cx).focus(window, cx);
      return true;
    }

    false
  }

  /// Close a panel by user-defined panel id.
  ///
  /// Returns `true` if a panel is found and closed.
  pub fn close_panel_by_id(
    &mut self, panel_id: &str, window: &mut Window, cx: &mut Context<Self>,
  ) -> bool {
    // Cache the lock state before updating TabPanels so they do not re-read
    // this DockArea while it is already inside an update.
    let dock_area_locked = self.is_locked();

    for tab_panel in self.all_tab_panels(cx) {
      let mut closed = false;
      tab_panel.update(cx, |tab_panel, cx| {
        closed = tab_panel.close_panel_by_id(panel_id, dock_area_locked, window, cx);
      });

      if closed {
        return true;
      }
    }

    false
  }

  /// Load the state of the DockArea from the DockAreaState.
  ///
  /// See also [DockeArea::dump].
  pub fn load(
    &mut self, state: DockAreaState, window: &mut Window, cx: &mut Context<Self>,
  ) -> Result<()> {
    self._subscriptions.clear();
    self.subscribed_panel_ids.clear();
    self.subscribed_tile_drop_ids.clear();
    self.version = state.version;
    self.center_enabled = state.center_enabled;
    let weak_self = cx.entity().downgrade();

    if let Some(left_dock_state) = state.left_dock {
      self.left_dock = left_dock_state.to_dock(weak_self.clone(), window, cx);
    }

    if let Some(right_dock_state) = state.right_dock {
      self.right_dock = right_dock_state.to_dock(weak_self.clone(), window, cx);
    }

    if let Some(bottom_dock_state) = state.bottom_dock {
      self.bottom_dock = bottom_dock_state.to_dock(weak_self.clone(), window, cx);
    }

    self.center = state.center.to_item(weak_self.clone(), window, cx);

    // Ensure the root StackPanel of the center knows about the dock_area
    if let DockItem::Split { view, .. } = &self.center {
      view.update(cx, |sp, _| sp.set_dock_area(weak_self));
    }

    self.update_toggle_button_tab_panels(window, cx);
    Ok(())
  }

  /// Dump the dock panels layout to PanelState.
  ///
  /// See also [DockArea::load].
  pub fn dump(&self, cx: &App) -> DockAreaState {
    let root = self.center.view();
    let center = root.dump(cx);

    DockAreaState {
      version: self.version,
      center,
      center_enabled: self.center_enabled,
      left_dock: Some(DockState::new(self.left_dock.clone(), cx)),
      right_dock: Some(DockState::new(self.right_dock.clone(), cx)),
      bottom_dock: Some(DockState::new(self.bottom_dock.clone(), cx)),
    }
  }

  /// Subscribe event on the panels
  #[allow(clippy::only_used_in_recursion)]
  #[allow(dead_code)]
  fn subscribe_item(&mut self, item: &DockItem, window: &mut Window, cx: &mut Context<Self>) {
    match item {
      DockItem::Split { items, view, .. } => {
        for item in items {
          self.subscribe_item(item, window, cx);
        }

        self._subscriptions.push(cx.subscribe_in(
          view,
          window,
          move |this, _, event, window, cx| {
            if let PanelEvent::LayoutChanged = event
              && !this.pending_layout_change
            {
              this.pending_layout_change = true;
              cx.spawn_in(window, async move |view, window| {
                _ = view.update_in(window, |view, window, cx| {
                  view.pending_layout_change = false;
                  view.update_toggle_button_tab_panels(window, cx);
                });
              })
              .detach();
              cx.emit(DockEvent::LayoutChanged);
            }
          },
        ));
      }
      DockItem::Tabs { .. } => {
        // We subscribe to the tab panel event in StackPanel's insert_panel
      }
      DockItem::Tiles { .. } => {
        // We subscribe to the tab panel event in Tiles's
        // [`add_item`](Tiles::add_item)
      }
      DockItem::Panel { .. } => {
        // Not supported
      }
    }
  }

  /// Subscribe zoom event on the panel
  pub(crate) fn subscribe_panel<P: Panel>(
    &mut self, view: &Entity<P>, window: &mut Window, cx: &mut Context<DockArea>,
  ) {
    if !self.subscribed_panel_ids.insert(view.entity_id()) {
      return;
    }

    let subscription =
      cx.subscribe_in(
        view,
        window,
        move |this, panel, event, window, cx| match event {
          PanelEvent::ZoomIn => {
            let panel = panel.clone();
            cx.spawn_in(window, async move |view, window| {
              _ = view.update_in(window, |view, window, cx| {
                view.set_zoomed_in(panel, window, cx);
                cx.notify();
              });
            })
            .detach();
          }
          PanelEvent::ZoomOut => cx
            .spawn_in(window, async move |view, window| {
              _ = view.update_in(window, |view, window, cx| {
                view.set_zoomed_out(window, cx);
              });
            })
            .detach(),
          PanelEvent::LayoutChanged => {
            if !this.pending_layout_change {
              this.pending_layout_change = true;
              cx.spawn_in(window, async move |view, window| {
                _ = view.update_in(window, |view, window, cx| {
                  view.pending_layout_change = false;
                  view.update_toggle_button_tab_panels(window, cx);
                });
              })
              .detach();
              cx.emit(DockEvent::LayoutChanged);
            }
          }
        },
      );

    self._subscriptions.push(subscription);
  }

  /// Returns the ID of the dock area.
  pub fn id(&self) -> SharedString {
    self.id.clone()
  }

  pub fn set_zoomed_in<P: Panel>(
    &mut self, panel: Entity<P>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.zoom_view = Some(panel.into());
    cx.notify();
  }

  pub fn set_zoomed_out(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    self.zoom_view = None;
    cx.notify();
  }

  fn render_items(&self, _window: &mut Window, _cx: &mut Context<Self>) -> AnyElement {
    match &self.center {
      DockItem::Split { view, .. } => view.clone().into_any_element(),
      DockItem::Tabs { view, .. } => view.clone().into_any_element(),
      DockItem::Tiles { view, .. } => view.clone().into_any_element(),
      DockItem::Panel { view, .. } => view.clone().view().into_any_element(),
    }
  }

  pub fn update_toggle_button_tab_panels(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    // Bottom toggle button
    self.toggle_button_panels.bottom = self
      .bottom_dock
      .read(cx)
      .panel
      .left_top_tab_panel(cx)
      .map(|view| view.entity_id());
  }
}
impl EventEmitter<DockEvent> for DockArea {}
impl Render for DockArea {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().clone();

    div()
      .id("dock-area")
      .relative()
      .size_full()
      .overflow_hidden()
      .on_drag_move(cx.listener(|this, drag, window, cx| {
        this.handle_drag_move(drag, window, cx);
      }))
      .on_mouse_up(
        MouseButton::Left,
        cx.listener(|this, _, _, cx| {
          for tab_panel in this.all_tab_panels(cx) {
            tab_panel.update(cx, |panel, cx| panel.clear_split_preview(cx));
          }
        }),
      )
      .on_prepaint(move |bounds, _, cx| view.update(cx, |r, _| r.bounds = bounds))
      .map(|this| {
        if let Some(zoom_view) = self.zoom_view.clone() {
          this.child(zoom_view)
        } else {
          match &self.center {
            DockItem::Tiles { view, .. } => {
              // render tiles
              this.child(view.clone())
            }
            _ => {
              let left_dock = self.left_dock.clone();
              let right_dock = self.right_dock.clone();
              let bottom_dock = self.bottom_dock.clone();

              // render dock
              this.child(
                div()
                  .flex()
                  .flex_row()
                  .h_full()
                  // Left dock (always present)
                  .child(div().flex().flex_none().child(left_dock.clone()))
                  // Center column
                  .child(
                    div()
                      .flex()
                      .flex_1()
                      .flex_col()
                      .overflow_hidden()
                      // Center content (or empty space when disabled)
                      .child(
                        div()
                          .flex_1()
                          .overflow_hidden()
                          .when(self.center_enabled, |this| {
                            this.child(self.render_items(window, cx))
                          }),
                      )
                      // Bottom Dock (always present)
                      .child(bottom_dock.clone()),
                  )
                  // Right Dock (always present)
                  .child(div().flex().flex_none().child(right_dock.clone())),
              )
            }
          }
        }
      })
  }
}

#[cfg(test)]
mod tests {
  use gpui::{FocusHandle, Focusable, TestAppContext};

  use super::*;
  use crate::Theme;

  struct TestPanel {
    focus_handle: FocusHandle,
  }

  impl EventEmitter<PanelEvent> for TestPanel {}

  impl Focusable for TestPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
      self.focus_handle.clone()
    }
  }

  impl Render for TestPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
      div()
    }
  }

  impl Panel for TestPanel {
    fn panel_name(&self) -> &'static str {
      "test_panel"
    }
  }

  #[gpui::test]
  #[ignore = "gpui 0.2 TestAppContext cannot configure an asset source, so rendering panics on missing icons"]
  async fn test_pending_layout_change_prevents_reentrant_update(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    cx.update(PanelRegistry::init);

    let panel = cx.new(|cx| TestPanel {
      focus_handle: cx.focus_handle(),
    });

    let window = cx
      .update(|app| {
        app.open_window(Default::default(), |window, cx| {
          cx.new(|cx| DockArea::new("test", None, window, cx))
        })
      })
      .unwrap();

    window
      .update(cx, |this, window, cx| {
        this.subscribe_panel(&panel, window, cx);
      })
      .unwrap();

    let initial = window
      .read_with(cx, |dock, _| dock.pending_layout_change)
      .unwrap();
    assert!(!initial, "pending_layout_change should be false initially");

    panel.update(cx, |_, cx| {
      cx.emit(PanelEvent::LayoutChanged);
    });

    let after = window
      .read_with(cx, |dock, _| dock.pending_layout_change)
      .unwrap();
    assert!(
      after,
      "pending_layout_change should be true after LayoutChanged event"
    );

    cx.run_until_parked();

    let after_parked = window
      .read_with(cx, |dock, _| dock.pending_layout_change)
      .unwrap();
    assert!(
      !after_parked,
      "pending_layout_change should be reset after spawned task completes"
    );
  }
}
