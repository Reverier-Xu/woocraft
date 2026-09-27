//! Single-line text input component with validation, formatting, and state
//! management.
//!
//! The input component provides a flexible text input field with built-in
//! support for pattern validation, custom masking (for passwords), undo/redo
//! history, clipboard operations, and full keyboard accessibility. It manages
//! internal caret position, text selection, and horizontal scrolling for long
//! text.
//!
//! # Features
//! - **Validation & Patterns**: Regex pattern matching and custom validation
//!   callbacks
//! - **Masking**: Display masked text (e.g., for password fields) while
//!   preserving actual value
//! - **Text Editing**: Full undo/redo history (up to 256 snapshots),
//!   copy/cut/paste operations
//! - **Keyboard Navigation**: Word-based movement, selection, home/end key
//!   support
//! - **Events**: Change, Focus, Blur, PressEnter (normal and
//!   secondary/Shift+Enter variants)
//! - **Accessibility**: Focus management, keyboard tab stop, IME support
//! - **Customization**: Placeholder text, custom text style, size variations
//!
//! # Example
//! ```rust,ignore
//! use woocraft::InputState;
//!
//! // Basic text input
//! let input = InputState::new(cx)
//!   .placeholder("Enter your name")
//!   .default_value("John");
//!
//! // Email input with validation
//! let email = InputState::new(cx)
//!   .placeholder("Email address")
//!   .validate(|text, _cx| {
//!     text.contains('@')
//!   });
//!
//! // Password input with masking
//! let password = InputState::new(cx)
//!   .placeholder("Password")
//!   .masked(true);
//! ```
//!
//! # Performance Notes
//! Input maintains an undo/redo stack capped at 256 snapshots per instance. For
//! heavy real-time validation, use a debounce approach in the validation
//! callback to avoid excessive cloning. Horizontal scrolling is calculated
//! on-demand during rendering and is efficient even for very long text.

mod number_input;
mod otp;
mod state;

pub use number_input::*;
pub use otp::*;
pub use state::*;
