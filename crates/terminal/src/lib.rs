//! # woocraft-terminal
//!
//! Cross-platform PTY terminal session core for the Woocraft design system.
//!
//! This crate is intentionally free of any GUI or async-runtime dependency:
//! sessions are driven by the PTY I/O thread owned by the bundled terminal
//! emulator backend, and events are delivered through a runtime-agnostic
//! channel ([`async_channel`]) that can be awaited from any executor (e.g. the
//! one built into GPUI) or consumed synchronously via
//! [`async_channel::Receiver::recv_blocking`].
//!
//! Typical external-control usage (no GUI required):
//!
//! ```no_run
//! use std::time::Duration;
//!
//! use woocraft_terminal::control::{spawn_with_events, wait_for_text_blocking};
//! use woocraft_terminal::{SpawnOptions, TerminalBounds};
//!
//! # fn main() -> anyhow::Result<()> {
//! let (session, events) = spawn_with_events(
//!   SpawnOptions::with_shell(("sh".into(), vec![])),
//!   TerminalBounds::new(20.0, 8.0, 80, 24),
//! )?;
//! session.input_str("echo hello-woocraft\r");
//! let text = wait_for_text_blocking(&events, &session, "hello-woocraft", Duration::from_secs(5))?;
//! assert!(text.contains("hello-woocraft"));
//! session.kill();
//! # Ok(())
//! # }
//! ```

mod backend;
pub mod control;
mod event;
mod options;
mod session;
mod types;

/// Re-exported so downstream crates resolve the exact same git revision of
/// `alacritty_terminal` as this crate (git deps are only unified when they
/// point to the identical source). Prefer
/// `woocraft_terminal::alacritty_terminal` over adding the git dependency
/// directly.
pub use alacritty_terminal;
pub use control::*;
pub use event::*;
pub use options::*;
pub use session::*;
pub use types::*;
