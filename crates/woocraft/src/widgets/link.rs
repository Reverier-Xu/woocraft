//! link activating an application-defined navigation target.
//!
//! this is a styled wrapper over [`gpui_base::Link`], which owns every
//! behavioral concern — activation, focus, and accessibility semantics.
//! the wrapper adds the design-system vocabulary: primary-colored
//! underlined text with hover and active feedback, disabled presentation,
//! and a default open strategy that hands the href to `cx.open_url`.
//! applications can override the strategy through
//! [`Link::open_with`] to route internal navigation instead.

use gpui::{
  AnyElement, App, ClickEvent, CursorStyle, ElementId, FocusHandle, InteractiveElement as _,
  IntoElement, ParentElement, Refineable as _, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Window, prelude::FluentBuilder as _,
};
use gpui_base::{Disableable, Link as BaseLink};

use crate::{ActiveTheme, theme::opacity};

/// interactive link element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Link;
///
/// Link::new("docs").href("https://craft.woooo.tech").child("documentation");
/// ```
#[derive(IntoElement)]
pub struct Link {
  id: ElementId,
  base: BaseLink,
  disabled: bool,
  open_with: Option<SharedOpenHandler>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

type SharedOpenHandler = std::rc::Rc<dyn Fn(&str, &ClickEvent, &mut Window, &mut App)>;

impl Link {
  /// creates a link with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseLink::new(id.clone()),
      id,
      disabled: false,
      open_with: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the application-defined navigation target.
  pub fn href(mut self, href: impl Into<SharedString>) -> Self {
    self.base = self.base.href(href);
    self
  }

  /// injects the strategy used to open the href on activation. the
  /// default strategy opens the URL through the platform handler.
  pub fn open_with(
    mut self, open: impl Fn(&str, &ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    let shared: SharedOpenHandler = std::rc::Rc::new(open);
    self.open_with = Some(shared.clone());
    self.base = self.base.open_with(move |href, event, window, cx| {
      shared(href, event, window, cx);
    });
    self
  }

  /// observes pointer or keyboard activation after the open strategy runs.
  pub fn on_activate(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_activate(handler);
    self
  }

  /// sets whether pointer and keyboard activation are ignored.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }

  /// sets the name exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }

  /// sets the focus traversal index. the default is `0`.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.base = self.base.tab_index(tab_index);
    self
  }

  /// sets whether the link participates in keyboard focus traversal.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.base = self.base.tab_stop(tab_stop);
    self
  }

  fn focus_handle(&self, window: &mut Window, cx: &mut App) -> FocusHandle {
    window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone()
  }
}

impl Disableable for Link {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }
}

impl Styled for Link {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Link {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Link {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let disabled = self.disabled;
    let focused = !disabled && self.focus_handle(window, cx).is_focused(window);
    let theme = cx.theme();

    // applications that did not inject an open strategy get the platform
    // URL handler; strategy injection stays visible in the wrapper so the
    // base rule — href is data, never an instruction — holds there too.
    let mut base = self.base;
    if self.open_with.is_none() {
      base = base.open_with(|href, _, _, cx| cx.open_url(href));
    }

    let accent = if focused { theme.ring } else { theme.primary };
    let muted_foreground = theme.muted_foreground;
    let hover = opacity::solid::HOVER;
    let active = opacity::solid::ACTIVE;

    base = base
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(if disabled { muted_foreground } else { accent })
      .text_decoration_1()
      .text_decoration_color(if disabled {
        gpui::transparent_black()
      } else {
        accent
      })
      .when(!disabled, |this| {
        this
          .cursor_pointer()
          .hover(move |s| s.opacity(hover))
          .active(move |s| s.opacity(active))
      })
      .when(disabled, |this| {
        this.cursor(CursorStyle::OperationNotAllowed)
      });

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base.children(self.children)
  }
}

#[cfg(test)]
mod tests {
  use super::Link;

  #[test]
  fn default_link_is_enabled() {
    let link = Link::new("test");
    assert!(!link.disabled);
    assert!(link.open_with.is_none());
  }

  #[test]
  fn disabled_flag_is_stored() {
    assert!(Link::new("test").disabled(true).disabled);
  }

  #[test]
  fn open_strategy_replaces_the_default() {
    let link = Link::new("test").open_with(|_, _, _, _| {});
    assert!(link.open_with.is_some());
  }
}
