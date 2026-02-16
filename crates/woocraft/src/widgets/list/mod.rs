pub(crate) mod cache;
mod delegate;
#[allow(clippy::module_inception)]
mod list;
mod list_item;
mod loading;
mod separator_item;

pub use delegate::*;
pub use list::*;
pub use list_item::*;
pub use separator_item::*;
use serde::{Deserialize, Serialize};

/// Settings for List.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListSettings {
  /// Whether to use active highlight style on ListItem.
  pub active_highlight: bool,
}

impl Default for ListSettings {
  fn default() -> Self {
    Self {
      active_highlight: true,
    }
  }
}
