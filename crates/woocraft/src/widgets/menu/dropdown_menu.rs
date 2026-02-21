use std::rc::Rc;

use gpui::{
  Context, Corner, ElementId, IntoElement, RenderOnce, SharedString, Styled, Window,
};

use crate::{Button, Popover, PopupMenu, Selectable};

use super::popover_menu::{MenuBuilderFn, render_popup_menu};

pub trait DropdownMenuTriggerId {
  fn dropdown_menu_trigger_id(&self) -> ElementId;
}

impl DropdownMenuTriggerId for Button {
  fn dropdown_menu_trigger_id(&self) -> ElementId {
    self.element_id()
  }
}

/// A dropdown menu trait for buttons and other interactive elements
pub trait DropdownMenu:
  Styled + Selectable + DropdownMenuTriggerId + IntoElement + 'static
{
  /// Create a dropdown menu with the given items, anchored to the TopLeft
  /// corner
  fn dropdown_menu(
    self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> DropdownMenuPopover<Self> {
    self.dropdown_menu_with_anchor(Corner::TopLeft, f)
  }

  /// Create a dropdown menu with the given items, anchored to the given corner
  fn dropdown_menu_with_anchor(
    self, anchor: impl Into<Corner>,
    f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> DropdownMenuPopover<Self> {
    let id = self.dropdown_menu_trigger_id();

    DropdownMenuPopover::new(id, anchor, self, f)
  }
}

impl DropdownMenu for Button {}

#[derive(IntoElement)]
pub struct DropdownMenuPopover<T: Selectable + IntoElement + 'static> {
  id: ElementId,
  anchor: Corner,
  trigger: T,
  builder: Rc<MenuBuilderFn>,
}

impl<T> DropdownMenuPopover<T>
where
  T: Selectable + IntoElement + 'static,
{
  fn new(
    id: ElementId, anchor: impl Into<Corner>, trigger: T,
    builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    Self {
      id: SharedString::from(format!("dropdown-menu:{:?}", id)).into(),
      anchor: anchor.into(),
      trigger,
      builder: Rc::new(builder),
    }
  }

  /// Set the anchor corner for the dropdown menu popover.
  pub fn anchor(mut self, anchor: impl Into<Corner>) -> Self {
    self.anchor = anchor.into();
    self
  }
}

impl<T> RenderOnce for DropdownMenuPopover<T>
where
  T: Selectable + IntoElement + 'static,
{
  fn render(self, _window: &mut Window, _cx: &mut gpui::App) -> impl IntoElement {
    let builder = self.builder.clone();
    let menu_state_id = self.id.clone();

    Popover::new(SharedString::from(format!("popover:{}", self.id)))
      .overlay_closable(false)
      .trigger(self.trigger)
      .anchor(self.anchor)
      .content(move |_, window, cx| {
        render_popup_menu(
          menu_state_id.clone(),
          builder.clone(),
          cx.entity(),
          window,
          cx,
        )
      })
  }
}
