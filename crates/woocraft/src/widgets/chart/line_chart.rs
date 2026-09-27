use std::rc::Rc;

use gpui::{App, Bounds, IntoElement, Pixels, RenderOnce, SharedString, Styled, Window, canvas};
use num_traits::{Num, ToPrimitive};

use super::axes::{XTickAlign, paint_axes};
use crate::{
  AXIS_GAP, ActiveTheme, Plot, StrokeStyle,
  scale::{Scale, ScaleLinear, ScalePoint, Sealed},
  shape::Line,
};

type XAccessor<T, X> = Rc<dyn Fn(&T) -> X>;
type YAccessor<T, Y> = Rc<dyn Fn(&T) -> Y>;

#[derive(IntoElement)]
pub struct LineChart<T, X, Y>
where
  T: 'static,
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static, {
  data: Vec<T>,
  x: Option<XAccessor<T, X>>,
  y: Option<YAccessor<T, Y>>,
  stroke: Option<gpui::Hsla>,
  stroke_style: StrokeStyle,
  dot: bool,
  tick_margin: usize,
}

impl<T, X, Y> LineChart<T, X, Y>
where
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  pub fn new<I>(data: I) -> Self
  where
    I: IntoIterator<Item = T>, {
    Self {
      data: data.into_iter().collect(),
      stroke: None,
      stroke_style: StrokeStyle::default(),
      dot: false,
      x: None,
      y: None,
      tick_margin: 1,
    }
  }

  pub fn x(mut self, x: impl Fn(&T) -> X + 'static) -> Self {
    self.x = Some(Rc::new(x));
    self
  }

  pub fn y(mut self, y: impl Fn(&T) -> Y + 'static) -> Self {
    self.y = Some(Rc::new(y));
    self
  }

  pub fn stroke(mut self, stroke: impl Into<gpui::Hsla>) -> Self {
    self.stroke = Some(stroke.into());
    self
  }

  pub fn natural(mut self) -> Self {
    self.stroke_style = StrokeStyle::Natural;
    self
  }

  pub fn linear(mut self) -> Self {
    self.stroke_style = StrokeStyle::Linear;
    self
  }

  pub fn step_after(mut self) -> Self {
    self.stroke_style = StrokeStyle::StepAfter;
    self
  }

  pub fn dot(mut self) -> Self {
    self.dot = true;
    self
  }

  pub fn tick_margin(mut self, tick_margin: usize) -> Self {
    self.tick_margin = tick_margin.max(1);
    self
  }
}

impl<T, X, Y> Plot for LineChart<T, X, Y>
where
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let (Some(x_fn), Some(y_fn)) = (self.x.as_ref(), self.y.as_ref()) else {
      return;
    };

    let width = bounds.size.width.as_f32();
    let height = bounds.size.height.as_f32() - AXIS_GAP;

    // Single accessor pass: each datum's (x, y) is materialized once and the
    // results drive both scale domains (and, through them, the tick lookups
    // below).
    let points: Vec<(X, Y)> = self.data.iter().map(|v| (x_fn(v), y_fn(v))).collect();
    let (x_domain, y_domain): (Vec<X>, Vec<Y>) = points.into_iter().unzip();

    let x = ScalePoint::new(x_domain, vec![0., width]);
    let y = ScaleLinear::new(
      y_domain.into_iter().chain(Some(Y::zero())).collect(),
      vec![height, 10.],
    );

    paint_axes(
      self.data.len(),
      |i| {
        let x_value = x_fn(&self.data[i]);
        let tick = x.tick(&x_value)?;
        Some((x_value.into(), tick))
      },
      self.tick_margin,
      XTickAlign::Edges,
      bounds,
      window,
      cx,
    );

    let stroke = self.stroke.unwrap_or(cx.theme().primary);
    let x_fn = x_fn.clone();
    let y_fn = y_fn.clone();
    let mut line = Line::new()
      .data(&self.data)
      .x(move |d| x.tick(&x_fn(d)))
      .y(move |d| y.tick(&y_fn(d)))
      .stroke(stroke)
      .stroke_style(self.stroke_style)
      .stroke_width(2.);

    if self.dot {
      line = line.dot().dot_size(8.).dot_fill_color(stroke);
    }

    line.paint(&bounds, window);
  }
}

impl<T, X, Y> RenderOnce for LineChart<T, X, Y>
where
  T: 'static,
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    canvas(
      move |_, _, _| {},
      move |bounds, _, window, cx| self.paint(bounds, window, cx),
    )
    .size_full()
  }
}
