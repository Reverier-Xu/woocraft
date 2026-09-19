//! radio control for exclusive selection within a group.
//!
//! this is a styled wrapper over [`gpui_base::Radio`], which owns every
//! behavioral concern — activation, focus, keyboard support, and
//! accessibility. the wrapper adds the design-system vocabulary: a `1rem`
//! rounded-rectangle indicator mirroring the checkbox states — background
//! fill while available, muted when disabled — with a square primary
//! accent that scales in over the control duration while checked. the
//! indicator morphs to 1.1x under hover. pair it with
//! [`RadioGroup`](crate::RadioGroup) and report positions through
//! [`Radio::set_position`] so assistive technology can announce
//! "option 2 of 5".

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, FocusHandle, InteractiveElement as _,
  IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, rems,
};
use gpui_base::{
  Radio as BaseRadio,
  motion::{Transition, transition},
};

use crate::{
  ActiveTheme,
  theme::{duration, opacity, with_alpha},
};

/// interactive radio element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Radio;
///
/// Radio::new("plan-pro")
///   .label("pro plan")
///   .checked(self.plan == Plan::Pro)
///   .set_position(2, 3)
///   .on_change(|checked, _event, _window, _cx| {
///     println!("selected: {checked}");
///   });
/// ```
#[derive(IntoElement)]
pub struct Radio {
  id: ElementId,
  base: BaseRadio,
  checked: bool,
  disabled: bool,
  label: Option<SharedString>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Radio {
  /// creates an unchecked radio with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseRadio::new(id.clone()),
      id,
      checked: false,
      disabled: false,
      label: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// updates the element identity used when the radio is rendered.
  ///
  /// a group that assigns positional ids after construction needs this so
  /// each radio keeps a distinct element identity.
  pub fn id(mut self, id: impl Into<ElementId>) -> Self {
    self.id = id.into();
    self.base = self.base.id(self.id.clone());
    self
  }

  /// sets the application-controlled checked state.
  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self.base = self.base.checked(checked);
    self
  }

  /// sets whether pointer and keyboard activation are ignored.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }

  /// sets the visible text label rendered to the right of the circle.
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

  /// sets this radio's one-based position and its group's total size.
  pub fn set_position(mut self, position: usize, size: usize) -> Self {
    self.base = self.base.set_position(position, size);
    self
  }

  /// handles a requested selection change. activating an already checked
  /// radio is a no-op because a radio cannot deselect itself.
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

  /// sets whether the radio participates in keyboard focus traversal.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.base = self.base.tab_stop(tab_stop);
    self
  }
}

impl Styled for Radio {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Radio {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Radio {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let disabled = self.disabled;
    // hover is tracked through a keyed slot so the ring morph animates
    // through the same motion transition as the dot; a hover style hook
    // could only snap.
    let hovered_slot = window.use_keyed_state((self.id.clone(), "hover"), cx, |_, _| false);
    let hovered = !disabled && *hovered_slot.read(cx);

    let theme = cx.theme();
    let (border, muted, background, foreground, muted_foreground, primary) = (
      theme.border,
      theme.muted,
      theme.background,
      theme.foreground,
      theme.muted_foreground,
      theme.primary,
    );
    let (border_width, radius, font_size) = (theme.border_width, theme.radius, theme.font_size);
    let font_family = theme.font_family.clone();

    // the inner square fills the content area behind a background-colored
    // ring that carves a one-pixel gap — filling instead of computing an
    // inset size keeps the ring uniform under fractional device scales. it
    // shows only while checked, and keeps a whisper of primary when
    // disabled so a checked state stays readable — the radio has no glyph
    // to carry it.
    let ring_size = transition(
      (self.id.clone(), "ring"),
      if hovered {
        rems(1.1).to_pixels(window.rem_size())
      } else {
        rems(1.).to_pixels(window.rem_size())
      },
      Transition::new(duration::CONTROL),
      window,
      cx,
    );
    let ring_border = if disabled {
      muted
    } else if checked {
      primary
    } else {
      border
    };
    let dot_bg = if disabled {
      with_alpha(primary, opacity::DISABLED)
    } else {
      primary
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
        // fixed-size slot keeps the row layout stable while the ring grows
        // under hover.
        let mut ring_el = div()
          .id((self.id.clone(), "ring"))
          .flex()
          .items_center()
          .justify_center()
          .size(ring_size)
          .rounded(radius)
          .border(border_width)
          .border_color(ring_border)
          .bg(if disabled { muted } else { background })
          .when(checked, |this| {
            this.child(
              div()
                .size_full()
                .rounded(radius)
                .border(border_width * 2.)
                .border_color(background)
                .bg(dot_bg),
            )
          });
        if !disabled {
          let hovered_slot = hovered_slot.clone();
          ring_el = ring_el.on_hover(move |entered: &bool, _, cx| {
            hovered_slot.update(cx, |slot, _| *slot = *entered);
          });
        }
        div()
          .flex_none()
          .size(rems(1.))
          .flex()
          .items_center()
          .justify_center()
          .child(ring_el)
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
  use super::Radio;

  #[test]
  fn default_radio_is_unchecked_and_enabled() {
    let radio = Radio::new("test");
    assert!(!radio.checked);
    assert!(!radio.disabled);
  }

  #[test]
  fn checked_setter_stores_the_boolean_value() {
    assert!(Radio::new("test").checked(true).checked);
    assert!(!Radio::new("test").checked(true).checked(false).checked);
  }

  #[test]
  fn id_setter_replaces_the_element_identity() {
    let radio = Radio::new("placeholder").id("renamed");
    assert_eq!(radio.id.to_string(), "renamed");
  }

  #[test]
  fn disabled_flag_is_stored() {
    assert!(Radio::new("test").disabled(true).disabled);
  }
}
