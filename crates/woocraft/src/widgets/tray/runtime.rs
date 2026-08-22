//! Tray runtime stored as a GPUI global, plus the `TrayAppContext` extension.
//!
//! Ported from gpui-tray (MPL-2.0), adapted so menu clicks carry string ids
//! and the event receiver is exposed to the application.

use std::sync::Arc;

use gpui::{App, Global};

use super::{Error, Result, Tray, TrayEvent, platform::PlatformTray};

pub(crate) struct TrayRuntime {
  backend: Arc<dyn PlatformTray>,
  current_tray: Option<Tray>,
  events: crossbeam_channel::Receiver<TrayEvent>,
}

impl Global for TrayRuntime {}

impl TrayRuntime {
  fn new(_cx: &mut App) -> Result<Self> {
    let (backend, events) = super::create_platform_tray()?;
    Ok(Self {
      backend: Arc::from(backend),
      current_tray: None,
      events,
    })
  }

  pub(crate) fn events(cx: &App) -> Option<crossbeam_channel::Receiver<TrayEvent>> {
    cx.try_global::<TrayRuntime>()
      .map(|runtime| runtime.events.clone())
  }
}

impl Drop for TrayRuntime {
  fn drop(&mut self) {
    let _ = self.backend.shutdown();
  }
}

/// GPUI `App` extension for managing the system tray.
pub trait TrayAppContext {
  /// Creates (or replaces) the tray icon with the given configuration.
  fn set_tray(&mut self, tray: Tray) -> Result<()>;

  /// Returns the currently configured tray, if any.
  fn tray(&self) -> Option<&Tray>;

  /// Mutates the current tray configuration and pushes it to the backend.
  fn update_tray(&mut self, f: impl FnOnce(&mut Tray)) -> Result<Tray>;

  /// Removes the tray icon.
  fn remove_tray(&mut self) -> Result<()>;
}

impl TrayAppContext for App {
  fn set_tray(&mut self, tray: Tray) -> Result<()> {
    let mut runtime = if self.has_global::<TrayRuntime>() {
      self.remove_global::<TrayRuntime>()
    } else {
      TrayRuntime::new(self)?
    };

    runtime.backend.set_tray(&tray)?;
    runtime.current_tray = Some(tray);

    self.set_global(runtime);
    Ok(())
  }

  fn tray(&self) -> Option<&Tray> {
    self
      .try_global::<TrayRuntime>()
      .and_then(|runtime| runtime.current_tray.as_ref())
  }

  fn update_tray(&mut self, f: impl FnOnce(&mut Tray)) -> Result<Tray> {
    if !self.has_global::<TrayRuntime>() {
      return Err(Error::NotFound);
    }

    let mut runtime = self.remove_global::<TrayRuntime>();
    let Some(tray) = runtime.current_tray.as_mut() else {
      self.set_global(runtime);
      return Err(Error::NotFound);
    };

    f(tray);
    let updated = tray.clone();
    runtime.backend.set_tray(&updated)?;

    self.set_global(runtime);
    Ok(updated)
  }

  fn remove_tray(&mut self) -> Result<()> {
    if !self.has_global::<TrayRuntime>() {
      return Err(Error::NotFound);
    }

    let mut runtime = self.remove_global::<TrayRuntime>();
    if runtime.current_tray.is_none() {
      self.set_global(runtime);
      return Err(Error::NotFound);
    }

    runtime.backend.remove_tray()?;
    runtime.current_tray = None;
    self.set_global(runtime);
    Ok(())
  }
}
