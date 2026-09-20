//! modal sheet with themed scrim and bottom-sheet surface.
//!
//! this is a styled wrapper over [`gpui_base::Sheet`], which owns focus
//! trapping, Escape handling, overlay dismissal, and close callback
//! ordering. the wrapper contributes the design system's defaults: the
//! shared modal scrim behind the surface and a bottom-anchored card surface
//! for the content. callers who need a side panel or custom geometry pass
//! their own overlay or surface, which replaces the matching default.
//!
//! ```rust,ignore
//! use woocraft::{Sheet, v_flex};
//!
//! Sheet::new(cx)
//!     .on_close(|_, _, _| {})
//!     .child(v_flex().child("filter settings"));
//! ```

use gpui::{
  AnyElement, App, ClickEvent, FocusHandle, IntoElement, ParentElement, Pixels, RenderOnce,
  StyleRefinement, Styled, Window, div, rems,
};
use gpui_base::{Sheet as BaseSheet, StyledExt as _, box_shadow};

use super::dialog::scrim;
use crate::{
  theme::{ActiveTheme, with_alpha},
  v_flex,
};

/// sheet element styled by the woocraft design system.
pub struct Sheet {
  base: BaseSheet,
  overlay: Option<AnyElement>,
  surface: Option<AnyElement>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Sheet {
  /// creates a sheet; the host renders until closed through the request
  /// mechanics the application owns.
  pub fn new(cx: &mut App) -> Self {
    Self {
      base: BaseSheet::new(cx),
      overlay: None,
      surface: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// replaces the themed scrim with a custom overlay element.
  pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
    self.overlay = Some(overlay.into_any_element());
    self
  }

  /// replaces the themed bottom sheet with a custom surface element.
  pub fn surface(mut self, surface: impl IntoElement) -> Self {
    self.surface = Some(surface.into_any_element());
    self
  }

  /// keeps the sheet open when the overlay is pressed.
  pub fn overlay_closable(mut self, closable: bool) -> Self {
    self.base = self.base.overlay_closable(closable);
    self
  }

  /// runs after every close, whatever triggered it.
  pub fn on_close(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_close(handler);
    self
  }

  #[doc(hidden)]
  pub fn overlay_interactive(mut self, interactive: bool) -> Self {
    self.base = self.base.overlay_interactive(interactive);
    self
  }

  #[doc(hidden)]
  pub fn focus_handle(mut self, focus: FocusHandle) -> Self {
    self.base = self.base.focus_handle(focus);
    self
  }

  #[doc(hidden)]
  pub fn dismiss_before_y(mut self, y: Pixels) -> Self {
    self.base = self.base.dismiss_before_y(y);
    self
  }

  #[doc(hidden)]
  pub fn request_close(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
    self.base = self.base.request_close(handler);
    self
  }
}

impl ParentElement for Sheet {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for Sheet {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Sheet {
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
    let style = self.style;

    let mut base = self.base;
    let overlay = match self.overlay {
      Some(overlay) => overlay,
      None => div().absolute().inset_0().bg(scrim()).into_any_element(),
    };
    base = base.overlay(overlay);

    match self.surface {
      Some(surface) => base = base.surface(surface),
      None if !self.children.is_empty() => {
        let children = self.children;
        base = base.surface(
          v_flex()
            .absolute()
            .bottom_0()
            .left_0()
            .right_0()
            .font_family(font_family)
            .text_size(text_size)
            .text_color(card_foreground)
            .bg(card)
            .border_t(border_width)
            .border_color(border_color)
            .rounded_tl(radius)
            .rounded_tr(radius)
            .shadow(vec![box_shadow(
              gpui::px(0.),
              gpui::px(-2.),
              shadow_blur,
              gpui::px(0.),
              with_alpha(gpui::black(), 0.15),
            )])
            .p(rems(1.5))
            .gap(rems(1.))
            .refine_style(&style)
            .children(children),
        );
      }
      None => {}
    }
    base
  }
}
