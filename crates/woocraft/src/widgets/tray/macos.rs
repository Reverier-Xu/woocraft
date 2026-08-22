//! macOS tray backend.
//!
//! Upstream gpui-tray does not implement macOS yet; this mirrors that state
//! with an explicit `UnsupportedPlatform` error.

use super::platform::PlatformTray;
use super::{Error, Result, Tray, TrayEvent};
use crossbeam_channel::Receiver;

pub(crate) fn create() -> Result<(Box<dyn PlatformTray>, Receiver<TrayEvent>)> {
    Err(Error::UnsupportedPlatform)
}
