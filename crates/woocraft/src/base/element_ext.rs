use gpui::{App, Bounds, ParentElement, Pixels, Styled as _, Window, canvas};

/// Extends [`gpui::Element`] with utility helpers.
pub trait ElementExt: ParentElement + Sized {
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
