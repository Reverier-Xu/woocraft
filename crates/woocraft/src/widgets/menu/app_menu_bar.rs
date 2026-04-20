use gpuim::{
  App, AppContext as _, Context, Entity, InteractiveElement as _, IntoElement, OwnedMenu,
  ParentElement, Render, StatefulInteractiveElement as _, Styled, Window,
};

use crate::{Button, ButtonVariants, DropdownMenu as _, Sizable, h_flex};

pub fn init(_: &mut App) {}

/// The application menu bar, for Windows and Linux.
pub struct AppMenuBar {
  menus: Vec<OwnedMenu>,
}

impl AppMenuBar {
  /// Create a new app menu bar.
  pub fn new(cx: &mut App) -> Entity<Self> {
    cx.new(|cx| {
      let mut this = Self { menus: Vec::new() };
      this.reload(cx);
      this
    })
  }

  /// Reload the menus from the app.
  pub fn reload(&mut self, cx: &mut Context<Self>) {
    self.menus = cx.get_menus().unwrap_or_default();
    cx.notify();
  }
}

impl Render for AppMenuBar {
  fn render(&mut self, _: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    h_flex()
      .flex_1()
      .id("app-menu-bar")
      .h_full()
      .min_w_0()
      .gap_x_1()
      .opacity(0.6)
      .overflow_x_scroll()
      .justify_start()
      .hover(|this| this.opacity(1.))
      .children(self.menus.iter().enumerate().map(|(ix, menu)| {
        let menu_name = menu.name.as_ref().to_owned();
        let menu_items = menu.items.clone();
        Button::new(("app-menu", ix))
          .medium()
          .flat()
          .label(menu_name)
          .dropdown_menu(move |menu, window, cx| {
            let mut menu = menu;
            if let Some(handle) = window.focused(cx) {
              menu = menu.action_context(handle);
            }
            menu.with_menu_items(menu_items.clone(), window, cx)
          })
          .into_any_element()
      }))
  }
}
