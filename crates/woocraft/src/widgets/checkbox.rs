//! checkbox control for boolean and mixed selection.
//!
//! this is a styled wrapper over [`gpui_base::Checkbox`], which owns every
//! behavioral concern — toggle, focus, keyboard activation, and
//! accessibility. the wrapper adds the design-system vocabulary: a `1rem`
//! indicator box with check and indeterminate marks, an optional text
//! label that joins the hit target, and theme-driven state colors. the
//! [`CheckboxState`] semantic value is re-exported so applications can
//! keep controlled state without naming the base crate.

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, FocusHandle, InteractiveElement as _,
  IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, rems,
};
pub use gpui_base::CheckboxState;
use gpui_base::{Checkbox as BaseCheckbox, Disableable, RoleOverride};

use crate::{
  ActiveTheme, Icon, IconName,
  theme::{opacity, with_alpha},
};

/// interactive checkbox element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::{Checkbox, CheckboxState};
///
/// Checkbox::new("agree")
///   .label("i agree to the terms")
///   .checked(false)
///   .on_change(|state, _event, _window, _cx| {
///     println!("next state: {state:?}");
///   });
/// ```
#[derive(IntoElement)]
pub struct Checkbox {
  id: ElementId,
  base: BaseCheckbox,
  state: CheckboxState,
  disabled: bool,
  label: Option<SharedString>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Checkbox {
  /// creates an unchecked checkbox with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseCheckbox::new(id.clone()),
      id,
      state: CheckboxState::Unchecked,
      disabled: false,
      label: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the controlled semantic state.
  pub fn state(mut self, state: CheckboxState) -> Self {
    self.state = state;
    self.base = self.base.state(state);
    self
  }

  /// sets the checked state, clearing any indeterminate state.
  pub fn checked(self, checked: bool) -> Self {
    self.state(if checked {
      CheckboxState::Checked
    } else {
      CheckboxState::Unchecked
    })
  }

  /// sets or clears the indeterminate state.
  ///
  /// clearing indeterminate leaves the checkbox unchecked, matching the
  /// base control's semantics.
  pub fn indeterminate(self, indeterminate: bool) -> Self {
    if indeterminate {
      self.state(CheckboxState::Indeterminate)
    } else if self.state == CheckboxState::Indeterminate {
      self.state(CheckboxState::Unchecked)
    } else {
      self
    }
  }

  /// sets whether pointer and keyboard activation are ignored.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }

  /// sets the visible text label rendered to the right of the box.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the label exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }

  /// overrides the accessibility role. the default is the checkbox role.
  pub fn role(mut self, role: impl Into<RoleOverride>) -> Self {
    self.base = self.base.role(role);
    self
  }

  /// uses a caller-owned focus handle instead of creating keyed state.
  pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
    self.base = self.base.track_focus(focus_handle);
    self
  }

  /// handles activation with the next controlled state.
  pub fn on_change(
    mut self, handler: impl Fn(CheckboxState, &ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_change(handler);
    self
  }

  /// defines application-owned styles for the checkbox's semantic states.
  pub fn styles(
    mut self, build: impl FnOnce(gpui_base::CheckboxStyles) -> gpui_base::CheckboxStyles,
  ) -> Self {
    self.base = self.base.styles(build);
    self
  }

  /// sets the focus traversal index. the default is `0`.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.base = self.base.tab_index(tab_index);
    self
  }

  /// sets whether the checkbox participates in keyboard focus traversal.
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

impl Disableable for Checkbox {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }
}

impl Styled for Checkbox {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Checkbox {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Checkbox {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state;
    let disabled = self.disabled;
    let marked = matches!(state, CheckboxState::Checked | CheckboxState::Indeterminate);
    let focused = !disabled && self.focus_handle(window, cx).is_focused(window);
    let theme = cx.theme();

    let accent = if focused { theme.ring } else { theme.primary };
    let (box_bg, box_border, mark) = if disabled {
      let faded = with_alpha(theme.primary, opacity::DISABLED);
      (
        if marked { faded } else { theme.background },
        theme.border,
        theme.muted_foreground,
      )
    } else if marked {
      (accent, accent, theme.primary_foreground)
    } else {
      (theme.background, accent, theme.ring)
    };

    let mark_icon = match state {
      CheckboxState::Checked => Icon::new(IconName::Checkmark),
      CheckboxState::Indeterminate => Icon::new(IconName::Subtract),
      CheckboxState::Unchecked => Icon::empty(),
    };

    let (foreground, muted_foreground, border_width) =
      (theme.foreground, theme.muted_foreground, theme.border_width);
    let radius = theme.radius;
    let mut base = self.base;

    base = base
      .flex()
      .items_center()
      .gap(rems(0.5))
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(if disabled {
        muted_foreground
      } else {
        foreground
      })
      .when(!disabled, |this| {
        this
          .cursor_pointer()
          .hover(|s| s.opacity(opacity::solid::HOVER))
          .active(|s| s.opacity(opacity::solid::ACTIVE))
      })
      .when(disabled, |this| {
        this.cursor(CursorStyle::OperationNotAllowed)
      })
      .child(
        div()
          .flex_none()
          .size(rems(1.))
          .flex()
          .items_center()
          .justify_center()
          .rounded(radius)
          .border(border_width)
          .border_color(box_border)
          .bg(box_bg)
          .when(marked, |this| {
            this.child(mark_icon.size(rems(0.875)).text_color(mark))
          }),
      )
      .when_some(self.label, |this, label| this.child(div().child(label)))
      .children(self.children);

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base
  }
}

#[cfg(test)]
mod tests {
  use super::{Checkbox, CheckboxState};

  #[test]
  fn default_state_is_unchecked() {
    let checkbox = Checkbox::new("test");
    assert_eq!(checkbox.state, CheckboxState::Unchecked);
  }

  #[test]
  fn checked_setter_selects_the_boolean_state() {
    assert_eq!(
      Checkbox::new("test").checked(true).state,
      CheckboxState::Checked
    );
    assert_eq!(
      Checkbox::new("test").checked(false).state,
      CheckboxState::Unchecked
    );
  }

  #[test]
  fn indeterminate_setter_round_trips_through_unchecked() {
    assert_eq!(
      Checkbox::new("test").indeterminate(true).state,
      CheckboxState::Indeterminate
    );
    assert_eq!(
      Checkbox::new("test")
        .indeterminate(true)
        .indeterminate(false)
        .state,
      CheckboxState::Unchecked
    );
  }

  #[test]
  fn checked_overrides_indeterminate() {
    assert_eq!(
      Checkbox::new("test")
        .indeterminate(true)
        .checked(false)
        .state,
      CheckboxState::Unchecked
    );
  }

  #[test]
  fn disabled_flag_is_stored() {
    let checkbox = Checkbox::new("test").disabled(true);
    assert!(checkbox.disabled);
  }
}
