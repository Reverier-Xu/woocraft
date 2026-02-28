use std::{borrow::Cow, collections::HashSet, marker::PhantomData, sync::Arc};

use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

use crate::IconNamed;

pub const BUILTIN_ASSET_PREFIX: &str = "tech.woooo.woocraft/assets";

/// Embedded application assets for woocraft.
#[derive(RustEmbed)]
#[folder = "src/assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/**/*.ttf"]
pub struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    load_builtin_asset(path)
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    Ok(list_builtin_assets(path))
  }
}

/// Adapter that turns any [`RustEmbed`] type into a [`gpui::AssetSource`].
#[derive(Default, Clone, Copy)]
pub struct EmbeddedSource<T>(PhantomData<T>);

impl<T> EmbeddedSource<T> {
  pub const fn new() -> Self {
    Self(PhantomData)
  }
}

impl<T> AssetSource for EmbeddedSource<T>
where
  T: RustEmbed + Send + Sync + 'static,
{
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    load_embedded_asset::<T>(path)
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    Ok(list_embedded_assets::<T>(path))
  }
}

/// A source that queries multiple asset sources in order.
///
/// The first source returning a value wins when loading assets.
#[derive(Default, Clone)]
pub struct CombinedSource {
  sources: Vec<Arc<dyn AssetSource>>,
}

impl CombinedSource {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn with(mut self, source: impl AssetSource) -> Self {
    self.push(source);
    self
  }

  pub fn with_shared(mut self, source: Arc<dyn AssetSource>) -> Self {
    self.push_shared(source);
    self
  }

  pub fn push(&mut self, source: impl AssetSource) -> &mut Self {
    self.sources.push(Arc::new(source));
    self
  }

  pub fn push_shared(&mut self, source: Arc<dyn AssetSource>) -> &mut Self {
    self.sources.push(source);
    self
  }

  pub fn is_empty(&self) -> bool {
    self.sources.is_empty()
  }
}

impl AssetSource for CombinedSource {
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    for source in &self.sources {
      if let Some(bytes) = source.load(path)? {
        return Ok(Some(bytes));
      }
    }
    Ok(None)
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    let mut seen = HashSet::<String>::new();
    let mut merged = Vec::new();

    for source in &self.sources {
      for asset_path in source.list(path)? {
        if seen.insert(asset_path.to_string()) {
          merged.push(asset_path);
        }
      }
    }

    Ok(merged)
  }
}

fn load_embedded_asset<T: RustEmbed>(path: &str) -> Result<Option<Cow<'static, [u8]>>> {
  if path.is_empty() {
    return Ok(None);
  }

  Ok(T::get(path).map(|file| file.data))
}

fn list_embedded_assets<T: RustEmbed>(path: &str) -> Vec<SharedString> {
  T::iter()
    .filter_map(|asset_path| asset_path.starts_with(path).then(|| asset_path.into()))
    .collect()
}

fn strip_builtin_asset_prefix(path: &str) -> Option<&str> {
  if path == BUILTIN_ASSET_PREFIX {
    Some("")
  } else {
    path
      .strip_prefix(BUILTIN_ASSET_PREFIX)
      .and_then(|rest| rest.strip_prefix('/'))
  }
}

fn prefix_builtin_asset_path(path: &str) -> SharedString {
  if path.is_empty() {
    BUILTIN_ASSET_PREFIX.into()
  } else {
    format!("{BUILTIN_ASSET_PREFIX}/{path}").into()
  }
}

fn load_builtin_asset(path: &str) -> Result<Option<Cow<'static, [u8]>>> {
  let Some(internal_path) = strip_builtin_asset_prefix(path) else {
    return Ok(None);
  };

  if internal_path.is_empty() {
    return Ok(None);
  }

  load_embedded_asset::<Assets>(internal_path)
}

fn list_builtin_assets(path: &str) -> Vec<SharedString> {
  let internal_prefix = if path.is_empty() {
    ""
  } else if let Some(internal_prefix) = strip_builtin_asset_prefix(path) {
    internal_prefix
  } else {
    return Vec::new();
  };

  list_embedded_assets::<Assets>(internal_prefix)
    .into_iter()
    .map(|path| prefix_builtin_asset_path(path.as_ref()))
    .collect()
}

pub fn has_asset(path: &str) -> bool {
  strip_builtin_asset_prefix(path)
    .filter(|internal_path| !internal_path.is_empty())
    .and_then(Assets::get)
    .is_some()
}

pub fn list_assets(path_prefix: &str) -> Vec<SharedString> {
  list_builtin_assets(path_prefix)
}

pub fn has_icon(icon: crate::IconName) -> bool {
  has_asset(icon.path().as_ref())
}

pub fn list_icons() -> Vec<crate::IconName> {
  crate::IconName::all()
    .iter()
    .copied()
    .filter(|icon| has_icon(*icon))
    .collect()
}

pub fn register_fonts(text_system: &gpui::TextSystem) -> Result<()> {
  let fonts = Assets::iter()
    .filter(|path| path.starts_with("fonts/"))
    .filter_map(|path| Assets::get(path.as_ref()).map(|file| file.data))
    .collect::<Vec<Cow<'static, [u8]>>>();

  text_system.add_fonts(fonts)?;

  Ok(())
}

#[cfg(test)]
mod tests {
  use std::{borrow::Cow, collections::HashMap};

  use gpui::{AssetSource, Result, SharedString};

  use crate::IconNamed;

  #[cfg(feature = "resources")]
  #[test]
  fn all_registered_icons_exist_in_assets() {
    for icon in crate::IconName::all() {
      assert!(
        super::has_icon(*icon),
        "missing icon asset: {}",
        (*icon).path()
      );
    }
  }

  #[derive(Default)]
  struct StaticSource {
    files: HashMap<&'static str, &'static [u8]>,
  }

  impl StaticSource {
    fn with_file(mut self, path: &'static str, data: &'static [u8]) -> Self {
      self.files.insert(path, data);
      self
    }
  }

  impl AssetSource for StaticSource {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
      Ok(self.files.get(path).map(|bytes| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
      Ok(
        self
          .files
          .keys()
          .filter_map(|asset_path| asset_path.starts_with(path).then(|| (*asset_path).into()))
          .collect(),
      )
    }
  }

  #[test]
  fn combined_source_loads_from_fallback_sources() {
    let source = super::CombinedSource::new()
      .with(StaticSource::default().with_file("icons/a.svg", b"a"))
      .with(StaticSource::default().with_file("icons/b.svg", b"b"));

    let bytes = source
      .load("icons/b.svg")
      .expect("combined source should not fail")
      .expect("asset should be loaded from fallback source");
    assert_eq!(bytes.as_ref(), b"b");
  }

  #[test]
  fn combined_source_lists_without_duplicates() {
    let source = super::CombinedSource::new()
      .with(
        StaticSource::default()
          .with_file("icons/a.svg", b"a")
          .with_file("icons/b.svg", b"b"),
      )
      .with(
        StaticSource::default()
          .with_file("icons/b.svg", b"b")
          .with_file("icons/c.svg", b"c"),
      );

    let listed = source.list("icons/").expect("list should succeed");
    let listed = listed.iter().map(|path| path.as_ref()).collect::<Vec<_>>();

    assert_eq!(listed, vec!["icons/a.svg", "icons/b.svg", "icons/c.svg"]);
  }

  #[cfg(feature = "resources")]
  #[test]
  fn builtin_assets_are_namespaced() {
    let icon = crate::IconName::all()[0];
    let prefixed_path = icon.path();

    assert!(prefixed_path.starts_with(super::BUILTIN_ASSET_PREFIX));
    assert!(super::has_asset(prefixed_path.as_ref()));

    let unprefixed_path = prefixed_path
      .as_ref()
      .strip_prefix(&(super::BUILTIN_ASSET_PREFIX.to_owned() + "/"))
      .expect("icon path should include built-in prefix");
    assert!(!super::has_asset(unprefixed_path));
  }
}
