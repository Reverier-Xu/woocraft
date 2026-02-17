use gpui::Axis;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DockPlacement {
  #[serde(rename = "center")]
  Center,
  #[serde(rename = "left")]
  Left,
  #[serde(rename = "bottom")]
  Bottom,
  #[serde(rename = "right")]
  Right,
}

impl DockPlacement {
  #[inline]
  pub fn axis(self) -> Axis {
    match self {
      Self::Left | Self::Right => Axis::Horizontal,
      Self::Bottom => Axis::Vertical,
      Self::Center => Axis::Horizontal,
    }
  }

  #[inline]
  pub fn is_left(self) -> bool {
    matches!(self, Self::Left)
  }

  #[inline]
  pub fn is_bottom(self) -> bool {
    matches!(self, Self::Bottom)
  }

  #[inline]
  pub fn is_right(self) -> bool {
    matches!(self, Self::Right)
  }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Placement {
  #[serde(rename = "top")]
  Top,
  #[serde(rename = "bottom")]
  Bottom,
  #[serde(rename = "left")]
  Left,
  #[serde(rename = "right")]
  Right,
}

impl Placement {
  #[inline]
  pub fn is_horizontal(self) -> bool {
    matches!(self, Self::Left | Self::Right)
  }

  #[inline]
  pub fn is_vertical(self) -> bool {
    matches!(self, Self::Top | Self::Bottom)
  }

  #[inline]
  pub fn axis(self) -> Axis {
    match self {
      Self::Top | Self::Bottom => Axis::Vertical,
      Self::Left | Self::Right => Axis::Horizontal,
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
