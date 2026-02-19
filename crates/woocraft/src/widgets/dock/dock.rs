//! Dock is a fixed container that places at left, bottom, right of the Windows.

use std::{ops::Deref, sync::Arc};

use gpui::{
  App, AppContext, Context, Element, Empty, Entity, IntoElement, MouseMoveEvent, MouseUpEvent,
  ParentElement as _, Pixels, Point, Render, Style, StyleRefinement, Styled as _, WeakEntity,
  Window, div, prelude::FluentBuilder as _, px,
};

use super::{
  super::resizable::{PANEL_MIN_SIZE, resize_handle},
  DockArea, DockItem, PanelView, TabPanel,
};
use crate::{ActiveTheme, DockPlacement, Size, StyledExt, TabBarDirection};

#[derive(Clone)]
struct ResizePanel;

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
  /// Whether the Dock is collapsible, default: true
  pub(super) collapsible: bool,
  /// The tab bar direction for this dock
  pub(super) tab_bar_direction: TabBarDirection,

  // Runtime state
  /// Whether the Dock is resizing
  resizing: bool,
}

impl Dock {
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
      collapsible: true,
      size: px(200.0),
      tab_bar_direction,
      resizing: false,
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

  /// Update the Dock to be collapsible or not.
  ///
  /// And if the Dock is not collapsible, it will be expanded.
  pub fn set_collapsible(&mut self, collapsible: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.collapsible = collapsible;
    if !collapsible {
      self.collapsed = false
    }
    cx.notify();
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

    let dock = Self {
      placement,
      dock_area,
      panel,
      collapsed,
      size,
      collapsible: true,
      tab_bar_direction,
      resizing: false,
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
    self.size = size.max(PANEL_MIN_SIZE);
    cx.notify();
  }

  /// Set the collapsed state of the Dock.
  pub fn set_collapsed(&mut self, collapsed: bool, window: &mut Window, cx: &mut Context<Self>) {
    self.collapsed = collapsed;
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

  fn render_resize_handle(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let axis = self.placement.axis();
    let view = cx.entity().clone();

    resize_handle("resize-handle", axis)
      .placement(self.placement)
      .on_drag(ResizePanel {}, move |info, _, _, cx| {
        cx.stop_propagation();
        view.update(cx, |view, _| {
          view.resizing = true;
        });
        cx.new(|_| info.deref().clone())
      })
  }
  fn resize(&mut self, mouse_position: Point<Pixels>, _: &mut Window, cx: &mut Context<Self>) {
    if !self.resizing {
      return;
    }

    let dock_area = self
      .dock_area
      .upgrade()
      .expect("DockArea is missing")
      .read(cx);
    let area_bounds = dock_area.bounds;
    let mut left_dock_size = px(0.0);
    let mut right_dock_size = px(0.0);

    // Get the size of the left dock if it's expanded and not the current dock
    if let Some(left_dock) = &dock_area.left_dock
      && left_dock.entity_id() != cx.entity().entity_id()
    {
      let left_dock_read = left_dock.read(cx);
      if !left_dock_read.is_collapsed() {
        left_dock_size = left_dock_read.size;
      }
    }

    if let Some(right_dock) = &dock_area.right_dock
      && right_dock.entity_id() != cx.entity().entity_id()
    {
      let right_dock_read = right_dock.read(cx);
      if !right_dock_read.is_collapsed() {
        right_dock_size = right_dock_read.size;
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
        self.size = size.clamp(PANEL_MIN_SIZE, max_size);
      }
      DockPlacement::Right => {
        let max_size = area_bounds.size.width - PANEL_MIN_SIZE - left_dock_size;
        self.size = size.clamp(PANEL_MIN_SIZE, max_size);
      }
      DockPlacement::Bottom => {
        let max_size = area_bounds.size.height - PANEL_MIN_SIZE;
        self.size = size.clamp(PANEL_MIN_SIZE, max_size);
      }
      DockPlacement::Center => unreachable!(),
    }

    cx.notify();
  }

  fn done_resizing(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    self.resizing = false;
  }
}

impl Render for Dock {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
    let cache_style = StyleRefinement::default().absolute().size_full();

    let collapsed_width = px(40.0);

    div()
      .relative()
      .overflow_hidden()
      .map(|this| match self.placement {
        DockPlacement::Left | DockPlacement::Right => this.h_flex().h_full().w(self.size),
        DockPlacement::Bottom => this.w_full().h(self.size),
        DockPlacement::Center => unreachable!(),
      })
      .when(self.collapsed, |this| match self.placement {
        DockPlacement::Left | DockPlacement::Right => this.w(collapsed_width),
        DockPlacement::Bottom => this.h(Size::Medium.container_height()),
        DockPlacement::Center => this,
      })
      .map(|this| match &self.panel {
        DockItem::Split { view, .. } => this.child(view.clone()),
        DockItem::Tabs { view, .. } => this.child(view.clone()),
        DockItem::Panel { view, .. } => this.child(view.clone().view().cached(cache_style)),
        DockItem::Tiles { .. } => this,
      })
      .when(!self.collapsed, |this| {
        this.child(self.render_resize_handle(window, cx))
      })
      .when(self.collapsed, |this| match self.placement {
        DockPlacement::Left => this.child(
          div()
            .absolute()
            .top_0()
            .bottom_0()
            .right_0()
            .w(px(1.0))
            .bg(cx.theme().border),
        ),
        DockPlacement::Right => this.child(
          div()
            .absolute()
            .top_0()
            .bottom_0()
            .left_0()
            .w(px(1.0))
            .bg(cx.theme().border),
        ),
        DockPlacement::Bottom => this.child(
          div()
            .absolute()
            .left_0()
            .right_0()
            .top_0()
            .h(px(1.0))
            .bg(cx.theme().border),
        ),
        DockPlacement::Center => this,
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
    window: &mut gpui::Window, cx: &mut App,
  ) {
    window.on_mouse_event({
      let view = self.view.clone();
      let resizing = view.read(cx).resizing;
      move |e: &MouseMoveEvent, phase, window, cx| {
        if !resizing {
          return;
        }
        if !phase.bubble() {
          return;
        }

        view.update(cx, |view, cx| view.resize(e.position, window, cx))
      }
    });

    // When any mouse up, stop dragging
    window.on_mouse_event({
      let view = self.view.clone();
      move |_: &MouseUpEvent, phase, window, cx| {
        if phase.bubble() {
          view.update(cx, |view, cx| view.done_resizing(window, cx));
        }
      }
    })
  }
}
