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
  prelude::FluentBuilder as _, px, rems,
};
pub use gpui_base::CheckboxState;
use gpui_base::{
  Checkbox as BaseCheckbox, Disableable, RoleOverride,
  motion::{Transition, transition},
};

use crate::{ActiveTheme, Icon, IconName, theme::duration};

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

    // remember the previous semantic state so the mark shrinks out with
    // the shape it was checked in instead of vanishing mid-transition.
    let glyph_slot = window.use_keyed_state((self.id.clone(), "glyph"), cx, |_, _| {
      CheckboxState::Unchecked
    });
    let glyph_state = if marked { state } else { *glyph_slot.read(cx) };
    glyph_slot.update(cx, |slot, _| *slot = state);

    // hover is tracked through a keyed slot so the box morph animates
    // through the same motion transition as the mark; a hover style hook
    // could only snap.
    let hovered_slot = window.use_keyed_state((self.id.clone(), "hover"), cx, |_, _| false);
    let hovered = !disabled && *hovered_slot.read(cx);

    let theme = cx.theme();
    let (background, border_color, muted, muted_foreground, primary, primary_foreground) = (
      theme.background,
      theme.border,
      theme.muted,
      theme.muted_foreground,
      theme.primary,
      theme.primary_foreground,
    );
    let border_width = theme.border_width;
    let (radius, font_size) = (theme.radius, theme.font_size);
    let font_family = theme.font_family.clone();
    let foreground = theme.foreground;

    // the mark scales in and out over the control duration. colors snap
    // instead of easing: hue interpolation across near-neutral theme colors
    // reads as a flash.
    let box_bg = if disabled {
      muted
    } else if marked {
      primary
    } else {
      background
    };
    let mark_size = transition(
      (self.id.clone(), "mark"),
      if marked {
        // a quarter-rem inset on each side keeps the mark off the frame.
        rems(0.5).to_pixels(window.rem_size())
      } else {
        px(0.)
      },
      Transition::new(duration::CONTROL),
      window,
      cx,
    );
    let box_size = transition(
      (self.id.clone(), "box"),
      if hovered {
        rems(1.1).to_pixels(window.rem_size())
      } else {
        rems(1.).to_pixels(window.rem_size())
      },
      Transition::new(duration::CONTROL),
      window,
      cx,
    );

    let mark_icon = match glyph_state {
      CheckboxState::Checked => Icon::new(IconName::Checkmark),
      CheckboxState::Indeterminate => Icon::new(IconName::Subtract),
      CheckboxState::Unchecked => Icon::empty(),
    };
    let mark_color = if disabled {
      muted_foreground
    } else {
      primary_foreground
    };

    let mut base = self.base;

    base = base
      .flex()
      .items_center()
      .gap(rems(0.5))
      .text_size(font_size)
      .font_family(font_family)
      .text_color(if disabled {
        muted_foreground
      } else {
        foreground
      })
      .when(!disabled, |this| this.cursor_pointer())
      .when(disabled, |this| {
        this.cursor(CursorStyle::OperationNotAllowed)
      })
      .child({
        // fixed-size slot keeps the row layout stable while the box grows
        // under hover.
        let mut box_el = div()
          .id((self.id.clone(), "box"))
          .flex()
          .items_center()
          .justify_center()
          .size(box_size)
          .rounded(radius)
          .border(border_width)
          .border_color(border_color)
          .bg(box_bg)
          .child(mark_icon.size(mark_size).text_color(mark_color));
        if !disabled {
          let hovered_slot = hovered_slot.clone();
          box_el = box_el.on_hover(move |entered: &bool, _, cx| {
            hovered_slot.update(cx, |slot, _| *slot = *entered);
          });
        }
        div()
          .flex_none()
          .size(rems(1.))
          .flex()
          .items_center()
          .justify_center()
          .child(box_el)
      })
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
