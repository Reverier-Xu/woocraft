//! tracing subscriber bootstrap for hosts without their own telemetry setup.
//!
//! woocraft only emits events through `tracing`; installing a subscriber is
//! the application's job. this module offers a batteries-included default for
//! prototypes and examples — applications that own their telemetry should
//! skip [`init`] and install their own subscriber instead.

use tracing_subscriber::{EnvFilter, fmt};

/// filter applied when `RUST_LOG` is unset or malformed.
pub const DEFAULT_FILTER: &str = "info";

/// installs the default fmt subscriber.
///
/// the filter comes from `RUST_LOG` when present, otherwise
/// [`DEFAULT_FILTER`] applies. returns `false` when a global subscriber is
/// already installed; the existing subscriber keeps working in that case.
pub fn init() -> bool {
  init_with_filter(DEFAULT_FILTER)
}

/// like [`init`], but with an explicit fallback filter for when `RUST_LOG`
/// is unset or malformed.
pub fn init_with_filter(default_filter: &str) -> bool {
  let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(default_filter));

  fmt().with_env_filter(filter).try_init().is_ok()
}

#[cfg(test)]
mod tests {
  #[test]
  fn repeated_init_keeps_existing_subscriber() {
    assert!(super::init());
    assert!(!super::init());
  }
}
