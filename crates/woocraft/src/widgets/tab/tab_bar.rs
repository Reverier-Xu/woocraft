use std::rc::Rc;

use gpui::{
  AnyElement, App, Corner, Div, ElementId, InteractiveElement, IntoElement, ParentElement,
  RenderOnce, ScrollHandle, Stateful, StatefulInteractiveElement as _, StyleRefinement, Styled,
  Window, div, prelude::FluentBuilder as _,
};
use smallvec::SmallVec;

use super::{
  super::{
    button::{Button, ButtonVariants as _},
    menu::{DropdownMenu as _, PopupMenuItem},
  },
  Tab,
};
use crate::{ActiveTheme, IconName, Selectable, Sizable, Size, StyleSized, StyledExt, h_flex};

type TabBarClickHandler = dyn Fn(&usize, &mut Window, &mut App);

/// A TabBar element that contains multiple [`Tab`] items.
#[derive(IntoElement)]
pub struct TabBar {
  base: Stateful<Div>,
  style: StyleRefinement,
  scroll_handle: Option<ScrollHandle>,
  prefix: Option<AnyElement>,
  suffix: Option<AnyElement>,
  children: SmallVec<[Tab; 2]>,
  last_empty_space: AnyElement,
  selected_index: Option<usize>,
  size: Size,
  menu: bool,
  on_click: Option<Rc<TabBarClickHandler>>,
}

impl TabBar {
  /// Create a new TabBar.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      base: h_flex().id(id),
      style: StyleRefinement::default(),
      children: SmallVec::new(),
      scroll_handle: None,
      prefix: None,
      suffix: None,
      size: Size::default(),
      last_empty_space: div().w_3().into_any_element(),
      selected_index: None,
      on_click: None,
      menu: false,
    }
  }

  /// Set whether to show the menu button when tabs overflow, default is false.
  pub fn menu(mut self, menu: bool) -> Self {
    self.menu = menu;
    self
  }

  /// Track the scroll of the TabBar.
  pub fn track_scroll(mut self, scroll_handle: &ScrollHandle) -> Self {
    self.scroll_handle = Some(scroll_handle.clone());
    self
  }

  /// Set the prefix element of the TabBar
  pub fn prefix(mut self, prefix: impl IntoElement) -> Self {
    self.prefix = Some(prefix.into_any_element());
    self
  }

  /// Set the suffix element of the TabBar
  pub fn suffix(mut self, suffix: impl IntoElement) -> Self {
    self.suffix = Some(suffix.into_any_element());
    self
  }

  /// Add children of the TabBar, all children will inherit the variant.
  pub fn children(mut self, children: impl IntoIterator<Item = impl Into<Tab>>) -> Self {
    self.children.extend(children.into_iter().map(Into::into));
    self
  }

  /// Add child of the TabBar, tab will inherit the variant.
  pub fn child(mut self, child: impl Into<Tab>) -> Self {
    self.children.push(child.into());
    self
  }

  /// Set the selected index of the TabBar.
  pub fn selected_index(mut self, index: usize) -> Self {
    self.selected_index = Some(index);
    self
  }

  /// Set the last empty space element of the TabBar.
  pub fn last_empty_space(mut self, last_empty_space: impl IntoElement) -> Self {
    self.last_empty_space = last_empty_space.into_any_element();
    self
  }

  /// Set the on_click callback of the TabBar, the first parameter is the index
  /// of the clicked tab.
  ///
  /// When this is set, the children's on_click will be ignored.
  pub fn on_click<F>(mut self, on_click: F) -> Self
  where
    F: Fn(&usize, &mut Window, &mut App) + 'static, {
    self.on_click = Some(Rc::new(on_click));
    self
  }
}

impl Styled for TabBar {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl Sizable for TabBar {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl RenderOnce for TabBar {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let mut item_labels = Vec::new();
    let selected_index = self.selected_index;
    let on_click = self.on_click.clone();

    self
      .base
      .group("tab-bar")
      .relative()
      .flex()
      .items_center()
      .container_size(self.size)
      .container_h(self.size)
      .gap_1()
      .text_color(cx.theme().tab_foreground)
      .refine_style(&self.style)
      .when_some(self.prefix, |this, prefix| this.child(prefix))
      .child(
        h_flex()
          .id("tabs")
          .flex_1()
          .gap_1()
          .items_center()
          .overflow_x_scroll()
          .when_some(self.scroll_handle, |this, scroll_handle| {
            this.track_scroll(&scroll_handle)
          })
          .children(self.children.into_iter().enumerate().map(|(ix, child)| {
            item_labels.push((child.label.clone(), child.disabled));
            child
              .ix(ix)
              .with_size(self.size)
              .when_some(self.selected_index, |this, selected_ix| {
                this.selected(selected_ix == ix)
              })
              .when_some(self.on_click.clone(), move |this, on_click| {
                this.on_click(move |_, window, cx| on_click(&ix, window, cx))
              })
          }))
          .when(self.suffix.is_some() || self.menu, |this| {
            this.child(self.last_empty_space)
          }),
      )
      .when(self.menu, |this| {
        this.child(
          Button::new("more")
            .flat()
            .icon(IconName::ChevronDown)
            .dropdown_menu(move |mut this: crate::PopupMenu, _, _| {
              this = this.scrollable(true);
              for (ix, (label, disabled)) in item_labels.iter().enumerate() {
                this = this.item(
                  PopupMenuItem::new(label.clone().unwrap_or_default())
                    .checked(selected_index == Some(ix))
                    .disabled(*disabled)
                    .when_some(on_click.clone(), |this, on_click| {
                      this.on_click(move |_, window, cx| on_click(&ix, window, cx))
                    }),
                )
              }

              this
            })
            .anchor(Corner::TopRight),
        )
      })
      .when_some(self.suffix, |this, suffix| this.child(suffix))
  }
}
