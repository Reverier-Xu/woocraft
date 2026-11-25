//! RadioGroup component for grouped and nested (tree) single-selection.
//!
//! RadioGroup renders a set of [`Radio`] options in a flat list or a nested
//! tree. Selection state is controlled by the caller: the group only owns
//! presentation, and reports user intent through [`RadioGroup::on_change`].
//!
//! # Tree semantics
//!
//! Only leaf options hold state: the selection is a single leaf value. A
//! parent radio is checked when the selected leaf lies within its subtree.
//! Clicking a parent selects the first leaf of its subtree (in tree order).
//!
//! # Example
//! ```rust,ignore
//! RadioGroup::new("plan")
//!   .option(
//!     RadioOption::new("team", "Team")
//!       .child(RadioOption::new("monthly", "Monthly"))
//!       .child(RadioOption::new("yearly", "Yearly")),
//!   )
//!   .option(RadioOption::new("free", "Free"))
//!   .selected(Some("monthly".into()))
//!   .on_change(|selected: &SharedString, _window, _cx| {
//!     // Handle selection
//!   })
//! ```

use std::rc::Rc;

use gpui::{
  AnyElement, App, Axis, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement,
  Styled, Window, prelude::FluentBuilder as _,
};

use super::selection_tree::{
  NodeIds, OptionNode, SubtreeSelection, first_leaf, layout_option_node,
};
use crate::{Disableable, Radio, Sizable, Size, StyledExt, h_flex, v_flex};

type RadioGroupChangeHandler = Rc<dyn Fn(&SharedString, &mut Window, &mut App) + 'static>;

/// A single option (node) in a [`RadioGroup`].
///
/// An option without children is a leaf and holds selection state. An option
/// with children is a tree node whose state is derived from its descendants.
#[derive(Clone, Debug)]
pub struct RadioOption {
  value: SharedString,
  label: SharedString,
  disabled: bool,
  children: Vec<Self>,
}

impl RadioOption {
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

impl OptionNode for RadioOption {
  fn value(&self) -> &SharedString {
    &self.value
  }

  fn child_options(&self) -> &[Self] {
    &self.children
  }
}

#[derive(IntoElement)]
/// Group of radios with flat or nested (tree) layout.
///
/// RadioGroup is a controlled component: it renders `selected` and reports
/// changes through [`RadioGroup::on_change`]. See the [module
/// documentation](self) for tree semantics.
pub struct RadioGroup {
  name: SharedString,
  style: StyleRefinement,
  options: Vec<RadioOption>,
  selected: Option<SharedString>,
  layout: Axis,
  disabled: bool,
  size: Size,
  on_change: Option<RadioGroupChangeHandler>,
}

impl RadioGroup {
  /// Create a new group with the given name.
  ///
  /// The name must be unique within the window; it namespaces the focus IDs
  /// of the rendered radios.
  pub fn new(name: impl Into<SharedString>) -> Self {
    Self {
      name: name.into(),
      style: StyleRefinement::default(),
      options: Vec::new(),
      selected: None,
      layout: Axis::Vertical,
      disabled: false,
      size: Size::default(),
      on_change: None,
    }
  }

  /// Append an option to the group.
  pub fn option(mut self, option: RadioOption) -> Self {
    self.options.push(option);
    self
  }

  /// Append multiple options to the group.
  pub fn options(mut self, options: impl IntoIterator<Item = RadioOption>) -> Self {
    self.options.extend(options);
    self
  }

  /// Set the selected leaf value (default: none).
  pub fn selected(mut self, selected: Option<SharedString>) -> Self {
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
  /// The handler receives the newly selected leaf value after the user picks
  /// any radio in the group.
  pub fn on_change(
    mut self, handler: impl Fn(&SharedString, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_change = Some(Rc::new(handler));
    self
  }

  fn render_option(&self, ids: &mut NodeIds, option: &RadioOption, cx: &App) -> AnyElement {
    let id = ids.next();
    let checked = SubtreeSelection::of(option, self.selected.as_slice()).any_selected();
    let target = first_leaf(option);
    let disabled = self.disabled || option.disabled;

    let radio = Radio::new(id)
      .checked(checked)
      .disabled(disabled)
      .with_size(self.size)
      .label(option.label.clone())
      .when_some(
        self.on_change.clone().filter(|_| !disabled),
        |radio, on_change| {
          radio.on_click(move |_next: &bool, window, cx| on_change(&target, window, cx))
        },
      );

    let children = option
      .children
      .iter()
      .map(|child| self.render_option(ids, child, cx))
      .collect();

    layout_option_node(self.size, cx, radio.into_any_element(), children)
  }
}

impl_disableable!(RadioGroup);
impl_sizable!(RadioGroup);
impl_styled!(RadioGroup);

impl RenderOnce for RadioGroup {
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

  fn tree() -> Vec<RadioOption> {
    vec![
      RadioOption::new("parent-1", "Parent 1")
        .child(RadioOption::new("leaf-a", "A"))
        .child(RadioOption::new("leaf-b", "B")),
      RadioOption::new("leaf-c", "C"),
    ]
  }

  fn sel(values: &[&str]) -> Vec<SharedString> {
    values.iter().map(|v| (*v).into()).collect()
  }

  /// A three-level tree so that parents can be nested deeper than one level.
  fn deep_tree() -> Vec<RadioOption> {
    vec![RadioOption::new("root", "Root").children([
      RadioOption::new("branch-a", "A").children([
        RadioOption::new("leaf-a1", "A1"),
        RadioOption::new("leaf-a2", "A2"),
      ]),
      RadioOption::new("branch-b", "B").child(RadioOption::new("leaf-b1", "B1")),
    ])]
  }

  #[test]
  fn leaf_state_is_direct() {
    let options = tree();
    assert!(SubtreeSelection::of(&options[1], &sel(&["leaf-c"])).any_selected());
    assert!(!SubtreeSelection::of(&options[1], &sel(&["leaf-a"])).any_selected());
  }

  #[test]
  fn node_state_is_derived_from_subtree() {
    let options = tree();
    assert!(SubtreeSelection::of(&options[0], &sel(&["leaf-a"])).any_selected());
    assert!(SubtreeSelection::of(&options[0], &sel(&["leaf-b"])).any_selected());
    assert!(!SubtreeSelection::of(&options[0], &sel(&["leaf-c"])).any_selected());
  }

  #[test]
  fn clicking_an_option_targets_its_first_leaf() {
    let options = tree();
    assert_eq!(first_leaf(&options[0]), SharedString::from("leaf-a"));
    assert_eq!(
      first_leaf(&options[0].children[1]),
      SharedString::from("leaf-b")
    );
    assert_eq!(first_leaf(&options[1]), SharedString::from("leaf-c"));
  }

  #[test]
  fn clicking_a_nested_parent_targets_the_first_leaf_of_the_subtree() {
    let options = deep_tree();
    assert_eq!(first_leaf(&options[0]), SharedString::from("leaf-a1"));
    assert_eq!(
      first_leaf(&options[0].children[0]),
      SharedString::from("leaf-a1")
    );
    assert_eq!(
      first_leaf(&options[0].children[1]),
      SharedString::from("leaf-b1")
    );
  }
}
