//! modal dialog with themed backdrop, surface, and parts.
//!
//! this is a styled wrapper over [`gpui_base::Dialog`], which owns the full
//! behavior surface: focus trapping, Escape/confirm keyboard actions,
//! backdrop dismissal, veto-able decision callbacks, imperative
//! open/close through a [`DialogHandle`], and reason-tracked open-change
//! notifications. the wrapper contributes the design system's defaults — a
//! scrim over the viewport, a card surface around the content, and themed
//! [`DialogTitle`]/[`DialogDescription`]/[`DialogClose`] parts — while every
//! behavioral setter forwards one-to-one to base. mount-time animation stays
//! with the application: the dialog unmounts with its `open` state, so there
//! is no persistent node to sample a fade from.
//!
//! ```rust,ignore
//! use woocraft::{Button, Dialog, DialogHandle, DialogTitle, DialogDescription};
//!
//! let handle = DialogHandle::new(false);
//! Dialog::new(cx)
//!     .handle(handle)
//!     .child(DialogTitle::new().child("delete project?"))
//!     .child(DialogDescription::new().child("this cannot be undone"))
//!     .child(Button::new("confirm").label("delete"));
//! ```

use gpui::{
  AnyElement, App, ClickEvent, FocusHandle, IntoElement, ParentElement, Pixels, RenderOnce,
  StyleRefinement, Styled, Window, black, div, px, rems,
};
use gpui_base::{Dialog as BaseDialog, StyledExt as _, box_shadow};
pub use gpui_base::{DialogChangeReason, DialogHandle};

use crate::{
  icon::{Icon, IconName},
  theme::{ActiveTheme, with_alpha},
  v_flex,
  widgets::button::{Button, ButtonVariants as _},
};

/// scrim applied between the viewport content and a modal surface.
///
/// dialogs and sheets share one wash so stacked modality reads as one depth
/// step, not two.
pub(crate) fn scrim() -> gpui::Hsla {
  with_alpha(black(), 0.4)
}

/// dialog element styled by the woocraft design system.
#[derive(IntoElement)]
pub struct Dialog {
  base: BaseDialog,
  backdrop: Option<AnyElement>,
  popup: Option<AnyElement>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Dialog {
  /// creates a closed dialog. it renders nothing until opened.
  pub fn new(cx: &mut App) -> Self {
    Self {
      base: BaseDialog::new(cx),
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

  /// keeps the dialog open when Escape is pressed.
  pub fn close_on_escape(mut self, value: bool) -> Self {
    self.base = self.base.close_on_escape(value);
    self
  }

  /// keeps the dialog open when the backdrop is pressed.
  pub fn close_on_backdrop_press(mut self, value: bool) -> Self {
    self.base = self.base.close_on_backdrop_press(value);
    self
  }

  /// ignores backdrop presses below `value`, so title bars stay draggable.
  pub fn dismiss_below_y(mut self, value: Pixels) -> Self {
    self.base = self.base.dismiss_below_y(value);
    self
  }

  /// vetoes or allows the confirm decision; the dialog closes on `true`.
  pub fn on_ok(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
  ) -> Self {
    self.base = self.base.on_ok(handler);
    self
  }

  /// vetoes or allows the cancel decision (Escape, backdrop, close part);
  /// the dialog closes on `true`.
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

impl ParentElement for Dialog {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for Dialog {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Dialog {
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
      None => div().absolute().inset_0().bg(scrim()).into_any_element(),
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

/// themed dialog heading: semibold weight at the base text size.
#[derive(IntoElement)]
pub struct DialogTitle {
  style: StyleRefinement,
  children: Vec<AnyElement>,
}

impl DialogTitle {
  /// creates an empty heading.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      children: Vec::new(),
    }
  }
}

impl Default for DialogTitle {
  fn default() -> Self {
    Self::new()
  }
}

impl ParentElement for DialogTitle {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for DialogTitle {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for DialogTitle {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let (font_family, text_size, foreground) = {
      let theme = cx.theme();
      (theme.font_family.clone(), theme.font_size, theme.foreground)
    };
    div()
      .font_family(font_family)
      .text_size(text_size)
      .text_color(foreground)
      .font_weight(gpui::FontWeight::SEMIBOLD)
      .children(self.children)
      .refine_style(&self.style)
  }
}

/// themed dialog body copy rendered in the muted foreground.
#[derive(IntoElement)]
pub struct DialogDescription {
  style: StyleRefinement,
  children: Vec<AnyElement>,
}

impl DialogDescription {
  /// creates an empty description.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      children: Vec::new(),
    }
  }
}

impl Default for DialogDescription {
  fn default() -> Self {
    Self::new()
  }
}

impl ParentElement for DialogDescription {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for DialogDescription {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for DialogDescription {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let (font_family, text_size, muted_foreground) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.muted_foreground,
      )
    };
    div()
      .font_family(font_family)
      .text_size(text_size)
      .text_color(muted_foreground)
      .children(self.children)
      .refine_style(&self.style)
  }
}

/// themed close control: a flat icon button that dispatches the dialog
/// cancel action, closing the nearest dialog.
#[derive(IntoElement)]
pub struct DialogClose {
  style: StyleRefinement,
}

impl DialogClose {
  /// creates the themed close button.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
    }
  }
}

impl Default for DialogClose {
  fn default() -> Self {
    Self::new()
  }
}

impl Styled for DialogClose {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for DialogClose {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    Button::new("dialog-close")
      .icon(Icon::new(IconName::Dismiss))
      .flat()
      .on_click(|_, window, cx| window.dispatch_action(Box::new(gpui_base::actions::Cancel), cx))
      .refine_style(&self.style)
  }
}

#[cfg(test)]
mod tests {
  use super::{DialogDescription, DialogTitle};

  #[test]
  fn dialog_parts_default_construct() {
    let _title = DialogTitle::default();
    let _description = DialogDescription::default();
  }
}
