//! group container for a set of exclusive radios.
//!
//! this is a styled wrapper over [`gpui_base::RadioGroup`], which owns the
//! radio-group role and orientation semantics. the wrapper adds the
//! design-system vocabulary: a flex column (or row) with a `0.5rem` gap.
//! the group does not own the selection — keep the selected value in the
//! application and derive each radio's `checked` from it.

use gpui::{
  AnyElement, App, Axis, ElementId, IntoElement, ParentElement, Refineable as _, RenderOnce,
  StyleRefinement, Styled, Window, rems,
};
use gpui_base::RadioGroup as BaseRadioGroup;

/// container element for a set of [`Radio`](crate::Radio)s styled by the
/// woocraft design system.
///
/// ```rust,ignore
/// use woocraft::RadioGroup;
///
/// RadioGroup::new("plan")
///   .child(Radio::new("basic").label("basic").set_position(1, 2))
///   .child(Radio::new("pro").label("pro").set_position(2, 2));
/// ```
#[derive(IntoElement)]
pub struct RadioGroup {
  base: BaseRadioGroup,
  axis: Axis,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl RadioGroup {
  /// creates a vertical radio group with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      base: BaseRadioGroup::new(id.into()),
      axis: Axis::Vertical,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the semantic axis of the group. the default is vertical.
  pub fn axis(mut self, axis: Axis) -> Self {
    self.axis = axis;
    self.base = self.base.axis(axis);
    self
  }
}

impl Styled for RadioGroup {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for RadioGroup {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for RadioGroup {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let mut base = self.base;

    base = match self.axis {
      Axis::Horizontal => base.flex().flex_row().items_center(),
      Axis::Vertical => base.flex().flex_col().items_start(),
    }
    .gap(rems(0.5));

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base.children(self.children)
  }
}

#[cfg(test)]
mod tests {
  use gpui::Axis;

  use super::RadioGroup;

  #[test]
  fn default_axis_is_vertical() {
    let group = RadioGroup::new("test");
    assert_eq!(group.axis, Axis::Vertical);
  }

  #[test]
  fn axis_setter_selects_the_requested_axis() {
    let group = RadioGroup::new("test").axis(Axis::Horizontal);
    assert_eq!(group.axis, Axis::Horizontal);
  }
}
