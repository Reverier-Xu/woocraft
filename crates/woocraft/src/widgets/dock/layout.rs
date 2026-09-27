//! Value-type layout tree for the dock (方案 B).
//!
//! The tree is pure data: containers are addressed by [`NodeId`], panels by
//! [`PanelId`], and no entity handle lives in here, so a tree can be built,
//! edited, compared and serialized without an `App`. Every edit normalizes
//! before returning, so callers can never observe an empty tab group, a
//! one-child split, or an out-of-range active index.
//!
//! Invariants enforced by normalization:
//! - a tab group never holds zero panels unless it is the root of an
//!   [`RootKind::Any`] tree (an emptied dock keeps an empty tab group as its
//!   drop target);
//! - a non-root split always holds at least two children (single-child splits
//!   collapse into their parent);
//! - the root of a [`RootKind::Split`] tree stays a split even when empty (the
//!   center serializes as a stack panel);
//! - a split's `sizes` always has one slot per child;
//! - a tab group's `active_ix` is always in range.
//!
//! Architecture reference: gpui-kit's `PaneTree`
//! (`crates/base/src/dock/layout`).

use std::sync::{
  Arc,
  atomic::{AtomicU64, Ordering},
};

use gpui::{App, Axis, Entity, EntityId, Pixels};
use smallvec::SmallVec;

use super::panel::{Panel, PanelView};
use crate::Placement;

/// Stable container identity. Survives structural edits and normalization so
/// the `DockArea` view cache does not tear down and rebuild entities.
///
/// Minted from a process-global counter: a `DockArea` owns several trees (the
/// center plus one per dock), and entity caches keyed by [`NodeId`] alone
/// must not collide across them.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct NodeId(u64);

impl NodeId {
  fn next() -> Self {
    static NEXT_NODE_ID: AtomicU64 = AtomicU64::new(0);
    Self(NEXT_NODE_ID.fetch_add(1, Ordering::Relaxed))
  }

  pub fn as_u64(self) -> u64 {
    self.0
  }
}

/// Stable panel identity, wrapping the panel entity's [`EntityId`] so the
/// layout algebra can be exercised without an `App`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct PanelId(u64);

impl PanelId {
  pub fn as_u64(self) -> u64 {
    self.0
  }
}

impl From<EntityId> for PanelId {
  fn from(id: EntityId) -> Self {
    Self(id.as_u64())
  }
}

/// The shape of one container. Private: every mutation goes through
/// [`PaneTree`] so normalization always runs.
#[derive(Clone, PartialEq, Debug)]
pub(crate) enum NodeKind {
  Split {
    axis: Axis,
    children: Vec<PaneNode>,
    sizes: Vec<Option<Pixels>>,
  },
  Tabs {
    panels: Vec<PanelId>,
    active_ix: usize,
  },
}

/// One container in the tree: a [`NodeKind`] paired with its [`NodeId`].
#[derive(Clone, PartialEq, Debug)]
pub struct PaneNode {
  id: NodeId,
  kind: NodeKind,
}

impl PaneNode {
  fn new(kind: NodeKind) -> Self {
    Self {
      id: NodeId::next(),
      kind,
    }
  }

  pub fn id(&self) -> NodeId {
    self.id
  }

  /// Borrowed read-only projection of this node's shape.
  pub fn kind(&self) -> PaneRef<'_> {
    match &self.kind {
      NodeKind::Split {
        axis,
        children,
        sizes,
      } => PaneRef::Split {
        axis: *axis,
        children,
        sizes,
      },
      NodeKind::Tabs { panels, active_ix } => PaneRef::Tabs {
        panels,
        active_ix: *active_ix,
      },
    }
  }

  /// Depth-first pre-order walk over this node and its descendants.
  pub fn walk(&self, f: &mut impl FnMut(&PaneNode)) {
    f(self);
    if let NodeKind::Split { children, .. } = &self.kind {
      for child in children {
        child.walk(f);
      }
    }
  }
}

/// Borrowed read-only projection of a node's shape.
#[derive(Clone, Copy, Debug)]
pub enum PaneRef<'a> {
  Split {
    axis: Axis,
    children: &'a [PaneNode],
    sizes: &'a [Option<Pixels>],
  },
  Tabs {
    panels: &'a [PanelId],
    active_ix: usize,
  },
}

/// Whether the root of a tree is pinned to a split.
///
/// The center of a `DockArea` must serialize as a stack panel even when
/// empty, which [`RootKind::Split`] guarantees. A dock's root is
/// unconstrained: it may collapse to a bare tab group, and an emptied dock
/// keeps an empty tab group as its drop target.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum RootKind {
  #[default]
  Split,
  Any,
}

/// Path from the root as a sequence of child indices.
type NodePath = SmallVec<[usize; 8]>;

/// One region's layout, as pure data.
///
/// A `DockArea` owns one of these per region — the center plus each dock it
/// has. Every edit method normalizes before returning and reports whether
/// anything changed as an [`EditResult`], so there is no window in which a
/// caller can observe a broken invariant.
#[derive(Clone, PartialEq, Debug)]
pub struct PaneTree {
  root: PaneNode,
  root_kind: RootKind,
}

impl PaneTree {
  pub fn new(root_kind: RootKind) -> Self {
    let root = match root_kind {
      RootKind::Split => PaneNode::new(NodeKind::Split {
        axis: Axis::Horizontal,
        children: Vec::new(),
        sizes: Vec::new(),
      }),
      RootKind::Any => PaneNode::new(NodeKind::Tabs {
        panels: Vec::new(),
        active_ix: 0,
      }),
    };
    Self { root, root_kind }
  }

  /// Builds a tree from a [`DockLayout`] description.
  pub fn from_layout(root_kind: RootKind, layout: DockLayout) -> Self {
    let mut tree = Self {
      root: layout.materialize(),
      root_kind,
    };
    tree.renormalize();
    tree
  }

  pub fn root(&self) -> &PaneNode {
    &self.root
  }

  pub fn root_kind(&self) -> RootKind {
    self.root_kind
  }

  /// Path from the root to the node with `id`, if present.
  pub fn path_of_node(&self, id: NodeId) -> Option<NodePath> {
    self.root.path_of_node(id)
  }

  /// Whether the tree holds `panel` in any tab group.
  pub fn contains_panel(&self, panel: PanelId) -> bool {
    fn search(node: &PaneNode, panel: PanelId) -> bool {
      match &node.kind {
        NodeKind::Split { children, .. } => children.iter().any(|child| search(child, panel)),
        NodeKind::Tabs { panels, .. } => panels.contains(&panel),
      }
    }

    search(&self.root, panel)
  }

  /// The tab group holding `panel`, if any.
  pub fn tab_group_of(&self, panel: PanelId) -> Option<NodeId> {
    fn search(node: &PaneNode, panel: PanelId) -> Option<NodeId> {
      match &node.kind {
        NodeKind::Split { children, .. } => children.iter().find_map(|child| search(child, panel)),
        NodeKind::Tabs { panels, .. } => panels.contains(&panel).then_some(node.id),
      }
    }

    search(&self.root, panel)
  }

  /// The panels and active index of the tab group `node`, if it is one.
  pub fn tabs_of(&self, node: NodeId) -> Option<(&[PanelId], usize)> {
    let path = self.path_of_node(node)?;
    match self.root.node_at(&path).kind() {
      PaneRef::Tabs { panels, active_ix } => Some((panels, active_ix)),
      PaneRef::Split { .. } => None,
    }
  }

  /// Every tab group in the tree, in pre-order.
  pub fn tab_groups(&self) -> Vec<NodeId> {
    let mut groups = Vec::new();
    self.root.walk(&mut |node| {
      if matches!(node.kind(), PaneRef::Tabs { .. }) {
        groups.push(node.id);
      }
    });
    groups
  }

  /// Inserts `panel` at `target`. A no-op if the panel is already present or
  /// the target node does not resolve.
  pub fn insert_panel(&mut self, panel: PanelId, target: InsertTarget) -> EditResult {
    self.edit(|root| root.apply_insert(panel, target))
  }
  /// Removes `panel` from whichever tab group holds it.
  pub fn remove_panel(&mut self, panel: PanelId) -> EditResult {
    self.edit(|root| root.detach_panel(panel))
  }

  /// Moves `panel` to a new home. A drag never has to fire "removed"
  /// semantics: from the tree's perspective the panel just changes address.
  pub fn move_panel(&mut self, panel: PanelId, target: InsertTarget) -> EditResult {
    self.edit(|root| {
      let detached = root.detach_panel(panel);
      let inserted = root.apply_insert(panel, target);
      detached || inserted
    })
  }

  /// Inserts `panel` beside `at`, in a fresh tab group placed by `placement`.
  pub fn split(
    &mut self, at: NodeId, panel: PanelId, placement: Placement, size: Option<Pixels>,
  ) -> EditResult {
    self.insert_panel(
      panel,
      InsertTarget::Split {
        node: at,
        placement,
        size,
      },
    )
  }

  /// Sets the active panel index of the tab group `node`.
  pub fn set_active(&mut self, node: NodeId, ix: usize) -> EditResult {
    self.edit(|root| {
      let Some(path) = root.path_of_node(node) else {
        return false;
      };
      let NodeKind::Tabs { active_ix, .. } = &mut root.node_at_mut(&path).kind else {
        return false;
      };
      if *active_ix == ix {
        return false;
      }
      *active_ix = ix;
      true
    })
  }

  /// Replaces a split's slot sizes wholesale.
  ///
  /// A no-op, like every other operation given input it cannot resolve, if
  /// `new_sizes.len()` does not match the split's child count: no rule in
  /// normalization repairs a length mismatch, so applying it would otherwise
  /// leave `children.len() != sizes.len()` and trip the normalization
  /// `debug_assert!`.
  pub fn set_sizes(&mut self, node: NodeId, new_sizes: Vec<Option<Pixels>>) -> EditResult {
    self.edit(|root| {
      let Some(path) = root.path_of_node(node) else {
        return false;
      };
      let NodeKind::Split {
        children, sizes, ..
      } = &mut root.node_at_mut(&path).kind
      else {
        return false;
      };
      if new_sizes.len() != children.len() || *sizes == new_sizes {
        return false;
      }
      *sizes = new_sizes;
      true
    })
  }

  /// Runs `apply` against the root and renormalizes when it reports a change.
  fn edit(&mut self, apply: impl FnOnce(&mut PaneNode) -> bool) -> EditResult {
    let changed = apply(&mut self.root);
    if changed {
      self.renormalize();
    }
    EditResult { changed }
  }

  fn renormalize(&mut self) {
    let root = std::mem::replace(
      &mut self.root,
      PaneNode::new(NodeKind::Tabs {
        panels: Vec::new(),
        active_ix: 0,
      }),
    );
    // The root of a tree is always kept: normalization maps a `Split`-pinned
    // root to itself (possibly emptied) and an `Any` root to a valid
    // container (a tab group, or a split with at least two children).
    self.root = normalize(root, true, self.root_kind).unwrap_or_else(|| {
      PaneNode::new(NodeKind::Tabs {
        panels: Vec::new(),
        active_ix: 0,
      })
    });
  }
}

/// Where [`PaneTree::insert_panel`] puts a panel.
#[derive(Clone, Copy, Debug)]
pub enum InsertTarget {
  /// Into an existing tab group, optionally at a specific index.
  Tabs {
    node: NodeId,
    ix: Option<usize>,
    activate: bool,
  },
  /// Beside an existing node, creating a new tab group for the panel.
  Split {
    node: NodeId,
    placement: Placement,
    size: Option<Pixels>,
  },
}

/// Whether one edit changed the tree.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EditResult {
  changed: bool,
}

impl EditResult {
  pub fn changed(&self) -> bool {
    self.changed
  }

  /// Panics when an edit unexpectedly did nothing (tests and examples).
  pub fn expect_changed(&self) {
    assert!(self.changed, "expected the edit to change the tree");
  }
}

impl PaneNode {
  fn path_of_node(&self, id: NodeId) -> Option<NodePath> {
    fn search(node: &PaneNode, id: NodeId, path: &mut NodePath) -> bool {
      if node.id == id {
        return true;
      }
      if let NodeKind::Split { children, .. } = &node.kind {
        for (ix, child) in children.iter().enumerate() {
          path.push(ix);
          if search(child, id, path) {
            return true;
          }
          path.pop();
        }
      }
      false
    }

    let mut path = NodePath::new();
    search(self, id, &mut path).then_some(path)
  }

  fn apply_insert(&mut self, panel: PanelId, target: InsertTarget) -> bool {
    match target {
      InsertTarget::Tabs { node, ix, activate } => {
        let Some(path) = self.path_of_node(node) else {
          return false;
        };
        let NodeKind::Tabs { panels, active_ix } = &mut self.node_at_mut(&path).kind else {
          return false;
        };
        if panels.contains(&panel) {
          return false;
        }
        let ix = ix.unwrap_or(panels.len()).min(panels.len());
        panels.insert(ix, panel);
        if activate {
          *active_ix = ix;
        }
        true
      }
      InsertTarget::Split {
        node,
        placement,
        size,
      } => self.apply_split_insert(panel, node, placement, size),
    }
  }

  fn apply_split_insert(
    &mut self, panel: PanelId, node: NodeId, placement: Placement, size: Option<Pixels>,
  ) -> bool {
    let Some(path) = self.path_of_node(node) else {
      return false;
    };
    let new_tabs = PaneNode::new(NodeKind::Tabs {
      panels: vec![panel],
      active_ix: 0,
    });
    let placement_axis = placement.axis();

    // A childless split target (an emptied center) simply gains the tab
    // group; its axis follows the placement.
    if let NodeKind::Split {
      axis,
      children,
      sizes,
    } = &mut self.node_at_mut(&path).kind
      && children.is_empty()
    {
      *axis = placement_axis;
      children.push(new_tabs);
      sizes.push(size);
      return true;
    }

    let target_axis = match self.node_at(&path).kind() {
      PaneRef::Split { axis, .. } => Some(axis),
      PaneRef::Tabs { .. } => None,
    };

    if target_axis == Some(placement_axis) {
      // Same axis: the new tab group becomes a sibling of the target. When
      // the target is the root split, it gains the group directly.
      let (parent_path, ix) = match path.as_slice() {
        [] => (&[][..], 0),
        [.., last] => (&path[..path.len() - 1], *last),
      };
      let NodeKind::Split {
        children, sizes, ..
      } = &mut self.node_at_mut(parent_path).kind
      else {
        return false;
      };
      let at = match placement {
        Placement::Left | Placement::Top => ix,
        Placement::Right | Placement::Bottom => ix + 1,
      }
      .min(children.len());
      children.insert(at, new_tabs);
      sizes.insert(at, size);
      return true;
    }

    // Different axis (or a root tab group): wrap the target in a fresh split
    // holding [target, new] or [new, target].
    let target = self.node_at(&path).clone();
    let (first, second, first_size, second_size) = match placement {
      Placement::Left | Placement::Top => (target, new_tabs, None, size),
      Placement::Right | Placement::Bottom => (new_tabs, target, size, None),
    };
    let replacement = PaneNode::new(NodeKind::Split {
      axis: placement_axis,
      children: vec![first, second],
      sizes: vec![first_size, second_size],
    });
    match path.as_slice() {
      [] => *self = replacement,
      [.., last] => {
        let parent_path = &path[..path.len() - 1];
        let NodeKind::Split { children, .. } = &mut self.node_at_mut(parent_path).kind else {
          unreachable!("path descends only through splits");
        };
        children[*last] = replacement;
      }
    }
    true
  }

  fn detach_panel(&mut self, panel: PanelId) -> bool {
    fn detach(node: &mut PaneNode, panel: PanelId) -> bool {
      match &mut node.kind {
        NodeKind::Tabs { panels, active_ix } => {
          let Some(ix) = panels.iter().position(|p| *p == panel) else {
            return false;
          };
          panels.remove(ix);
          *active_ix = (*active_ix).min(panels.len().saturating_sub(1));
          true
        }
        NodeKind::Split { children, .. } => children.iter_mut().any(|child| detach(child, panel)),
      }
    }

    detach(self, panel)
  }

  fn node_at(&self, path: &[usize]) -> &PaneNode {
    let mut node = self;
    for &ix in path {
      let NodeKind::Split { children, .. } = &node.kind else {
        unreachable!("paths only descend through splits");
      };
      node = &children[ix];
    }
    node
  }

  fn node_at_mut(&mut self, path: &[usize]) -> &mut PaneNode {
    let mut node = self;
    for &ix in path {
      let NodeKind::Split { children, .. } = &mut node.kind else {
        unreachable!("paths only descend through splits");
      };
      node = &mut children[ix];
    }
    node
  }
}

/// Post-order normalization. Returns `None` when the parent must drop the
/// node (an emptied non-root tab group or an emptied non-root split).
fn normalize(node: PaneNode, is_root: bool, root_kind: RootKind) -> Option<PaneNode> {
  let mut node = node;

  // 1) Normalize children first, dropping the ones that want to vanish.
  if let NodeKind::Split {
    children, sizes, ..
  } = &mut node.kind
  {
    let mut kept_children = Vec::with_capacity(children.len());
    let mut kept_sizes = Vec::with_capacity(sizes.len());
    for (child, size) in children.drain(..).zip(sizes.drain(..)) {
      if let Some(kept) = normalize(child, false, root_kind) {
        kept_children.push(kept);
        kept_sizes.push(size);
      }
    }
    *children = kept_children;
    *sizes = kept_sizes;
  }

  // 2) Structural rules for this node.
  match &mut node.kind {
    NodeKind::Tabs { panels, active_ix } => {
      *active_ix = (*active_ix).min(panels.len().saturating_sub(1));
      if panels.is_empty() && !(is_root && root_kind == RootKind::Any) {
        return None;
      }
    }
    NodeKind::Split {
      children, sizes, ..
    } => {
      debug_assert_eq!(children.len(), sizes.len(), "split slots out of sync");
      if children.is_empty() {
        if is_root && root_kind == RootKind::Split {
          return Some(node); // the emptied center keeps its split root
        }
        if is_root {
          // An `Any` root becomes an empty tab group.
          return Some(PaneNode::new(NodeKind::Tabs {
            panels: Vec::new(),
            active_ix: 0,
          }));
        }
        return None;
      }
      // A non-root split with a single child collapses into its parent; the
      // child was already normalized above and is structurally valid.
      if children.len() == 1 && !is_root {
        return children.pop();
      }
    }
  }
  Some(node)
}

/// Builder for a [`DockLayout`] tree, mirroring the shape of the former
/// `DockItem` constructors (`tabs` / `split` / `panel` / `size` /
/// `active_index`).
///
/// ```ignore
/// DockLayout::v_split()
///   .child(DockLayout::tabs().panel_view(left, cx), Some(px(200.)))
///   .child(DockLayout::tabs().panel_view(center, cx), None)
/// ```
#[derive(Clone, Debug)]
pub struct DockLayout {
  kind: LayoutSeed,
  children: Vec<(DockLayout, Option<Pixels>)>,
  active_ix: usize,
}

#[derive(Clone, Debug)]
enum LayoutSeed {
  Split { axis: Axis },
  Tabs { panels: Vec<PanelId> },
}

impl DockLayout {
  pub fn h_split() -> Self {
    Self {
      kind: LayoutSeed::Split {
        axis: Axis::Horizontal,
      },
      children: Vec::new(),
      active_ix: 0,
    }
  }

  pub fn v_split() -> Self {
    Self {
      kind: LayoutSeed::Split {
        axis: Axis::Vertical,
      },
      children: Vec::new(),
      active_ix: 0,
    }
  }

  pub fn tabs() -> Self {
    Self {
      kind: LayoutSeed::Tabs { panels: Vec::new() },
      children: Vec::new(),
      active_ix: 0,
    }
  }

  /// Attaches `child` with a slot size (`None` lets the resizable container
  /// distribute the leftover space).
  pub fn child(mut self, child: DockLayout, size: Option<Pixels>) -> Self {
    debug_assert!(
      matches!(self.kind, LayoutSeed::Split { .. }),
      "children can only be attached to a split"
    );
    self.children.push((child, size));
    self
  }

  /// Adds a panel view to this tab group.
  pub fn panel_view(mut self, panel: Arc<dyn PanelView>, cx: &App) -> Self {
    debug_assert!(
      matches!(self.kind, LayoutSeed::Tabs { .. }),
      "panels can only be added to a tab group"
    );
    if let LayoutSeed::Tabs { panels } = &mut self.kind {
      panels.push(PanelId::from(panel.entity_id(cx)));
    }
    self
  }

  /// Adds a panel entity to this tab group.
  pub fn panel<P: Panel>(self, panel: Entity<P>, cx: &App) -> Self {
    let view: Arc<dyn PanelView> = Arc::new(panel);
    self.panel_view(view, cx)
  }

  /// Adds an already-addressed panel to this tab group (crate-internal:
  /// tests and the registry wiring).
  pub(crate) fn panel_id(mut self, panel: PanelId) -> Self {
    debug_assert!(
      matches!(self.kind, LayoutSeed::Tabs { .. }),
      "panels can only be added to a tab group"
    );
    if let LayoutSeed::Tabs { panels } = &mut self.kind {
      panels.push(panel);
    }
    self
  }

  /// Sets the initially active tab index.
  pub fn active_index(mut self, ix: usize) -> Self {
    self.active_ix = ix;
    self
  }

  fn materialize(self) -> PaneNode {
    match self.kind {
      LayoutSeed::Split { axis } => {
        let mut children = Vec::with_capacity(self.children.len());
        let mut sizes = Vec::with_capacity(self.children.len());
        for (child, size) in self.children {
          children.push(child.materialize());
          sizes.push(size);
        }
        PaneNode::new(NodeKind::Split {
          axis,
          children,
          sizes,
        })
      }
      LayoutSeed::Tabs { panels } => PaneNode::new(NodeKind::Tabs {
        panels,
        active_ix: self.active_ix,
      }),
    }
  }
}

#[cfg(test)]
mod tests {
  use gpui::px;

  use super::*;

  fn panel(id: u64) -> PanelId {
    PanelId(id)
  }

  fn split_node(tree: &PaneTree) -> NodeId {
    tree.root().id()
  }

  fn tabs_ids(tree: &PaneTree) -> Vec<NodeId> {
    tree.tab_groups()
  }

  fn tabs_panel_count(tree: &PaneTree, node: NodeId) -> usize {
    tree.tabs_of(node).unwrap().0.len()
  }

  impl DockLayout {
    /// Test-side alias for [`DockLayout::panel_id`], keeping test bodies
    /// focused on the shape they describe.
    fn panel_view_raw(self, panel: PanelId) -> Self {
      self.panel_id(panel)
    }
  }

  #[test]
  fn split_pinned_root_survives_emptied_center() {
    let mut tree = PaneTree::new(RootKind::Split);
    let root = split_node(&tree);

    tree
      .insert_panel(
        panel(1),
        InsertTarget::Split {
          node: root,
          placement: Placement::Right,
          size: None,
        },
      )
      .expect_changed();
    assert!(tree.contains_panel(panel(1)));

    tree.remove_panel(panel(1)).expect_changed();
    assert!(!tree.contains_panel(panel(1)));
    // The root stays a split even though it is empty.
    assert!(matches!(tree.root().kind(), PaneRef::Split { .. }));
    assert!(tree.tab_groups().is_empty());
  }

  #[test]
  fn any_root_keeps_empty_tabs_as_drop_target() {
    let mut tree = PaneTree::new(RootKind::Any);
    assert!(matches!(tree.root().kind(), PaneRef::Tabs { .. }));

    tree
      .insert_panel(
        panel(1),
        InsertTarget::Tabs {
          node: tree.root().id(),
          ix: None,
          activate: true,
        },
      )
      .expect_changed();
    tree.remove_panel(panel(1)).expect_changed();

    // The emptied dock keeps a tab group as its drop target.
    assert_eq!(tabs_ids(&tree).len(), 1);
    assert_eq!(tabs_panel_count(&tree, tabs_ids(&tree)[0]), 0);
  }

  #[test]
  fn insert_tabs_target_activates_and_respects_index() {
    let mut tree = PaneTree::new(RootKind::Any);
    let root = tree.root().id();

    tree
      .insert_panel(
        panel(1),
        InsertTarget::Tabs {
          node: root,
          ix: None,
          activate: true,
        },
      )
      .expect_changed();
    tree
      .insert_panel(
        panel(2),
        InsertTarget::Tabs {
          node: root,
          ix: None,
          activate: true,
        },
      )
      .expect_changed();
    tree
      .insert_panel(
        panel(3),
        InsertTarget::Tabs {
          node: root,
          ix: Some(1),
          activate: false,
        },
      )
      .expect_changed();

    let (panels, active_ix) = tree.tabs_of(root).unwrap();
    assert_eq!(
      panels,
      &[panel(1), panel(3), panel(2)][..],
      "inserted at index 1 without activating"
    );
    assert_eq!(
      active_ix, 1,
      "activate: false must keep the previously active panel"
    );
  }

  #[test]
  fn insert_is_noop_when_panel_already_present() {
    let mut tree = PaneTree::new(RootKind::Any);
    let root = tree.root().id();
    tree
      .insert_panel(
        panel(1),
        InsertTarget::Tabs {
          node: root,
          ix: None,
          activate: true,
        },
      )
      .expect_changed();

    let before = tree.clone();
    let result = tree.insert_panel(
      panel(1),
      InsertTarget::Tabs {
        node: root,
        ix: None,
        activate: true,
      },
    );
    assert!(!result.changed());
    assert_eq!(tree, before);
  }

  #[test]
  fn split_same_axis_inserts_sibling() {
    let mut tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split().child(DockLayout::tabs().panel_view_raw(panel(1)), None),
    );
    let tabs = tabs_ids(&tree)[0];

    tree
      .split(tabs, panel(2), Placement::Right, None)
      .expect_changed();

    // The root split now holds both tab groups, side by side.
    assert_eq!(tabs_ids(&tree).len(), 2);
    assert!(tree.contains_panel(panel(1)) && tree.contains_panel(panel(2)));
  }

  #[test]
  fn split_different_axis_wraps_target() {
    let mut tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split().child(DockLayout::tabs().panel_view_raw(panel(1)), None),
    );
    let tabs = tabs_ids(&tree)[0];

    tree
      .split(tabs, panel(2), Placement::Bottom, Some(px(200.)))
      .expect_changed();

    // Root (h-split) -> child (v-split wrapping the original tabs) -> new.
    assert_eq!(tabs_ids(&tree).len(), 2);
    let PaneRef::Split { axis, children, .. } = tree.root().kind() else {
      panic!("root must stay a split");
    };
    assert_eq!(axis, Axis::Horizontal);
    let PaneRef::Split { axis, .. } = children[0].kind() else {
      panic!("the wrapped node must be a vertical split");
    };
    assert_eq!(axis, Axis::Vertical);
  }

  #[test]
  fn remove_clamps_active_ix() {
    let mut tree = PaneTree::from_layout(
      RootKind::Any,
      DockLayout::tabs()
        .panel_view_raw(panel(1))
        .panel_view_raw(panel(2))
        .panel_view_raw(panel(3))
        .active_index(2),
    );
    let root = tree.root().id();

    tree.remove_panel(panel(3)).expect_changed();
    let (panels, active_ix) = tree.tabs_of(root).unwrap();
    assert_eq!(panels.len(), 2);
    assert_eq!(active_ix, 1, "active index must clamp into range");
  }

  #[test]
  fn remove_last_panel_of_non_root_tabs_prunes_the_group() {
    let mut tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split()
        .child(DockLayout::tabs().panel_view_raw(panel(1)), Some(px(200.)))
        .child(DockLayout::tabs().panel_view_raw(panel(2)), None),
    );
    assert_eq!(tabs_ids(&tree).len(), 2);

    tree.remove_panel(panel(1)).expect_changed();
    // The emptied tab group is pruned and the one-child split collapses.
    assert_eq!(tabs_ids(&tree).len(), 1);
    assert!(tree.contains_panel(panel(2)));
  }

  #[test]
  fn move_panel_detaches_and_inserts_atomically() {
    let mut tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split()
        .child(DockLayout::tabs().panel_view_raw(panel(1)), None)
        .child(DockLayout::tabs().panel_view_raw(panel(2)), None),
    );
    let tabs = tabs_ids(&tree);
    let [_, target_b] = tabs[..] else {
      panic!("two tab groups expected");
    };

    tree
      .move_panel(
        panel(1),
        InsertTarget::Tabs {
          node: target_b,
          ix: None,
          activate: true,
        },
      )
      .expect_changed();

    // The emptied source group is pruned by normalization; the panel itself
    // lives on in the target group.
    assert_eq!(tabs_ids(&tree), vec![target_b]);
    assert_eq!(tabs_panel_count(&tree, target_b), 2);
    assert!(tree.contains_panel(panel(1)));
    let (panels, active_ix) = tree.tabs_of(target_b).unwrap();
    assert_eq!(panels, &[panel(2), panel(1)][..]);
    assert_eq!(active_ix, 1);

    // Moving within the same group reorders without duplicating.
    tree
      .move_panel(
        panel(1),
        InsertTarget::Tabs {
          node: target_b,
          ix: Some(0),
          activate: true,
        },
      )
      .expect_changed();
    let (panels, _) = tree.tabs_of(target_b).unwrap();
    assert_eq!(panels, &[panel(1), panel(2)][..]);
  }

  #[test]
  fn set_sizes_guards_length() {
    let mut tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split()
        .child(DockLayout::tabs().panel_view_raw(panel(1)), None)
        .child(DockLayout::tabs().panel_view_raw(panel(2)), None),
    );
    let root = split_node(&tree);

    let result = tree.set_sizes(root, vec![Some(px(100.))]);
    assert!(!result.changed(), "length mismatch must be a no-op");

    let result = tree.set_sizes(root, vec![Some(px(100.)), Some(px(200.))]);
    assert!(result.changed());
  }

  #[test]
  fn builder_builds_expected_shape_and_normalizes() {
    // A tabs builder with no panels under a `Split` root normalizes away.
    let tree = PaneTree::from_layout(
      RootKind::Split,
      DockLayout::h_split()
        .child(DockLayout::tabs(), Some(px(200.)))
        .child(DockLayout::tabs().panel_view_raw(panel(1)), None),
    );
    assert_eq!(tabs_ids(&tree).len(), 1);
    assert!(tree.contains_panel(panel(1)));
  }

  #[test]
  fn node_ids_are_unique_across_trees_and_stable_under_edits() {
    let mut tree_a = PaneTree::new(RootKind::Split);
    let tree_b = PaneTree::new(RootKind::Any);
    assert_ne!(tree_a.root().id(), tree_b.root().id());

    let root = tree_a.root().id();
    tree_a
      .insert_panel(
        panel(9),
        InsertTarget::Split {
          node: root,
          placement: Placement::Right,
          size: None,
        },
      )
      .expect_changed();
    let group = tree_a.tab_groups()[0];

    tree_a
      .insert_panel(
        panel(8),
        InsertTarget::Tabs {
          node: group,
          ix: None,
          activate: false,
        },
      )
      .expect_changed();

    // Normalization must not re-mint the surviving container's id.
    assert_eq!(tree_a.tab_groups()[0], group);
  }
}
