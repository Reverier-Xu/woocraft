//! A dock region: one [`PaneTree`] plus the panel registry and the mirror
//! cache that renders it.
//!
//! The tree is the structure's single source of truth; mirrors are created
//! when a node appears, dropped when it vanishes, and survive edits because
//! node ids are stable across normalization.

use std::{
  collections::{HashMap, HashSet},
  sync::Arc,
};

use gpui::{App, AppContext, Axis, Entity, WeakEntity, Window};

use super::{
  Dock, DockArea, DockLayout, NodeId, PaneRef, PaneTree, PanelId, PanelInfo, PanelRegistry,
  PanelState, PanelView, RootKind, StackPanel, TabPanel,
};
use crate::widgets::resizable::PANEL_MIN_SIZE;

/// Which region of the dock area.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub(crate) enum RegionKind {
  Center,
  Left,
  Right,
  Bottom,
}

/// A mirror entity created by [`DockRegion::sync`] that still needs its
/// subscriptions registered by the owning `DockArea`.
pub(crate) enum CreatedMirror {
  Tab(Entity<TabPanel>),
  Split(Entity<StackPanel>),
}

pub(crate) struct DockRegion {
  pub(crate) tree: PaneTree,
  pub(crate) registry: HashMap<PanelId, Arc<dyn PanelView>>,
  pub(crate) tab_mirrors: HashMap<NodeId, Entity<TabPanel>>,
  pub(crate) split_mirrors: HashMap<NodeId, Entity<StackPanel>>,
  /// The owning dock, when this region is a side dock; its tab groups are
  /// not closable and clicks expand a collapsed dock.
  pub(crate) dock: Option<WeakEntity<Dock>>,
  /// Whether tab groups in this region may be closed (docks disallow it).
  pub(crate) tab_closable: bool,
}

impl DockRegion {
  pub(crate) fn new(root_kind: RootKind) -> Self {
    Self {
      tree: PaneTree::new(root_kind),
      registry: HashMap::new(),
      tab_mirrors: HashMap::new(),
      split_mirrors: HashMap::new(),
      dock: None,
      tab_closable: true,
    }
  }

  /// Marks this region as belonging to a dock.
  pub(crate) fn set_dock(&mut self, dock: WeakEntity<Dock>) {
    self.dock = Some(dock);
    self.tab_closable = false;
  }

  /// Re-syncs the mirror entities from the tree after an edit, returning the
  /// mirrors that were just created (their subscriptions are the caller's
  /// responsibility).
  ///
  /// Diff-driven: panels joining a group get their `on_added_to` hook;
  /// panels leaving a group stay silent — the close path fires `on_removed`
  /// from the command layer, by design: a drag is not a close.
  pub(crate) fn sync(
    &mut self, dock_area: &WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
  ) -> Vec<CreatedMirror> {
    let tree = self.tree.clone();
    let mut created = Vec::new();
    let mut seen_tabs = HashSet::new();
    let mut seen_stacks = HashSet::new();
    self.sync_node(
      &tree,
      tree.root().id(),
      None,
      dock_area,
      &mut created,
      &mut seen_tabs,
      &mut seen_stacks,
      window,
      cx,
    );
    self.tab_mirrors.retain(|node, _| seen_tabs.contains(node));
    self
      .split_mirrors
      .retain(|node, _| seen_stacks.contains(node));
    self.registry.retain(|id, _| tree.contains_panel(*id));
    created
  }

  #[allow(clippy::too_many_arguments)]
  fn sync_node(
    &mut self, tree: &PaneTree, node: NodeId, parent_stack: Option<WeakEntity<StackPanel>>,
    dock_area: &WeakEntity<DockArea>, created: &mut Vec<CreatedMirror>,
    seen_tabs: &mut HashSet<NodeId>, seen_stacks: &mut HashSet<NodeId>, window: &mut Window,
    cx: &mut App,
  ) {
    let Some(pane_ref) = tree.pane_ref(node) else {
      return;
    };
    match pane_ref {
      PaneRef::Tabs { panels, active_ix } => {
        seen_tabs.insert(node);
        let panels: Vec<Arc<dyn PanelView>> = panels
          .iter()
          .filter_map(|id| self.registry.get(id).cloned())
          .collect();
        let dock = self.dock.clone();
        let tab_closable = self.tab_closable;
        let tab_panel = match self.tab_mirrors.get(&node) {
          Some(tp) => tp.clone(),
          None => {
            let tp = cx.new(|cx| {
              let mut tp = TabPanel::new(parent_stack.clone(), dock_area.clone(), window, cx);
              tp.node_id = Some(node);
              tp.closable = tab_closable;
              if let Some(dock) = dock {
                tp.set_dock(dock);
              }
              tp
            });
            self.tab_mirrors.insert(node, tp.clone());
            created.push(CreatedMirror::Tab(tp.clone()));
            tp
          }
        };
        tab_panel.update(cx, |tp, cx| {
          // Refresh the parent link: normalization can collapse a split
          // between edits, changing which stack this group sits under.
          if let Some(parent_stack) = parent_stack.clone() {
            tp.set_parent(parent_stack);
          }
          tp.sync_from_tree(panels, active_ix, window, cx);
        });
      }
      PaneRef::Split {
        axis,
        children,
        sizes,
      } => {
        seen_stacks.insert(node);
        let stack_panel = match self.split_mirrors.get(&node) {
          Some(sp) => sp.clone(),
          None => {
            let sp = cx.new(|cx| {
              let mut sp = StackPanel::new(axis, window, cx);
              sp.set_dock_area(dock_area.clone());
              sp.node_id = Some(node);
              sp
            });
            self.split_mirrors.insert(node, sp.clone());
            created.push(CreatedMirror::Split(sp.clone()));
            sp
          }
        };
        // Recurse into children first so their mirrors exist before the
        // parent's sync references them.
        let child_nodes: Vec<NodeId> = children.iter().map(|child| child.id()).collect();
        for child_id in child_nodes {
          self.sync_node(
            tree,
            child_id,
            Some(stack_panel.downgrade()),
            dock_area,
            created,
            seen_tabs,
            seen_stacks,
            window,
            cx,
          );
        }
        let child_views: Vec<Arc<dyn PanelView>> = children
          .iter()
          .filter_map(|child| match child.kind() {
            PaneRef::Tabs { .. } => self
              .tab_mirrors
              .get(&child.id())
              .map(|tp| Arc::new(tp.clone()) as Arc<dyn PanelView>),
            PaneRef::Split { .. } => self
              .split_mirrors
              .get(&child.id())
              .map(|sp| Arc::new(sp.clone()) as Arc<dyn PanelView>),
          })
          .collect();
        stack_panel.update(cx, |sp, cx| {
          sp.sync_children(axis, child_views, sizes, cx);
        });
      }
    }
  }

  /// The root mirror view of the region, for rendering.
  pub(crate) fn root_view(&self) -> Option<Arc<dyn PanelView>> {
    let root = self.tree.root().id();
    match self.tree.pane_ref(root)? {
      PaneRef::Tabs { .. } => self
        .tab_mirrors
        .get(&root)
        .map(|tp| Arc::new(tp.clone()) as Arc<dyn PanelView>),
      PaneRef::Split { .. } => self
        .split_mirrors
        .get(&root)
        .map(|sp| Arc::new(sp.clone()) as Arc<dyn PanelView>),
    }
  }

  /// The first tab group of the region, if any.
  pub(crate) fn first_tab_group(&self) -> Option<NodeId> {
    self.tree.tab_groups().first().copied()
  }

  /// Whether the region holds any real panel.
  pub(crate) fn has_real_panels(&self) -> bool {
    self.tree.tab_groups().iter().any(|node| {
      self
        .tree
        .tabs_of(*node)
        .is_some_and(|(panels, _)| !panels.is_empty())
    })
  }

  /// Serializes the region tree into a [`PanelState`], reading live divider
  /// sizes from the split mirrors when they exist.
  pub(crate) fn to_panel_state(&self, cx: &App) -> PanelState {
    fn node_to_state(region: &DockRegion, node: &super::layout::PaneNode, cx: &App) -> PanelState {
      match node.kind() {
        PaneRef::Split {
          axis,
          children,
          sizes,
        } => {
          let mut state = PanelState {
            panel_name: "StackPanel".to_string(),
            ..Default::default()
          };
          for child in children {
            state.add_child(node_to_state(region, child, cx));
          }
          let sizes = match region.split_mirrors.get(&node.id()) {
            Some(stack) => stack.read(cx).sizes(cx),
            None => sizes
              .iter()
              .map(|slot| slot.unwrap_or(PANEL_MIN_SIZE))
              .collect(),
          };
          state.info = PanelInfo::stack(sizes, axis);
          state
        }
        PaneRef::Tabs { panels, active_ix } => {
          let mut state = PanelState {
            panel_name: "TabPanel".to_string(),
            ..Default::default()
          };
          for id in panels {
            if let Some(view) = region.registry.get(id) {
              state.add_child(view.dump(cx));
            }
          }
          state.info = PanelInfo::tabs(active_ix);
          state
        }
      }
    }

    node_to_state(self, self.tree.root(), cx)
  }

  /// Builds a region from a serialized layout, constructing panels through
  /// the [`PanelRegistry`]. Tiles states are not representable in a tree and
  /// yield an empty region.
  pub(crate) fn from_panel_state(
    state: &PanelState, root_kind: RootKind, dock_area: &WeakEntity<DockArea>, window: &mut Window,
    cx: &mut App,
  ) -> Self {
    fn layout_of(
      state: &PanelState, registry: &mut HashMap<PanelId, Arc<dyn PanelView>>,
      dock_area: &WeakEntity<DockArea>, window: &mut Window, cx: &mut App,
    ) -> DockLayout {
      match &state.info {
        PanelInfo::Stack { sizes, .. } => {
          let axis = state.info.axis().unwrap_or(Axis::Horizontal);
          let mut layout = match axis {
            Axis::Horizontal => DockLayout::h_split(),
            Axis::Vertical => DockLayout::v_split(),
          };
          for (ix, child) in state.children.iter().enumerate() {
            layout = layout.child(
              layout_of(child, registry, dock_area, window, cx),
              sizes.get(ix).copied(),
            );
          }
          layout
        }
        PanelInfo::Tabs { active_index } => {
          let mut layout = DockLayout::tabs().active_index(*active_index);
          for child in &state.children {
            let view: Arc<dyn PanelView> = PanelRegistry::build_panel(
              &child.panel_name,
              dock_area.clone(),
              child,
              &child.info,
              window,
              cx,
            )
            .into();
            let id = PanelId::from(view.entity_id(cx));
            registry.insert(id, view);
            layout = layout.panel_id(id);
          }
          layout
        }
        PanelInfo::Panel(_) => {
          let view: Arc<dyn PanelView> = PanelRegistry::build_panel(
            &state.panel_name,
            dock_area.clone(),
            state,
            &state.info,
            window,
            cx,
          )
          .into();
          let id = PanelId::from(view.entity_id(cx));
          registry.insert(id, view);
          DockLayout::tabs().panel_id(id)
        }
        // Tiles states predate tree-driven regions; the layout version bump
        // on the caller side retires them. Yield an empty group.
        PanelInfo::Tiles { .. } => DockLayout::tabs(),
      }
    }

    let mut registry = HashMap::new();
    let layout = layout_of(state, &mut registry, dock_area, window, cx);
    let mut region = DockRegion::new(root_kind);
    region.tree = PaneTree::from_layout(root_kind, layout);
    region.registry = registry;
    region
  }
}
