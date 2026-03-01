//! Navigation breadcrumb trail showing the user's current location in a hierarchy.
//!
//! Breadcrumb displays a sequence of navigation items separated by chevron icons,
//! helping users understand and navigate the document structure. The last item
//! is typically the current page and appears in normal color; earlier items are
//! dimmed and clickable to jump to parent levels.
//!
//! # Example
//! ```rust,ignore
//! use woocraft::Breadcrumb;
//!
//! let breadcrumbs = Breadcrumb::new()
//!   .child("Home")
//!   .child("Products")
//!   .child("Electronics");
//! ```

use std::rc::Rc;

use gpui::{
  App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
  SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, Icon, IconName, Sizable, Size, StyleSized, StyledExt, h_flex};

type BreadcrumbClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

#[derive(IntoElement)]
/// Navigation breadcrumb trail showing document hierarchy.
///
/// Container for a sequence of clickable breadcrumb items separated by chevrons.
/// The last item displays in normal text color; prior items are dimmed.
pub struct Breadcrumb {
  style: StyleRefinement,
  size: Size,
  items: Vec<BreadcrumbItem>,
}

#[derive(IntoElement)]
/// Single item in a breadcrumb trail.
///
/// Represents one level in the navigation hierarchy. Can be labeled, disabled,
/// and optionally handle click events to navigate to that level.
pub struct BreadcrumbItem {
  id: ElementId,
  style: StyleRefinement,
  label: SharedString,
  on_click: Option<BreadcrumbClickHandler>,
  disabled: bool,
  is_last: bool,
}

impl Default for Breadcrumb {
  fn default() -> Self {
    Self::new()
  }
}

impl Breadcrumb {
  /// Creates a new empty breadcrumb trail container.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      size: Size::Medium,
      items: Vec::new(),
    }
  }

  /// Adds a single breadcrumb item to the trail.
  pub fn child(mut self, item: impl Into<BreadcrumbItem>) -> Self {
    self.items.push(item.into());
    self
  }

  /// Adds multiple breadcrumb items to the trail.
  pub fn children(mut self, items: impl IntoIterator<Item = impl Into<BreadcrumbItem>>) -> Self {
    self.items.extend(items.into_iter().map(Into::into));
    self
  }
}

impl_styled!(Breadcrumb);
impl_sizable!(Breadcrumb);

impl RenderOnce for Breadcrumb {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let items_count = self.items.len();

    let mut children = vec![];
    for (index, item) in self.items.into_iter().enumerate() {
      let is_last = index == items_count.saturating_sub(1);

      children.push(item.id(index).with_last(is_last).into_any_element());
      if !is_last {
        children.push(BreadcrumbSeparator.into_any_element());
      }
    }

    h_flex()
      .component_gap(self.size)
      .text_sm()
      .text_color(cx.theme().muted_foreground)
      .refine_style(&self.style)
      .children(children)
  }
}

impl BreadcrumbItem {
  /// Creates a new breadcrumb item with the given label.
  pub fn new(label: impl Into<SharedString>) -> Self {
    Self {
      id: ElementId::Integer(0),
      style: StyleRefinement::default(),
      label: label.into(),
      on_click: None,
      disabled: false,
      is_last: false,
    }
  }

  /// Sets the click handler for this breadcrumb item.
  pub fn on_click(
    mut self, on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(on_click));
    self
  }

  /// Disables the breadcrumb item, making it non-clickable and visually faded.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  fn id(mut self, id: impl Into<ElementId>) -> Self {
    self.id = id.into();
    self
  }

  fn with_last(mut self, is_last: bool) -> Self {
    self.is_last = is_last;
    self
  }
}

impl_styled!(BreadcrumbItem);

impl RenderOnce for BreadcrumbItem {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    div()
      .id(self.id)
      .child(self.label)
      .text_color(cx.theme().muted_foreground)
      .when(self.is_last, |this| this.text_color(cx.theme().foreground))
      .when(self.disabled, |this| {
        this.text_color(cx.theme().muted_foreground).opacity(0.7)
      })
      .when(!self.disabled, |this| {
        this.hover(|this| this.text_color(cx.theme().foreground))
      })
      .refine_style(&self.style)
      .when(!self.disabled, |this| {
        this.when_some(self.on_click, |this, on_click| {
          this.cursor_pointer().on_click(move |event, window, cx| {
            on_click(event, window, cx);
          })
        })
      })
  }
}

impl From<&'static str> for BreadcrumbItem {
  fn from(value: &'static str) -> Self {
    Self::new(value)
  }
}

impl From<String> for BreadcrumbItem {
  fn from(value: String) -> Self {
    Self::new(value)
  }
}

impl From<SharedString> for BreadcrumbItem {
  fn from(value: SharedString) -> Self {
    Self::new(value)
  }
}

#[derive(IntoElement)]
struct BreadcrumbSeparator;

impl RenderOnce for BreadcrumbSeparator {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    Icon::new(IconName::ChevronRight)
      .small()
      .text_color(cx.theme().muted_foreground)
      .into_any_element()
  }
}
