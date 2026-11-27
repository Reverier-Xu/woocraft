mod axis;
mod grid;
pub mod label;
pub mod scale;
pub mod shape;

use std::{fmt::Debug, ops::Add};

pub use axis::{AXIS_GAP, AxisText, PlotAxis};
use gpui::{App, Bounds, Pixels, Point, Window, point};
pub use grid::Grid;
pub use label::PlotLabel;

pub trait Plot {
  fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App);
}

#[derive(Clone, Copy, Default)]
pub enum StrokeStyle {
  #[default]
  Natural,
  Linear,
  StepAfter,
}

pub fn origin_point<T>(x: T, y: T, origin: Point<T>) -> Point<T>
where
  T: Default + Clone + Debug + PartialEq + Add<Output = T>, {
  point(x, y) + origin
}
