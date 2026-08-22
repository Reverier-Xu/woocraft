//! macOS tray backend.
//!
//! Upstream gpui-tray does not implement macOS yet; this mirrors that state
//! with an explicit `UnsupportedPlatform` error.

use crossbeam_channel::Receiver;

use super::{Error, Result, Tray, TrayEvent, platform::PlatformTray};

pub(crate) fn create() -> Result<(Box<dyn PlatformTray>, Receiver<TrayEvent>)> {
  Err(Error::UnsupportedPlatform)
}
