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

use crate::{ActiveTheme, Checkbox, Disableable, Sizable, Size, StyledExt, h_flex, v_flex};

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

/// Collect all leaf values of an option subtree, in tree order.
fn option_leaves(option: &CheckboxOption) -> Vec<SharedString> {
  if option.children.is_empty() {
    vec![option.value.clone()]
  } else {
    option.children.iter().flat_map(option_leaves).collect()
  }
}

/// Collect all leaf values of a whole option tree, in tree order.
fn tree_leaves(options: &[CheckboxOption]) -> Vec<SharedString> {
  options.iter().flat_map(option_leaves).collect()
}

/// Derive the (checked, indeterminate) presentation state of an option.
fn option_state(option: &CheckboxOption, selected: &[SharedString]) -> (bool, bool) {
  let leaves = option_leaves(option);
  let checked_count = leaves
    .iter()
    .filter(|value| selected.contains(value))
    .count();
  let checked = checked_count == leaves.len() && !leaves.is_empty();
  let indeterminate = !checked && checked_count > 0;
  (checked, indeterminate)
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

  fn render_option(&self, index: &mut usize, option: &CheckboxOption, cx: &App) -> AnyElement {
    let id = (self.name.clone(), *index);
    *index += 1;

    let (checked, indeterminate) = option_state(option, &self.selected);
    let leaves = option_leaves(option);
    let disabled = self.disabled || option.disabled;

    let checkbox = Checkbox::new(id)
      .checked(checked)
      .indeterminate(indeterminate)
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

    if option.children.is_empty() {
      checkbox.into_any_element()
    } else {
      v_flex()
        .gap(self.size.component_gap())
        .child(checkbox)
        .child(
          v_flex()
            .gap(self.size.component_gap())
            .pl(self.size.em(1.))
            .ml(self.size.em(0.25))
            .border_l(self.size.em(0.125))
            .border_color(cx.theme().border)
            .children(
              option
                .children
                .iter()
                .map(|child| self.render_option(index, child, cx)),
            ),
        )
        .into_any_element()
    }
  }
}

impl_disableable!(CheckboxGroup);
impl_sizable!(CheckboxGroup);
impl_styled!(CheckboxGroup);

impl RenderOnce for CheckboxGroup {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let mut index = 0;
    match self.layout {
      Axis::Vertical => v_flex(),
      Axis::Horizontal => h_flex().flex_wrap(),
    }
    .gap(self.size.em(0.75))
    .children(
      self
        .options
        .iter()
        .map(|option| self.render_option(&mut index, option, cx)),
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

  #[test]
  fn leaf_state_is_direct() {
    let options = tree();
    let (checked, indeterminate) = option_state(&options[1], &sel(&["leaf-c"]));
    assert!(checked);
    assert!(!indeterminate);
  }

  #[test]
  fn node_state_is_derived() {
    let options = tree();
    let (checked, indeterminate) = option_state(&options[0], &sel(&["leaf-a"]));
    assert!(!checked);
    assert!(indeterminate);

    let (checked, _) = option_state(&options[0], &sel(&["leaf-a", "leaf-b"]));
    assert!(checked);
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
  fn values_outside_the_tree_are_preserved() {
    let options = tree();
    let current = sel(&["leaf-a", "external"]);
    let next = apply_toggle(&options, &current, &option_leaves(&options[0]), true);
    assert_eq!(next, sel(&["leaf-a", "leaf-b", "external"]));
  }
}
