mod axis;
mod grid;
pub mod label;
pub mod scale;
pub mod shape;
pub mod tooltip;

use std::{fmt::Debug, ops::Add};

pub use axis::{AXIS_GAP, AxisText, PlotAxis};
use gpuim::{App, Bounds, Path, PathBuilder, Pixels, Point, Window, point, px};
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

pub fn polygon<T>(points: &[Point<T>], bounds: &Bounds<Pixels>) -> Option<Path<Pixels>>
where
  T: Default + Clone + Copy + Debug + Into<f32> + PartialEq, {
  let mut path = PathBuilder::stroke(px(1.));
  let points = &points
    .iter()
    .map(|p| {
      point(
        px(p.x.into() + bounds.origin.x.as_f32()),
        px(p.y.into() + bounds.origin.y.as_f32()),
      )
    })
    .collect::<Vec<_>>();
  path.add_polygon(points, false);
  path.build().ok()
}
