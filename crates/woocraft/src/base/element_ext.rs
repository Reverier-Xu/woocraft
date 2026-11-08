//! Extension trait for GPUI elements providing prepaint callback support.

use gpui::{App, Bounds, ParentElement, Pixels, Styled as _, Window, canvas};

/// Extends elements with a prepaint callback hook.
///
/// Allows registering a callback to run before element rendering, useful for
/// custom drawing, layout measurements, or animations that depend on bounds.
pub trait ElementExt: ParentElement + Sized {
  /// Registers a prepaint callback that runs before the element is rendered.
  ///
  /// The callback receives the element bounds, window, and app context.
  /// Useful for custom canvas drawing or layout-dependent operations.
  /// Add a prepaint callback to the element.
  fn on_prepaint<F>(self, f: F) -> Self
  where
    F: FnOnce(Bounds<Pixels>, &mut Window, &mut App) + 'static, {
    self.child(
      canvas(
        move |bounds, window, cx| f(bounds, window, cx),
        |_, _, _, _| {},
      )
      .absolute()
      .size_full(),
    )
  }
}

impl<T: ParentElement> ElementExt for T {}
