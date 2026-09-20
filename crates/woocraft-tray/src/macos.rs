//! macOS tray backend using AppKit's `NSStatusItem` via `objc2`.
//!
//! AppKit UI is main-thread only; the runtime calls `set_tray` / `remove_tray`
//! / `shutdown` from the GPUI main thread, which matches AppKit's
//! requirements. The status item shows the icon (PNG/JPEG bytes, rendered as
//! a template image) or a title, and the context menu is built from
//! [`TrayMenuItem`]s; activating an item emits `TrayEvent::MenuClicked`.
//! Click/double-click events are not emitted on macOS: when a menu is set the
//! system shows it for both button presses, which is the platform convention.

use std::sync::Mutex;

use crossbeam_channel::{Receiver, Sender};
use objc2::{
  AnyThread, DefinedClass, define_class, msg_send,
  rc::Retained,
  runtime::{NSObject, NSObjectProtocol},
  sel,
};
use objc2_app_kit::{
  NSImage, NSMenu, NSMenuItem, NSStatusBar, NSStatusItem, NSVariableStatusItemLength,
};
use objc2_foundation::{MainThreadMarker, NSData, NSString};

use super::{
  Error, Result, Tray, TrayEvent, TrayMenuItem,
  platform::{BackendError, PlatformTray},
};

/// Objective-C target shared by all menu items: reads the item's
/// `representedObject` (the action id) and pushes `TrayEvent::MenuClicked`
/// into the event channel.
struct MenuTargetIvars {
  sender: Sender<TrayEvent>,
}

define_class!(
  #[unsafe(super(NSObject))]
  #[ivars = MenuTargetIvars]
  struct MenuTarget;

  unsafe impl NSObjectProtocol for MenuTarget {}

  impl MenuTarget {
    #[unsafe(method(menuItemClicked:))]
    fn menu_item_clicked(&self, sender: &NSMenuItem) {
      if let Some(object) = sender.representedObject()
        && let Some(id) = object.downcast_ref::<NSString>()
      {
        let _ = self.ivars().sender.send(TrayEvent::MenuClicked { id: id.to_string() });
      }
    }
  }
);

impl MenuTarget {
  fn new(sender: Sender<TrayEvent>) -> Retained<Self> {
    let this = Self::alloc().set_ivars(MenuTargetIvars { sender });
    // SAFETY: `NSObject`'s `init` is its designated initializer.
    unsafe { msg_send![super(this), init] }
  }
}

/// The installed tray state. AppKit objects are main-thread only and the
/// runtime only touches them from the GPUI main thread, so the state can be
/// `Send + Sync` for `PlatformTray`; every method re-checks the thread.
struct MacTrayState {
  status_item: Retained<NSStatusItem>,
  /// `NSMenuItem.target` is not retained; keep the target alive as long as
  /// the tray is installed.
  _target: Retained<MenuTarget>,
}

// SAFETY: `status_item` is only accessed on the main thread (enforced by
// `MainThreadMarker::new()` in every method). See the module docs.
unsafe impl Send for MacTrayState {}
unsafe impl Sync for MacTrayState {}

pub(crate) struct MacTrayBackend {
  state: Mutex<Option<MacTrayState>>,
  sender: Sender<TrayEvent>,
}

pub(crate) fn create() -> Result<(Box<dyn PlatformTray>, Receiver<TrayEvent>)> {
  let (sender, receiver) = crossbeam_channel::unbounded();
  Ok((
    Box::new(MacTrayBackend {
      state: Mutex::new(None),
      sender,
    }),
    receiver,
  ))
}

impl MacTrayBackend {
  /// Runs `f` on the main thread with access to the current tray state.
  fn with_main_thread<R>(
    &self, f: impl FnOnce(&mut Option<MacTrayState>, MainThreadMarker) -> R,
  ) -> Result<R> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
      Error::Backend(BackendError::platform(
        "main thread",
        "AppKit requires the main thread",
      ))
    })?;
    let mut guard = self
      .state
      .lock()
      .unwrap_or_else(|poisoned| poisoned.into_inner());
    Ok(f(&mut guard, mtm))
  }
}

/// Applies the icon, title and tooltip to the status item's button (the
/// `NSStatusItem`-level setters are deprecated in favour of the button's).
fn apply_button(status_item: &NSStatusItem, tray: &Tray, mtm: MainThreadMarker) {
  let Some(button) = status_item.button(mtm) else {
    return;
  };
  if let Some(title) = tray.title.as_ref() {
    button.setTitle(&NSString::from_str(title.as_str()));
  }
  if let Some(tooltip) = tray.tooltip.as_ref() {
    button.setToolTip(Some(&NSString::from_str(tooltip.as_str())));
  }
  match tray.icon_bytes.as_deref().filter(|bytes| !bytes.is_empty()) {
    Some(bytes) => {
      // SAFETY: `bytes` is a valid slice for the duration of the call; the
      // data is copied into an `NSData`.
      let data = unsafe { NSData::dataWithBytes_length(bytes.as_ptr().cast(), bytes.len()) };
      if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
        // Template images adapt to the light/dark menu bar.
        image.setTemplate(true);
        button.setImage(Some(&image));
      }
    }
    None => button.setImage(None),
  }
}

fn build_menu(
  items: &[TrayMenuItem], target: &MenuTarget, mtm: MainThreadMarker,
) -> Retained<NSMenu> {
  let menu = NSMenu::new(mtm);
  // The backend controls item enabled-state explicitly.
  menu.setAutoenablesItems(false);
  for item in items {
    match item {
      TrayMenuItem::Separator => {
        menu.addItem(&NSMenuItem::separatorItem(mtm));
      }
      TrayMenuItem::Action { id, label, enabled } => {
        let menu_item = NSMenuItem::new(mtm);
        menu_item.setTitle(&NSString::from_str(label));
        menu_item.setEnabled(*enabled);
        // `representedObject` is a strong property; the id string is retained
        // by the item and read back in `menuItemClicked:`.
        let id_object = NSString::from_str(id);
        // SAFETY: target/action are valid for the menu item's lifetime (the
        // target is retained by the tray state).
        unsafe {
          menu_item.setRepresentedObject(Some(&id_object));
          menu_item.setTarget(Some(target));
          menu_item.setAction(Some(sel!(menuItemClicked:)));
        }
        menu.addItem(&menu_item);
      }
      TrayMenuItem::Submenu { label, items } => {
        let menu_item = NSMenuItem::new(mtm);
        menu_item.setTitle(&NSString::from_str(label));
        menu_item.setSubmenu(Some(&build_menu(items, target, mtm)));
        menu.addItem(&menu_item);
      }
    }
  }
  menu
}

impl PlatformTray for MacTrayBackend {
  fn set_tray(&self, tray: &Tray) -> Result<()> {
    self.with_main_thread(|state, mtm| {
      if let Some(state) = state.as_mut() {
        // Update the existing status item in place.
        state.status_item.setVisible(tray.visible);
        apply_button(&state.status_item, tray, mtm);
        state
          .status_item
          .setMenu(Some(&build_menu(&tray.menu, &state._target, mtm)));
      } else {
        let target = MenuTarget::new(self.sender.clone());
        let bar = NSStatusBar::systemStatusBar();
        // Variable length lets a title show next to the icon.
        let item = bar.statusItemWithLength(NSVariableStatusItemLength);
        apply_button(&item, tray, mtm);
        item.setMenu(Some(&build_menu(&tray.menu, &target, mtm)));
        item.setVisible(tray.visible);
        *state = Some(MacTrayState {
          status_item: item,
          _target: target,
        });
      }
    })
  }

  fn remove_tray(&self) -> Result<()> {
    self.with_main_thread(|state, _mtm| {
      // Dropping the status item removes it from the system status bar.
      *state = None;
    })
  }

  fn shutdown(&self) -> Result<()> {
    self.remove_tray()
  }
}
