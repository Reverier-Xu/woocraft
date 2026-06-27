//! Anchor positioning for popover and floating element placement.
//!
//! Defines nine anchor points around an element (top-left, top-center,
//! top-right, bottom-left, bottom-center, bottom-right, plus center variations)
//! for positioning popovers, dropdowns, and other floating containers.

use gpui::Anchor as GpuiAnchor;

/// Anchor point for positioning floating elements (popovers, dropdowns, menus).
///
/// Specifies where a floating container should be positioned relative to a
/// trigger element. Example: TopRight anchors the floating element to the
/// top-right corner of the trigger.
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

impl From<GpuiAnchor> for Anchor {
  fn from(anchor: GpuiAnchor) -> Self {
    match anchor {
      GpuiAnchor::TopLeft => Self::TopLeft,
      GpuiAnchor::TopCenter => Self::TopCenter,
      GpuiAnchor::TopRight => Self::TopRight,
      GpuiAnchor::BottomLeft => Self::BottomLeft,
      GpuiAnchor::BottomCenter => Self::BottomCenter,
      GpuiAnchor::BottomRight => Self::BottomRight,
      GpuiAnchor::LeftCenter => Self::TopLeft,
      GpuiAnchor::RightCenter => Self::TopRight,
    }
  }
}

impl From<Anchor> for GpuiAnchor {
  fn from(anchor: Anchor) -> Self {
    match anchor {
      Anchor::TopLeft => Self::TopLeft,
      Anchor::TopCenter => Self::TopCenter,
      Anchor::TopRight => Self::TopRight,
      Anchor::BottomLeft => Self::BottomLeft,
      Anchor::BottomCenter => Self::BottomCenter,
      Anchor::BottomRight => Self::BottomRight,
    }
  }
}
