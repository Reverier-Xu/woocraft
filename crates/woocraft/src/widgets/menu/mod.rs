use gpui::App;

mod app_menu_bar;
mod menu_ext;
mod popup_menu;

pub use app_menu_bar::AppMenuBar;
pub use menu_ext::{MenuExt, MenuPopover};
pub use popup_menu::{PopupMenu, PopupMenuItem, Side};

pub(crate) fn init(cx: &mut App) {
  app_menu_bar::init(cx);
  popup_menu::init(cx);
}
