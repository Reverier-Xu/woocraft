use std::sync::Arc;

use gpui::{
  App, AppContext as _, Axis, Context, DismissEvent, Entity, EventEmitter, FocusHandle, Focusable,
  IntoElement, ParentElement, Pixels, Render, SharedString, Styled, Subscription, WeakEntity,
  Window,
};
use smallvec::SmallVec;

use super::{
  super::resizable::{ResizablePanelEvent, ResizablePanelGroup, ResizableState, resizable_panel},
  DockArea, NodeId, Panel, PanelEvent, PanelInfo, PanelState, PanelView,
};
use crate::{ActiveTheme, IconName, h_flex};

pub struct StackPanel {
  pub(super) parent: Option<WeakEntity<StackPanel>>,
  pub(super) axis: Axis,
  pub(super) dock_area: Option<WeakEntity<DockArea>>,
  /// The `Split` node of the region's layout tree this stack mirrors, when
  /// the region is tree-driven. Side docks are still entity-driven, so `None`
  /// selects the legacy path.
  pub(crate) node_id: Option<NodeId>,
  focus_handle: FocusHandle,
  pub(crate) panels: SmallVec<[Arc<dyn PanelView>; 2]>,
  state: Entity<ResizableState>,
  _subscriptions: Vec<Subscription>,
}

impl Panel for StackPanel {
  fn panel_name(&self) -> &'static str {
    "StackPanel"
  }

  fn title(&self, _cx: &App) -> SharedString {
    "StackPanel".into()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Grid
  }

  fn set_active(&mut self, active: bool, window: &mut Window, cx: &mut Context<Self>) {
    for panel in &self.panels {
      panel.set_active(active, window, cx);
    }
  }
  fn dump(&self, cx: &App) -> PanelState {
    let sizes = self.state.read(cx).sizes().clone();
    let mut state = PanelState::new(self);
    for panel in &self.panels {
      state.add_child(panel.dump(cx));
      state.info = PanelInfo::stack(sizes.clone(), self.axis);
    }

    state
  }
}

impl StackPanel {
  pub fn new(axis: Axis, _: &mut Window, cx: &mut Context<Self>) -> Self {
    let state = cx.new(|_| ResizableState::default());

    let _subscriptions = vec![
      // Bubble up the resize event.
      cx.subscribe(&state, |_, _, _: &ResizablePanelEvent, cx| {
        cx.emit(PanelEvent::LayoutChanged)
      }),
    ];

    Self {
      axis,
      parent: None,
      dock_area: None,
      node_id: None,
      focus_handle: cx.focus_handle(),
      panels: SmallVec::new(),
      state,
      _subscriptions,
    }
  }

  /// Set the dock_area reference. Used so root StackPanel can create
  /// placeholder TabPanels when it becomes empty.
  pub(super) fn set_dock_area(&mut self, dock_area: WeakEntity<DockArea>) {
    self.dock_area = Some(dock_area);
  }

  /// Return true if self or parent only have last panel.
  pub(super) fn is_last_panel(&self, cx: &App) -> bool {
    if self.panels.len() > 1 {
      return false;
    }

    if let Some(parent) = &self.parent
      && let Some(parent) = parent.upgrade()
    {
      return parent.read(cx).is_last_panel(cx);
    }

    true
  }
}

impl StackPanel {
  /// The current divider sizes (used when adopting this stack into a layout
  /// tree, e.g. `DockArea::load`).
  pub(crate) fn sizes(&self, cx: &App) -> Vec<Pixels> {
    self.state.read(cx).sizes().clone()
  }

  /// Sync this stack's children from the layout tree: replaces the child
  /// view list and re-syncs the resizable state. Preserves the user's current
  /// divider positions when only the child set changes proportionally.
  pub(crate) fn sync_children(
    &mut self, axis: Axis, children: Vec<Arc<dyn PanelView>>, sizes: &[Option<Pixels>],
    cx: &mut Context<Self>,
  ) {
    // Split nodes hold children in order; the resizable state mirrors that
    // order with one slot per child.
    self.panels = children.into_iter().collect();
    self.axis = axis;
    let panels_len = self.panels.len();
    self.state.update(cx, |state, cx| {
      state.sync_panels_count(axis, panels_len, cx);
      state.sync_sizes(sizes.to_vec(), cx);
    });
    cx.notify();
  }
}

impl Focusable for StackPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}
impl EventEmitter<PanelEvent> for StackPanel {}
impl EventEmitter<DismissEvent> for StackPanel {}
impl Render for StackPanel {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    h_flex()
      .size_full()
      .overflow_hidden()
      .bg(cx.theme().tab_bar)
      .child(
        ResizablePanelGroup::new("stack-panel-group")
          .with_state(&self.state)
          .axis(self.axis)
          .children(self.panels.clone().into_iter().map(|panel| {
            resizable_panel()
              .child(panel.view())
              .visible(panel.visible(cx))
          })),
      )
  }
}
