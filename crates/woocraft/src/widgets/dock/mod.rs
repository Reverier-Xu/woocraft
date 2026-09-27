#[allow(clippy::module_inception)]
mod dock;
mod invalid_panel;
mod layout;
mod panel;
mod region;
mod stack_panel;
mod state;
mod tab_panel;
mod tiles;

use std::{collections::HashSet, sync::Arc};

use anyhow::Result;
pub use dock::*;
use gpui::{
  AnyElement, AnyView, App, AppContext, Bounds, Context, Edges, Empty, Entity, EntityId,
  EventEmitter, InteractiveElement as _, IntoElement, MouseButton, ParentElement as _, Pixels,
  Point, Render, SharedString, Styled, Subscription, WeakEntity, Window, actions, div,
  prelude::FluentBuilder, px,
};
pub(crate) use layout::{
  DockLayout, EditResult, InsertTarget, NodeId, PaneRef, PaneTree, PanelId, RootKind,
};
pub use panel::*;
pub(crate) use region::{CreatedMirror, DockRegion, RegionKind};
pub use stack_panel::*;
pub use state::*;
pub use tab_panel::*;
pub use tiles::{AnyDrag, DragDrop, DragMoving, DragResizing, TileItem, Tiles};

use crate::{ActiveTheme as _, DockPlacement, ElementExt, TabBarDirection};

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

  /// The center region of the dock_area. Its [`PaneTree`] is the structure's
  /// single source of truth; every structural mutation goes through
  /// [`DockArea::edit_region`], then mirrors are synced from the tree.
  pub(crate) center_region: DockRegion,
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
  pending_layout_change: bool,

  /// Tracks which [`TabPanel`] drop zone the mouse was over during the last
  /// `DragPanel` drag move so that the single global listener can efficiently
  /// clear stale previews when the pointer leaves a zone.
  last_drag_hover: Option<(WeakEntity<TabPanel>, TabPanelDropZone)>,
  /// Mouse position captured from the latest `DragPanel` drag-move event. The
  /// actual drop-zone computation is deferred to the next frame render so that
  /// high-frequency mouse reports are coalesced into one preview update pass.
  pending_drag_position: Option<Point<Pixels>>,
}

impl DockArea {
  pub fn new(
    id: impl Into<SharedString>, version: Option<usize>, window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let weak_self = cx.entity().downgrade();

    // The center region's structure lives in its tree; mirrors are created
    // by the region's sync. The initial layout is a single (empty) tab
    // group, which normalization keeps as the center's drop target.
    let mut center_region = DockRegion::new(RootKind::Split);
    center_region.tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split().child(DockLayout::tabs(), None),
    );
    let created = center_region.sync(&weak_self, window, cx);

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
      center_region,
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
      pending_layout_change: false,
      last_drag_hover: None,
      pending_drag_position: None,
    };

    for mirror in created {
      match mirror {
        CreatedMirror::Tab(tp) => this.subscribe_panel(&tp, window, cx),
        CreatedMirror::Split(sp) => this.subscribe_panel(&sp, window, cx),
      }
    }
    // The dock/center dividers depend on each dock's collapsed state, so the
    // area re-renders whenever a dock changes.
    for dock in [
      this.left_dock.clone(),
      this.right_dock.clone(),
      this.bottom_dock.clone(),
    ] {
      this
        ._subscriptions
        .push(cx.observe(&dock, |_, _, cx| cx.notify()));
    }
    this.update_toggle_button_tab_panels(window, cx);

    this
  }

  /// Return the bounds of the dock area.
  pub fn bounds(&self) -> Bounds<Pixels> {
    self.bounds
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
  /// The root view of the center region, if it has been built.
  pub fn center_root_view(&self) -> Option<Arc<dyn PanelView>> {
    self.center_region.root_view()
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
    let panel_id = PanelId::from(panel.view().entity_id());
    self.center_region.registry.insert(panel_id, panel);
    if let Some(node) = self.center_region.first_tab_group() {
      self.edit_region(RegionKind::Center, window, cx, |tree| {
        tree.insert_panel(
          panel_id,
          InsertTarget::Tabs {
            node,
            ix: None,
            activate: true,
          },
        )
      });
    } else {
      cx.notify();
    }
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
  /// Add a panel item to the dock area at the given placement.
  ///
  /// `bounds` only applies to tiles containers, which stay entity-managed.
  pub fn add_panel(
    &mut self, panel: Arc<dyn PanelView>, placement: DockPlacement, bounds: Option<Bounds<Pixels>>,
    window: &mut Window, cx: &mut Context<Self>,
  ) {
    let _ = bounds;
    match placement {
      DockPlacement::Center => self.add_to_center(panel, window, cx),
      _ => {
        let region = Self::region_of_placement(placement);
        let node = self.dock(region).read(cx).region.first_tab_group();
        if let Some(node) = node {
          self.place_panel(
            panel,
            region,
            InsertTarget::Tabs {
              node,
              ix: None,
              activate: true,
            },
            window,
            cx,
          );
        }
      }
    }
  }

  fn region_of_placement(placement: DockPlacement) -> RegionKind {
    match placement {
      DockPlacement::Left => RegionKind::Left,
      DockPlacement::Right => RegionKind::Right,
      DockPlacement::Bottom => RegionKind::Bottom,
      DockPlacement::Center => RegionKind::Center,
    }
  }

  /// Remove panel from the DockArea at the given placement.
  pub fn remove_panel(
    &mut self, panel: Arc<dyn PanelView>, placement: DockPlacement, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let region = Self::region_of_placement(placement);
    let panel_id = PanelId::from(panel.view().entity_id());
    let holds = match region {
      RegionKind::Center => self.center_region.tree.contains_panel(panel_id),
      _ => self
        .dock(region)
        .read(cx)
        .region
        .tree
        .contains_panel(panel_id),
    };
    if holds {
      self.close_panel(region, panel, window, cx);
    }
    cx.notify();
  }

  /// Remove a panel from all docks.
  pub fn remove_panel_from_all_docks(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let panel_id = PanelId::from(panel.view().entity_id());
    for region in [
      RegionKind::Center,
      RegionKind::Left,
      RegionKind::Right,
      RegionKind::Bottom,
    ] {
      let holds = match region {
        RegionKind::Center => self.center_region.tree.contains_panel(panel_id),
        _ => self
          .dock(region)
          .read(cx)
          .region
          .tree
          .contains_panel(panel_id),
      };
      if holds {
        self.close_panel(region, panel.clone(), window, cx);
      }
    }
  }

  /// Single global handler for `DragPanel` drag-move events.
  ///
  /// Previously each [`TabPanel`] registered three `on_drag_move` listeners,
  /// giving 3N capture-phase callbacks for N panels. This method replaces them
  /// with one listener on the [`DockArea`] root that performs a cheap bounds
  /// test against the drop zones recorded during each panel's last prepaint.
  ///
  /// To avoid doing O(N) work on every high-frequency mouse report, the handler
  /// only records the latest position; the actual drop-zone computation is
  /// deferred to the next frame render via
  /// [`DockArea::apply_pending_drag_move`].
  fn handle_drag_move(
    &mut self, drag: &gpui::DragMoveEvent<DragPanel>, _window: &mut Window, _cx: &mut Context<Self>,
  ) {
    let position = drag.event.position;
    if self.pending_drag_position != Some(position) {
      self.pending_drag_position = Some(position);
    }
  }

  /// Compute the drag drop-zone preview from the latest captured mouse
  /// position. Called once per frame during render so that high-frequency
  /// drag-move events are coalesced into a single preview update pass.
  fn apply_pending_drag_move(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    let Some(position) = self.pending_drag_position.take() else {
      return;
    };

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
        // The zone match above already guaranteed the bounds were recorded
        // during prepaint, but keep this defensive: a panel that has not
        // been prepainted yet (e.g. right after a zoom switch) has no bounds
        // and simply cannot be updated this frame.
        let bounds = match zone {
          TabPanelDropZone::TabBar => panel.tab_bar_bounds,
          TabPanelDropZone::VerticalTabBar => panel.vertical_tab_bar_bounds,
          TabPanelDropZone::PanelContent => panel.panel_content_bounds,
        };
        let Some(bounds) = bounds else {
          return;
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

  fn all_tab_panels(&self, cx: &App) -> Vec<Entity<TabPanel>> {
    let mut panels: Vec<Entity<TabPanel>> =
      self.center_region.tab_mirrors.values().cloned().collect();
    for kind in [RegionKind::Left, RegionKind::Right, RegionKind::Bottom] {
      panels.extend(
        self
          .dock(kind)
          .read(cx)
          .region
          .tab_mirrors
          .values()
          .cloned(),
      );
    }
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

    // Build the center region straight from the serialized tree and sync the
    // mirrors; created mirrors need their panel subscriptions.
    self.center_region =
      DockRegion::from_panel_state(&state.center, RootKind::Split, &weak_self, window, cx);
    let created = self.center_region.sync(&weak_self, window, cx);
    self.subscribe_created(created, window, cx);

    self.update_toggle_button_tab_panels(window, cx);
    Ok(())
  }

  /// Dump the dock panels layout to PanelState.
  ///
  /// See also [DockArea::load].
  pub fn dump(&self, cx: &App) -> DockAreaState {
    DockAreaState {
      version: self.version,
      center: self.center_region.to_panel_state(cx),
      center_enabled: self.center_enabled,
      left_dock: Some(DockState::new(self.left_dock.clone(), cx)),
      right_dock: Some(DockState::new(self.right_dock.clone(), cx)),
      bottom_dock: Some(DockState::new(self.bottom_dock.clone(), cx)),
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
    match self.center_region.root_view() {
      Some(view) => view.view().into_any_element(),
      None => Empty.into_any_element(),
    }
  }

  pub fn update_toggle_button_tab_panels(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    // Bottom toggle button
    self.toggle_button_panels.bottom = self
      .bottom_dock
      .read(cx)
      .first_tab_panel()
      .map(|view| view.entity_id());
  }

  /// The single write path for a region's structure.
  ///
  /// Applies `edit` to the region's [`PaneTree`]; when the tree changed,
  /// mirror entities are re-synced from it and `DockEvent::LayoutChanged`
  /// fires. Structural mutations of any region must go through here — mirror
  /// entities are read-only reflections of the tree.
  pub(crate) fn edit_region(
    &mut self, region: RegionKind, window: &mut Window, cx: &mut Context<Self>,
    apply: impl FnOnce(&mut PaneTree) -> EditResult,
  ) {
    match region {
      RegionKind::Center => {
        if !apply(&mut self.center_region.tree).changed() {
          return;
        }
        let weak_self = cx.entity().downgrade();
        let created = self.center_region.sync(&weak_self, window, cx);
        self.subscribe_created(created, window, cx);
        cx.emit(DockEvent::LayoutChanged);
        cx.notify();
      }
      RegionKind::Left => self.left_dock.update(cx, |dock, cx| {
        dock.edit_tree(apply, window, cx);
      }),
      RegionKind::Right => self.right_dock.update(cx, |dock, cx| {
        dock.edit_tree(apply, window, cx);
      }),
      RegionKind::Bottom => self.bottom_dock.update(cx, |dock, cx| {
        dock.edit_tree(apply, window, cx);
      }),
    }
  }

  fn subscribe_created(
    &mut self, created: Vec<CreatedMirror>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    for mirror in created {
      match mirror {
        CreatedMirror::Tab(tp) => self.subscribe_panel(&tp, window, cx),
        CreatedMirror::Split(sp) => self.subscribe_panel(&sp, window, cx),
      }
    }
  }

  /// The region a tab panel belongs to: its dock's placement when it sits in
  /// a side dock, otherwise the center.
  pub(crate) fn region_of(tab_panel: &TabPanel, cx: &App) -> RegionKind {
    match tab_panel.dock_placement(cx) {
      Some(DockPlacement::Left) => RegionKind::Left,
      Some(DockPlacement::Right) => RegionKind::Right,
      Some(DockPlacement::Bottom) => RegionKind::Bottom,
      Some(DockPlacement::Center) | None => RegionKind::Center,
    }
  }

  /// Inserts `panel` into the region at `target`. If the panel lives in
  /// another region, it is detached there first (a drag is not a close, so
  /// no `on_removed` hook fires). A panel already in the target region moves
  /// atomically. A target node that was normalized away between scheduling
  /// and execution falls back to the region's first tab group.
  pub(crate) fn place_panel(
    &mut self, panel: Arc<dyn PanelView>, region: RegionKind, target: InsertTarget,
    window: &mut Window, cx: &mut Context<Self>,
  ) {
    let panel_id = PanelId::from(panel.view().entity_id());

    // Resolve the target against the tree as of execution time.
    let target = {
      let target_node = match &target {
        InsertTarget::Tabs { node, .. } | InsertTarget::Split { node, .. } => *node,
      };
      let tree = match region {
        RegionKind::Center => &self.center_region.tree,
        _ => &self.dock(region).read(cx).region.tree,
      };
      if tree.path_of_node(target_node).is_none() {
        match tree.tab_groups().first().copied() {
          Some(node) => InsertTarget::Tabs {
            node,
            ix: None,
            activate: true,
          },
          None => target,
        }
      } else {
        target
      }
    };

    // A panel already in the target region moves atomically; otherwise
    // detach it from whichever region currently holds it.
    let in_target = match region {
      RegionKind::Center => self.center_region.tree.contains_panel(panel_id),
      _ => self
        .dock(region)
        .read(cx)
        .region
        .tree
        .contains_panel(panel_id),
    };
    if in_target {
      self.edit_region(region, window, cx, |tree| tree.move_panel(panel_id, target));
    } else {
      for kind in [
        RegionKind::Center,
        RegionKind::Left,
        RegionKind::Right,
        RegionKind::Bottom,
      ] {
        if kind == region {
          continue;
        }
        let holds = match kind {
          RegionKind::Center => self.center_region.tree.contains_panel(panel_id),
          _ => self
            .dock(kind)
            .read(cx)
            .region
            .tree
            .contains_panel(panel_id),
        };
        if holds {
          self.detach_panel(kind, panel.clone(), window, cx);
        }
      }
      match region {
        RegionKind::Center => {
          self.center_region.registry.insert(panel_id, panel);
          self.edit_region(RegionKind::Center, window, cx, |tree| {
            tree.insert_panel(panel_id, target)
          });
        }
        _ => {
          let dock = self.dock(region).clone();
          dock.update(cx, |dock, cx| {
            dock.region.registry.insert(panel_id, panel);
            dock.edit_tree(|tree| tree.insert_panel(panel_id, target), window, cx);
          });
        }
      }
    }

    // A collapsed target dock expands when a panel lands in it.
    if region != RegionKind::Center {
      let dock = self.dock(region).clone();
      if dock.read(cx).is_collapsed() {
        dock.update(cx, |dock, cx| {
          dock.set_collapsed(false, window, cx);
        });
      }
    }
  }

  /// Closes `panel` in its region: fires the panel's `on_removed` hook and
  /// removes it from the tree.
  pub(crate) fn close_panel(
    &mut self, region: RegionKind, panel: Arc<dyn PanelView>, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    panel.on_removed(window, cx);
    let panel_id = PanelId::from(panel.view().entity_id());
    self.edit_region(region, window, cx, |tree| tree.remove_panel(panel_id));
    // Dock regions collapse once empty; the center keeps its placeholder.
    if region != RegionKind::Center {
      let dock = self.dock(region).clone();
      let empty = !dock.read(cx).region.has_real_panels();
      if empty {
        dock.update(cx, |dock, cx| {
          dock.set_collapsed(true, window, cx);
        });
      }
    }
  }

  /// Detaches `panel` from its region without firing `on_removed` (used by
  /// drags; the panel keeps its resources).
  pub(crate) fn detach_panel(
    &mut self, region: RegionKind, panel: Arc<dyn PanelView>, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let panel_id = PanelId::from(panel.view().entity_id());
    self.edit_region(region, window, cx, |tree| tree.remove_panel(panel_id));
    if region != RegionKind::Center {
      let dock = self.dock(region).clone();
      let empty = !dock.read(cx).region.has_real_panels();
      if empty {
        dock.update(cx, |dock, cx| {
          dock.set_collapsed(true, window, cx);
        });
      }
    }
  }

  /// Activates tab `ix` of tab group `node` in the region. Dock regions
  /// expand first when collapsed.
  pub(crate) fn activate_tab(
    &mut self, region: RegionKind, node: NodeId, ix: usize, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if region != RegionKind::Center {
      let dock = self.dock(region).clone();
      if dock.read(cx).is_collapsed() {
        dock.update(cx, |dock, cx| {
          dock.set_collapsed(false, window, cx);
        });
      }
    }
    self.edit_region(region, window, cx, |tree| tree.set_active(node, ix));
  }

  fn dock(&self, region: RegionKind) -> &Entity<Dock> {
    match region {
      RegionKind::Left => &self.left_dock,
      RegionKind::Right => &self.right_dock,
      RegionKind::Bottom => &self.bottom_dock,
      RegionKind::Center => unreachable!("the center has no dock"),
    }
  }
}
impl EventEmitter<DockEvent> for DockArea {}
impl Render for DockArea {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.apply_pending_drag_move(window, cx);

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
          // Split previews are only ever set on the panel currently hovered
          // by a drag (see `apply_pending_drag_move`), so read-check that
          // single panel instead of sweeping and updating every TabPanel:
          // entity updates are far more expensive than reads.
          if let Some((panel, _)) = this.last_drag_hover.as_ref()
            && let Some(panel) = panel.upgrade()
            && panel.read(cx).has_pending_preview()
          {
            panel.update(cx, |panel, cx| panel.clear_split_preview(cx));
          }
        }),
      )
      .on_prepaint(move |bounds, _, cx| view.update(cx, |r, _| r.bounds = bounds))
      .map(|this| {
        if let Some(zoom_view) = self.zoom_view.clone() {
          this.child(zoom_view)
        } else {
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
              // Divider between the left dock and the center; hidden while
              // the dock is collapsed, highlighted while it is being resized.
              .when(!left_dock.read(cx).collapsed, |this| {
                let color = if left_dock.read(cx).is_resizing() {
                  cx.theme().primary
                } else {
                  cx.theme().border
                };
                this.child(div().flex_none().w(px(1.)).h_full().bg(color))
              })
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
                  // Divider between the center content and the bottom dock;
                  // hidden while the bottom dock is collapsed.
                  .when(!bottom_dock.read(cx).collapsed, |this| {
                    let color = if bottom_dock.read(cx).is_resizing() {
                      cx.theme().primary
                    } else {
                      cx.theme().border
                    };
                    this.child(div().flex_none().h(px(1.)).w_full().bg(color))
                  })
                  // Bottom Dock (always present)
                  .child(bottom_dock.clone()),
              )
              // Divider between the center and the right dock; hidden while
              // the dock is collapsed, highlighted while it is being resized.
              .when(!right_dock.read(cx).collapsed, |this| {
                let color = if right_dock.read(cx).is_resizing() {
                  cx.theme().primary
                } else {
                  cx.theme().border
                };
                this.child(div().flex_none().w(px(1.)).h_full().bg(color))
              })
              // Right Dock (always present)
              .child(div().flex().flex_none().child(right_dock.clone())),
          )
        }
      })
  }
}

#[cfg(test)]
mod tests {
  use gpui::{FocusHandle, Focusable, TestAppContext};

  use super::*;
  use crate::{Placement, Theme};

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

  // Regression anchors for the dock layout convergence: UI-driven changes
  // (dropping a panel into a tab group, closing it from its ✕) mutate only
  // the entities, so all structural reads must see the live entity graph
  // (see `docs/dock-layout-refactor.md`).
  //
  // The window hosts a bare root: the test asset source is empty, so letting
  // `DockArea` render would panic on icon validation. Its construction and
  // event flow only need a `Window` handle; the dock is never drawn.
  struct TestRoot;

  impl Render for TestRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
      div()
    }
  }

  #[gpui::test]
  async fn region_tree_drives_mirrors(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    cx.update(PanelRegistry::init);

    let panel_a = cx.new(|cx| TestPanel {
      focus_handle: cx.focus_handle(),
    });
    let panel_b = cx.new(|cx| TestPanel {
      focus_handle: cx.focus_handle(),
    });
    let panel_c = cx.new(|cx| TestPanel {
      focus_handle: cx.focus_handle(),
    });

    let window = cx
      .update(|app| app.open_window(Default::default(), |_, cx| cx.new(|_| TestRoot)))
      .unwrap();

    // Build the dock area with panel A in the center placeholder group.
    let dock_area = window
      .update(cx, |_, window, cx| {
        let dock_area = cx.new(|cx| DockArea::new("test", None, window, cx));
        dock_area.update(cx, |dock, cx| {
          dock.add_to_center(Arc::new(panel_a.clone()), window, cx);
        });
        dock_area
      })
      .unwrap();
    // The deferred mirror sync upgrades the DockArea by weak reference, so
    // the entity must stay alive for the whole test.
    let _keep_alive = &dock_area;
    cx.run_until_parked();

    // The tree holds A; its mirror shows it and stays drop-capable.
    window
      .update(cx, |_, _, cx| {
        let dock = dock_area.read(cx);
        assert!(
          dock
            .center_region
            .tree
            .contains_panel(PanelId::from(panel_a.entity_id())),
          "the tree holds the placed panel"
        );
        let (node, mirror) = dock
          .center_region
          .tab_mirrors
          .iter()
          .next()
          .expect("a tab mirror exists");
        assert_eq!(mirror.read(cx).panels.len(), 1);
        assert!(
          mirror.read(cx).allows_split_drop(),
          "mirror {node:?} must allow split drops"
        );
      })
      .unwrap();

    // Split B to the right of A's group, then C right of B's group.
    let group_a = window
      .update(cx, |_, _, cx| {
        dock_area
          .read(cx)
          .center_region
          .tree
          .tab_group_of(PanelId::from(panel_a.entity_id()))
          .expect("A lives in a tab group")
      })
      .unwrap();
    window
      .update(cx, |_, window, cx| {
        dock_area.update(cx, |dock, cx| {
          dock.place_panel(
            Arc::new(panel_b.clone()),
            RegionKind::Center,
            InsertTarget::Split {
              node: group_a,
              placement: Placement::Right,
              size: None,
            },
            window,
            cx,
          );
        });
      })
      .unwrap();
    cx.run_until_parked();

    let group_b = window
      .update(cx, |_, _, cx| {
        let tree = &dock_area.read(cx).center_region.tree;
        assert_eq!(tree.tab_groups().len(), 2, "first split adds a group");
        tree
          .tab_group_of(PanelId::from(panel_b.entity_id()))
          .expect("B lives in a tab group")
      })
      .unwrap();
    window
      .update(cx, |_, window, cx| {
        dock_area.update(cx, |dock, cx| {
          dock.place_panel(
            Arc::new(panel_c.clone()),
            RegionKind::Center,
            InsertTarget::Split {
              node: group_b,
              placement: Placement::Right,
              size: None,
            },
            window,
            cx,
          );
        });
      })
      .unwrap();
    cx.run_until_parked();

    window
      .update(cx, |_, _, cx| {
        let dock = dock_area.read(cx);
        assert_eq!(dock.center_region.tree.tab_groups().len(), 3);
        // Every mirror stays drop-capable after repeated splits.
        for (node, mirror) in dock.center_region.tab_mirrors.iter() {
          let tp = mirror.read(cx);
          assert!(!tp.panels.is_empty(), "mirror {node:?} lost its panel");
          assert!(
            tp.allows_split_drop(),
            "mirror {node:?} lost split-drop capability"
          );
        }
      })
      .unwrap();

    // Close every panel: the placeholder group survives as the drop target.
    window
      .update(cx, |_, window, cx| {
        for panel in [&panel_a, &panel_b, &panel_c] {
          dock_area.update(cx, |dock, cx| {
            dock.close_panel(RegionKind::Center, Arc::new(panel.clone()), window, cx);
          });
        }
      })
      .unwrap();
    cx.run_until_parked();

    window
      .update(cx, |_, _, cx| {
        let dock = dock_area.read(cx);
        let groups = dock.center_region.tree.tab_groups();
        assert_eq!(groups.len(), 1, "one placeholder group survives");
        let mirror = &dock.center_region.tab_mirrors[&groups[0]];
        assert!(mirror.read(cx).panels.is_empty());
      })
      .unwrap();

    // A drop into the emptied group lands and stays splittable.
    window
      .update(cx, |_, window, cx| {
        dock_area.update(cx, |dock, cx| {
          let node = dock.center_region.first_tab_group().unwrap();
          dock.place_panel(
            Arc::new(panel_a.clone()),
            RegionKind::Center,
            InsertTarget::Tabs {
              node,
              ix: None,
              activate: true,
            },
            window,
            cx,
          );
        });
      })
      .unwrap();
    cx.run_until_parked();

    window
      .update(cx, |_, _, cx| {
        let dock = dock_area.read(cx);
        let mirror = dock
          .center_region
          .tab_mirrors
          .values()
          .next()
          .expect("the placeholder mirror persists");
        assert_eq!(mirror.read(cx).panels.len(), 1);
        assert!(mirror.read(cx).allows_split_drop());
      })
      .unwrap();
  }
}
