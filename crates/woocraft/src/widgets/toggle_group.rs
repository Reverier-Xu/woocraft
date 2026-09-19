//! group container for a set of related toggles.
//!
//! this is a styled wrapper over [`gpui_base::ToggleGroup`], which owns the
//! toolbar role and orientation semantics. the wrapper adds the
//! design-system vocabulary: a flex row (or column) with a `0.5rem` gap,
//! refinable through the standard [`Styled`] chain — a segmented look is a
//! `gap_0()` away.

use gpui::{
  AnyElement, App, Axis, ElementId, IntoElement, ParentElement, Refineable as _, RenderOnce,
  StyleRefinement, Styled, Window, rems,
};
use gpui_base::ToggleGroup as BaseToggleGroup;

/// container element for a set of [`Toggle`](crate::Toggle)s styled by the
/// woocraft design system.
///
/// the group projects the toolbar role and its orientation to assistive
/// technology; it does not own the toggles' pressed state — keep that in
/// the application and select through each toggle's `on_change`.
///
/// ```rust,ignore
/// use gpui::Axis;
/// use woocraft::ToggleGroup;
///
/// ToggleGroup::new("align").axis(Axis::Horizontal)
///   .child(Toggle::new("left").label("left"))
///   .child(Toggle::new("center").label("center"))
///   .child(Toggle::new("right").label("right"));
/// ```
#[derive(IntoElement)]
pub struct ToggleGroup {
  base: BaseToggleGroup,
  axis: Axis,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl ToggleGroup {
  /// creates a horizontal toggle group with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      base: BaseToggleGroup::new(id.into()),
      axis: Axis::Horizontal,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the semantic axis of the group. the default is horizontal.
  pub fn axis(mut self, axis: Axis) -> Self {
    self.axis = axis;
    self.base = self.base.axis(axis);
    self
  }
}

impl Styled for ToggleGroup {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for ToggleGroup {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for ToggleGroup {
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

  use super::ToggleGroup;

  #[test]
  fn default_axis_is_horizontal() {
    let group = ToggleGroup::new("test");
    assert_eq!(group.axis, Axis::Horizontal);
  }

  #[test]
  fn axis_setter_selects_the_requested_axis() {
    let group = ToggleGroup::new("test").axis(Axis::Vertical);
    assert_eq!(group.axis, Axis::Vertical);
  }
}
