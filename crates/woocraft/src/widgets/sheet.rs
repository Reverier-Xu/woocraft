//! modal sheet with themed scrim and an edge-anchored surface.
//!
//! this is a styled wrapper over [`gpui_base::Sheet`], which owns focus
//! trapping, Escape handling, overlay dismissal, and close callback
//! ordering. the wrapper contributes the design system's defaults: the
//! shared modal scrim behind the surface and a themed card anchored to one
//! window edge — bottom by default, any side through
//! [`Sheet::placement`]. callers who need a custom geometry pass their own
//! overlay or surface, which replaces the matching default.
//!
//! the base sheet paints in normal element order rather than through a
//! deferred layer, so the host must render it after the content it covers.
//!
//! ```rust,ignore
//! use woocraft::{Sheet, base::Placement};
//!
//! Sheet::new(cx)
//!     .placement(Placement::Right)
//!     .on_close(|_, _, _| {})
//!     .child("filter settings");
//! ```

use gpui::{
  AnyElement, App, ClickEvent, DefiniteLength, FocusHandle, IntoElement, ParentElement, Pixels,
  RenderOnce, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, rems,
};
use gpui_base::{Placement, Sheet as BaseSheet, StyledExt as _};

use super::dialog::scrim;
use crate::{
  theme::{ActiveTheme, with_alpha},
  v_flex,
};

/// sheet element styled by the woocraft design system.
#[derive(IntoElement)]
pub struct Sheet {
  base: BaseSheet,
  placement: Placement,
  size: Option<DefiniteLength>,
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
      placement: Placement::Bottom,
      size: None,
      overlay: None,
      surface: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the window edge the surface anchors to; the default is
  /// [`Placement::Bottom`].
  pub fn placement(mut self, placement: Placement) -> Self {
    self.placement = placement;
    self
  }

  /// sets the surface size along the placement axis: the height of a top or
  /// bottom sheet, the width of a side sheet. the default is content-sized
  /// for top and bottom, `22rem` for the sides.
  pub fn size(mut self, size: impl Into<DefiniteLength>) -> Self {
    self.size = Some(size.into());
    self
  }

  /// replaces the themed scrim with a custom overlay element.
  pub fn overlay(mut self, overlay: impl IntoElement) -> Self {
    self.overlay = Some(overlay.into_any_element());
    self
  }

  /// replaces the themed surface with a custom element.
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
        let placement = self.placement;
        // the shadow lifts the surface off the content on the side facing
        // the window's interior.
        let shadow_offset = match placement {
          Placement::Bottom => gpui::point(gpui::px(0.), gpui::px(-2.)),
          Placement::Top => gpui::point(gpui::px(0.), gpui::px(2.)),
          Placement::Left => gpui::point(gpui::px(2.), gpui::px(0.)),
          Placement::Right => gpui::point(gpui::px(-2.), gpui::px(0.)),
        };
        // side sheets default to a 22rem panel; top and bottom sheets stay
        // content-sized until the caller sets a height.
        let size = self.size.or_else(|| match placement {
          Placement::Left | Placement::Right => Some(rems(22.).into()),
          Placement::Top | Placement::Bottom => None,
        });
        base = base.surface(
          v_flex()
            .absolute()
            .map(|this| match placement {
              Placement::Bottom => this
                .bottom_0()
                .left_0()
                .right_0()
                .rounded_tl(radius)
                .rounded_tr(radius),
              Placement::Top => this
                .top_0()
                .left_0()
                .right_0()
                .rounded_bl(radius)
                .rounded_br(radius),
              Placement::Left => this
                .left_0()
                .top_0()
                .bottom_0()
                .rounded_tr(radius)
                .rounded_br(radius),
              Placement::Right => this
                .right_0()
                .top_0()
                .bottom_0()
                .rounded_tl(radius)
                .rounded_bl(radius),
            })
            .when_some(size, |this, size| match placement.axis() {
              // side sheets size their width, top and bottom their height.
              gpui::Axis::Horizontal => this.w(size),
              gpui::Axis::Vertical => this.h(size),
            })
            .font_family(font_family)
            .text_size(text_size)
            .text_color(card_foreground)
            .bg(card)
            .border_color(border_color)
            .map(|this| match placement {
              Placement::Bottom => this.border_t(border_width),
              Placement::Top => this.border_b(border_width),
              Placement::Left => this.border_r(border_width),
              Placement::Right => this.border_l(border_width),
            })
            .shadow(vec![gpui::BoxShadow {
              color: with_alpha(gpui::black(), 0.15),
              offset: shadow_offset,
              blur_radius: shadow_blur,
              spread_radius: gpui::px(0.),
              inset: false,
            }])
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
