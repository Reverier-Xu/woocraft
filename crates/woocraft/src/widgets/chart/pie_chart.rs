use std::rc::Rc;

use gpui::{App, Bounds, IntoElement, Pixels, RenderOnce, Styled, Window, canvas};

use crate::{
  ActiveTheme, Plot,
  base::PixelsExt,
  shape::{Arc, ArcData, Pie},
};

type ArcRadiusFn<T> = Rc<dyn Fn(&ArcData<T>) -> f32 + 'static>;
type ValueFn<T> = Rc<dyn Fn(&T) -> f32>;
type ColorFn<T> = Rc<dyn Fn(&T) -> gpui::Hsla>;

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
    cx.theme().yellow,
    cx.theme().red,
  ];
  colors[index % colors.len()]
}

#[derive(IntoElement)]
pub struct PieChart<T: 'static> {
  data: Vec<T>,
  inner_radius: f32,
  inner_radius_fn: Option<ArcRadiusFn<T>>,
  outer_radius: f32,
  outer_radius_fn: Option<ArcRadiusFn<T>>,
  pad_angle: f32,
  value: Option<ValueFn<T>>,
  color: Option<ColorFn<T>>,
}

impl<T> PieChart<T> {
  pub fn new<I>(data: I) -> Self
  where
    I: IntoIterator<Item = T>, {
    Self {
      data: data.into_iter().collect(),
      inner_radius: 0.,
      inner_radius_fn: None,
      outer_radius: 0.,
      outer_radius_fn: None,
      pad_angle: 0.,
      value: None,
      color: None,
    }
  }

  pub fn inner_radius(mut self, inner_radius: f32) -> Self {
    self.inner_radius = inner_radius;
    self
  }

  pub fn inner_radius_fn(mut self, inner_radius_fn: impl Fn(&ArcData<T>) -> f32 + 'static) -> Self {
    self.inner_radius_fn = Some(Rc::new(inner_radius_fn));
    self
  }

  fn get_inner_radius(&self, arc: &ArcData<T>) -> f32 {
    self
      .inner_radius_fn
      .as_ref()
      .map_or(self.inner_radius, |inner_radius_fn| inner_radius_fn(arc))
  }

  pub fn outer_radius(mut self, outer_radius: f32) -> Self {
    self.outer_radius = outer_radius;
    self
  }

  pub fn outer_radius_fn(mut self, outer_radius_fn: impl Fn(&ArcData<T>) -> f32 + 'static) -> Self {
    self.outer_radius_fn = Some(Rc::new(outer_radius_fn));
    self
  }

  fn get_outer_radius(&self, arc: &ArcData<T>, fallback_outer_radius: f32) -> f32 {
    self.outer_radius_fn.as_ref().map_or_else(
      || {
        if self.outer_radius.abs() <= f32::EPSILON {
          fallback_outer_radius
        } else {
          self.outer_radius
        }
      },
      |outer_radius_fn| outer_radius_fn(arc),
    )
  }

  pub fn pad_angle(mut self, pad_angle: f32) -> Self {
    self.pad_angle = pad_angle;
    self
  }

  pub fn value(mut self, value: impl Fn(&T) -> f32 + 'static) -> Self {
    self.value = Some(Rc::new(value));
    self
  }

  pub fn color<H>(mut self, color: impl Fn(&T) -> H + 'static) -> Self
  where
    H: Into<gpui::Hsla> + 'static, {
    self.color = Some(Rc::new(move |t| color(t).into()));
    self
  }
}

impl<T> Plot for PieChart<T> {
  fn paint(&self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut App) {
    let Some(value_fn) = self.value.as_ref() else {
      return;
    };

    let outer_radius = if self.outer_radius.abs() <= f32::EPSILON {
      bounds.size.height.as_f32() * 0.4
    } else {
      self.outer_radius
    };

    let arc = Arc::new()
      .inner_radius(self.inner_radius)
      .outer_radius(outer_radius);
    let value_fn = value_fn.clone();
    let pie = Pie::<T>::new()
      .value(move |d| Some(value_fn(d)))
      .pad_angle(self.pad_angle);
    let arcs = pie.arcs(&self.data);

    for (i, arc_data) in arcs.iter().enumerate() {
      let inner_radius = self.get_inner_radius(arc_data);
      let outer_radius = self.get_outer_radius(arc_data, outer_radius);
      let color = if let Some(color_fn) = self.color.as_ref() {
        color_fn(arc_data.data)
      } else {
        palette(i, cx)
      };

      arc.paint(
        arc_data,
        color,
        Some(inner_radius),
        Some(outer_radius),
        &bounds,
        window,
      );
    }
  }
}

impl<T: 'static> RenderOnce for PieChart<T> {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    canvas(
      move |_, _, _| {},
      move |bounds, _, window, cx| self.paint(bounds, window, cx),
    )
    .size_full()
  }
}
