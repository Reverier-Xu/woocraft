use std::rc::Rc;

use gpuim::{Context, DismissEvent, ElementId, Entity, Focusable, Window};

use crate::{PopoverState, PopupMenu};

pub(crate) type MenuBuilderFn =
  dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu;

#[derive(Default)]
struct PopupMenuHostState {
  menu: Option<Entity<PopupMenu>>,
}

pub(crate) fn render_popup_menu(
  state_id: ElementId, builder: Rc<MenuBuilderFn>, popover: Entity<PopoverState>,
  window: &mut Window, cx: &mut Context<PopoverState>,
) -> Entity<PopupMenu> {
  let menu_state = window.use_keyed_state(state_id, cx, |_, _| PopupMenuHostState::default());

  match menu_state.read(cx).menu.clone() {
    Some(menu) => menu,
    None => {
      let builder = builder.clone();
      let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
        builder(menu, window, cx)
      });

      menu_state.update(cx, |state, _| {
        state.menu = Some(menu.clone());
      });

      menu.focus_handle(cx).focus(window, cx);

      window
        .subscribe(&menu, cx, {
          let menu_state = menu_state.clone();
          move |_, _: &DismissEvent, window, cx| {
            popover.update(cx, |state, cx| {
              state.dismiss(window, cx);
            });
            menu_state.update(cx, |state, _| {
              state.menu = None;
            });
          }
        })
        .detach();

      menu
    }
  }
}
