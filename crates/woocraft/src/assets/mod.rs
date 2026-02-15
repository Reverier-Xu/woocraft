use std::borrow::Cow;

use anyhow::anyhow;
use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;

use crate::IconNamed;

/// Embedded application assets for woocraft.
#[derive(RustEmbed)]
#[folder = "src/assets"]
#[include = "icons/**/*.svg"]
#[include = "fonts/**/*.ttf"]
pub struct Assets;

impl AssetSource for Assets {
  fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
    if path.is_empty() {
      return Ok(None);
    }

    Self::get(path)
      .map(|file| Some(file.data))
      .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
  }

  fn list(&self, path: &str) -> Result<Vec<SharedString>> {
    Ok(
      Self::iter()
        .filter_map(|asset_path| asset_path.starts_with(path).then(|| asset_path.into()))
        .collect(),
    )
  }
}

pub fn has_asset(path: &str) -> bool {
  !path.is_empty() && Assets::get(path).is_some()
}

pub fn list_assets(path_prefix: &str) -> Vec<SharedString> {
  Assets::iter()
    .filter_map(|asset_path| {
      asset_path
        .starts_with(path_prefix)
        .then(|| asset_path.into())
    })
    .collect()
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

  text_system.add_fonts(fonts)
}

#[cfg(test)]
mod tests {
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
}
