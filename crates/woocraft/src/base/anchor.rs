//! Anchor positioning for popover and floating element placement.
//!
//! Defines nine anchor points around an element (top-left, top-center, top-right,
//! bottom-left, bottom-center, bottom-right, plus center variations) for positioning
//! popovers, dropdowns, and other floating containers.

use gpui::Corner;

/// Anchor point for positioning floating elements (popovers, dropdowns, menus).
///
/// Specifies where a floating container should be positioned relative to a trigger element.
/// Example: TopRight anchors the floating element to the top-right corner of the trigger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
  #[default]
  TopLeft,
  TopCenter,
  TopRight,
  BottomLeft,
  BottomCenter,
  BottomRight,
}

impl Anchor {
  #[inline]
  pub fn is_center(self) -> bool {
    matches!(self, Self::TopCenter | Self::BottomCenter)
  }

  #[inline]
  pub fn swap_vertical(self) -> Self {
    match self {
      Self::TopLeft => Self::BottomLeft,
      Self::TopCenter => Self::BottomCenter,
      Self::TopRight => Self::BottomRight,
      Self::BottomLeft => Self::TopLeft,
      Self::BottomCenter => Self::TopCenter,
      Self::BottomRight => Self::TopRight,
    }
  }
}

impl From<Corner> for Anchor {
  fn from(corner: Corner) -> Self {
    match corner {
      Corner::TopLeft => Self::TopLeft,
      Corner::TopRight => Self::TopRight,
      Corner::BottomLeft => Self::BottomLeft,
      Corner::BottomRight => Self::BottomRight,
    }
  }
}

impl From<Anchor> for Corner {
  fn from(anchor: Anchor) -> Self {
    match anchor {
      Anchor::TopLeft => Corner::TopLeft,
      Anchor::TopCenter => Corner::TopLeft,
      Anchor::TopRight => Corner::TopRight,
      Anchor::BottomLeft => Corner::BottomLeft,
      Anchor::BottomCenter => Corner::BottomLeft,
      Anchor::BottomRight => Corner::BottomRight,
    }
  }
}
