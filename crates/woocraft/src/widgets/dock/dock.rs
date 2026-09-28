//! Dock is a fixed container that places at left, bottom, right of the Windows.

use std::{ops::Deref, sync::Arc};

use gpui::{
  App, AppContext, Context, Element, Empty, Entity, InteractiveElement, IntoElement,
  MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Rems, Render, Style,
  StyleRefinement, Styled as _, WeakEntity, Window, deferred, div, prelude::FluentBuilder as _, px,
  rems,
};

use super::{
  super::resizable::{PANEL_MIN_SIZE, resize_handle},
  DockArea, DockRegion, InsertTarget, PaneTree, PanelId, PanelView, RootKind, TabPanel,
};
use crate::{DockPlacement, Size, StyledExt, TabBarDirection};

/// Side docks (left/right) include a vertical tab rail plus title-bar controls.
/// They need a larger minimum width than the generic panel minimum to prevent
/// title/content overflow.
const SIDE_DOCK_MIN_SIZE: Rems = rems(16.);

#[derive(Clone)]
pub(super) struct ResizePanel;

impl Render for ResizePanel {
  fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    Empty
  }
}

/// The Dock is a fixed container that places at left, bottom, right of the
/// Windows.
///
/// This is unlike Panel, it can't be move or add any other panel.
pub struct Dock {
  pub(super) placement: DockPlacement,
  dock_area: WeakEntity<DockArea>,
  /// The dock's layout tree: the single source of truth for what lives in
  /// this dock.
  pub(crate) region: DockRegion,
  /// The size is means the width or height of the Dock, if the placement is
  /// left or right, the size is width, otherwise the size is height.
  pub(super) size: Pixels,
  pub(super) collapsed: bool,
  /// The tab bar direction for this dock
  pub(super) tab_bar_direction: TabBarDirection,
  preview_size: Option<Pixels>,

  // Runtime state
  /// Whether the Dock is resizing
  resizing: bool,
  /// Last mouse position processed during a resize, used to skip duplicate
  /// events when the mouse has not moved.
  last_resize_position: Option<Point<Pixels>>,
  /// Mouse position captured from the latest resize event. The actual
  /// preview-size computation is deferred to the next frame render so that
  /// high-frequency mouse reports are coalesced into one layout pass.
  pending_resize_position: Option<Point<Pixels>>,
  /// Cached style refinements used to enable child view caching in render.
  /// They are identical every frame, so they are built once and cloned
  /// instead of being re-allocated per frame.
  panel_cache_style: StyleRefinement,
}

impl Dock {
  #[inline]
  fn min_size_for_placement(placement: DockPlacement, rem_size: Pixels) -> Pixels {
    match placement {
      DockPlacement::Left | DockPlacement::Right => SIDE_DOCK_MIN_SIZE.to_pixels(rem_size),
      DockPlacement::Bottom | DockPlacement::Center => PANEL_MIN_SIZE.to_pixels(rem_size),
    }
  }

  #[inline]
  fn min_size(&self, rem_size: Pixels) -> Pixels {
    Self::min_size_for_placement(self.placement, rem_size)
  }

  #[inline]
  pub(super) fn display_size(&self) -> Pixels {
    self.preview_size.unwrap_or(self.size)
  }

  pub(crate) fn new(
    dock_area: WeakEntity<DockArea>, placement: DockPlacement, window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let tab_bar_direction = match placement {
      DockPlacement::Left => TabBarDirection::Left,
      DockPlacement::Right => TabBarDirection::Right,
      DockPlacement::Bottom => TabBarDirection::default(),
      DockPlacement::Center => TabBarDirection::default(),
    };

    let mut region = DockRegion::new(RootKind::Any);
    region.set_dock(cx.entity().downgrade());
    let created = region.sync(&dock_area, window, cx);
    Self::defer_subscribe_created(dock_area.clone(), created, window, cx);

    Self {
      placement,
      dock_area,
      region,
      collapsed: false,
      size: rems(13.).to_pixels(window.rem_size()),
      tab_bar_direction,
      preview_size: None,
      resizing: false,
      last_resize_position: None,
      pending_resize_position: None,
      panel_cache_style: StyleRefinement::default().size_full(),
    }
  }

  pub fn left(
    dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    Self::new(dock_area, DockPlacement::Left, window, cx)
  }

  pub fn bottom(
    dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    Self::new(dock_area, DockPlacement::Bottom, window, cx)
  }

  pub fn right(
    dock_area: WeakEntity<DockArea>, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    Self::new(dock_area, DockPlacement::Right, window, cx)
  }

  /// Return true if the dock has any real panels (not just empty placeholders).
  pub fn has_panels(&self) -> bool {
    self.region.has_real_panels()
  }

  /// The first tab group mirror in this dock, if any (used by the toggle
  /// button placement).
  pub(crate) fn first_tab_panel(&self) -> Option<Entity<TabPanel>> {
    let node = self.region.first_tab_group()?;
    self.region.tab_mirrors.get(&node).cloned()
  }

  /// Applies `edit` to the dock's layout tree, syncs the mirrors and emits
  /// the layout-changed event on the owning `DockArea`.
  pub(crate) fn edit_tree(
    &mut self, apply: impl FnOnce(&mut PaneTree) -> super::EditResult, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if !apply(&mut self.region.tree).changed() {
      return;
    }
    let created = self.region.sync(&self.dock_area, window, cx);
    Self::defer_subscribe_created(self.dock_area.clone(), created, window, cx);
    // Emit the layout change on the owning DockArea once this update
    // completes (the DockArea may be the entity currently being updated).
    let dock_area = self.dock_area.clone();
    window.defer(cx, move |window, cx| {
      if let Some(dock_area) = dock_area.upgrade() {
        dock_area.update(cx, |dock_area, cx| {
          dock_area.update_toggle_button_tab_panels(window, cx);
          cx.emit(super::DockEvent::LayoutChanged);
        });
      }
    });
    cx.notify();
  }

  /// Subscribes freshly created mirror entities via the owning `DockArea`,
  /// deferred until the current update completes (the `DockArea` may be the
  /// entity currently being updated).
  fn defer_subscribe_created(
    dock_area: WeakEntity<DockArea>, created: Vec<super::CreatedMirror>, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if created.is_empty() {
      return;
    }
    window.defer(cx, move |window, cx| {
      if let Some(dock_area) = dock_area.upgrade() {
        dock_area.update(cx, |dock_area, cx| {
          for mirror in created {
            match mirror {
              super::CreatedMirror::Tab(tp) => dock_area.subscribe_panel(&tp, window, cx),
              super::CreatedMirror::Split(sp) => dock_area.subscribe_panel(&sp, window, cx),
            }
          }
        });
      }
    });
  }

  pub(super) fn from_state(
    dock_area: WeakEntity<DockArea>, placement: DockPlacement, size: Pixels,
    panel: &super::PanelState, collapsed: bool, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    let tab_bar_direction = match placement {
      DockPlacement::Left => TabBarDirection::Left,
      DockPlacement::Right => TabBarDirection::Right,
      DockPlacement::Bottom => TabBarDirection::default(),
      DockPlacement::Center => TabBarDirection::default(),
    };
    let min_size = Self::min_size_for_placement(placement, window.rem_size());

    let mut region = DockRegion::from_panel_state(panel, RootKind::Any, &dock_area, window, cx);
    region.set_dock(cx.entity().downgrade());
    let created = region.sync(&dock_area, window, cx);
    Self::defer_subscribe_created(dock_area.clone(), created, window, cx);

    Self {
      placement,
      dock_area,
      region,
      collapsed,
      size: size.max(min_size),
      tab_bar_direction,
      preview_size: None,
      resizing: false,
      last_resize_position: None,
      pending_resize_position: None,
      panel_cache_style: StyleRefinement::default().size_full(),
    }
  }

  pub fn is_collapsed(&self) -> bool {
    self.collapsed
  }

  pub fn toggle_collapsed(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.set_collapsed(!self.collapsed, window, cx);
  }

  /// Returns the size of the Dock, the size is means the width or height of
  /// the Dock, if the placement is left or right, the size is width,
  /// otherwise the size is height.
  pub fn size(&self) -> Pixels {
    self.size
  }

  /// Set the size of the Dock.
  pub fn set_size(&mut self, size: Pixels, window: &mut Window, cx: &mut Context<Self>) {
    self.size = size.max(self.min_size(window.rem_size()));
    self.preview_size = None;
    cx.notify();
  }

  /// Set the collapsed state of the Dock.
  pub fn set_collapsed(&mut self, collapsed: bool, window: &mut Window, cx: &mut Context<Self>) {
    self.collapsed = collapsed;
    self.preview_size = None;
    // Defer the per-panel active-state update: the collapse can be triggered
    // from a tab click while that TabPanel is mid-update.
    let mirrors: Vec<Entity<TabPanel>> = self.region.tab_mirrors.values().cloned().collect();
    cx.defer_in(window, move |_, window, cx| {
      for mirror in mirrors {
        mirror.update(cx, |tab_panel, cx| {
          tab_panel.set_collapsed(collapsed, window, cx);
        });
      }
    });
    cx.notify();
  }

  /// Add item to the Dock.
  pub fn add_panel(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let panel_id = PanelId::from(panel.view().entity_id());
    self.region.registry.insert(panel_id, panel);
    let Some(node) = self.region.first_tab_group() else {
      cx.notify();
      return;
    };
    self.edit_tree(
      |tree| {
        tree.insert_panel(
          panel_id,
          InsertTarget::Tabs {
            node,
            ix: None,
            activate: true,
          },
        )
      },
      window,
      cx,
    );
    cx.notify();
  }

  /// Remove item from the Dock.
  pub fn remove_panel(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    panel.on_removed(window, cx);
    let panel_id = PanelId::from(panel.view().entity_id());
    self.edit_tree(|tree| tree.remove_panel(panel_id), window, cx);
    cx.notify();
  }

  pub(super) fn set_resizing(&mut self, resizing: bool, cx: &mut Context<Self>) {
    if self.resizing == resizing {
      return;
    }
    self.resizing = resizing;
    if resizing {
      self.last_resize_position = None;
    }
    // The owning `DockArea` draws the dock/center divider and observes this
    // dock, so notifying lets it switch the divider to the primary highlight.
    cx.notify();
  }

  /// Whether the user is currently dragging this dock's resize handle.
  #[inline]
  pub(super) fn is_resizing(&self) -> bool {
    self.resizing
  }

  fn resize(
    &mut self, mouse_position: Point<Pixels>, _window: &mut Window, _cx: &mut Context<Self>,
  ) {
    if !self.resizing {
      return;
    }

    if self.last_resize_position == Some(mouse_position) {
      return;
    }
    self.last_resize_position = Some(mouse_position);
    self.pending_resize_position = Some(mouse_position);
    // The drag is active, so dispatch_mouse_event will call window.refresh()
    // for every MouseMoveEvent. Rely on that to drive the next frame instead
    // of notifying this entity on every high-frequency mouse report.
  }

  /// Compute the dock preview size from the latest captured mouse position.
  /// Called once per frame during render so that high-frequency mouse events
  /// are coalesced into a single layout pass.
  fn apply_pending_resize(&mut self, window: &Window, cx: &mut Context<Self>) {
    let Some(mouse_position) = self.pending_resize_position.take() else {
      return;
    };

    let dock_area = self
      .dock_area
      .upgrade()
      .expect("DockArea is missing")
      .read(cx);
    let area_bounds = dock_area.bounds;
    let mut left_dock_size = px(0.0);
    let mut right_dock_size = px(0.0);

    // Get the size of the left dock if it's expanded and not the current dock
    {
      let left_dock = &dock_area.left_dock;
      if left_dock.entity_id() != cx.entity().entity_id() {
        let left_dock_read = left_dock.read(cx);
        if !left_dock_read.is_collapsed() {
          left_dock_size = left_dock_read.size;
        }
      }
    }

    {
      let right_dock = &dock_area.right_dock;
      if right_dock.entity_id() != cx.entity().entity_id() {
        let right_dock_read = right_dock.read(cx);
        if !right_dock_read.is_collapsed() {
          right_dock_size = right_dock_read.size;
        }
      }
    }

    let size = match self.placement {
      DockPlacement::Left => mouse_position.x - area_bounds.left(),
      DockPlacement::Right => area_bounds.right() - mouse_position.x,
      DockPlacement::Bottom => area_bounds.bottom() - mouse_position.y,
      DockPlacement::Center => unreachable!(),
    };
    let panel_min = PANEL_MIN_SIZE.to_pixels(window.rem_size());
    let max_size = match self.placement {
      DockPlacement::Left => area_bounds.size.width - panel_min - right_dock_size,
      DockPlacement::Right => area_bounds.size.width - panel_min - left_dock_size,
      DockPlacement::Bottom => area_bounds.size.height - panel_min,
      DockPlacement::Center => unreachable!(),
    };
    self.preview_size = Some(
      size
        .clamp(self.min_size(window.rem_size()), max_size)
        .round(),
    );
    // This runs during render; the current frame already picks up the new
    // preview_size, so do not schedule another notification here.
  }

  fn done_resizing(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    self.resizing = false;
    self.last_resize_position = None;
    self.pending_resize_position = None;
    if let Some(preview_size) = self.preview_size.take()
      && self.size != preview_size
    {
      self.size = preview_size;
    }
    // Always notify: the divider highlight has to clear even when the drag
    // ended on the original size.
    cx.notify();
  }
}

impl Render for Dock {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
    self.apply_pending_resize(window, cx);

    let collapsed_width = rems(2.5).to_pixels(window.rem_size()) + px(1.);
    let collapsed_height = Size::Medium.container_height().to_pixels(window.rem_size()) + px(1.);
    let view = cx.entity().clone();

    let create_resize_handle = || {
      let dock = view.clone();
      let handle = resize_handle::<ResizePanel, ResizePanel>("dock-resize", self.placement.axis())
        .on_drag(ResizePanel, move |info, _, _, cx| {
          cx.stop_propagation();
          dock.update(cx, |dock, cx| dock.set_resizing(true, cx));
          cx.new(|_| info.deref().clone())
        });

      match self.placement {
        DockPlacement::Left => deferred(
          div()
            .absolute()
            .right(px(0.))
            .top_0()
            .bottom_0()
            .w(px(0.))
            .child(handle)
            .occlude(),
        ),
        DockPlacement::Right => deferred(
          div()
            .absolute()
            .left(px(0.))
            .top_0()
            .bottom_0()
            .w(px(0.))
            .child(handle)
            .occlude(),
        ),
        DockPlacement::Bottom => deferred(
          div()
            .absolute()
            .top(px(0.))
            .left_0()
            .right_0()
            .h(px(0.))
            .child(handle)
            .occlude(),
        ),
        DockPlacement::Center => unreachable!(),
      }
    };

    div()
      .relative()
      .map(|this| match self.placement {
        DockPlacement::Left | DockPlacement::Right => this.h_flex().h_full().w(self.display_size()),
        DockPlacement::Bottom => this.v_flex().w_full().h(self.display_size()),
        DockPlacement::Center => unreachable!(),
      })
      .when(self.collapsed, |this| match self.placement {
        DockPlacement::Left | DockPlacement::Right => this.w(collapsed_width),
        DockPlacement::Bottom => this.h(collapsed_height),
        DockPlacement::Center => this,
      })
      .map(|this| {
        let panel = div()
          .flex_1()
          .overflow_hidden()
          .map(|this| match self.placement {
            DockPlacement::Left | DockPlacement::Right => this.h_full(),
            DockPlacement::Bottom => this.w_full(),
            DockPlacement::Center => this,
          })
          .map(|this| match self.region.root_view() {
            Some(view) => this.child(view.view().cached(self.panel_cache_style.clone())),
            None => this,
          });

        this.child(panel)
      })
      .when(!self.collapsed, |this| this.child(create_resize_handle()))
      .child(DockElement {
        view: cx.entity().clone(),
      })
  }
}

struct DockElement {
  view: Entity<Dock>,
}

impl IntoElement for DockElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for DockElement {
  type RequestLayoutState = ();
  type PrepaintState = ();

  fn id(&self) -> Option<gpui::ElementId> {
    None
  }

  fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    window: &mut gpui::Window, cx: &mut App,
  ) -> (gpui::LayoutId, Self::RequestLayoutState) {
    (window.request_layout(Style::default(), None, cx), ())
  }

  fn prepaint(
    &mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    _: gpui::Bounds<Pixels>, _: &mut Self::RequestLayoutState, _window: &mut gpui::Window,
    _cx: &mut App,
  ) {
  }

  fn paint(
    &mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    _: gpui::Bounds<Pixels>, _: &mut Self::RequestLayoutState, _: &mut Self::PrepaintState,
    window: &mut gpui::Window, cx: &mut App,
  ) {
    // Only install the global dock-resize listeners while a resize handle is
    // actively being dragged, so side docks do not accumulate always-on window
    // listeners.
    if !self.view.read(cx).resizing {
      return;
    }

    window.on_mouse_event({
      let view = self.view.clone();
      move |e: &MouseMoveEvent, phase, window, cx| {
        if !phase.bubble() {
          return;
        }
        if !view.read(cx).resizing {
          return;
        }

        let position = e.position;
        let view_read = view.read(cx);
        if view_read.last_resize_position == Some(position) {
          return;
        }

        view.update(cx, |view, cx| view.resize(position, window, cx))
      }
    });

    // When any mouse up, stop dragging
    window.on_mouse_event({
      let view = self.view.clone();
      move |_: &MouseUpEvent, phase, window, cx| {
        if !phase.bubble() {
          return;
        }
        if !view.read(cx).resizing {
          return;
        }
        view.update(cx, |view, cx| view.done_resizing(window, cx));
      }
    })
  }
}
