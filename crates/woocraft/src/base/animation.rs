//! Element-level animation helpers that stay on GPUI's `with_animation`.
//!
//! Continuous loops — spinner rotation, caret blink — are the one case GPUI's
//! repeating animation models better than the value-level motion system, so
//! they live here. Durations, easing curves, transitions, springs and
//! keyframes all belong to [`crate::base::motion`].

use gpui::{Animation, linear};

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
