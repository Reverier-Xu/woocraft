use std::rc::Rc;

use gpui::{
  App, Bounds, IntoElement, Pixels, RenderOnce, SharedString, Styled, TextAlign, Window, canvas, px,
};
use num_traits::{Num, ToPrimitive};

use crate::{
  AXIS_GAP, ActiveTheme, AxisText, Grid, Plot, PlotAxis, StrokeStyle,
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

    let x = ScalePoint::new(self.data.iter().map(|v| x_fn(v)).collect(), vec![0., width]);
    let y = ScaleLinear::new(
      self
        .data
        .iter()
        .map(|v| y_fn(v))
        .chain(Some(Y::zero()))
        .collect(),
      vec![height, 10.],
    );

    let data_len = self.data.len();
    let x_label = self.data.iter().enumerate().filter_map(|(i, d)| {
      if (i + 1) % self.tick_margin == 0 {
        x.tick(&x_fn(d)).map(|x_tick| {
          let align = match i {
            0 if data_len == 1 => TextAlign::Center,
            0 => TextAlign::Left,
            i if i == data_len - 1 => TextAlign::Right,
            _ => TextAlign::Center,
          };

          AxisText::new(x_fn(d).into(), x_tick, cx.theme().muted_foreground).align(align)
        })
      } else {
        None
      }
    });

    PlotAxis::new()
      .x(height)
      .x_label(x_label)
      .stroke(cx.theme().border)
      .paint(&bounds, window, cx);

    Grid::new()
      .y((0..=3).map(|i| height * i as f32 / 4.0).collect())
      .stroke(cx.theme().border)
      .dash_array(&[px(4.), px(2.)])
      .paint(&bounds, window);

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
