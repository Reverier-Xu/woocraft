use std::rc::Rc;

use gpui::{
  App, Background, Bounds, IntoElement, Pixels, RenderOnce, SharedString, Styled, TextAlign,
  Window, canvas, px,
};
use num_traits::{Num, ToPrimitive};

use crate::{
  AXIS_GAP, ActiveTheme, AxisText, Grid, PixelsExt, Plot, PlotAxis, StrokeStyle,
  scale::{Scale, ScaleLinear, ScalePoint, Sealed},
  shape::Area,
};

type XAccessor<T, X> = Rc<dyn Fn(&T) -> X>;
type YAccessor<T, Y> = Rc<dyn Fn(&T) -> Y>;

#[inline]
fn palette(index: usize, cx: &App) -> gpui::Hsla {
  let colors = [
    cx.theme().primary,
    cx.theme().accent,
    cx.theme().success,
    cx.theme().warning,
    cx.theme().danger,
    cx.theme().blue,
    cx.theme().cyan,
    cx.theme().green,
  ];
  colors[index % colors.len()]
}

#[derive(IntoElement)]
pub struct AreaChart<T, X, Y>
where
  T: 'static,
  X: Clone + PartialEq + Into<SharedString> + 'static,
  Y: Clone + Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static, {
  data: Vec<T>,
  x: Option<XAccessor<T, X>>,
  y: Vec<YAccessor<T, Y>>,
  strokes: Vec<gpui::Hsla>,
  stroke_styles: Vec<StrokeStyle>,
  fills: Vec<Background>,
  tick_margin: usize,
}

impl<T, X, Y> AreaChart<T, X, Y>
where
  X: Clone + PartialEq + Into<SharedString> + 'static,
  Y: Clone + Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  pub fn new<I>(data: I) -> Self
  where
    I: IntoIterator<Item = T>, {
    Self {
      data: data.into_iter().collect(),
      stroke_styles: vec![],
      strokes: vec![],
      fills: vec![],
      tick_margin: 1,
      x: None,
      y: vec![],
    }
  }

  pub fn x(mut self, x: impl Fn(&T) -> X + 'static) -> Self {
    self.x = Some(Rc::new(x));
    self
  }

  pub fn y(mut self, y: impl Fn(&T) -> Y + 'static) -> Self {
    self.y.push(Rc::new(y));
    self
  }

  pub fn stroke(mut self, stroke: impl Into<gpui::Hsla>) -> Self {
    self.strokes.push(stroke.into());
    self
  }

  pub fn fill(mut self, fill: impl Into<Background>) -> Self {
    self.fills.push(fill.into());
    self
  }

  pub fn natural(mut self) -> Self {
    self.stroke_styles.push(StrokeStyle::Natural);
    self
  }

  pub fn linear(mut self) -> Self {
    self.stroke_styles.push(StrokeStyle::Linear);
    self
  }

  pub fn step_after(mut self) -> Self {
    self.stroke_styles.push(StrokeStyle::StepAfter);
    self
  }

  pub fn tick_margin(mut self, tick_margin: usize) -> Self {
    self.tick_margin = tick_margin.max(1);
    self
  }
}

impl<T, X, Y> Plot for AreaChart<T, X, Y>
where
  X: Clone + PartialEq + Into<SharedString> + 'static,
  Y: Clone + Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let Some(x_fn) = self.x.as_ref() else {
      return;
    };

    if self.y.is_empty() {
      return;
    }

    let width = bounds.size.width.as_f32();
    let height = bounds.size.height.as_f32() - AXIS_GAP;

    let x = ScalePoint::new(self.data.iter().map(|v| x_fn(v)).collect(), vec![0., width]);

    let domain = self
      .data
      .iter()
      .flat_map(|v| self.y.iter().map(|y_fn| y_fn(v)))
      .chain(Some(Y::zero()))
      .collect::<Vec<_>>();
    let y = ScaleLinear::new(domain, vec![height, 10.]);

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

    for (i, y_fn) in self.y.iter().enumerate() {
      let x = x.clone();
      let y = y.clone();
      let x_fn = x_fn.clone();
      let y_fn = y_fn.clone();

      let stroke = *self.strokes.get(i).unwrap_or(&palette(i, cx));
      let fill = *self.fills.get(i).unwrap_or(&stroke.opacity(0.28).into());
      let stroke_style = *self.stroke_styles.get(i).unwrap_or(
        self
          .stroke_styles
          .first()
          .unwrap_or(&StrokeStyle::default()),
      );

      Area::new()
        .data(&self.data)
        .x(move |d| x.tick(&x_fn(d)))
        .y0(height)
        .y1(move |d| y.tick(&y_fn(d)))
        .stroke(stroke)
        .stroke_style(stroke_style)
        .fill(fill)
        .paint(&bounds, window);
    }
  }
}

impl<T, X, Y> RenderOnce for AreaChart<T, X, Y>
where
  T: 'static,
  X: Clone + PartialEq + Into<SharedString> + 'static,
  Y: Clone + Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    canvas(
      move |_, _, _| {},
      move |bounds, _, window, cx| self.paint(bounds, window, cx),
    )
    .size_full()
  }
}
