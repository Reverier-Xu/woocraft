//! Dock is a fixed container that places at left, bottom, right of the Windows.

use std::{sync::Arc, time::Instant};

use gpui::{
  App, AppContext, Context, Element, Empty, Entity, IntoElement, MouseMoveEvent, MouseUpEvent,
  ParentElement as _, Pixels, Point, Render, Style, StyleRefinement, Styled as _, WeakEntity,
  Window, div, prelude::FluentBuilder as _, px,
};

use super::{super::resizable::PANEL_MIN_SIZE, DockArea, DockItem, PanelView, TabPanel};
use crate::{DockPlacement, Size, StyledExt, TabBarDirection};

/// Side docks (left/right) include a vertical tab rail plus title-bar controls.
/// They need a larger minimum width than the generic panel minimum to prevent
/// title/content overflow.
const SIDE_DOCK_MIN_SIZE: Pixels = px(16.0 * 16.0);

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
  pub(crate) panel: DockItem,
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
  /// Timestamp of the last live resize update, used to throttle high-frequency
  /// mouse events to the display refresh rate.
  last_resize_update: Option<Instant>,
}

impl Dock {
  #[inline]
  fn min_size_for_placement(placement: DockPlacement) -> Pixels {
    match placement {
      DockPlacement::Left | DockPlacement::Right => SIDE_DOCK_MIN_SIZE,
      DockPlacement::Bottom | DockPlacement::Center => PANEL_MIN_SIZE,
    }
  }

  #[inline]
  fn min_size(&self) -> Pixels {
    Self::min_size_for_placement(self.placement)
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

    let tab_panel = cx.new(|cx| {
      let mut tab = TabPanel::new(None, dock_area.clone(), window, cx);
      tab.closable = false;
      tab
    });

    let panel = DockItem::Tabs {
      size: None,
      items: Vec::new(),
      active_ix: 0,
      view: tab_panel.clone(),
    };

    Self::subscribe_panel_events(dock_area.clone(), &panel, window, cx);

    let dock = Self {
      placement,
      dock_area,
      panel,
      collapsed: false,
      size: px(200.0),
      tab_bar_direction,
      preview_size: None,
      resizing: false,
      last_resize_update: None,
    };

    let dock_entity = cx.entity().clone();
    tab_panel.update(cx, |tab_panel, _| {
      tab_panel.set_dock(dock_entity.downgrade());
    });

    dock
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
  pub fn has_panels(&self, cx: &App) -> bool {
    self.panel.has_real_panels(cx)
  }

  pub(super) fn from_state(
    dock_area: WeakEntity<DockArea>, placement: DockPlacement, size: Pixels, panel: DockItem,
    collapsed: bool, window: &mut Window, cx: &mut Context<Self>,
  ) -> Self {
    Self::subscribe_panel_events(dock_area.clone(), &panel, window, cx);

    if collapsed {
      match panel.clone() {
        DockItem::Tabs { view, .. } => {
          view.update(cx, |panel, cx| {
            panel.set_collapsed(true, window, cx);
          });
        }
        DockItem::Split { items, .. } => {
          for item in items {
            item.set_collapsed(true, window, cx);
          }
        }
        _ => {}
      }
    }

    let tab_bar_direction = match placement {
      DockPlacement::Left => TabBarDirection::Left,
      DockPlacement::Right => TabBarDirection::Right,
      DockPlacement::Bottom => TabBarDirection::default(),
      DockPlacement::Center => TabBarDirection::default(),
    };
    let min_size = Self::min_size_for_placement(placement);

    let dock = Self {
      placement,
      dock_area,
      panel,
      collapsed,
      size: size.max(min_size),
      tab_bar_direction,
      preview_size: None,
      resizing: false,
      last_resize_update: None,
    };

    let dock_entity = cx.entity().clone();
    Self::set_dock_reference(&dock.panel, dock_entity.downgrade(), cx);

    dock
  }

  fn set_dock_reference(panel: &DockItem, dock: WeakEntity<Self>, cx: &mut App) {
    match panel {
      DockItem::Tabs { view, .. } => {
        view.update(cx, |tab_panel, _| {
          tab_panel.set_dock(dock);
        });
      }
      DockItem::Split { items, .. } => {
        for item in items {
          Self::set_dock_reference(item, dock.clone(), cx);
        }
      }
      _ => {}
    }
  }

  fn subscribe_panel_events(
    dock_area: WeakEntity<DockArea>, panel: &DockItem, window: &mut Window, cx: &mut Context<Self>,
  ) {
    match panel {
      DockItem::Tabs { view, .. } => {
        window.defer(cx, {
          let view = view.clone();
          move |window, cx| {
            _ = dock_area.update(cx, |this, cx| {
              this.subscribe_panel(&view, window, cx);
            });
          }
        });
      }
      DockItem::Split { items, view, .. } => {
        for item in items {
          Self::subscribe_panel_events(dock_area.clone(), item, window, cx);
        }
        window.defer(cx, {
          let view = view.clone();
          move |window, cx| {
            _ = dock_area.update(cx, |this, cx| {
              this.subscribe_panel(&view, window, cx);
            });
          }
        });
      }
      DockItem::Tiles { view, .. } => {
        window.defer(cx, {
          let view = view.clone();
          move |window, cx| {
            _ = dock_area.update(cx, |this, cx| {
              this.subscribe_panel(&view, window, cx);
            });
          }
        });
      }
      DockItem::Panel { .. } => {
        // Not supported
      }
    }
  }

  pub fn set_panel(&mut self, panel: DockItem, _: &mut Window, cx: &mut Context<Self>) {
    let dock_weak = cx.entity().downgrade();
    Self::set_dock_reference(&panel, dock_weak, cx);
    self.panel = panel;
    cx.notify();
  }

  pub fn panel(&self) -> &DockItem {
    &self.panel
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
  pub fn set_size(&mut self, size: Pixels, _: &mut Window, cx: &mut Context<Self>) {
    self.size = size.max(self.min_size());
    self.preview_size = None;
    cx.notify();
  }

  /// Set the collapsed state of the Dock.
  pub fn set_collapsed(&mut self, collapsed: bool, window: &mut Window, cx: &mut Context<Self>) {
    self.collapsed = collapsed;
    self.preview_size = None;
    let item = self.panel.clone();
    cx.defer_in(window, move |_, window, cx| {
      item.set_collapsed(collapsed, window, cx);
    });
    cx.notify();
  }

  /// Add item to the Dock.
  pub fn add_panel(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self
      .panel
      .add_panel(panel, &self.dock_area, None, window, cx);
    cx.notify();
  }

  /// Remove item from the Dock.
  pub fn remove_panel(
    &mut self, panel: Arc<dyn PanelView>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.panel.remove_panel(panel, window, cx);
    cx.notify();
  }

  pub(super) fn set_resizing(&mut self, resizing: bool) {
    self.resizing = resizing;
    if resizing {
      self.last_resize_update = None;
    }
  }

  fn resize(&mut self, mouse_position: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
    if !self.resizing {
      return;
    }

    const RESIZE_THROTTLE: std::time::Duration = std::time::Duration::from_millis(8);
    let now = Instant::now();
    if let Some(last) = self.last_resize_update
      && now.duration_since(last) < RESIZE_THROTTLE
    {
      return;
    }
    self.last_resize_update = Some(now);

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
    match self.placement {
      DockPlacement::Left => {
        let max_size = area_bounds.size.width - PANEL_MIN_SIZE - right_dock_size;
        let next_size = size.clamp(self.min_size(), max_size);
        if self.preview_size != Some(next_size) {
          self.preview_size = Some(next_size);
          window.refresh();
        }
      }
      DockPlacement::Right => {
        let max_size = area_bounds.size.width - PANEL_MIN_SIZE - left_dock_size;
        let next_size = size.clamp(self.min_size(), max_size);
        if self.preview_size != Some(next_size) {
          self.preview_size = Some(next_size);
          window.refresh();
        }
      }
      DockPlacement::Bottom => {
        let max_size = area_bounds.size.height - PANEL_MIN_SIZE;
        let next_size = size.clamp(self.min_size(), max_size);
        if self.preview_size != Some(next_size) {
          self.preview_size = Some(next_size);
          window.refresh();
        }
      }
      DockPlacement::Center => unreachable!(),
    }
  }

  fn done_resizing(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    self.resizing = false;
    self.last_resize_update = None;
    if let Some(preview_size) = self.preview_size.take()
      && self.size != preview_size
    {
      self.size = preview_size;
      cx.notify();
    }
  }
}

impl Render for Dock {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
    let cache_style = StyleRefinement::default().absolute().size_full();

    let collapsed_width = px(40.0);

    div()
      .relative()
      .map(|this| match self.placement {
        DockPlacement::Left | DockPlacement::Right => this.h_flex().h_full().w(self.display_size()),
        DockPlacement::Bottom => this.v_flex().w_full().h(self.display_size()),
        DockPlacement::Center => unreachable!(),
      })
      .when(self.collapsed, |this| match self.placement {
        DockPlacement::Left | DockPlacement::Right => this.w(collapsed_width + px(1.0)),
        DockPlacement::Bottom => this.h(Size::Medium.container_height() + px(1.0)),
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
          .map(|this| match self.panel.clone() {
            DockItem::Split { view, .. } => this.child(view),
            DockItem::Tabs { view, .. } => this.child(view),
            DockItem::Panel { view, .. } => this.child(view.view().cached(cache_style)),
            DockItem::Tiles { .. } => this,
          });

        // Resize handles are rendered at the DockArea level as overlays
        // to ensure correct paint order across sibling dock containers.
        this.child(panel)
      })
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
    window: &mut gpui::Window, _cx: &mut App,
  ) {
    window.on_mouse_event({
      let view = self.view.clone();
      move |e: &MouseMoveEvent, phase, window, cx| {
        if !phase.bubble() {
          return;
        }
        if !view.read(cx).resizing {
          return;
        }

        view.update(cx, |view, cx| view.resize(e.position, window, cx))
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
