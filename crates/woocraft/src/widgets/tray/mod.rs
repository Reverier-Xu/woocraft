//! System tray integration for GPUI.
//!
//! Ported and adapted from [gpui-tray](https://github.com/Yamrc/gpui-tray)
//! (MPL-2.0). Notable changes from upstream:
//! - menu items use string ids instead of GPUI `Action`s, so the consuming
//!   app maps `TrayEvent::MenuClicked { id }` to its own behaviors;
//! - the icon is supplied as raw bytes instead of a `gpui::Image`;
//! - logging uses `tracing` instead of `log`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use woocraft::{Tray, TrayAppContext, TrayMenuItem};
//!
//! cx.set_tray(
//!     Tray::new()
//!         .tooltip("WebSocket Reflector X")
//!         .icon_bytes(logo_png_bytes)
//!         .menu(vec![
//!             TrayMenuItem::action("show", "Show"),
//!             TrayMenuItem::separator(),
//!             TrayMenuItem::action("quit", "Quit"),
//!         ]),
//! )?;
//! let events = woocraft::tray_events(cx);
//! ```

#[cfg(target_os = "linux")]
mod linux;
#[cfg(not(any(target_os = "linux", target_os = "windows")))]
mod macos;
mod platform;
mod runtime;
#[cfg(target_os = "windows")]
mod windows;

pub use platform::{BackendError, Error, Result};
pub use runtime::TrayAppContext;

use std::fmt;

/// Mouse button used in a tray click event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayMouseButton {
    Left,
    Middle,
    Right,
}

/// A click on the tray icon.
#[derive(Debug, Clone, PartialEq)]
pub struct TrayClickEvent {
    pub button: TrayMouseButton,
    pub position: (f32, f32),
}

/// A context-menu entry.
#[derive(Debug, Clone)]
pub enum TrayMenuItem {
    /// A clickable menu entry; `MenuClicked { id }` is emitted on activation.
    Action { id: String, label: String, enabled: bool },
    /// A separator line.
    Separator,
    /// A submenu with the given label and children.
    Submenu { label: String, items: Vec<TrayMenuItem> },
}

impl TrayMenuItem {
    pub fn action(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self::Action {
            id: id.into(),
            label: label.into(),
            enabled: true,
        }
    }

    pub fn separator() -> Self {
        Self::Separator
    }

    pub fn submenu(label: impl Into<String>, items: Vec<TrayMenuItem>) -> Self {
        Self::Submenu {
            label: label.into(),
            items,
        }
    }
}

/// Runtime events emitted by the tray backend.
#[derive(Debug, Clone)]
pub enum TrayEvent {
    /// The tray icon was clicked (left click activates, right click opens the
    /// context menu on most platforms).
    Click { button: TrayMouseButton, position: (f32, f32) },
    /// The tray icon was double-clicked (Windows).
    DoubleClick,
    /// A context menu entry was activated.
    MenuClicked { id: String },
}

/// Configuration for a system tray icon.
#[derive(Clone, Default)]
pub struct Tray {
    pub tooltip: Option<gpui::SharedString>,
    pub title: Option<gpui::SharedString>,
    pub icon_bytes: Option<Vec<u8>>,
    pub visible: bool,
    pub menu: Vec<TrayMenuItem>,
}

impl Tray {
    pub fn new() -> Self {
        Self {
            tooltip: None,
            title: None,
            icon_bytes: None,
            visible: true,
            menu: Vec::new(),
        }
    }

    pub fn tooltip(mut self, tooltip: impl Into<gpui::SharedString>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn title(mut self, title: impl Into<gpui::SharedString>) -> Self {
        self.title = Some(title.into());
        self
    }

    pub fn icon_bytes(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.icon_bytes = Some(bytes.into());
        self
    }

    pub fn visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    pub fn menu(mut self, menu: Vec<TrayMenuItem>) -> Self {
        self.menu = menu;
        self
    }
}

impl fmt::Debug for Tray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Tray")
            .field("tooltip", &self.tooltip)
            .field("title", &self.title)
            .field("visible", &self.visible)
            .field("icon_bytes", &self.icon_bytes.as_ref().map(|b| b.len()))
            .field("menu", &self.menu.len())
            .finish()
    }
}

/// Returns the tray event receiver, when a tray was set up on this app.
pub fn tray_events(cx: &gpui::App) -> Option<crossbeam_channel::Receiver<TrayEvent>> {
    runtime::TrayRuntime::events(cx)
}

pub(crate) fn create_platform_tray() -> Result<(Box<dyn platform::PlatformTray>, crossbeam_channel::Receiver<TrayEvent>)> {
    #[cfg(target_os = "linux")]
    {
        linux::create()
    }
    #[cfg(target_os = "windows")]
    {
        windows::create()
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        macos::create()
    }
}
