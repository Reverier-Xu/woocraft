use std::{collections::HashMap, sync::{OnceLock, RwLock}};

use gpui::{
  AnyElement, App, AppContext, Context, Entity, Hsla, IntoElement, Radians, Render, RenderOnce,
  SharedString, StyleRefinement, Styled, Svg, Transformation, Window, prelude::FluentBuilder as _,
  svg,
};

use crate::base::{Sizable, Size};

/// Types implementing this trait can automatically be converted to [`Icon`].
pub trait IconNamed {
  fn path(self) -> SharedString;
}

static CUSTOM_ICON_REGISTRY: OnceLock<RwLock<HashMap<String, SharedString>>> = OnceLock::new();

fn custom_icon_registry() -> &'static RwLock<HashMap<String, SharedString>> {
  CUSTOM_ICON_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

fn resolve_icon_path(name_or_path: &str) -> SharedString {
  custom_icon_path(name_or_path)
    .unwrap_or_else(|| SharedString::from(name_or_path.to_owned()))
}

pub fn register_icon(name: impl Into<SharedString>, path: impl Into<SharedString>) {
  let name = name.into().to_string();
  let path = path.into();
  let mut registry = custom_icon_registry()
    .write()
    .expect("custom icon registry poisoned");
  registry.insert(name, path);
}

pub fn unregister_icon(name: &str) -> Option<SharedString> {
  let mut registry = custom_icon_registry()
    .write()
    .expect("custom icon registry poisoned");
  registry.remove(name)
}

pub fn clear_custom_icons() {
  let mut registry = custom_icon_registry()
    .write()
    .expect("custom icon registry poisoned");
  registry.clear();
}

pub fn custom_icon_path(name: &str) -> Option<SharedString> {
  let registry = custom_icon_registry()
    .read()
    .expect("custom icon registry poisoned");
  registry.get(name).cloned()
}

impl IconNamed for &str {
  fn path(self) -> SharedString {
    resolve_icon_path(self)
  }
}

impl IconNamed for String {
  fn path(self) -> SharedString {
    resolve_icon_path(&self)
  }
}

impl IconNamed for SharedString {
  fn path(self) -> SharedString {
    resolve_icon_path(self.as_ref())
  }
}

impl<T: IconNamed> From<T> for Icon {
  fn from(value: T) -> Self {
    Icon::build(value)
  }
}

include!(concat!(env!("OUT_DIR"), "/icon_names.rs"));

impl IconName {
  pub fn view(self, cx: &mut App) -> Entity<Icon> {
    Icon::build(self).view(cx)
  }
}

impl From<IconName> for AnyElement {
  fn from(val: IconName) -> Self {
    Icon::build(val).into_any_element()
  }
}

impl RenderOnce for IconName {
  fn render(self, _: &mut Window, _cx: &mut App) -> impl IntoElement {
    Icon::build(self)
  }
}

#[derive(IntoElement)]
pub struct Icon {
  base: Svg,
  style: StyleRefinement,
  path: SharedString,
  text_color: Option<Hsla>,
  size: Option<Size>,
  rotation: Option<Radians>,
}

impl Default for Icon {
  fn default() -> Self {
    Self {
      base: svg().flex_none().size_4(),
      style: StyleRefinement::default(),
      path: "".into(),
      text_color: None,
      size: None,
      rotation: None,
    }
  }
}

impl Clone for Icon {
  fn clone(&self) -> Self {
    let mut this = Self::default().path(self.path.clone());
    this.style = self.style.clone();
    this.rotation = self.rotation;
    this.size = self.size;
    this.text_color = self.text_color;
    this
  }
}

impl Icon {
  pub fn new(icon: impl Into<Icon>) -> Self {
    icon.into()
  }

  fn build(name: impl IconNamed) -> Self {
    Self::default().path(name.path())
  }

  pub fn path(mut self, path: impl Into<SharedString>) -> Self {
    let path = path.into();
    #[cfg(all(feature = "resources", debug_assertions))]
    {
      debug_assert!(
        crate::has_asset(path.as_ref()),
        "icon asset not found: {}",
        path
      );
    }
    self.path = path;
    self
  }

  pub fn view(self, cx: &mut App) -> Entity<Icon> {
    cx.new(|_| self)
  }

  pub fn transform(mut self, transformation: gpui::Transformation) -> Self {
    self.base = self.base.with_transformation(transformation);
    self
  }

  pub fn empty() -> Self {
    Self::default()
  }

  pub fn rotate(mut self, radians: impl Into<Radians>) -> Self {
    self.base = self
      .base
      .with_transformation(Transformation::rotate(radians));
    self
  }
}

impl Styled for Icon {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }

  fn text_color(mut self, color: impl Into<Hsla>) -> Self {
    self.text_color = Some(color.into());
    self
  }
}

impl Sizable for Icon {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = Some(size.into());
    self
  }
}

impl RenderOnce for Icon {
  fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
    let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
    let text_size = window.text_style().font_size.to_pixels(window.rem_size());
    let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

    let mut base = self.base;
    *base.style() = self.style;

    base
      .flex_shrink_0()
      .text_color(text_color)
      .when(!has_base_size, |this| this.size(text_size))
      .when_some(self.size, |this, size| this.size(size.icon_size()))
      .path(self.path)
  }
}

impl From<Icon> for AnyElement {
  fn from(val: Icon) -> Self {
    val.into_any_element()
  }
}

impl Render for Icon {
  fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
    let text_size = window.text_style().font_size.to_pixels(window.rem_size());
    let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

    let mut base = svg().flex_none();
    *base.style() = self.style.clone();

    base
      .flex_shrink_0()
      .text_color(text_color)
      .when(!has_base_size, |this| this.size(text_size))
      .when_some(self.size, |this, size| this.size(size.icon_size()))
      .path(self.path.clone())
      .when_some(self.rotation, |this, rotation| {
        this.with_transformation(Transformation::rotate(rotation))
      })
  }
}
