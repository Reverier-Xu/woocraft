//! CheckboxGroup component for grouped and nested (tree) multi-selection.
//!
//! CheckboxGroup renders a set of [`Checkbox`] options in a flat list or a
//! nested tree. Selection state is controlled by the caller: the group only
//! owns presentation, and reports user intent through
//! [`CheckboxGroup::on_change`].
//!
//! # Tree semantics
//!
//! Only leaf options hold state. A parent checkbox is fully checked when all
//! of its descendants are checked, and indeterminate when some but not all
//! are. Toggling a parent checks or unchecks its entire subtree. The
//! `on_change` payload always lists selected leaf values in tree order.
//!
//! # Example
//! ```rust,ignore
//! CheckboxGroup::new("frameworks")
//!   .option(
//!     CheckboxOption::new("frontend", "Frontend")
//!       .child(CheckboxOption::new("react", "React"))
//!       .child(CheckboxOption::new("vue", "Vue")),
//!   )
//!   .option(CheckboxOption::new("cli", "CLI Tools"))
//!   .selected(vec!["react".into()])
//!   .on_change(|selected: &Vec<SharedString>, _window, _cx| {
//!     // Handle new selection
//!   })
//! ```

use std::rc::Rc;

use gpui::{
  AnyElement, App, Axis, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement,
  Styled, Window, prelude::FluentBuilder as _,
};

use super::selection_tree::{
  NodeIds, OptionNode, SubtreeSelection, layout_option_node, option_leaves, tree_leaves,
};
use crate::{Checkbox, Disableable, Sizable, Size, StyledExt, h_flex, v_flex};

type CheckboxGroupChangeHandler = Rc<dyn Fn(&Vec<SharedString>, &mut Window, &mut App) + 'static>;

/// A single option (node) in a [`CheckboxGroup`].
///
/// An option without children is a leaf and holds selection state. An option
/// with children is a tree node whose state is derived from its descendants.
#[derive(Clone, Debug)]
pub struct CheckboxOption {
  value: SharedString,
  label: SharedString,
  disabled: bool,
  children: Vec<Self>,
}

impl CheckboxOption {
  /// Create a new option with a value and display label.
  ///
  /// Leaf values must be unique within the group.
  pub fn new(value: impl Into<SharedString>, label: impl Into<SharedString>) -> Self {
    Self {
      value: value.into(),
      label: label.into(),
      disabled: false,
      children: Vec::new(),
    }
  }

  /// Disable this option (and, for a node, its subtree).
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  /// Append a nested child option, making this option a tree node.
  pub fn child(mut self, child: Self) -> Self {
    self.children.push(child);
    self
  }

  /// Append multiple nested child options.
  pub fn children(mut self, children: impl IntoIterator<Item = Self>) -> Self {
    self.children.extend(children);
    self
  }
}

impl OptionNode for CheckboxOption {
  fn value(&self) -> &SharedString {
    &self.value
  }

  fn child_options(&self) -> &[Self] {
    &self.children
  }
}

/// Apply a toggle of `leaves` to `selected` and return the new selection in
/// tree order (values outside the tree are preserved at the tail).
fn apply_toggle(
  options: &[CheckboxOption], selected: &[SharedString], leaves: &[SharedString], next: bool,
) -> Vec<SharedString> {
  let all_leaves = tree_leaves(options);
  let mut next_selected = Vec::new();
  for leaf in &all_leaves {
    let is_on = if leaves.contains(leaf) {
      next
    } else {
      selected.contains(leaf)
    };
    if is_on {
      next_selected.push(leaf.clone());
    }
  }
  next_selected.extend(
    selected
      .iter()
      .filter(|value| !all_leaves.contains(value))
      .cloned(),
  );
  next_selected
}

#[derive(IntoElement)]
/// Group of checkboxes with flat or nested (tree) layout.
///
/// CheckboxGroup is a controlled component: it renders `selected` and reports
/// changes through [`CheckboxGroup::on_change`]. See the [module
/// documentation](self) for tree semantics.
pub struct CheckboxGroup {
  name: SharedString,
  style: StyleRefinement,
  options: Vec<CheckboxOption>,
  selected: Vec<SharedString>,
  layout: Axis,
  disabled: bool,
  size: Size,
  on_change: Option<CheckboxGroupChangeHandler>,
}

impl CheckboxGroup {
  /// Create a new group with the given name.
  ///
  /// The name must be unique within the window; it namespaces the focus IDs
  /// of the rendered checkboxes.
  pub fn new(name: impl Into<SharedString>) -> Self {
    Self {
      name: name.into(),
      style: StyleRefinement::default(),
      options: Vec::new(),
      selected: Vec::new(),
      layout: Axis::Vertical,
      disabled: false,
      size: Size::default(),
      on_change: None,
    }
  }

  /// Append an option to the group.
  pub fn option(mut self, option: CheckboxOption) -> Self {
    self.options.push(option);
    self
  }

  /// Append multiple options to the group.
  pub fn options(mut self, options: impl IntoIterator<Item = CheckboxOption>) -> Self {
    self.options.extend(options);
    self
  }

  /// Set the selected leaf values (default: empty).
  pub fn selected(mut self, selected: Vec<SharedString>) -> Self {
    self.selected = selected;
    self
  }

  /// Set the top-level layout axis (default: vertical). Nested tree levels
  /// are always vertical.
  pub fn layout(mut self, layout: Axis) -> Self {
    self.layout = layout;
    self
  }

  /// Lay the top-level options out horizontally.
  pub fn horizontal(self) -> Self {
    self.layout(Axis::Horizontal)
  }

  /// Attach a change handler.
  ///
  /// The handler receives the new selection (leaf values in tree order) after
  /// the user toggles any checkbox in the group.
  pub fn on_change(
    mut self, handler: impl Fn(&Vec<SharedString>, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_change = Some(Rc::new(handler));
    self
  }

  fn render_option(&self, ids: &mut NodeIds, option: &CheckboxOption, cx: &App) -> AnyElement {
    let id = ids.next();
    let selection = SubtreeSelection::of(option, &self.selected);
    let leaves = option_leaves(option);
    let disabled = self.disabled || option.disabled;

    let checkbox = Checkbox::new(id)
      .checked(selection.all_selected())
      .indeterminate(selection.some_selected())
      .disabled(disabled)
      .with_size(self.size)
      .label(option.label.clone())
      .when_some(
        self.on_change.clone().filter(|_| !disabled),
        |checkbox, on_change| {
          let options = self.options.clone();
          let selected = self.selected.clone();
          checkbox.on_click(move |next: &bool, window, cx| {
            let next_selected = apply_toggle(&options, &selected, &leaves, *next);
            on_change(&next_selected, window, cx);
          })
        },
      );

    let children = option
      .children
      .iter()
      .map(|child| self.render_option(ids, child, cx))
      .collect();

    layout_option_node(self.size, cx, checkbox.into_any_element(), children)
  }
}

impl_disableable!(CheckboxGroup);
impl_sizable!(CheckboxGroup);
impl_styled!(CheckboxGroup);

impl RenderOnce for CheckboxGroup {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let mut ids = NodeIds::new(self.name.clone());
    match self.layout {
      Axis::Vertical => v_flex(),
      Axis::Horizontal => h_flex().flex_wrap(),
    }
    .gap(self.size.em(0.75))
    .children(
      self
        .options
        .iter()
        .map(|option| self.render_option(&mut ids, option, cx)),
    )
    .refine_style(&self.style)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn tree() -> Vec<CheckboxOption> {
    vec![
      CheckboxOption::new("parent-1", "Parent 1")
        .child(CheckboxOption::new("leaf-a", "A"))
        .child(CheckboxOption::new("leaf-b", "B")),
      CheckboxOption::new("leaf-c", "C"),
    ]
  }

  fn sel(values: &[&str]) -> Vec<SharedString> {
    values.iter().map(|v| (*v).into()).collect()
  }

  /// A three-level tree so that state can be derived through an inner branch.
  fn deep_tree() -> Vec<CheckboxOption> {
    vec![CheckboxOption::new("root", "Root").children([
      CheckboxOption::new("branch-a", "A").children([
        CheckboxOption::new("leaf-a1", "A1"),
        CheckboxOption::new("leaf-a2", "A2"),
      ]),
      CheckboxOption::new("branch-b", "B").child(CheckboxOption::new("leaf-b1", "B1")),
    ])]
  }

  #[test]
  fn leaf_state_is_direct() {
    let options = tree();
    let selection = SubtreeSelection::of(&options[1], &sel(&["leaf-c"]));
    assert!(selection.all_selected());
    assert!(!selection.some_selected());
  }

  #[test]
  fn node_state_is_derived() {
    let options = tree();
    let selection = SubtreeSelection::of(&options[0], &sel(&["leaf-a"]));
    assert!(!selection.all_selected());
    assert!(selection.some_selected());

    let selection = SubtreeSelection::of(&options[0], &sel(&["leaf-a", "leaf-b"]));
    assert!(selection.all_selected());
  }

  #[test]
  fn partially_checked_subtree_bubbles_indeterminate_to_every_ancestor() {
    let options = deep_tree();
    let selected = sel(&["leaf-a2"]);

    let root = SubtreeSelection::of(&options[0], &selected);
    assert!(!root.all_selected());
    assert!(root.some_selected());

    let branch = SubtreeSelection::of(&options[0].children[0], &selected);
    assert!(!branch.all_selected());
    assert!(branch.some_selected());

    let sibling = SubtreeSelection::of(&options[0].children[1], &selected);
    assert_eq!(sibling, SubtreeSelection::None);
  }

  #[test]
  fn toggling_node_selects_subtree_in_tree_order() {
    let options = tree();
    let next = apply_toggle(&options, &[], &option_leaves(&options[0]), true);
    assert_eq!(next, sel(&["leaf-a", "leaf-b"]));

    let next = apply_toggle(
      &options,
      &sel(&["leaf-a", "leaf-b"]),
      &option_leaves(&options[0]),
      false,
    );
    assert_eq!(next, sel(&[]));
  }

  #[test]
  fn toggling_nested_node_leaves_siblings_untouched() {
    let options = deep_tree();
    let current = sel(&["leaf-b1"]);

    let next = apply_toggle(
      &options,
      &current,
      &option_leaves(&options[0].children[0]),
      true,
    );
    assert_eq!(next, sel(&["leaf-a1", "leaf-a2", "leaf-b1"]));

    let next = apply_toggle(
      &options,
      &next,
      &option_leaves(&options[0].children[0]),
      false,
    );
    assert_eq!(next, sel(&["leaf-b1"]));
  }

  #[test]
  fn toggling_orders_the_selection_by_tree_not_by_input_order() {
    let options = tree();
    // The caller's selection may arrive in any order; the reported selection
    // always follows tree order.
    let next = apply_toggle(&options, &sel(&["leaf-b", "leaf-a"]), &[], false);
    assert_eq!(next, sel(&["leaf-a", "leaf-b"]));
  }

  #[test]
  fn values_outside_the_tree_are_preserved() {
    let options = tree();
    let current = sel(&["leaf-a", "external"]);
    let next = apply_toggle(&options, &current, &option_leaves(&options[0]), true);
    assert_eq!(next, sel(&["leaf-a", "leaf-b", "external"]));
  }
}
