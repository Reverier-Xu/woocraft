//! icon system with support for built-in icons and custom icon registration.
//!
//! provides a comprehensive icon rendering system with 1600+ pre-defined
//! icons generated at build time from `src/assets/icons/*.svg`. icons render
//! at the surrounding text size (`1rem` by default) and inherit the text
//! color unless colorization is disabled. custom icon names can be
//! registered to extend the built-in set.

#[cfg(debug_assertions)]
use std::collections::HashSet;
use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

use gpui::{
  AnyElement, App, AppContext, Context, Entity, Hsla, IntoElement, Radians, Refineable as _,
  Render, RenderOnce, SharedString, StyleRefinement, Styled, Svg, Transformation, Window, img,
  prelude::FluentBuilder as _, svg,
};
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
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    if validated.contains(path) {
      return;
    }
  }

  match cx.asset_source().load(path.as_ref()) {
    Ok(Some(_)) => {
      let mut validated = validated_icon_paths()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  registry.insert(name, path);
}

/// Unregisters a custom icon from the global registry.
///
/// Returns the previously registered path if found, or None if the icon
/// was not registered.
pub fn unregister_icon(name: &str) -> Option<SharedString> {
  let mut registry = custom_icon_registry()
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  registry.remove(name)
}

/// Clears all custom icons from the registry.
pub fn clear_custom_icons() {
  let mut registry = custom_icon_registry()
    .write()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
  registry.clear();
}

/// Looks up a custom icon path by name.
///
/// Returns Some(path) if the icon is registered, or None otherwise.
pub fn custom_icon_path(name: &str) -> Option<SharedString> {
  let registry = custom_icon_registry()
    .read()
    .unwrap_or_else(|poisoned| poisoned.into_inner());
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
/// SVG-based icon with customizable color and rotation.
///
/// Icon renders as an inline SVG, defaulting to the surrounding text size
/// (`1rem`) and inheriting the text color. Accepts built-in [`IconName`]
/// variants or custom SVG paths.
pub struct Icon {
  base: Svg,
  style: StyleRefinement,
  path: SharedString,
  text_color: Option<Hsla>,
  rotation: Option<Radians>,
  colorized: bool,
}

impl Default for Icon {
  fn default() -> Self {
    Self {
      base: svg().flex_none(),
      style: StyleRefinement::default(),
      path: "".into(),
      text_color: None,
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
    this.text_color = self.text_color;
    this.colorized = self.colorized;
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

impl RenderOnce for Icon {
  fn render(self, window: &mut Window, _cx: &mut App) -> impl IntoElement {
    #[cfg(debug_assertions)]
    debug_validate_icon_path(&self.path, _cx);

    let text_color = self.text_color.unwrap_or_else(|| window.text_style().color);
    let text_size = window.text_style().font_size.to_pixels(window.rem_size());

    if self.colorized {
      let mut base = self.base;
      base = base
        .flex_shrink_0()
        .text_color(text_color)
        .size(text_size)
        .path(self.path);
      // the surrounding text size is the default; an explicit caller size
      // refinement wins over it.
      base.style().refine(&self.style);
      base.into_any_element()
    } else {
      let mut base = img(self.path);
      base = base.flex_shrink_0().size(text_size);
      base.style().refine(&self.style);
      base.into_any_element()
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

    if self.colorized {
      let mut base = svg().flex_none();
      base = base
        .flex_shrink_0()
        .text_color(text_color)
        .size(text_size)
        .path(self.path.clone())
        .when_some(self.rotation, |this, rotation| {
          this.with_transformation(Transformation::rotate(rotation))
        });
      base.style().refine(&self.style);

      base.into_any_element()
    } else {
      let mut base = img(self.path.clone());
      base = base.flex_shrink_0().size(text_size);
      base.style().refine(&self.style);

      base.into_any_element()
    }
  }
}

#[cfg(test)]
mod tests {
  use gpui::SharedString;

  use super::{IconName, IconNamed, custom_icon_path, register_icon, unregister_icon};

  #[test]
  fn generated_icon_names_resolve_to_namespaced_paths() {
    let names = IconName::all();
    assert!(!names.is_empty(), "embedded icons should generate variants");

    let path = names[0].path();
    assert!(
      path.starts_with("tech.woooo.craft/assets/icons/"),
      "icon path should be namespaced: {path}"
    );
  }

  #[test]
  fn custom_icons_override_resolution() {
    register_icon("custom-test", "icons/custom-test.svg");
    assert_eq!(
      custom_icon_path("custom-test").as_ref(),
      Some(&SharedString::from("icons/custom-test.svg"))
    );
    assert_eq!(
      "custom-test".path(),
      SharedString::from("icons/custom-test.svg")
    );

    assert_eq!(
      unregister_icon("custom-test").as_ref(),
      Some(&SharedString::from("icons/custom-test.svg"))
    );
    assert_eq!(custom_icon_path("custom-test"), None);
  }
}
