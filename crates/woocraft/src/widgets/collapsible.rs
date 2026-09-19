//! collapsible region with an animated height reveal.
//!
//! this is a styled wrapper over [`gpui_base::Collapsible`]. the wrapper
//! drives the base [`MotionReveal`] with a retargeting transition over the
//! theme reveal duration, so toggling `open` animates the content height —
//! unmounting it once the close transition settles and short-circuiting to
//! the final height under the system reduce-motion preference.

use gpui::{
  AnyElement, App, ElementId, IntoElement, ParentElement, Refineable as _, RenderOnce,
  StyleRefinement, Styled, Window,
};
use gpui_base::{
  Collapsible as BaseCollapsible,
  motion::{MotionStatus, Transition, transition_with_status},
};

use crate::theme::duration;

/// collapsible element styled by the woocraft design system.
///
/// children render unconditionally (triggers, headers); the content slot
/// renders through the animated reveal:
///
/// ```rust,ignore
/// use gpui::prelude::FluentBuilder as _;
/// use woocraft::Collapsible;
///
/// Collapsible::new("details")
///   .open(self.open)
///   .when(!self.open, |this| this.child(trigger))
///   .content(details_body);
/// ```
#[derive(IntoElement)]
pub struct Collapsible {
  id: ElementId,
  base: BaseCollapsible,
  open: bool,
  content: Vec<AnyElement>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Collapsible {
  /// creates a collapsed region with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      base: BaseCollapsible::new(),
      open: false,
      content: Vec::new(),
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the application-controlled open state.
  pub fn open(mut self, open: bool) -> Self {
    self.open = open;
    self.base = self.base.open(open);
    self
  }

  /// sets the content revealed while the region is open.
  pub fn content(mut self, content: impl IntoElement) -> Self {
    self.content.push(content.into_any_element());
    self
  }
}

impl Styled for Collapsible {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Collapsible {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Collapsible {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let open = self.open;
    let target = if open { 1.0 } else { 0.0 };
    let sample = transition_with_status(
      self.id.clone(),
      target,
      Transition::new(duration::REVEAL),
      window,
      cx,
    );
    let closing = matches!(sample.status, MotionStatus::Delayed | MotionStatus::Running);

    let mut base = self.base;
    if open || closing {
      base = base.reveal((self.id.clone(), "reveal"), sample.value);
      for content in self.content {
        base = base.content(content);
      }
    }

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base.children(self.children)
  }
}

#[cfg(test)]
mod tests {
  use super::Collapsible;

  #[test]
  fn default_state_is_closed() {
    let collapsible = Collapsible::new("test");
    assert!(!collapsible.open);
  }

  #[test]
  fn open_setter_stores_the_boolean_value() {
    assert!(Collapsible::new("test").open(true).open);
    assert!(!Collapsible::new("test").open(true).open(false).open);
  }
}
