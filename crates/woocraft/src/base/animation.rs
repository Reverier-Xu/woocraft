//! Pre-configured animation helpers for common UI animations.
//!
//! Provides ready-to-use animations (e.g., spinner rotation) utilizing theme
//! duration settings.

use gpuim::{Animation, linear};

use crate::base::theme::duration;

/// Creates a continuous spinning animation using the theme spinner duration.
///
/// Produces a repeating 360-degree rotation animation suitable for loading
/// indicators.
pub fn spinner_animation() -> Animation {
  Animation::new(duration::SPINNER)
    .repeat()
    .with_easing(linear)
}
