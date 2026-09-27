use std::{cell::RefCell, collections::BTreeSet, rc::Rc};

use gpui::SharedString;

use crate::IconName;

#[derive(Debug, Default)]
struct TreeItemState {
  expanded: bool,
  disabled: bool,
  loading: bool,
}

/// A tree item with an id, label and nested children.
#[derive(Clone)]
pub struct TreeItem {
  pub id: SharedString,
  pub label: SharedString,
  pub icon: Option<IconName>,
  pub children: Vec<TreeItem>,
  state: Rc<RefCell<TreeItemState>>,
}

/// A flattened entry used by tree-like renderers.
///
/// The entry's item is a shallow copy: it shares expand/disabled/loading
/// state with the model's item but intentionally carries **no children** —
/// a full copy here would deep-clone a whole subtree per entry on every
/// rebuild. Read the complete item (with children) from the tree's roots
/// instead.
#[derive(Clone)]
pub struct TreeEntry {
  item: TreeItem,
  /// Whether the source item has children (entries never carry them).
  is_folder: bool,
  depth: usize,
}

impl TreeEntry {
  /// The entry's item. Shallow: children are absent, while expanded/
  /// disabled/loading state is shared with the model's item.
  #[inline]
  pub fn item(&self) -> &TreeItem {
    &self.item
  }

  #[inline]
  pub fn depth(&self) -> usize {
    self.depth
  }

  #[inline]
  pub fn is_root(&self) -> bool {
    self.depth == 0
  }

  #[inline]
  pub fn is_folder(&self) -> bool {
    self.is_folder
  }

  #[inline]
  pub fn is_expanded(&self) -> bool {
    self.item.is_expanded()
  }

  #[inline]
  pub fn is_disabled(&self) -> bool {
    self.item.is_disabled()
  }

  #[inline]
  pub fn is_loading(&self) -> bool {
    self.item.is_loading()
  }

  #[inline]
  pub fn icon(&self) -> Option<IconName> {
    self.item.icon
  }

  #[inline]
  pub fn icon_or_default(&self) -> IconName {
    self.item.icon.unwrap_or_else(|| {
      if self.is_folder() {
        if self.is_expanded() {
          IconName::FolderOpen
        } else {
          IconName::Folder
        }
      } else {
        IconName::Document
      }
    })
  }
}

impl TreeItem {
  /// Create a tree item.
  ///
  /// `id` should be globally unique in the tree. `label` is display text.
  pub fn new(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
    Self {
      id: id.into(),
      label: label.into(),
      icon: None,
      children: Vec::new(),
      state: Rc::new(RefCell::new(TreeItemState::default())),
    }
  }

  pub fn icon(mut self, icon: impl Into<IconName>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  pub fn child(mut self, child: TreeItem) -> Self {
    self.children.push(child);
    self
  }

  pub fn children(mut self, children: impl IntoIterator<Item = TreeItem>) -> Self {
    self.children.extend(children);
    self
  }

  pub fn expanded(self, expanded: bool) -> Self {
    self.state.borrow_mut().expanded = expanded;
    self
  }

  pub fn disabled(self, disabled: bool) -> Self {
    self.state.borrow_mut().disabled = disabled;
    self
  }

  pub fn loading(self, loading: bool) -> Self {
    self.state.borrow_mut().loading = loading;
    self
  }

  #[inline]
  pub fn is_folder(&self) -> bool {
    !self.children.is_empty()
  }

  #[inline]
  pub fn is_disabled(&self) -> bool {
    self.state.borrow().disabled
  }

  #[inline]
  pub fn is_loading(&self) -> bool {
    self.state.borrow().loading
  }

  #[inline]
  pub fn is_expanded(&self) -> bool {
    self.state.borrow().expanded
  }

  /// Shallow copy used for flattened [`TreeEntry`] values: clones scalars,
  /// shares the `Rc`-held state, and drops children so a rebuild stays O(1)
  /// per entry instead of deep-cloning subtrees.
  fn clone_for_entry(&self) -> Self {
    Self {
      id: self.id.clone(),
      label: self.label.clone(),
      icon: self.icon,
      children: Vec::new(),
      state: Rc::clone(&self.state),
    }
  }

  fn find_ancestors(&self, target_id: &SharedString) -> Option<Vec<TreeItem>> {
    if self.id == *target_id {
      return Some(vec![]);
    }

    for child in &self.children {
      if let Some(mut path) = child.find_ancestors(target_id) {
        let mut ancestor = self.clone();
        ancestor.children = Vec::new();
        path.push(ancestor);
        return Some(path);
      }
    }

    None
  }

  /// Collect expanded states from this item and all descendants into a map.
  fn collect_expand_state(&self, states: &mut std::collections::HashMap<SharedString, bool>) {
    if !self.children.is_empty() {
      states.insert(self.id.clone(), self.is_expanded());
    }
    for child in &self.children {
      child.collect_expand_state(states);
    }
  }

  /// Apply expand states from a map (by ID). Items not in the map keep their
  /// current state.
  fn apply_expand_state(&self, states: &std::collections::HashMap<SharedString, bool>) {
    if let Some(&expanded) = states.get(&self.id) {
      self.state.borrow_mut().expanded = expanded;
    }
    for child in &self.children {
      child.apply_expand_state(states);
    }
  }
}

/// Tree model for flattening, expansion and selection state.
#[derive(Clone, Default)]
pub struct TreeModel {
  roots: Vec<TreeItem>,
  entries: Vec<TreeEntry>,
  selected_ix: Option<usize>,
  /// Indices of all selected items (used in multi-select mode).
  selected_indices: BTreeSet<usize>,
  /// Whether multi-selection is enabled.
  multi_selectable: bool,
  /// Precomputed parent index for each entry (O(1) lookup).
  parent_indices: Vec<Option<usize>>,
  /// Precomputed subtree end index (exclusive) for each entry.
  subtree_ends: Vec<usize>,
  /// Precomputed next-sibling flag for each entry.
  has_next_sibling_flags: Vec<bool>,
  /// Flat flags for precomputed ancestor guide lines: for entry `ix`, the
  /// slice `ancestor_guide_flags[ranges[ix].0..ranges[ix].1]` is indexed by
  /// ancestor depth (0 = root level) and tells whether the ancestor at that
  /// depth has a next sibling (so the row must render a vertical
  /// continuation line there). The immediate parent is excluded; its line is
  /// replaced by the row's own branch segment.
  ancestor_guide_flags: Vec<bool>,
  /// Start/end offsets of each entry's slice in `ancestor_guide_flags`.
  ancestor_guide_ranges: Vec<(usize, usize)>,
}

impl TreeModel {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn items(mut self, items: impl Into<Vec<TreeItem>>) -> Self {
    self.set_items(items);
    self
  }

  pub fn set_items(&mut self, items: impl Into<Vec<TreeItem>>) {
    self.roots = items.into();
    self.rebuild_entries();
    self.selected_ix = None;
    self.selected_indices.clear();
  }

  /// Update tree items while preserving expand/collapse state and selection.
  ///
  /// This is the preferred method for hot-reloading scenarios (e.g.,
  /// lazy-loaded file trees). It:
  /// 1. Transfers expand/collapse state from old items to new items by matching
  ///    item IDs.
  /// 2. Preserves single selection by item ID (remaps the index).
  /// 3. Preserves multi-selection by item IDs (remaps indices).
  ///
  /// Items whose IDs don't exist in the old tree keep whatever expand state
  /// they were constructed with.
  pub fn update_items(&mut self, items: impl Into<Vec<TreeItem>>) {
    // Collect expand states from old tree.
    let mut expand_states = std::collections::HashMap::new();
    for root in &self.roots {
      root.collect_expand_state(&mut expand_states);
    }

    // Remember selected item IDs.
    let selected_id = self
      .selected_ix
      .and_then(|ix| self.entries.get(ix))
      .map(|entry| entry.item.id.clone());

    let multi_selected_ids: Vec<SharedString> = self
      .selected_indices
      .iter()
      .filter_map(|&ix| self.entries.get(ix).map(|e| e.item.id.clone()))
      .collect();

    // Replace roots and apply preserved expand states.
    self.roots = items.into();
    for root in &self.roots {
      root.apply_expand_state(&expand_states);
    }
    self.rebuild_entries();

    // Build an ID→index map for O(1) lookups
    let id_to_ix: std::collections::HashMap<SharedString, usize> = self
      .entries
      .iter()
      .enumerate()
      .map(|(ix, e)| (e.item.id.clone(), ix))
      .collect();

    // Restore selection by ID.
    self.selected_ix = selected_id
      .as_ref()
      .and_then(|id| id_to_ix.get(id).copied());

    self.selected_indices.clear();
    if self.multi_selectable {
      for id in &multi_selected_ids {
        if let Some(&ix) = id_to_ix.get(id) {
          self.selected_indices.insert(ix);
        }
      }
    }
  }

  pub fn entries(&self) -> &[TreeEntry] {
    &self.entries
  }

  pub fn len(&self) -> usize {
    self.entries.len()
  }

  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  pub fn entry(&self, ix: usize) -> Option<&TreeEntry> {
    self.entries.get(ix)
  }

  pub fn selected_index(&self) -> Option<usize> {
    self.selected_ix
  }

  pub fn set_selected_index(&mut self, ix: Option<usize>) {
    self.selected_ix = ix;
  }

  pub fn set_selected_item(&mut self, item: Option<&TreeItem>) {
    let Some(item) = item else {
      self.selected_ix = None;
      return;
    };

    let id = item.id.clone();
    self.selected_ix = self.entries.iter().position(|entry| entry.item.id == id);
    if self.selected_ix.is_none() {
      self.expand_ancestors(id.clone());
      let id_to_ix: std::collections::HashMap<SharedString, usize> = self
        .entries
        .iter()
        .enumerate()
        .map(|(ix, e)| (e.item.id.clone(), ix))
        .collect();
      self.selected_ix = id_to_ix.get(&id).copied();
    }
  }

  /// The selected entry's item. Shallow like all flattened entries: it
  /// carries no children — see [`TreeEntry::item`].
  pub fn selected_item(&self) -> Option<&TreeItem> {
    self
      .selected_ix
      .and_then(|ix| self.entries.get(ix).map(|entry| &entry.item))
  }

  pub fn selected_entry(&self) -> Option<&TreeEntry> {
    self.selected_ix.and_then(|ix| self.entries.get(ix))
  }

  // -- Multi-selection -------------------------------------------------------

  /// Whether multi-selection mode is enabled.
  pub fn multi_selectable(&self) -> bool {
    self.multi_selectable
  }

  /// Set multi-selection mode.
  pub fn set_multi_selectable(&mut self, enabled: bool) {
    self.multi_selectable = enabled;
    if !enabled {
      self.selected_indices.clear();
    }
  }

  /// Returns all selected indices (sorted).
  pub fn selected_indices(&self) -> &BTreeSet<usize> {
    &self.selected_indices
  }

  /// Returns whether the given index is selected (multi-select).
  pub fn is_selected(&self, ix: usize) -> bool {
    if self.multi_selectable {
      self.selected_indices.contains(&ix)
    } else {
      self.selected_ix == Some(ix)
    }
  }

  /// Select exactly one index, replacing any multi-selection.
  pub fn select_only(&mut self, ix: usize) {
    self.selected_indices.clear();
    self.selected_indices.insert(ix);
    self.selected_ix = Some(ix);
  }

  /// Toggle an index in the multi-selection set.
  pub fn toggle_selected(&mut self, ix: usize) {
    if !self.selected_indices.remove(&ix) {
      self.selected_indices.insert(ix);
    }
    // Keep primary selection in sync with the last toggled item.
    self.selected_ix = Some(ix);
  }

  /// Select a contiguous range from the current primary selection to `ix`.
  pub fn select_range_to(&mut self, ix: usize) {
    let anchor = self.selected_ix.unwrap_or(0);
    let (start, end) = if anchor <= ix {
      (anchor, ix)
    } else {
      (ix, anchor)
    };
    for i in start..=end {
      self.selected_indices.insert(i);
    }
    self.selected_ix = Some(ix);
  }

  /// Clear all selections (both single and multi).
  pub fn clear_selection(&mut self) {
    self.selected_ix = None;
    self.selected_indices.clear();
  }

  /// Return selected items in multi-select mode. Shallow like all flattened
  /// entries: they carry no children — see [`TreeEntry::item`].
  pub fn selected_items(&self) -> Vec<&TreeItem> {
    self
      .selected_indices
      .iter()
      .filter_map(|&ix| self.entries.get(ix).map(|e| &e.item))
      .collect()
  }

  // -- Expand / Collapse ----------------------------------------------------

  /// Returns the precomputed parent index for an entry (O(1)).
  pub fn parent_index(&self, ix: usize) -> Option<usize> {
    self.parent_indices.get(ix).copied().flatten()
  }

  /// Returns the precomputed subtree end index (exclusive) for an entry (O(1)).
  pub fn subtree_end(&self, ix: usize) -> usize {
    self
      .subtree_ends
      .get(ix)
      .copied()
      .unwrap_or(self.entries.len())
  }

  /// Returns whether an entry has a next sibling (O(1)).
  pub fn has_next_sibling(&self, ix: usize) -> bool {
    self
      .has_next_sibling_flags
      .get(ix)
      .copied()
      .unwrap_or(false)
  }

  /// Returns the precomputed ancestor guide-line flags for an entry,
  /// indexed by ancestor depth (index 0 = ancestor at depth 0). `true`
  /// means the row renders a vertical continuation line at that depth.
  /// Empty when the entry has no ancestors above its parent.
  pub fn ancestor_guides(&self, ix: usize) -> &[bool] {
    let Some(&(start, end)) = self.ancestor_guide_ranges.get(ix) else {
      return &[];
    };
    self.ancestor_guide_flags.get(start..end).unwrap_or(&[])
  }

  pub fn toggle_expand(&mut self, ix: usize) {
    let Some(entry) = self.entries.get_mut(ix) else {
      return;
    };
    if !entry.is_folder() || entry.is_loading() {
      return;
    }

    let expanded = entry.item.is_expanded();
    entry.item.state.borrow_mut().expanded = !expanded;
    self.rebuild_entries();
  }

  fn expand_ancestors(&mut self, target_id: SharedString) {
    let mut ancestors = Vec::new();
    for item in &self.roots {
      if let Some(found_ancestors) = item.find_ancestors(&target_id) {
        ancestors = found_ancestors;
        break;
      }
    }

    if ancestors.is_empty() {
      return;
    }

    for ancestor in ancestors {
      ancestor.state.borrow_mut().expanded = true;
    }
    self.rebuild_entries();
  }

  fn rebuild_entries(&mut self) {
    self.entries.clear();
    for root in &self.roots {
      Self::add_entry(&mut self.entries, root, 0);
    }

    self.parent_indices = vec![None; self.entries.len()];
    self.subtree_ends = vec![0; self.entries.len()];
    self.has_next_sibling_flags = vec![false; self.entries.len()];
    self.compute_structural_cache();

    if let Some(ix) = self.selected_ix
      && ix >= self.entries.len()
    {
      self.selected_ix = None;
    }
    // Prune out-of-range multi-selection indices.
    let len = self.entries.len();
    self.selected_indices.retain(|&ix| ix < len);
  }

  fn compute_structural_cache(&mut self) {
    self.ancestor_guide_flags.clear();
    self.ancestor_guide_ranges.clear();
    let n = self.entries.len();
    if n == 0 {
      return;
    }

    let mut stack: Vec<usize> = Vec::new();

    for i in 0..n {
      let depth = self.entries[i].depth;

      while let Some(&top) = stack.last()
        && self.entries[top].depth >= depth
      {
        self.subtree_ends[stack.pop().unwrap()] = i;
      }

      if let Some(&parent) = stack.last() {
        self.parent_indices[i] = Some(parent);
      }

      stack.push(i);
    }

    while let Some(idx) = stack.pop() {
      self.subtree_ends[idx] = n;
    }

    for i in 0..n {
      let end = self.subtree_ends[i];
      self.has_next_sibling_flags[i] = end < n && self.entries[end].depth == self.entries[i].depth;
    }

    // Precompute ancestor continuation-guide flags so the renderer can map
    // rows to lines without walking the parent chain every frame.
    for i in 0..n {
      let start = self.ancestor_guide_flags.len();
      // Continuation lines start above the immediate parent (which gets a
      // branch segment instead): collect ancestors from depth - 2 up to 0.
      let mut cursor = self.parent_indices[i].and_then(|p| self.parent_indices[p]);
      while let Some(parent_ix) = cursor {
        let guide = self.has_next_sibling(parent_ix);
        self.ancestor_guide_flags.push(guide);
        cursor = self.parent_indices[parent_ix];
      }
      // Collected deepest-first; restore ascending depth order so the slice
      // can be indexed by ancestor depth.
      self.ancestor_guide_flags[start..].reverse();
      self
        .ancestor_guide_ranges
        .push((start, self.ancestor_guide_flags.len()));
    }
  }

  /// Pushes `item` and its expanded descendants into `entries`.
  ///
  /// Takes the entry list as a separate parameter and the item by reference
  /// so [`TreeModel::rebuild_entries`] can walk `self.roots` in place.
  /// Entries receive shallow copies ([`TreeItem::clone_for_entry`]): a full
  /// clone here cost O(subtree) per visible entry per rebuild.
  fn add_entry(entries: &mut Vec<TreeEntry>, item: &TreeItem, depth: usize) {
    entries.push(TreeEntry {
      item: item.clone_for_entry(),
      is_folder: !item.children.is_empty(),
      depth,
    });

    if item.is_expanded() {
      for child in &item.children {
        Self::add_entry(entries, child, depth + 1);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::{TreeItem, TreeModel};

  fn sample_items() -> Vec<TreeItem> {
    vec![
      TreeItem::new("src", "src")
        .expanded(true)
        .child(
          TreeItem::new("src/ui", "ui")
            .expanded(true)
            .child(TreeItem::new("src/ui/button.rs", "button.rs"))
            .child(TreeItem::new("src/ui/icon.rs", "icon.rs"))
            .child(TreeItem::new("src/ui/mod.rs", "mod.rs")),
        )
        .child(TreeItem::new("src/lib.rs", "lib.rs")),
      TreeItem::new("Cargo.toml", "Cargo.toml"),
      TreeItem::new("Cargo.lock", "Cargo.lock").disabled(true),
      TreeItem::new("README.md", "README.md"),
    ]
  }

  fn flatten_labels(model: &TreeModel) -> Vec<String> {
    model
      .entries()
      .iter()
      .map(|entry| {
        format!(
          "{}{}",
          "    ".repeat(entry.depth()),
          entry.item().label.as_ref()
        )
      })
      .collect()
  }

  fn assert_entries_shallow(model: &TreeModel) {
    for entry in model.entries() {
      assert!(
        entry.item().children.is_empty(),
        "flattened entries must not carry children"
      );
    }
  }

  #[test]
  fn flattens_and_toggles() {
    let mut model = TreeModel::new().items(sample_items());
    assert_eq!(
      flatten_labels(&model),
      vec![
        "src",
        "    ui",
        "        button.rs",
        "        icon.rs",
        "        mod.rs",
        "    lib.rs",
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
      ]
    );

    assert!(model.entries()[0].is_root());
    assert!(model.entries()[1].is_folder());
    assert!(model.entries()[1].is_expanded());
    assert!(model.entries()[7].is_disabled());

    model.toggle_expand(1);
    assert_eq!(
      flatten_labels(&model),
      vec![
        "src",
        "    ui",
        "    lib.rs",
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
      ]
    );
    assert!(!model.entries()[1].is_expanded());
  }

  #[test]
  fn selecting_hidden_item_expands_ancestors() {
    let mut model = TreeModel::new().items(vec![
      TreeItem::new("root", "root").child(
        TreeItem::new("a", "a")
          .child(TreeItem::new("b", "b").child(TreeItem::new("target", "target"))),
      ),
    ]);

    let target = TreeItem::new("target", "target");
    model.set_selected_item(Some(&target));

    assert_eq!(
      flatten_labels(&model),
      vec!["root", "    a", "        b", "            target"]
    );
    assert_eq!(
      model.selected_item().map(|item| item.id.as_ref()),
      Some("target")
    );
  }

  #[test]
  fn update_items_rebuilds_entries_preserving_state() {
    let mut model = TreeModel::new().items(sample_items());
    model.set_multi_selectable(true);
    model.toggle_expand(1); // collapse "src/ui"
    model.toggle_selected(0); // multi-select "src"
    model.set_selected_index(Some(3)); // primary: "Cargo.toml"

    // Replace with a differently shaped tree whose "src/ui" is constructed
    // expanded; it must inherit the collapsed state of the old "src/ui".
    model.update_items(vec![
      TreeItem::new("src", "src").expanded(true).child(
        TreeItem::new("src/ui", "ui")
          .expanded(true)
          .child(TreeItem::new("src/ui/new.rs", "new.rs")),
      ),
      TreeItem::new("Cargo.toml", "Cargo.toml"),
    ]);

    // Entries are rebuilt from the new roots only; the stale constructed
    // `expanded(true)` on the new "src/ui" is overridden by the old state.
    assert_eq!(flatten_labels(&model), vec!["src", "    ui", "Cargo.toml"]);
    assert!(!model.entries()[1].is_expanded());
    // Selection survived by id remapping (not by index); "Cargo.toml" sits
    // at index 2 because "src/ui" is collapsed and hides "src/ui/new.rs".
    assert_eq!(model.selected_index(), Some(2));
    assert_eq!(model.selected_indices().len(), 1);
    assert!(model.selected_indices().contains(&0));

    // Expanding again flattens newly added descendants correctly.
    model.toggle_expand(1);
    assert_eq!(
      flatten_labels(&model),
      vec!["src", "    ui", "        new.rs", "Cargo.toml"]
    );
  }

  #[test]
  fn entries_are_shallow_but_behavior_preserved() {
    let mut model = TreeModel::new().items(sample_items());
    assert_entries_shallow(&model);
    assert!(model.entries()[0].is_folder());
    assert!(!model.entries()[5].is_folder()); // leaf "src/lib.rs"

    model.toggle_expand(1); // collapse "src/ui"
    assert_entries_shallow(&model);
    assert_eq!(
      flatten_labels(&model),
      vec![
        "src",
        "    ui",
        "    lib.rs",
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
      ]
    );

    // Selection still works on flattened (shallow) items.
    model.toggle_selected(2); // "src/lib.rs"
    assert!(model.is_selected(2));
    assert_eq!(model.selected_items().len(), 1);
    assert_eq!(model.selected_items()[0].id.as_ref(), "src/lib.rs");
    assert!(model.selected_items()[0].children.is_empty());

    // Lazy loading: a loading folder refuses to toggle.
    let mut lazy = TreeModel::new().items(vec![
      TreeItem::new("dir", "dir")
        .expanded(true)
        .loading(true)
        .child(TreeItem::new("dir/x", "x")),
    ]);
    assert_eq!(lazy.len(), 2);
    lazy.toggle_expand(0);
    assert!(lazy.entries()[0].is_expanded());
    assert_eq!(lazy.len(), 2);
    assert_entries_shallow(&lazy);
  }
}
