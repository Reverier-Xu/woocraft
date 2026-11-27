use gpui::Axis;
use serde::{Deserialize, Serialize};

/// Where something sits inside a container.
///
/// One type for every placement decision in the layout system — a dock slot,
/// a split direction, a tab-bar edge — so callers never convert between
/// per-widget direction enums. [`Self::Center`] is the no-side case: a dock
/// pinned to the middle of the window, or a drop that joins the target tab
/// group instead of splitting it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Placement {
  #[default]
  #[serde(rename = "top")]
  Top,
  #[serde(rename = "bottom")]
  Bottom,
  #[serde(rename = "left")]
  Left,
  #[serde(rename = "right")]
  Right,
  #[serde(rename = "center")]
  Center,
}

impl Placement {
  /// The axis a container splits along to place something on this side.
  ///
  /// [`Self::Center`] never splits; it reports the horizontal axis so callers
  /// that only need a stable default (a resize handle, a stack direction) do
  /// not have to special-case it.
  #[inline]
  pub fn axis(self) -> Axis {
    match self {
      Self::Top | Self::Bottom => Axis::Vertical,
      Self::Left | Self::Right | Self::Center => Axis::Horizontal,
    }
  }
}

pub trait AxisExt {
  fn is_horizontal(&self) -> bool;
  fn is_vertical(&self) -> bool;
}

impl AxisExt for Axis {
  #[inline]
  fn is_horizontal(&self) -> bool {
    *self == Axis::Horizontal
  }

  #[inline]
  fn is_vertical(&self) -> bool {
    *self == Axis::Vertical
  }
}
