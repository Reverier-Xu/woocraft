use std::rc::Rc;

use gpui::{
  App, Bounds, IntoElement, Pixels, RenderOnce, SharedString, Styled, TextAlign, Window, canvas,
  fill, px,
};
use num_traits::{Num, ToPrimitive};

use crate::{
  AXIS_GAP, ActiveTheme, AxisText, Grid, Plot, PlotAxis, origin_point,
  scale::{Scale, ScaleBand, ScaleLinear, Sealed},
};

type XAccessor<T, X> = Rc<dyn Fn(&T) -> X>;
type YAccessor<T, Y> = Rc<dyn Fn(&T) -> Y>;

#[derive(IntoElement)]
pub struct CandlestickChart<T, X, Y>
where
  T: 'static,
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static, {
  data: Vec<T>,
  x: Option<XAccessor<T, X>>,
  open: Option<YAccessor<T, Y>>,
  high: Option<YAccessor<T, Y>>,
  low: Option<YAccessor<T, Y>>,
  close: Option<YAccessor<T, Y>>,
  tick_margin: usize,
  body_width_ratio: f32,
}

impl<T, X, Y> CandlestickChart<T, X, Y>
where
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  pub fn new<I>(data: I) -> Self
  where
    I: IntoIterator<Item = T>, {
    Self {
      data: data.into_iter().collect(),
      x: None,
      open: None,
      high: None,
      low: None,
      close: None,
      tick_margin: 1,
      body_width_ratio: 0.8,
    }
  }

  pub fn x(mut self, x: impl Fn(&T) -> X + 'static) -> Self {
    self.x = Some(Rc::new(x));
    self
  }

  pub fn open(mut self, open: impl Fn(&T) -> Y + 'static) -> Self {
    self.open = Some(Rc::new(open));
    self
  }

  pub fn high(mut self, high: impl Fn(&T) -> Y + 'static) -> Self {
    self.high = Some(Rc::new(high));
    self
  }

  pub fn low(mut self, low: impl Fn(&T) -> Y + 'static) -> Self {
    self.low = Some(Rc::new(low));
    self
  }

  pub fn close(mut self, close: impl Fn(&T) -> Y + 'static) -> Self {
    self.close = Some(Rc::new(close));
    self
  }

  pub fn tick_margin(mut self, tick_margin: usize) -> Self {
    self.tick_margin = tick_margin.max(1);
    self
  }

  pub fn body_width_ratio(mut self, ratio: f32) -> Self {
    self.body_width_ratio = ratio.clamp(0.1, 1.0);
    self
  }
}

impl<T, X, Y> Plot for CandlestickChart<T, X, Y>
where
  X: PartialEq + Into<SharedString> + 'static,
  Y: Copy + PartialOrd + Num + ToPrimitive + Sealed + 'static,
{
  fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let (Some(x_fn), Some(open_fn), Some(high_fn), Some(low_fn), Some(close_fn)) = (
      self.x.as_ref(),
      self.open.as_ref(),
      self.high.as_ref(),
      self.low.as_ref(),
      self.close.as_ref(),
    ) else {
      return;
    };

    let width = bounds.size.width.as_f32();
    let height = bounds.size.height.as_f32() - AXIS_GAP;

    let x = ScaleBand::new(self.data.iter().map(|v| x_fn(v)).collect(), vec![0., width])
      .padding_inner(0.4)
      .padding_outer(0.2);
    let band_width = x.band_width();

    let all_values: Vec<Y> = self
      .data
      .iter()
      .flat_map(|d| [high_fn(d), low_fn(d), open_fn(d), close_fn(d)])
      .collect();
    let y = ScaleLinear::new(all_values, vec![height, 10.]);

    let x_label = self.data.iter().enumerate().filter_map(|(i, d)| {
      if (i + 1) % self.tick_margin == 0 {
        x.tick(&x_fn(d)).map(|x_tick| {
          AxisText::new(
            x_fn(d).into(),
            x_tick + band_width / 2.,
            cx.theme().muted_foreground,
          )
          .align(TextAlign::Center)
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

    let origin = bounds.origin;

    for d in &self.data {
      let Some(x_tick) = x.tick(&x_fn(d)) else {
        continue;
      };

      let open = open_fn(d);
      let high = high_fn(d);
      let low = low_fn(d);
      let close = close_fn(d);

      let (Some(open_y), Some(high_y), Some(low_y), Some(close_y)) =
        (y.tick(&open), y.tick(&high), y.tick(&low), y.tick(&close))
      else {
        continue;
      };

      let is_bullish = close >= open;
      let color = if is_bullish {
        cx.theme().success
      } else {
        cx.theme().danger
      };

      let center_x = x_tick + band_width / 2.;
      let body_width = band_width * self.body_width_ratio;
      let body_left = center_x - body_width / 2.;
      let body_right = center_x + body_width / 2.;

      let mut wick_builder = gpui::PathBuilder::stroke(px(1.));
      wick_builder.move_to(origin_point(px(center_x), px(high_y), origin));
      wick_builder.line_to(origin_point(px(center_x), px(low_y), origin));

      if let Ok(path) = wick_builder.build() {
        window.paint_path(path, color);
      }

      let (top, bottom) = if is_bullish {
        (close_y, open_y)
      } else {
        (open_y, close_y)
      };

      let body_bounds = Bounds::from_corners(
        origin_point(px(body_left), px(top), origin),
        origin_point(px(body_right), px(bottom), origin),
      );

      window.paint_quad(fill(body_bounds, color));
    }
  }
}

impl<T, X, Y> RenderOnce for CandlestickChart<T, X, Y>
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
