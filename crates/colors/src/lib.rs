//! # woocraft-colors
//!
//! HCT color space and perceptual color utilities for the Woocraft design
//! system.
//!
//! ## Why HCT
//!
//! Perceptual spaces like OKLCH give orthogonal *hue*, *chroma* and
//! *lightness*, which is what a color picker wants. Their lightness, however,
//! is not a function of relative luminance, so two colors with the same OKLCH
//! lightness can still differ in contrast against any given background — in
//! this design system's palette by more than 2x. Contrast therefore has to be
//! audited per color after the fact, and a theme built on such a space ends up
//! patching its own lightness axis.
//!
//! HCT splits the two concerns:
//!
//! - **Hue and chroma** come from CAM16, a color appearance model, so equal
//!   steps look equal and hue stays put when chroma changes.
//! - **Tone** is CIE L\*, which is a fixed function of relative luminance.
//!
//! Because tone is a pure function of luminance, two colors that share a tone
//! share a contrast ratio against any background, whatever their hue and
//! chroma. A palette can be stated in tone numbers — "primary is tone 40,
//! on-primary is tone 100" — and the accessible pairing holds before any hue
//! is chosen. That is the property this crate exists to provide.
//!
//! ## Design notes
//!
//! - No dependency on a color-appearance library: the CAM16 and HCT solver
//!   algorithms are implemented here from the published equations, and pinned
//!   against reference values from Material Color Utilities in the test suite.
//! - `f64` throughout. The solver bisects in linear RGB, where the difference
//!   between the right and wrong answer is far below `f32` precision.
//! - sRGB only. Gamut mapping keeps hue and tone and reduces chroma until the
//!   color fits, which is the property a design system wants from a mapping.
//!
//! ## Example
//!
//! ```rust
//! use woocraft_colors::{Hct, Srgb, Tone};
//!
//! // State a role by tone, not by hex.
//! let primary = Hct::new(250.0, 48.0, Tone::new(40.0));
//! let on_primary = Hct::new(250.0, 48.0, Tone::new(100.0));
//! assert!(primary.contrast_ratio(on_primary) >= 4.5);
//!
//! // And read a color back into the same coordinates.
//! let hct = Hct::from_srgb(Srgb::from_hex("#6750a4").unwrap());
//! assert!((hct.tone().value() - 40.0).abs() < 2.0);
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod cam16;
mod contrast;
mod hct;
mod solver;
mod srgb;
mod tone;

#[cfg(test)]
mod reference;

pub use cam16::{Cam16, ViewingConditions, in_p3, in_srgb, linear_srgb};
pub use contrast::Contrast;
pub use hct::Hct;
pub use solver::HctSolver;
pub use srgb::{Hsl, Srgb};
pub use tone::Tone;
