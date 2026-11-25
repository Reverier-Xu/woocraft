//! Private option-tree helpers shared by [`CheckboxGroup`](crate::CheckboxGroup)
//! and [`RadioGroup`](crate::RadioGroup).
//!
//! Both groups render the same tree of options and derive each node's
//! presentation from the leaves it contains. Traversal, state derivation,
//! focus-id assignment and nested layout live here so the groups only differ in
//! the leaf widget they render and the payload they report.

use gpui::{AnyElement, App, ElementId, IntoElement, ParentElement, SharedString, Styled};

use crate::{ActiveTheme, Size, v_flex};

/// A node of an option tree, implemented by
/// [`CheckboxOption`](crate::CheckboxOption) and
/// [`RadioOption`](crate::RadioOption).
pub(super) trait OptionNode {
  /// The option's stable value.
  fn value(&self) -> &SharedString;

  /// The nested child options; empty for a leaf.
  fn child_options(&self) -> &[Self]
  where
    Self: Sized;
}

/// How the leaves of an option subtree relate to the current selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SubtreeSelection {
  /// No leaf of the subtree is selected.
  None,
  /// Some, but not all, leaves of the subtree are selected.
  Partial,
  /// Every leaf of the subtree is selected.
  All,
}

impl SubtreeSelection {
  /// Derive the selection state of `option`'s subtree.
  ///
  /// Only leaves hold state, so a node derives its state from its descendants.
  pub(super) fn of<N: OptionNode>(option: &N, selected: &[SharedString]) -> Self {
    let (leaves, selected_leaves) = count_selected_leaves(option, selected);
    if selected_leaves == 0 {
      Self::None
    } else if selected_leaves == leaves {
      Self::All
    } else {
      Self::Partial
    }
  }

  /// Whether a checkbox node shows as fully checked.
  pub(super) fn all_selected(self) -> bool {
    matches!(self, Self::All)
  }

  /// Whether a checkbox node shows as indeterminate.
  pub(super) fn some_selected(self) -> bool {
    matches!(self, Self::Partial)
  }

  /// Whether a radio node shows as checked, i.e. the selected leaf lies
  /// anywhere in its subtree.
  pub(super) fn any_selected(self) -> bool {
    !matches!(self, Self::None)
  }
}

/// Count the leaves of `option`'s subtree and how many of them are selected.
fn count_selected_leaves<N: OptionNode>(option: &N, selected: &[SharedString]) -> (usize, usize) {
  if option.child_options().is_empty() {
    return (1, usize::from(selected.contains(option.value())));
  }
  let mut leaves = 0;
  let mut selected_leaves = 0;
  for child in option.child_options() {
    let (child_leaves, child_selected_leaves) = count_selected_leaves(child, selected);
    leaves += child_leaves;
    selected_leaves += child_selected_leaves;
  }
  (leaves, selected_leaves)
}

/// Collect the leaf values of `option`'s subtree in tree order.
pub(super) fn option_leaves<N: OptionNode>(option: &N) -> Vec<SharedString> {
  if option.child_options().is_empty() {
    vec![option.value().clone()]
  } else {
    option
      .child_options()
      .iter()
      .flat_map(|child| option_leaves(child))
      .collect()
  }
}

/// Collect the leaf values of a whole option tree in tree order.
pub(super) fn tree_leaves<N: OptionNode>(options: &[N]) -> Vec<SharedString> {
  options
    .iter()
    .flat_map(|option| option_leaves(option))
    .collect()
}

/// The leaf a click on `option` selects: the first leaf of its subtree in tree
/// order (for a leaf option, its own value).
pub(super) fn first_leaf<N: OptionNode>(option: &N) -> SharedString {
  match option.child_options().first() {
    Some(first) => first_leaf(first),
    None => option.value().clone(),
  }
}

/// Hands out focus element ids to option nodes in tree (pre-order) order.
///
/// Nodes are addressed by `(group name, position)` so focus state stays with a
/// node when the tree is rebuilt.
pub(super) struct NodeIds {
  name: SharedString,
  next: usize,
}

impl NodeIds {
  pub(super) fn new(name: SharedString) -> Self {
    Self { name, next: 0 }
  }

  /// The id of the next node in tree order.
  pub(super) fn next(&mut self) -> ElementId {
    let position = self.next;
    self.next += 1;
    (self.name.clone(), position).into()
  }
}

/// Lay out one rendered option node: a leaf on its own, or a branch with its
/// children nested behind an indent guide.
pub(super) fn layout_option_node(
  size: Size, cx: &App, leaf: AnyElement, children: Vec<AnyElement>,
) -> AnyElement {
  if children.is_empty() {
    return leaf;
  }
  v_flex()
    .gap(size.component_gap())
    .child(leaf)
    .child(
      v_flex()
        .gap(size.component_gap())
        .pl(size.em(1.))
        .ml(size.em(0.25))
        .border_l(size.em(0.125))
        .border_color(cx.theme().border)
        .children(children),
    )
    .into_any_element()
}
