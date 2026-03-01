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
#[derive(Clone)]
pub struct TreeEntry {
  item: TreeItem,
  depth: usize,
}

impl TreeEntry {
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
    self.item.is_folder()
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

  fn find_ancestors(&self, target_id: &SharedString) -> Option<Vec<TreeItem>> {
    if self.id == *target_id {
      return Some(vec![]);
    }

    for child in &self.children {
      if let Some(mut path) = child.find_ancestors(target_id) {
        path.push(self.clone());
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

    // Restore selection by ID.
    self.selected_ix = selected_id
      .as_ref()
      .and_then(|id| self.entries.iter().position(|e| e.item.id == *id));

    self.selected_indices.clear();
    if self.multi_selectable {
      for id in &multi_selected_ids {
        if let Some(ix) = self.entries.iter().position(|e| e.item.id == *id) {
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
      self.selected_ix = self.entries.iter().position(|entry| entry.item.id == id);
    }
  }

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

  /// Return selected items in multi-select mode.
  pub fn selected_items(&self) -> Vec<&TreeItem> {
    self
      .selected_indices
      .iter()
      .filter_map(|&ix| self.entries.get(ix).map(|e| &e.item))
      .collect()
  }

  // -- Expand / Collapse ----------------------------------------------------

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
    let roots = self.roots.clone();
    for root in roots {
      self.add_entry(root, 0);
    }

    if let Some(ix) = self.selected_ix
      && ix >= self.entries.len()
    {
      self.selected_ix = None;
    }
    // Prune out-of-range multi-selection indices.
    let len = self.entries.len();
    self.selected_indices.retain(|&ix| ix < len);
  }

  fn add_entry(&mut self, item: TreeItem, depth: usize) {
    self.entries.push(TreeEntry {
      item: item.clone(),
      depth,
    });

    if item.is_expanded() {
      for child in item.children {
        self.add_entry(child, depth + 1);
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
}
