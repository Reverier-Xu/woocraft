//! Icon system with support for built-in icons and custom icon registration.
//!
//! Provides a comprehensive icon rendering system with 300+ pre-defined icons
//! from the Feather icon set. Icons can be customized: change size, color (text
//! color), rotation, and enable/disable colorization. Supports custom icon
//! registration for user-defined SVG icons beyond the built-in set.

#[cfg(debug_assertions)]
use std::collections::HashSet;
use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use gpui::{
  AnyElement, App, AppContext, Context, Entity, Hsla, IntoElement, Radians, Render, RenderOnce,
  SharedString, StyleRefinement, Styled, Svg, Transformation, Window, img,
  prelude::FluentBuilder as _, svg,
};

use crate::base::{Sizable, Size};

/// Trait for types that can be converted to an icon path/name.
///
/// Implement this trait for custom icon name types to support conversion to
/// Icon.
pub trait IconNamed {
  fn path(self) -> SharedString;
}

static CUSTOM_ICON_REGISTRY: OnceLock<RwLock<HashMap<String, SharedString>>> = OnceLock::new();
#[cfg(debug_assertions)]
static VALIDATED_ICON_PATHS: OnceLock<RwLock<HashSet<SharedString>>> = OnceLock::new();

fn custom_icon_registry() -> &'static RwLock<HashMap<String, SharedString>> {
  CUSTOM_ICON_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

#[cfg(debug_assertions)]
fn validated_icon_paths() -> &'static RwLock<HashSet<SharedString>> {
  VALIDATED_ICON_PATHS.get_or_init(|| RwLock::new(HashSet::new()))
}

#[cfg(debug_assertions)]
fn debug_validate_icon_path(path: &SharedString, cx: &App) {
  if path.is_empty() {
    return;
  }

  {
    let validated = validated_icon_paths()
      .read()
      .expect("icon validation cache poisoned");
    if validated.contains(path) {
      return;
    }
  }

  match cx.asset_source().load(path.as_ref()) {
    Ok(Some(_)) => {
      let mut validated = validated_icon_paths()
        .write()
        .expect("icon validation cache poisoned");
      validated.insert(path.clone());
    }
    Ok(None) => {
      debug_assert!(false, "icon asset not found in app asset source: {}", path);
    }
    Err(err) => {
      debug_assert!(
        false,
        "failed to load icon asset \"{}\" from app asset source: {err}",
        path
      );
    }
  }
}

fn resolve_icon_path(name_or_path: &str) -> SharedString {
  custom_icon_path(name_or_path).unwrap_or_else(|| SharedString::from(name_or_path.to_owned()))
}

/// Registers a custom icon in the global registry.
///
/// Maps a custom name to an SVG asset path. Once registered, the icon can be
/// used anywhere by its custom name (name takes precedence over path
/// resolution).
pub fn register_icon(name: impl Into<SharedString>, path: impl Into<SharedString>) {
  let name = name.into().to_string();
  let path = path.into();
  let mut registry = custom_icon_registry()
    .write()
    .expect("custom icon registry poisoned");
  registry.insert(name, path);
}

/// Unregisters a custom icon from the global registry.
///
/// Returns the previously registered path if found, or None if the icon
/// was not registered.
pub fn unregister_icon(name: &str) -> Option<SharedString> {
  let mut registry = custom_icon_registry()
    .write()
    .expect("custom icon registry poisoned");
  registry.remove(name)
}

/// Clears all custom icons from the registry.
pub fn clear_custom_icons() {
  let mut registry = custom_icon_registry()
    .write()
    .expect("custom icon registry poisoned");
  registry.clear();
}

/// Looks up a custom icon path by name.
///
/// Returns Some(path) if the icon is registered, or None otherwise.
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
/// SVG-based icon with customizable size, color, and rotation.
///
/// Icon renders as an inline SVG, defaulting to 1em size and inheriting text
/// color. Supports 300+ built-in icons from IconName enum, or use custom SVG
/// paths. Can be rotated, resized, and colorized (or kept as original SVG
/// colors).
pub struct Icon {
  base: Svg,
  style: StyleRefinement,
  path: SharedString,
  text_color: Option<Hsla>,
  size: Option<Size>,
  rotation: Option<Radians>,
  colorized: bool,
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
      colorized: true,
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
  /// Creates a new icon from an IconName enum value or custom path.
  ///
  /// Can accept:
  /// - [`IconName`] enum variants (e.g., `IconName::Check`, `IconName::X`)
  /// - `&str` paths (e.g., `"icons/custom.svg"`)
  /// - [`SharedString`] paths
  pub fn new(icon: impl Into<Icon>) -> Self {
    icon.into()
  }

  fn build(name: impl IconNamed) -> Self {
    Self::default().path(name.path())
  }

  /// Sets the SVG asset path for this icon.
  ///
  /// Can be a built-in icon name (resolved via IconName) or a custom asset
  /// path.
  pub fn path(mut self, path: impl Into<SharedString>) -> Self {
    self.path = path.into();
    self
  }

  /// Creates a new Entity<Icon> for use as a stateful component in views.
  ///
  /// Useful when icon state needs to be managed within the app context.
  pub fn view(self, cx: &mut App) -> Entity<Icon> {
    cx.new(|_| self)
  }

  /// Applies a transformation (scale, rotate, translate) to the icon.
  ///
  /// Use for custom transforms beyond the rotate() method.
  pub fn transform(mut self, transformation: gpui::Transformation) -> Self {
    self.base = self.base.with_transformation(transformation);
    self
  }

  /// Returns an empty icon (no path, invisible).
  pub fn empty() -> Self {
    Self::default()
  }

  /// Rotates the icon by the specified angle in radians.
  ///
  /// Example: `Icon::new(IconName::ChevronRight).rotate(90.0.to_radians())`
  pub fn rotate(mut self, radians: impl Into<Radians>) -> Self {
    self.base = self
      .base
      .with_transformation(Transformation::rotate(radians));
    self
  }

  /// Controls whether the icon is colorized with the text color.
  ///
  /// When `true` (default), the icon inherits the text color from the context.
  /// When `false`, the icon retains its original SVG colors.
  pub fn colorized(mut self, colorized: bool) -> Self {
    self.colorized = colorized;
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
    #[cfg(debug_assertions)]
    debug_validate_icon_path(&self.path, _cx);

    let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
    let text_size = window.text_style().font_size.to_pixels(window.rem_size());
    let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

    if self.colorized {
      let mut base = self.base;
      *base.style() = self.style;

      base
        .flex_shrink_0()
        .text_color(text_color)
        .when(!has_base_size, |this| this.size(text_size))
        .when_some(self.size, |this, size| this.size(size.icon_size()))
        .path(self.path)
        .into_any_element()
    } else {
      let mut base = img(self.path);
      *base.style() = self.style;

      base
        .flex_shrink_0()
        .when(!has_base_size, |this| this.size(text_size))
        .when_some(self.size, |this, size| this.size(size.icon_size()))
        .into_any_element()
    }
  }
}

impl From<Icon> for AnyElement {
  fn from(val: Icon) -> Self {
    val.into_any_element()
  }
}

impl Render for Icon {
  fn render(&mut self, window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    #[cfg(debug_assertions)]
    debug_validate_icon_path(&self.path, _cx);

    let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
    let text_size = window.text_style().font_size.to_pixels(window.rem_size());
    let has_base_size = self.style.size.width.is_some() || self.style.size.height.is_some();

    if self.colorized {
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
        .into_any_element()
    } else {
      let mut base = img(self.path.clone());
      *base.style() = self.style.clone();

      base
        .flex_shrink_0()
        .when(!has_base_size, |this| this.size(text_size))
        .when_some(self.size, |this, size| this.size(size.icon_size()))
        .into_any_element()
    }
  }
}
