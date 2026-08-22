//! Platform backend trait and error types for the tray component.
//!
//! Ported from gpui-tray (MPL-2.0).

use thiserror::Error;

use super::Tray;

/// Errors that can occur when working with the system tray.
#[derive(Error, Debug)]
pub enum Error {
    /// The tray was not found (not set up yet).
    #[error("Tray not found")]
    NotFound,

    /// The tray runtime has already been initialized.
    #[error("Tray runtime already initialized")]
    AlreadyInitialized,

    /// The current platform does not support tray integration.
    #[error("Current platform is not supported yet")]
    UnsupportedPlatform,

    /// The backend runtime is closed.
    #[error("Tray runtime is closed")]
    RuntimeClosed,

    /// Backend-specific error.
    #[error(transparent)]
    Backend(#[from] BackendError),

    /// The provided icon data is invalid or unsupported.
    #[error("Invalid icon data")]
    InvalidIcon,
}

/// Errors raised from platform backend implementations.
#[derive(Error, Debug)]
pub enum BackendError {
    /// Failed to send a command to the backend worker.
    #[error("Failed to send command to backend worker")]
    ChannelSend,

    /// Failed to receive a response from the backend worker.
    #[error("Failed to receive response from backend worker")]
    ChannelReceive,

    /// A native platform API call failed.
    #[error("Platform call `{operation}` failed: {message}")]
    Platform {
        operation: &'static str,
        message: String,
    },
}

impl BackendError {
    pub fn platform(operation: &'static str, message: impl Into<String>) -> Self {
        Self::Platform {
            operation,
            message: message.into(),
        }
    }
}

/// A specialized Result type for tray operations.
pub type Result<T> = std::result::Result<T, Error>;

/// Platform-specific tray backend.
///
/// Backends are isolated from GPUI's `App`; they communicate through immutable
/// tray snapshots and an event channel.
pub trait PlatformTray: Send + Sync {
    /// Applies the latest tray snapshot.
    fn set_tray(&self, tray: &Tray) -> Result<()>;

    /// Removes the tray icon.
    fn remove_tray(&self) -> Result<()>;

    /// Requests graceful shutdown of the backend runtime.
    fn shutdown(&self) -> Result<()>;
}
