//! embedded built-in resources: icons and fonts shipped with woocraft.
//!
//! assets are embedded at compile time via [`Assets`] and exposed to gpui
//! through a reserved reverse-dns namespace ([`BUILTIN_ASSET_PREFIX`]).
//! fonts are stored zstd-compressed and decompressed transparently on load;
//! [`register_fonts`] hands every embedded face to the gpui text system.
//!
//! [`EmbeddedSource`] adapts any other [`rust_embed::RustEmbed`] type into a
//! gpui [`AssetSource`], and [`CombinedSource`] layers several sources with
//! first-match-wins semantics for applications that overlay their own assets
//! on top of the built-ins.

use std::{borrow::Cow, collections::HashSet, marker::PhantomData, sync::Arc};

use gpui::{AssetSource, SharedString};
use rust_embed::RustEmbed;

use crate::{
  error::{Error, Result},
  icon::{IconName, IconNamed},
};

pub const BUILTIN_ASSET_PREFIX: &str = "tech.woooo.craft/assets";

const FONT_ASSET_PREFIX: &str = "fonts/";
const ZSTD_EXTENSION: &str = ".zst";

/// Embedded application assets for woocraft.
#[derive(RustEmbed)]
#[folder = "src/assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/**/*.ttf.zst"]
#[include = "fonts/**/*.otf.zst"]
pub struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
    load_builtin_asset(path)
  }

  fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
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
  fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
    Ok(load_embedded_asset::<T>(path)?)
  }

  fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
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

  pub fn with(mut self, source: impl AssetSource + 'static) -> Self {
    self.push(source);
    self
  }

  pub fn with_shared(mut self, source: Arc<dyn AssetSource>) -> Self {
    self.push_shared(source);
    self
  }

  pub fn push(&mut self, source: impl AssetSource + 'static) -> &mut Self {
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
  fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
    for source in &self.sources {
      if let Some(bytes) = source.load(path)? {
        return Ok(Some(bytes));
      }
    }
    Ok(None)
  }

  fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
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

fn is_font_asset(path: &str) -> bool {
  path.starts_with(FONT_ASSET_PREFIX)
}

fn is_compressed_font_asset(path: &str) -> bool {
  is_font_asset(path) && path.ends_with(ZSTD_EXTENSION)
}

fn compressed_font_asset_path(path: &str) -> Option<String> {
  if is_font_asset(path) && !path.ends_with('/') && !path.ends_with(ZSTD_EXTENSION) {
    Some(format!("{path}{ZSTD_EXTENSION}"))
  } else {
    None
  }
}

fn visible_asset_path(path: &str) -> &str {
  if is_compressed_font_asset(path) {
    path.strip_suffix(ZSTD_EXTENSION).unwrap_or(path)
  } else {
    path
  }
}

fn decode_embedded_asset(path: &str, data: Cow<'static, [u8]>) -> Result<Cow<'static, [u8]>> {
  if !is_compressed_font_asset(path) {
    return Ok(data);
  }

  let decompressed =
    zstd::stream::decode_all(data.as_ref()).map_err(|source| Error::AssetDecompress {
      asset: path.to_owned(),
      source,
    })?;

  Ok(Cow::Owned(decompressed))
}

fn embedded_asset_exists<T: RustEmbed>(path: &str) -> bool {
  T::get(path).is_some()
    || compressed_font_asset_path(path)
      .is_some_and(|compressed_path| T::get(compressed_path.as_str()).is_some())
}

fn load_embedded_asset<T: RustEmbed>(path: &str) -> Result<Option<Cow<'static, [u8]>>> {
  if path.is_empty() {
    return Ok(None);
  }

  if let Some(file) = T::get(path) {
    return decode_embedded_asset(path, file.data).map(Some);
  }

  let Some(compressed_path) = compressed_font_asset_path(path) else {
    return Ok(None);
  };

  T::get(compressed_path.as_str())
    .map(|file| decode_embedded_asset(compressed_path.as_str(), file.data))
    .transpose()
}

fn list_embedded_assets<T: RustEmbed>(path: &str) -> Vec<SharedString> {
  let mut seen = HashSet::<String>::new();

  T::iter()
    .filter_map(|asset_path| {
      let asset_path = asset_path.as_ref();
      let visible_path = visible_asset_path(asset_path);

      (asset_path.starts_with(path) || visible_path.starts_with(path))
        .then(|| visible_path.to_owned())
    })
    .filter_map(|asset_path| seen.insert(asset_path.clone()).then(|| asset_path.into()))
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

fn load_builtin_asset(path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
  let Some(internal_path) = strip_builtin_asset_prefix(path) else {
    return Ok(None);
  };

  if internal_path.is_empty() {
    return Ok(None);
  }

  Ok(load_embedded_asset::<Assets>(internal_path)?)
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

/// Returns whether a built-in asset exists under the namespaced `path`.
pub fn has_asset(path: &str) -> bool {
  strip_builtin_asset_prefix(path)
    .filter(|internal_path| !internal_path.is_empty())
    .is_some_and(embedded_asset_exists::<Assets>)
}

/// Lists built-in assets under the namespaced `path_prefix`.
pub fn list_assets(path_prefix: &str) -> Vec<SharedString> {
  list_builtin_assets(path_prefix)
}

/// Returns whether the built-in icon asset for `icon` exists.
pub fn has_icon(icon: IconName) -> bool {
  has_asset(icon.path().as_ref())
}

/// Lists built-in icons that resolve to an embedded asset.
pub fn list_icons() -> Vec<IconName> {
  IconName::all()
    .iter()
    .copied()
    .filter(|icon| has_icon(*icon))
    .collect()
}

/// Registers every embedded font face with the gpui text system.
///
/// Stored zstd-compressed faces are decompressed on the way in.
pub fn register_fonts(text_system: &gpui::TextSystem) -> Result<()> {
  let mut fonts = Vec::new();

  for path in Assets::iter().filter(|path| is_font_asset(path.as_ref())) {
    if let Some(font) = load_embedded_asset::<Assets>(path.as_ref())? {
      fonts.push(font);
    }
  }

  let count = fonts.len();
  text_system
    .add_fonts(fonts)
    .map_err(|err| Error::FontRegistration(err.to_string()))?;

  tracing::debug!(count, "registered embedded fonts");
  Ok(())
}

#[cfg(test)]
mod tests {
  use std::{borrow::Cow, collections::HashMap};

  use gpui::{AssetSource, SharedString};

  use super::BUILTIN_ASSET_PREFIX;

  #[test]
  fn every_embedded_icon_is_a_valid_svg_path() {
    let icons = super::list_assets(BUILTIN_ASSET_PREFIX)
      .into_iter()
      .filter(|path| path.as_ref().contains("/icons/"))
      .collect::<Vec<_>>();

    assert!(!icons.is_empty(), "embedded icons should be present");
    assert!(
      icons
        .iter()
        .all(|path| path.as_ref().starts_with(BUILTIN_ASSET_PREFIX)),
      "listed assets should carry the built-in namespace"
    );
  }

  #[test]
  fn every_registered_icon_exists_in_assets() {
    use crate::IconNamed;

    for icon in crate::IconName::all() {
      assert!(
        super::has_icon(*icon),
        "missing icon asset: {}",
        icon.path()
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
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
      Ok(self.files.get(path).map(|bytes| Cow::Borrowed(*bytes)))
    }

    fn list(&self, path: &str) -> gpui::Result<Vec<SharedString>> {
      Ok(
        self
          .files
          .keys()
          .filter(|&asset_path| asset_path.starts_with(path))
          .map(|asset_path| (*asset_path).into())
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
    let mut listed = listed.iter().map(|path| path.as_ref()).collect::<Vec<_>>();
    listed.sort();

    assert_eq!(listed, vec!["icons/a.svg", "icons/b.svg", "icons/c.svg"]);
  }

  #[test]
  fn builtin_assets_are_namespaced() {
    let internal = super::Assets::iter()
      .find(|path| path.starts_with("icons/"))
      .expect("embedded icons should exist");
    let internal = internal.as_ref();

    assert!(
      super::has_asset(&format!("{BUILTIN_ASSET_PREFIX}/{internal}")),
      "namespaced path should resolve"
    );
    assert!(
      !super::has_asset(internal),
      "un-namespaced paths must not reach built-in assets"
    );
    assert!(
      !super::has_asset(BUILTIN_ASSET_PREFIX),
      "bare prefix should not resolve to any asset"
    );
  }

  #[test]
  fn builtin_fonts_are_listed_without_compression_suffix() {
    let font_assets = super::list_assets(BUILTIN_ASSET_PREFIX)
      .into_iter()
      .filter(|path| path.as_ref().contains("/fonts/"))
      .collect::<Vec<_>>();

    assert!(
      font_assets
        .iter()
        .any(|path| path.as_ref().ends_with("fonts/maple-mono-regular.ttf")),
      "compressed font should be exposed via its original extension"
    );
    assert!(
      font_assets
        .iter()
        .all(|path| !path.as_ref().ends_with(".zst")),
      "compression suffix should stay internal to asset loading"
    );

    // The embedded family ships the core static faces plus the Nerd Font
    // build so bold/italic runs resolve within the same family.
    for face in [
      "maple-mono-regular",
      "maple-mono-bold",
      "maple-mono-italic",
      "maple-mono-bolditalic",
      "maple-mono-nf-regular",
    ] {
      assert!(
        super::has_asset(&format!("{BUILTIN_ASSET_PREFIX}/fonts/{face}.ttf")),
        "missing embedded face: {face}"
      );
    }
  }

  #[test]
  fn builtin_fonts_are_decompressed_on_load() {
    let raw_font = super::Assets::get("fonts/maple-mono-regular.ttf.zst")
      .expect("compressed font should be embedded in test builds");

    assert!(
      raw_font.data.as_ref().starts_with(b"\x28\xb5\x2f\xfd"),
      "embedded font should be stored in zstd format"
    );

    let font_path = format!("{BUILTIN_ASSET_PREFIX}/fonts/maple-mono-regular.ttf");
    let loaded_font = super::load_builtin_asset(&font_path)
      .expect("font load should succeed")
      .expect("font asset should exist");

    assert_ne!(loaded_font.as_ref(), raw_font.data.as_ref());
    assert!(
      !loaded_font.as_ref().starts_with(b"\x28\xb5\x2f\xfd"),
      "font bytes should be decompressed before registration"
    );
  }
}
