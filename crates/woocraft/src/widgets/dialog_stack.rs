//! the imperative dialog stack: one per window, mounted once, hosting every
//! dialog the application opens on the fly.
//!
//! declarative dialogs ([`Dialog`] rendered in place) cover static flows; the
//! stack covers the imperative ones — a confirmation opened from a menu
//! action, a settings modal opened deep in a handler, a second dialog stacked
//! on the first. mount a [`DialogStack`] once as a child of the window's
//! root view, then open and close through [`WindowExt`] on any `&mut Window`:
//!
//! ```rust,ignore
//! use woocraft::{DialogStack, WindowExt as _};
//!
//! // in the root view's constructor:
//! let dialogs = DialogStack::new(cx);
//! // in the root view's render:
//! //   v_flex().child(content).child(dialogs.clone())
//!
//! // anywhere with a window:
//! window.open_dialog(cx, |dialog, _, _| {
//!     dialog
//!         .child(DialogTitle::new().child("delete branch?"))
//!         .on_ok(|_, _, _| true)
//! });
//! window.close_dialog(cx); // closes the topmost
//! ```
//!
//! the stack owns each dialog's open state, focus handle, and layer: the
//! builder configures content and decision callbacks, while `.handle()`,
//! `.open()`, and `.on_open_change()` belong to the stack and are replaced.
//! the builder runs on every render, so keep it cheap and free of entity
//! creation. dialogs mount un-animated: closing unmounts in the same frame.
//!
//! stacking follows the base layering rules — every entry paints above the
//! previous one, only the topmost layer shows the scrim, answers backdrop
//! presses, and receives Escape; closing a layer returns focus to whatever
//! held it before that layer opened.

use std::{collections::HashMap, rc::Rc};

use gpui::{
  AnyElement, AnyWindowHandle, App, AppContext as _, Context, Entity, FocusHandle, Global,
  IntoElement, ParentElement, Render, WeakEntity, WeakFocusHandle, Window, div,
};

use super::{
  alert_dialog::AlertDialog,
  dialog::{Dialog, DialogChangeReason, DialogHandle},
};

/// the modal surface types the stack can host. constructed fresh on every
/// render from the entry's builder, then configured by the stack.
enum StackModalKind {
  Dialog(Dialog),
  Alert(AlertDialog),
}

impl StackModalKind {
  /// applies the stack-owned configuration and renders the modal.
  fn into_configured(
    self, entry: &DialogEntry, layer: usize, topmost: bool, stack: WeakEntity<DialogStack>,
  ) -> AnyElement {
    let id = entry.id;
    let handle = entry.handle.clone();
    let focus = entry.focus.clone();
    let on_open_change =
      move |open: bool, _: DialogChangeReason, window: &mut Window, cx: &mut App| {
        if open {
          return;
        }
        if let Some(stack) = stack.upgrade() {
          stack.update(cx, |stack, cx| stack.remove(id, window, cx));
        }
      };
    match self {
      Self::Dialog(dialog) => dialog
        .open(true)
        .handle(handle)
        .focus_handle(focus)
        .layer(layer, topmost)
        .overlay_visible(topmost)
        .on_open_change(on_open_change)
        .into_any_element(),
      Self::Alert(alert) => alert
        .open(true)
        .handle(handle)
        .focus_handle(focus)
        .layer(layer, topmost)
        .overlay_visible(topmost)
        .on_open_change(on_open_change)
        .into_any_element(),
    }
  }
}

/// builds a stack-hosted modal; runs on every render, so it must stay
/// cheap and free of entity creation.
type StackBuilder = Rc<dyn Fn(&mut Window, &mut App) -> StackModalKind>;

/// one open dialog in the stack.
struct DialogEntry {
  id: usize,
  /// the stack-owned open state; user decisions close through it.
  handle: DialogHandle,
  /// persisted across renders — the base dialog would otherwise create a
  /// fresh focus handle per construction and the trap would lose its anchor.
  focus: FocusHandle,
  /// whatever held focus before this layer opened, restored when it closes.
  previous_focus: Option<WeakFocusHandle>,
  build: StackBuilder,
}

/// a window's stack of imperatively opened dialogs.
///
/// mount the returned entity once as a child of the window's root view; the
/// stack renders its dialogs through the base deferred layer, so the mount
/// position does not constrain z-order. the stack registers itself per
/// window, which is how [`WindowExt`] finds it.
pub struct DialogStack {
  entries: Vec<DialogEntry>,
  next_id: usize,
}

impl DialogStack {
  /// creates the per-window stack entity.
  pub fn new(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self {
      entries: Vec::new(),
      next_id: 0,
    })
  }

  /// the number of dialogs currently open.
  pub fn len(&self) -> usize {
    self.entries.len()
  }

  /// whether no dialog is open.
  pub fn is_empty(&self) -> bool {
    self.entries.is_empty()
  }

  /// opens a dialog on top of the stack.
  ///
  /// the builder receives a default dialog and configures it; it runs on
  /// every render, so keep it cheap and free of entity creation. the stack
  /// owns the open state, the focus handle, and the open-change channel —
  /// build content and decision callbacks, nothing else.
  pub fn open_dialog(
    &mut self, build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
    window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.open(
      Rc::new(move |window, cx| StackModalKind::Dialog(build(Dialog::new(cx), window, cx))),
      window,
      cx,
    );
  }

  /// opens an alert dialog on top of the stack — the same flow as
  /// [`DialogStack::open_dialog`] with the alert semantics pinned.
  pub fn open_alert_dialog(
    &mut self, build: impl Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static,
    window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.open(
      Rc::new(move |window, cx| StackModalKind::Alert(build(AlertDialog::new(cx), window, cx))),
      window,
      cx,
    );
  }

  fn open(&mut self, build: StackBuilder, window: &mut Window, cx: &mut Context<Self>) {
    let previous_focus = window.focused(cx).map(|handle| handle.downgrade());
    let focus = cx.focus_handle();
    focus.focus(window, cx);

    let id = self.next_id;
    self.next_id += 1;
    self.entries.push(DialogEntry {
      id,
      handle: DialogHandle::new(true),
      focus,
      previous_focus,
      build,
    });
    cx.notify();
  }

  /// closes the topmost dialog, returning focus to whatever held it before
  /// the layer opened. decision callbacks do not run: imperative removal is
  /// not a user decision.
  pub fn close_top(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(entry) = self.entries.pop() {
      if let Some(previous) = entry.previous_focus.and_then(|handle| handle.upgrade()) {
        window.focus(&previous, cx);
      }
      cx.notify();
    }
  }

  /// closes every open dialog, returning focus to whatever held it before
  /// the bottom-most layer opened.
  pub fn close_all(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let previous_focus = self
      .entries
      .first()
      .and_then(|entry| entry.previous_focus.clone())
      .and_then(|handle| handle.upgrade());
    self.entries.clear();
    if let Some(previous) = previous_focus {
      window.focus(&previous, cx);
    }
    cx.notify();
  }

  /// removes a layer after its own decision flow closed it (escape, a
  /// backdrop press, the close part). focus returns to the previous holder
  /// only when the topmost layer went away.
  fn remove(&mut self, id: usize, window: &mut Window, cx: &mut Context<Self>) {
    let Some(position) = self.entries.iter().position(|entry| entry.id == id) else {
      return;
    };
    let entry = self.entries.remove(position);
    if position == self.entries.len()
      && let Some(previous) = entry.previous_focus.and_then(|handle| handle.upgrade())
    {
      window.focus(&previous, cx);
    }
    cx.notify();
  }
}

/// per-window registry of mounted stacks, so [`WindowExt`] can find the
/// stack from any handler without threading the entity through.
#[derive(Default)]
struct DialogStackRegistry {
  stacks: HashMap<AnyWindowHandle, WeakEntity<DialogStack>>,
}

impl Global for DialogStackRegistry {}

fn lookup_stack(window: &Window, cx: &App) -> Option<Entity<DialogStack>> {
  cx.try_global::<DialogStackRegistry>()?
    .stacks
    .get(&window.window_handle())
    .and_then(WeakEntity::upgrade)
}

impl Render for DialogStack {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    if !cx.has_global::<DialogStackRegistry>() {
      cx.set_global(DialogStackRegistry::default());
    }
    let weak = cx.weak_entity();
    cx.global_mut::<DialogStackRegistry>()
      .stacks
      .insert(window.window_handle(), weak);

    let len = self.entries.len();
    let stack = cx.weak_entity();
    div().children(self.entries.iter().enumerate().map(|(layer, entry)| {
      (entry.build)(window, cx).into_configured(entry, layer, layer + 1 == len, stack.clone())
    }))
  }
}

/// imperative dialog management on any window. requires a mounted
/// [`DialogStack`]; without one the calls log an error and do nothing.
pub trait WindowExt {
  /// opens a dialog on top of the window's stack.
  fn open_dialog(
    &mut self, cx: &mut App, build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
  );

  /// opens an alert dialog on top of the window's stack.
  fn open_alert_dialog(
    &mut self, cx: &mut App,
    build: impl Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static,
  );

  /// closes the topmost dialog, if any.
  fn close_dialog(&mut self, cx: &mut App);

  /// closes every open dialog.
  fn close_all_dialogs(&mut self, cx: &mut App);
}

impl WindowExt for Window {
  fn open_dialog(
    &mut self, cx: &mut App, build: impl Fn(Dialog, &mut Window, &mut App) -> Dialog + 'static,
  ) {
    match lookup_stack(self, cx) {
      Some(stack) => stack.update(cx, |stack, cx| stack.open_dialog(build, self, cx)),
      None => tracing::error!(
        "no DialogStack is mounted in this window; add DialogStack::new(cx) as a child of the \
         window's root view"
      ),
    }
  }

  fn open_alert_dialog(
    &mut self, cx: &mut App,
    build: impl Fn(AlertDialog, &mut Window, &mut App) -> AlertDialog + 'static,
  ) {
    match lookup_stack(self, cx) {
      Some(stack) => stack.update(cx, |stack, cx| stack.open_alert_dialog(build, self, cx)),
      None => tracing::error!(
        "no DialogStack is mounted in this window; add DialogStack::new(cx) as a child of the \
         window's root view"
      ),
    }
  }

  fn close_dialog(&mut self, cx: &mut App) {
    match lookup_stack(self, cx) {
      Some(stack) => stack.update(cx, |stack, cx| stack.close_top(self, cx)),
      None => tracing::error!("no DialogStack is mounted in this window"),
    }
  }

  fn close_all_dialogs(&mut self, cx: &mut App) {
    match lookup_stack(self, cx) {
      Some(stack) => stack.update(cx, |stack, cx| stack.close_all(self, cx)),
      None => tracing::error!("no DialogStack is mounted in this window"),
    }
  }
}

#[cfg(test)]
mod tests {
  use gpui::{
    Context, Entity, FocusHandle, InteractiveElement as _, ParentElement, Render, Styled, div, px,
  };

  use super::{DialogStack, WindowExt as _};
  use crate::widgets::dialog::DialogTitle;

  struct Host {
    dialogs: Entity<DialogStack>,
    focus: FocusHandle,
  }

  impl Render for Host {
    fn render(&mut self, _: &mut gpui::Window, _: &mut Context<Self>) -> impl gpui::IntoElement {
      div()
        .size_full()
        .child(
          div()
            .id("host-content")
            .track_focus(&self.focus)
            .size(px(40.)),
        )
        .child(self.dialogs.clone())
    }
  }

  fn window_with_stack(
    cx: &mut gpui::TestAppContext,
  ) -> (Entity<Host>, &mut gpui::VisualTestContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (view, cx) = cx.add_window_view(|_, cx| Host {
      dialogs: DialogStack::new(cx),
      focus: cx.focus_handle(),
    });
    (view, cx)
  }

  #[gpui::test]
  fn opened_dialog_renders_and_close_removes_it(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = window_with_stack(cx);
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("woocraft-dialog-popup-0").is_none());

    cx.update(|window, cx| {
      window.open_dialog(cx, |dialog, _, _| {
        dialog.child(DialogTitle::new().child("stacked"))
      });
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
      cx.debug_bounds("woocraft-dialog-popup-0").is_some(),
      "the stack renders the opened dialog"
    );

    cx.update(|window, cx| window.close_dialog(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
      cx.debug_bounds("woocraft-dialog-popup-0").is_none(),
      "closing the topmost dialog unmounts it"
    );
  }

  #[gpui::test]
  fn stacked_dialogs_layer_in_open_order(cx: &mut gpui::TestAppContext) {
    let (_view, cx) = window_with_stack(cx);
    cx.update(|window, cx| {
      window.open_dialog(cx, |dialog, _, _| dialog.child("first"));
      window.open_alert_dialog(cx, |alert, _, _| alert.child("second"));
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("woocraft-dialog-popup-0").is_some());
    assert!(cx.debug_bounds("woocraft-dialog-popup-1").is_some());

    cx.update(|window, cx| window.close_all_dialogs(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(cx.debug_bounds("woocraft-dialog-popup-0").is_none());
    assert!(cx.debug_bounds("woocraft-dialog-popup-1").is_none());
  }

  #[gpui::test]
  fn closing_returns_focus_to_the_previous_holder(cx: &mut gpui::TestAppContext) {
    let (view, cx) = window_with_stack(cx);
    let host_focus = cx.update(|_, cx| view.read(cx).focus.clone());
    cx.update(|window, cx| {
      host_focus.focus(window, cx);
      window.draw(cx).clear(cx);
    });
    cx.update(|window, _| assert!(host_focus.is_focused(window)));

    cx.update(|window, cx| {
      window.open_dialog(cx, |dialog, _, _| dialog.child("stacked"));
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, _| {
      assert!(
        !host_focus.is_focused(window),
        "the opened dialog takes focus"
      )
    });

    cx.update(|window, cx| window.close_dialog(cx));
    cx.update(|window, _| {
      assert!(
        host_focus.is_focused(window),
        "closing returns focus to the previous holder"
      )
    });
  }
}
