/// Implements `gpui::Styled` for a type with a `style: StyleRefinement` field.
///
/// ```ignore
/// impl_styled!(MyWidget);            // uses self.style
/// impl_styled!(MyWidget, my_style);  // uses self.my_style
/// ```
macro_rules! impl_styled {
  ($ty:ty) => {
    impl gpui::Styled for $ty {
      fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.style
      }
    }
  };
  ($ty:ty, $field:ident) => {
    impl gpui::Styled for $ty {
      fn style(&mut self) -> &mut gpui::StyleRefinement {
        &mut self.$field
      }
    }
  };
}

/// Implements `Sizable` for a type with a `size: Size` field.
///
/// ```ignore
/// impl_sizable!(MyWidget);
/// ```
macro_rules! impl_sizable {
  ($ty:ty) => {
    impl $crate::Sizable for $ty {
      fn with_size(mut self, size: impl Into<$crate::Size>) -> Self {
        self.size = size.into();
        self
      }
    }
  };
}

/// Implements `Disableable` for a type with a `disabled: bool` field.
///
/// ```ignore
/// impl_disableable!(MyWidget);
/// ```
macro_rules! impl_disableable {
  ($ty:ty) => {
    impl $crate::Disableable for $ty {
      fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
      }
    }
  };
}

/// Implements `Selectable` for a type with a `selected: bool` field.
///
/// ```ignore
/// impl_selectable!(MyWidget);             // uses self.selected
/// impl_selectable!(MyWidget, checked);    // uses self.checked
/// ```
macro_rules! impl_selectable {
  ($ty:ty) => {
    impl $crate::Selectable for $ty {
      fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
      }

      fn is_selected(&self) -> bool {
        self.selected
      }
    }
  };
  ($ty:ty, $field:ident) => {
    impl $crate::Selectable for $ty {
      fn selected(mut self, selected: bool) -> Self {
        self.$field = selected;
        self
      }

      fn is_selected(&self) -> bool {
        self.$field
      }
    }
  };
}

/// Implements `gpui::ParentElement` for a type with a `children: Vec<AnyElement>` field.
///
/// ```ignore
/// impl_parent_element!(MyWidget);
/// ```
macro_rules! impl_parent_element {
  ($ty:ty) => {
    impl gpui::ParentElement for $ty {
      fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
        self.children.extend(elements);
      }
    }
  };
}
