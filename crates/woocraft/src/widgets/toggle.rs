//! toggle button for persistent on/off selection.
//!
//! this is a styled wrapper over [`gpui_base::Toggle`], which owns every
//! behavioral concern — activation, focus, and accessibility semantics.
//! the wrapper adds the design-system vocabulary: ghost-button geometry
//! with a `2rem` height, an accent-filled pressed state, and disabled
//! presentation. pair it with [`ToggleGroup`](crate::ToggleGroup) to build
//! a toolbar of related toggles.

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, FocusHandle, InteractiveElement as _,
  IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, rems,
};
use gpui_base::Toggle as BaseToggle;

use crate::{
  ActiveTheme, Icon,
  theme::{opacity, with_alpha},
};

/// interactive toggle element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Toggle;
///
/// Toggle::new("bold")
///   .icon(IconName::Bold)
///   .pressed(self.bold)
///   .on_change(|pressed, _event, _window, _cx| {
///     println!("bold: {pressed}");
///   });
/// ```
#[derive(IntoElement)]
pub struct Toggle {
  id: ElementId,
  base: BaseToggle,
  pressed: bool,
  disabled: bool,
  icon: Option<Icon>,
  label: Option<SharedString>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Toggle {
  /// creates an unpressed toggle with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseToggle::new(id.clone()),
      id,
      pressed: false,
      disabled: false,
      icon: None,
      label: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the application-controlled pressed state.
  pub fn pressed(mut self, pressed: bool) -> Self {
    self.pressed = pressed;
    self.base = self.base.pressed(pressed);
    self
  }

  /// sets whether pointer and keyboard activation are ignored.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }

  /// sets the leading icon of the toggle.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  /// sets the visible text label of the toggle.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the label exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }

  /// uses a caller-owned focus handle instead of creating keyed state.
  pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
    self.base = self.base.track_focus(focus_handle);
    self
  }

  /// handles a request to change the controlled pressed state.
  pub fn on_change(
    mut self, handler: impl Fn(bool, &ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_change(handler);
    self
  }

  /// sets the focus traversal index. the default is `0`.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.base = self.base.tab_index(tab_index);
    self
  }

  /// sets whether the toggle participates in keyboard focus traversal.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.base = self.base.tab_stop(tab_stop);
    self
  }

  fn focus_handle(&self, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
      .use_keyed_state((self.id.clone(), "focus"), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone()
  }
}

impl Styled for Toggle {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Toggle {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Toggle {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let pressed = self.pressed;
    let disabled = self.disabled;
    let focused = !disabled && self.focus_handle(window, cx).is_focused(window);
    let theme = cx.theme();

    // horizontal and vertical padding are synced at 0.5rem minus the always
    // present border so the control stays 2rem tall; the transparent border
    // gains the ring color when focused without shifting geometry. the
    // pressed wash is the foreground overlay one step heavier than hover.
    let border_width = theme.border_width;
    let pad = rems(0.5).to_pixels(window.rem_size()) - border_width;
    let hover_bg = with_alpha(theme.foreground, opacity::transparent::HOVER);
    let active_bg = with_alpha(theme.foreground, opacity::transparent::ACTIVE);
    let pressed_bg = with_alpha(theme.foreground, opacity::transparent::ACTIVE);

    let (foreground, muted, muted_foreground) =
      (theme.foreground, theme.muted, theme.muted_foreground);
    let radius = theme.radius;
    let font_size = theme.font_size;
    let mut base = self.base;

    base = base
      .min_w(rems(2.))
      .p(pad)
      .gap(rems(0.5))
      .rounded(radius)
      .border(border_width)
      .border_color(if focused {
        theme.ring
      } else {
        gpui::transparent_black()
      })
      .text_size(font_size)
      .font_family(theme.font_family.clone())
      .text_color(foreground)
      .when(!disabled, |this| {
        this.cursor_pointer().when(!pressed, |this| {
          this
            .hover(move |s| s.bg(hover_bg))
            .active(move |s| s.bg(active_bg))
        })
      })
      .when(disabled, |this| {
        this.cursor(CursorStyle::OperationNotAllowed)
      })
      .styles(move |s| {
        s.pressed(move |st| st.bg(pressed_bg))
          .disabled(|st| st.bg(muted).text_color(muted_foreground))
      });

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    let mut children: Vec<AnyElement> = Vec::new();
    if let Some(icon) = self.icon {
      children.push(icon.into_any_element());
    }
    if let Some(label) = self.label {
      children.push(div().child(label).into_any_element());
    }
    children.extend(self.children);

    base.children(children)
  }
}

#[cfg(test)]
mod tests {
  use super::Toggle;

  #[test]
  fn default_toggle_is_unpressed_and_enabled() {
    let toggle = Toggle::new("test");
    assert!(!toggle.pressed);
    assert!(!toggle.disabled);
  }

  #[test]
  fn pressed_setter_stores_the_boolean_value() {
    assert!(Toggle::new("test").pressed(true).pressed);
    assert!(!Toggle::new("test").pressed(true).pressed(false).pressed);
  }

  #[test]
  fn disabled_flag_is_stored() {
    assert!(Toggle::new("test").disabled(true).disabled);
  }
}
