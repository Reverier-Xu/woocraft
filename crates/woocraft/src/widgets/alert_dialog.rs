//! alert dialog: the interruption-grade specialization of the themed dialog.
//!
//! this is a styled wrapper over [`gpui_base::AlertDialog`], which pins the
//! accessible alert role and disables backdrop dismissal — an alert demands
//! an explicit decision, so neither Escape-proof defaults nor accidental
//! clicks close it. the wrapper adds the design system's scrim, centered
//! card surface, and themed action parts: [`AlertDialogAction`] renders the
//! primary confirmation and [`AlertDialogCancel`] the escape hatch, both
//! dispatching the dialog keyboard actions through a focus anchor so the
//! host's decision callbacks stay the single source of truth.
//!
//! ```rust,ignore
//! use woocraft::{AlertDialog, AlertDialogAction, AlertDialogCancel};
//!
//! AlertDialog::new(cx)
//!     .on_ok(|_, _, _| true)
//!     .child("delete project?")
//!     .child(AlertDialogCancel::new().child("keep it"))
//!     .child(AlertDialogAction::new().child("delete").danger());
//! ```

use gpui::{
  AnyElement, App, ClickEvent, FocusHandle, InteractiveElement as _, IntoElement, ParentElement,
  Pixels, RenderOnce, StyleRefinement, Styled, Window, div,
};
pub use gpui_base::AlertDialogTrigger;
use gpui_base::{AlertDialog as BaseAlertDialog, DialogChangeReason, DialogHandle};

use super::{
  dialog::{DispatchAnchor, modal_card, modal_geometry, scrim},
  title_bar::TITLE_BAR_HEIGHT,
  window_border::window_paddings,
};
use crate::widgets::button::{Button, ButtonVariant, ButtonVariants as _};

/// themed alert dialog element.
#[derive(IntoElement)]
pub struct AlertDialog {
  base: BaseAlertDialog,
  backdrop: Option<AnyElement>,
  popup: Option<AnyElement>,
  width: Option<Pixels>,
  max_width: Option<Pixels>,
  margin_top: Option<Pixels>,
  dismiss_below_y: Option<Pixels>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl AlertDialog {
  /// creates a closed alert dialog; it renders nothing until opened through
  /// `.open(true)` or a [`DialogHandle`].
  pub fn new(cx: &mut App) -> Self {
    Self {
      base: BaseAlertDialog::new(cx).open(false),
      backdrop: None,
      popup: None,
      width: None,
      max_width: None,
      margin_top: None,
      dismiss_below_y: None,
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

  /// sets the card width; the default is `28rem`, clamped so the card
  /// always leaves a `1rem` margin on each side of the content area.
  pub fn width(mut self, width: impl Into<Pixels>) -> Self {
    self.width = Some(width.into());
    self
  }

  /// caps the card width on top of the viewport clamp.
  pub fn max_w(mut self, max_width: impl Into<Pixels>) -> Self {
    self.max_width = Some(max_width.into());
    self
  }

  /// sets the card's offset from the top of the content area; the default
  /// is a tenth of the content height.
  pub fn margin_top(mut self, margin_top: impl Into<Pixels>) -> Self {
    self.margin_top = Some(margin_top.into());
    self
  }

  /// ignores backdrop presses below `value`; the default is
  /// [`TITLE_BAR_HEIGHT`](crate::widgets::title_bar::TITLE_BAR_HEIGHT).
  /// alert dialogs never close on backdrop press, so this only matters for
  /// keeping the title bar draggable through the scrim.
  pub fn dismiss_below_y(mut self, value: Pixels) -> Self {
    self.dismiss_below_y = Some(value);
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
    let geometry = modal_geometry(window, self.width, self.margin_top);
    let dismiss_below_y = self
      .dismiss_below_y
      .unwrap_or_else(|| TITLE_BAR_HEIGHT.to_pixels(window.rem_size()));
    let max_width = self.max_width;
    let style = self.style;

    let mut base = self.base.dismiss_below_y(dismiss_below_y);
    let backdrop = match self.backdrop {
      Some(element) => element,
      None => {
        let paddings = window_paddings(window);
        let viewport = window.viewport_size();
        div()
          .absolute()
          .top(paddings.top)
          .left(paddings.left)
          .w(viewport.width - paddings.left - paddings.right)
          .h(viewport.height - paddings.top - paddings.bottom)
          .window_control_area(gpui::WindowControlArea::Drag)
          .bg(scrim())
          .into_any_element()
      }
    };
    base = base.backdrop(backdrop);
    match self.popup {
      Some(popup) => base = base.popup(popup),
      None => {
        let children = self.children;
        base = base.popup(modal_card(
          &geometry, max_width, &style, children, window, cx,
        ));
      }
    }
    base
  }
}

/// themed confirm control: the primary button dispatching the alert's
/// confirm action through a focus anchor, so the decision reaches the alert
/// whatever holds focus at that moment.
#[derive(IntoElement)]
pub struct AlertDialogAction {
  button: Button,
}

impl AlertDialogAction {
  /// creates the themed confirm button.
  pub fn new() -> Self {
    Self {
      button: Button::new("alert-dialog-action").primary(),
    }
  }

  /// styles the confirm button with the destructive intent.
  pub fn danger(mut self) -> Self {
    self.button = self.button.with_variant(ButtonVariant::Danger);
    self
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
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let anchor = DispatchAnchor::new("woocraft-alert-confirm", window, cx);
    let dispatch = anchor.clone();
    div()
      .child(anchor.element())
      .child(self.button.on_click(move |_, window, cx| {
        dispatch.dispatch(
          &gpui_base::actions::Confirm { secondary: false },
          window,
          cx,
        )
      }))
  }
}

/// themed cancel control: the default-variant button dispatching the
/// alert's cancel action through a focus anchor.
#[derive(IntoElement)]
pub struct AlertDialogCancel {
  button: Button,
}

impl AlertDialogCancel {
  /// creates the themed cancel button.
  pub fn new() -> Self {
    Self {
      button: Button::new("alert-dialog-cancel"),
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
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let anchor = DispatchAnchor::new("woocraft-alert-cancel", window, cx);
    let dispatch = anchor.clone();
    div().child(anchor.element()).child(
      self
        .button
        .on_click(move |_, window, cx| dispatch.dispatch(&gpui_base::actions::Cancel, window, cx)),
    )
  }
}
