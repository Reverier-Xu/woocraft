//! alert dialog: the interruption-grade specialization of the themed dialog.
//!
//! this is a styled wrapper over [`gpui_base::AlertDialog`], which pins the
//! accessible alert role and disables backdrop dismissal — an alert demands
//! an explicit decision, so neither Escape-proof defaults nor accidental
//! clicks close it. the wrapper adds the design system's scrim, card
//! surface, and themed action parts: [`AlertDialogAction`] renders the
//! primary confirmation and [`AlertDialogCancel`] the escape hatch, both
//! dispatching the dialog keyboard actions so the host's decision callbacks
//! stay the single source of truth.
//!
//! ```rust,ignore
//! use woocraft::{AlertDialog, AlertDialogAction, AlertDialogCancel};
//!
//! AlertDialog::new(cx)
//!     .on_ok(|_, _, _| true)
//!     .child("delete project?")
//!     .child(AlertDialogCancel::new().label("keep it"))
//!     .child(AlertDialogAction::new().label("delete"));
//! ```

use gpui::{
  AnyElement, App, ClickEvent, FocusHandle, IntoElement, ParentElement, RenderOnce,
  StyleRefinement, Styled, Window, black, div, px, rems,
};
use gpui_base::{
  AlertDialog as BaseAlertDialog, DialogChangeReason, DialogHandle, StyledExt as _, box_shadow,
};

use crate::{
  theme::{ActiveTheme, with_alpha},
  v_flex,
  widgets::button::{Button, ButtonVariants as _},
};

/// themed alert dialog element.
pub struct AlertDialog {
  base: BaseAlertDialog,
  backdrop: Option<AnyElement>,
  popup: Option<AnyElement>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl AlertDialog {
  /// creates a closed alert dialog; it renders nothing until opened.
  pub fn new(cx: &mut App) -> Self {
    Self {
      base: BaseAlertDialog::new(cx),
      backdrop: None,
      popup: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// pins the open state.
  pub fn open(mut self, open: bool) -> Self {
    self.base = self.base.open(open);
    self
  }

  /// attaches a handle for imperative open/close and state reads.
  pub fn handle(mut self, handle: DialogHandle) -> Self {
    self.base = self.base.handle(handle);
    self
  }

  /// subscribes to open-state transitions with the change reason.
  pub fn on_open_change(
    mut self, handler: impl Fn(bool, DialogChangeReason, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_open_change(handler);
    self
  }

  /// replaces the themed scrim with a custom backdrop element.
  pub fn backdrop(mut self, element: impl IntoElement) -> Self {
    self.backdrop = Some(element.into_any_element());
    self
  }

  /// replaces the themed card surface with a custom popup element.
  pub fn popup(mut self, element: impl IntoElement) -> Self {
    self.popup = Some(element.into_any_element());
    self
  }

  /// keeps the alert open when Escape is pressed.
  pub fn close_on_escape(mut self, value: bool) -> Self {
    self.base = self.base.close_on_escape(value);
    self
  }

  /// vetoes or allows the confirm decision; the alert closes on `true`.
  pub fn on_ok(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
  ) -> Self {
    self.base = self.base.on_ok(handler);
    self
  }

  /// vetoes or allows the cancel decision; the alert closes on `true`.
  pub fn on_cancel(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
  ) -> Self {
    self.base = self.base.on_cancel(handler);
    self
  }

  /// runs after every close, whatever the reason.
  pub fn on_close(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_close(handler);
    self
  }

  #[doc(hidden)]
  pub fn layer(mut self, index: usize, topmost: bool) -> Self {
    self.base = self.base.layer(index, topmost);
    self
  }

  #[doc(hidden)]
  pub fn focus_handle(mut self, value: FocusHandle) -> Self {
    self.base = self.base.focus_handle(value);
    self
  }

  #[doc(hidden)]
  pub fn request_close(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
    self.base = self.base.request_close(handler);
    self
  }
}

impl ParentElement for AlertDialog {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for AlertDialog {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for AlertDialog {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // theme values are copied out before the render borrows end.
    let (font_family, text_size, card, card_foreground, border_color, border_width, radius) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.card,
        theme.card_foreground,
        theme.border,
        theme.border_width,
        theme.radius_lg,
      )
    };
    let shadow_blur = rems(1.).to_pixels(window.rem_size());
    let shadow_offset = rems(0.5).to_pixels(window.rem_size());
    let style = self.style;

    let mut base = self.base;
    let backdrop = match self.backdrop {
      Some(element) => element,
      None => div()
        .absolute()
        .inset_0()
        .bg(super::dialog::scrim())
        .into_any_element(),
    };
    base = base.backdrop(backdrop);
    match self.popup {
      Some(popup) => base = base.popup(popup),
      None => {
        let children = self.children;
        base = base.popup(
          v_flex()
            .font_family(font_family)
            .text_size(text_size)
            .text_color(card_foreground)
            .bg(card)
            .rounded(radius)
            .border(border_width)
            .border_color(border_color)
            .shadow(vec![box_shadow(
              px(0.),
              shadow_offset,
              shadow_blur,
              px(0.),
              with_alpha(black(), 0.2),
            )])
            .p(rems(1.5))
            .gap(rems(1.))
            .refine_style(&style)
            .children(children),
        );
      }
    }
    base
  }
}

/// themed confirm control: the primary button dispatching the alert's
/// confirm action.
#[derive(IntoElement)]
pub struct AlertDialogAction {
  button: Button,
}

impl AlertDialogAction {
  /// creates the themed confirm button.
  pub fn new() -> Self {
    Self {
      button: Button::new("alert-dialog-action")
        .primary()
        .on_click(|_, window, cx| {
          window.dispatch_action(
            Box::new(gpui_base::actions::Confirm { secondary: false }),
            cx,
          )
        }),
    }
  }
}

impl Default for AlertDialogAction {
  fn default() -> Self {
    Self::new()
  }
}

impl ParentElement for AlertDialogAction {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.button.extend(elements);
  }
}

impl Styled for AlertDialogAction {
  fn style(&mut self) -> &mut StyleRefinement {
    self.button.style()
  }
}

impl RenderOnce for AlertDialogAction {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    self.button
  }
}

/// themed cancel control: the default-variant button dispatching the alert's
/// cancel action.
#[derive(IntoElement)]
pub struct AlertDialogCancel {
  button: Button,
}

impl AlertDialogCancel {
  /// creates the themed cancel button.
  pub fn new() -> Self {
    Self {
      button: Button::new("alert-dialog-cancel")
        .on_click(|_, window, cx| window.dispatch_action(Box::new(gpui_base::actions::Cancel), cx)),
    }
  }
}

impl Default for AlertDialogCancel {
  fn default() -> Self {
    Self::new()
  }
}

impl ParentElement for AlertDialogCancel {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.button.extend(elements);
  }
}

impl Styled for AlertDialogCancel {
  fn style(&mut self) -> &mut StyleRefinement {
    self.button.style()
  }
}

impl RenderOnce for AlertDialogCancel {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    self.button
  }
}
