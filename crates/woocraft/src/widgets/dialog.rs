//! Modal dialog component with backdrop overlay.
//!
//! Provides a centered dialog window that renders above all other content with
//! a semi-transparent backdrop. Supports two interaction modes:
//!
//! - **Light** (`DialogMode::Light`): clicking outside the dialog dismisses it
//!   (similar to how `Popover` and `Menu` work with `on_mouse_down_out`).
//! - **Modal** (`DialogMode::Modal`): the backdrop is fully opaque to input;
//!   the dialog remains until the user explicitly closes it (e.g. via a close
//!   button or the Escape key when `closable_by_escape` is enabled).
//!
//! The backdrop adapts to both server-decorated windows (no shadow padding) and
//! client-decorated windows with optional shadow insets and rounded corners,
//! including tiling/maximised states where specific edges lose their rounding.
//!
//! # Usage
//!
//! ```rust,ignore
//! // Render a dialog from a view's render method:
//! Dialog::new("my-dialog")
//!     .mode(DialogMode::Modal)
//!     .width(px(480.))
//!     .height(px(320.))
//!     .title("Confirm Action")
//!     .open(self.dialog_open)
//!     .on_close(cx.listener(|this, _, _, cx| {
//!         this.dialog_open = false;
//!         cx.notify();
//!     }))
//!     .content(|_, _, _| {
//!         div().child("Dialog body goes here")
//!     })
//! ```

use std::rc::Rc;

use gpui::{
  AnyElement, App, BoxShadow, Corners, Decorations, DismissEvent, ElementId, EventEmitter,
  FocusHandle, Focusable, Hsla, InteractiveElement as _, IntoElement, MouseButton, ParentElement,
  Pixels, Render, RenderOnce, SharedString, StyleRefinement, Styled, Subscription, Window,
  anchored, deferred, div, point, prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, CardStyle, ColorExt, Icon, IconLabel, IconName, Sizable,
  Size, StyleSized,
  actions::{Cancel, DIALOG_CONTEXT},
  h_flex, v_flex,
  widgets::window_border::WINDOW_SHADOW_SIZE,
  window_paddings,
};

// ─── Interaction mode ───────────────────────────────────────────────────────

/// Controls how the dialog backdrop interacts with user input.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum DialogMode {
  /// Clicking outside the dialog panel dismisses it (light / non-blocking).
  #[default]
  Light,
  /// The backdrop blocks all input to the underlying window until the dialog
  /// is explicitly closed.  The dialog cannot be dismissed by clicking outside.
  Modal,
}

// ─── Builder callbacks ──────────────────────────────────────────────────────

type DialogContentBuilder = Rc<
  dyn Fn(&mut DialogState, &mut Window, &mut gpui::Context<DialogState>) -> AnyElement + 'static,
>;

type DialogCloseHandler = Rc<dyn Fn(&mut Window, &mut App) + 'static>;

// ─── DialogState ────────────────────────────────────────────────────────────

/// Retained state for a [`Dialog`] element.
///
/// Stored inside the window's keyed-state map (one entry per `ElementId`) so
/// that open/closed state survives across re-renders.
pub struct DialogState {
  focus_handle: FocusHandle,
  open: bool,
  dismiss_subscription: Option<Subscription>,
}

impl DialogState {
  pub fn new(default_open: bool, cx: &mut App) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      open: default_open,
      dismiss_subscription: None,
    }
  }

  /// Returns `true` if the dialog is currently visible.
  pub fn is_open(&self) -> bool {
    self.open
  }

  /// Imperatively open the dialog and move focus into it.
  pub fn show(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
    if !self.open {
      self.set_open(true, window, cx);
    }
  }

  /// Imperatively close the dialog.
  pub fn dismiss(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
    if self.open {
      self.set_open(false, window, cx);
    }
  }

  fn on_action_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut gpui::Context<Self>) {
    self.dismiss(window, cx);
  }

  fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut gpui::Context<Self>) {
    self.open = open;

    if open {
      self.focus_handle.focus(window);

      // Subscribe to `DismissEvent` so external code can close us via the
      // event bus (same pattern as `PopoverState`).
      let entity = cx.entity().clone();
      let entity_for_cb = entity.clone();
      self.dismiss_subscription =
        Some(
          window.subscribe(&entity, cx, move |_, _: &DismissEvent, window, cx| {
            entity_for_cb.update(cx, |state, cx| {
              state.dismiss(window, cx);
            });
            window.refresh();
          }),
        );
    } else {
      self.dismiss_subscription = None;
    }

    cx.notify();
  }
}

impl Focusable for DialogState {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for DialogState {
  fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
    div()
  }
}

impl EventEmitter<DismissEvent> for DialogState {}

// ─── Dialog element ─────────────────────────────────────────────────────────

/// A centered modal/non-modal dialog that renders above all other content.
///
/// See the [module-level documentation](self) for usage examples.
#[derive(IntoElement)]
pub struct Dialog {
  id: ElementId,
  style: StyleRefinement,
  mode: DialogMode,
  default_open: bool,
  /// Externally-controlled open state.  When `Some`, overrides the stored
  /// value every render cycle.
  open: Option<bool>,
  /// Dialog panel width.  When `None` the panel uses auto-sizing.
  width: Option<Pixels>,
  /// Dialog panel height.  When `None` the panel uses auto-sizing.
  height: Option<Pixels>,
  title: Option<SharedString>,
  show_close_button: bool,
  /// Whether pressing Escape closes the dialog.
  closable_by_escape: bool,
  content: Option<DialogContentBuilder>,
  children: Vec<AnyElement>,
  on_close: Option<DialogCloseHandler>,
  size: Size,
}

impl Dialog {
  /// Create a new dialog with the given element id.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      mode: DialogMode::default(),
      default_open: false,
      open: None,
      width: None,
      height: None,
      title: None,
      show_close_button: true,
      closable_by_escape: true,
      content: None,
      children: vec![],
      on_close: None,
      size: Size::Medium,
    }
  }

  /// Set the interaction mode (Light = click-outside dismisses, Modal =
  /// explicit close only).
  pub fn mode(mut self, mode: DialogMode) -> Self {
    self.mode = mode;
    self
  }

  /// Set default open state used when the state is first created.
  pub fn default_open(mut self, open: bool) -> Self {
    self.default_open = open;
    self
  }

  /// Override the open state every render cycle.
  pub fn open(mut self, open: bool) -> Self {
    self.open = Some(open);
    self
  }

  /// Set the dialog panel width.
  pub fn width(mut self, width: Pixels) -> Self {
    self.width = Some(width);
    self
  }

  /// Set the dialog panel height.
  pub fn height(mut self, height: Pixels) -> Self {
    self.height = Some(height);
    self
  }

  /// Set the dialog title shown in the header bar.
  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  /// Show or hide the built-in close button in the header.
  pub fn show_close_button(mut self, show: bool) -> Self {
    self.show_close_button = show;
    self
  }

  /// Whether pressing Escape while the dialog is focused closes it.
  ///
  /// Defaults to `true`.  For fully-blocking workflows you may want to set
  /// this to `false`.
  pub fn closable_by_escape(mut self, closable: bool) -> Self {
    self.closable_by_escape = closable;
    self
  }

  /// Register a callback invoked whenever the dialog requests to be closed
  /// (close button, Escape key, or backdrop click in Light mode).
  ///
  /// The callback is responsible for updating external open state, e.g.
  /// `this.dialog_open = false; cx.notify();`.
  pub fn on_close<F>(mut self, callback: F) -> Self
  where
    F: Fn(&mut Window, &mut App) + 'static, {
    self.on_close = Some(Rc::new(callback));
    self
  }

  /// Provide the body content of the dialog.
  pub fn content<F, E>(mut self, content: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut DialogState, &mut Window, &mut gpui::Context<DialogState>) -> E + 'static, {
    self.content = Some(Rc::new(move |state, window, cx| {
      content(state, window, cx).into_any_element()
    }));
    self
  }
}

impl_parent_element!(Dialog);
impl_sizable!(Dialog);
impl_styled!(Dialog);

// ─── Rendering ──────────────────────────────────────────────────────────────

impl RenderOnce for Dialog {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // ── Resolve / initialise per-element state ──────────────────────────
    let default_open = self.default_open;
    let force_open = self.open;

    let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
      DialogState::new(default_open, cx)
    });

    state.update(cx, |state, _| {
      if let Some(forced) = force_open {
        state.open = forced;
      }
    });

    let open = state.read(cx).open;
    let focus_handle = state.read(cx).focus_handle.clone();

    // Nothing to render when closed — return a zero-size placeholder.
    if !open {
      return div().id(self.id).size_0().into_any_element();
    }

    // ── Window geometry for correct overlay sizing ───────────────────────
    //
    // Use the current window bounds as the overlay reference rect.
    let decorations = window.window_decorations();
    let paddings = window_paddings(window);
    let window_size = window.bounds().size;
    let backdrop_width = (window_size.width - paddings.left - paddings.right).max(px(0.));
    let backdrop_height = (window_size.height - paddings.top - paddings.bottom).max(px(0.));
    let (backdrop_x, backdrop_y) = match decorations {
      Decorations::Server => (px(0.), px(0.)),
      Decorations::Client { tiling } => (
        if tiling.left {
          -WINDOW_SHADOW_SIZE
        } else {
          px(0.)
        },
        if tiling.top {
          -WINDOW_SHADOW_SIZE
        } else {
          px(0.)
        },
      ),
    };

    // Compute the corner radii for the overlay so that it tracks the
    // rounded window corners regardless of decoration mode or tiling state.
    let border_radius = cx.theme().radius_container;
    let overlay_corners: Corners<Pixels> = match decorations {
      Decorations::Server => Corners::all(px(0.)),
      Decorations::Client { tiling } => Corners {
        top_left: if tiling.top || tiling.left {
          px(0.)
        } else {
          border_radius
        },
        top_right: if tiling.top || tiling.right {
          px(0.)
        } else {
          border_radius
        },
        bottom_left: if tiling.bottom || tiling.left {
          px(0.)
        } else {
          border_radius
        },
        bottom_right: if tiling.bottom || tiling.right {
          px(0.)
        } else {
          border_radius
        },
      },
    };

    // ── Build the close-request handler ─────────────────────────────────
    let on_close_from_state = {
      let state = state.clone();
      let on_close = self.on_close.clone();
      Rc::new(move |window: &mut Window, cx: &mut App| {
        state.update(cx, |state, cx| {
          state.dismiss(window, cx);
        });
        if let Some(cb) = on_close.as_ref() {
          cb(window, cx);
        }
      }) as Rc<dyn Fn(&mut Window, &mut App)>
    };

    // ── Dialog panel ────────────────────────────────────────────────────
    let has_title = self.title.is_some();
    let show_close_button = self.show_close_button;
    let closable_by_escape = self.closable_by_escape;
    let size = self.size;
    let title = self.title.clone();
    let width = self.width;
    let height = self.height;
    let content_builder = self.content;
    let extra_children = self.children;
    let theme_border = cx.theme().border;

    let on_close_btn = on_close_from_state.clone();

    // Build the header div first (needs `h_flex()` return type stability).
    let header_el: Option<gpui::AnyElement> = if has_title || show_close_button {
      let on_close_btn2 = on_close_btn.clone();
      Some(
        h_flex()
          .id("dialog-header")
          .w_full()
          .items_center()
          .justify_between()
          .container_padding(size)
          .border_b_1()
          .border_color(theme_border)
          .when_some(title, |this, t| {
            this.child(
              IconLabel::new("dialog-title")
                .with_size(size)
                .icon(Icon::new(IconName::Info))
                .label(t),
            )
          })
          .when(!has_title, |this: gpui::Stateful<gpui::Div>| {
            this.child(div().flex_1())
          })
          .when(show_close_button, |this: gpui::Stateful<gpui::Div>| {
            this.child(
              Button::new("dialog-close-btn")
                .flat()
                .icon(Icon::new(IconName::Dismiss))
                .with_size(size)
                .on_click(move |_, window, cx| on_close_btn2(window, cx)),
            )
          })
          .into_any_element(),
      )
    } else {
      None
    };

    // Build the body.
    let body_el = v_flex()
      .id("dialog-body")
      .w_full()
      .flex_1()
      .container_padding(size)
      .container_gap(size)
      .when_some(content_builder, |this, content| {
        this.child(state.update(cx, |state, cx| (content)(state, window, cx)))
      })
      .children(extra_children);

    let panel = v_flex()
      .id("dialog-panel")
      .occlude() // blocks mouse events from passing through
      .tab_group()
      .track_focus(&focus_handle)
      .key_context(DIALOG_CONTEXT)
      .when(closable_by_escape, |this| {
        this.on_action(window.listener_for(&state, DialogState::on_action_cancel))
      })
      .card_style(cx.theme())
      // Panel size — apply only when set
      .when_some(width, |this: gpui::Stateful<gpui::Div>, w| this.w(w))
      .when_some(height, |this: gpui::Stateful<gpui::Div>, h| this.h(h))
      // Elevated shadow
      .shadow(vec![BoxShadow {
        color: Hsla {
          h: 0.,
          s: 0.,
          l: 0.,
          a: 0.25,
        },
        blur_radius: px(24.),
        spread_radius: px(0.),
        offset: point(px(0.), px(4.)),
      }])
      .when_some(header_el, |this, hdr| this.child(hdr))
      .child(body_el)
      // Stop click events from bubbling to the backdrop.
      .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
      .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation());

    // ── Backdrop ────────────────────────────────────────────────────────
    let mode = self.mode;
    let backdrop = div()
      .id("dialog-backdrop")
      // Fill the entire window overlay space.
      .absolute()
      .top(backdrop_y)
      .left(backdrop_x)
      .w(backdrop_width)
      .h(backdrop_height)
      // Rounded corners matching the window frame.
      .rounded_tl(overlay_corners.top_left)
      .rounded_tr(overlay_corners.top_right)
      .rounded_bl(overlay_corners.bottom_left)
      .rounded_br(overlay_corners.bottom_right)
      // Semi-transparent dark scrim.
      .bg(cx.theme().background.opacity(0.35))
      // Always block pointer events reaching content underneath.
      .occlude()
      // Center the dialog panel.
      .flex()
      .items_center()
      .justify_center()
      .child(panel)
      // In Light mode: click outside the panel dismisses the dialog.
      .when(
        mode == DialogMode::Light,
        |this: gpui::Stateful<gpui::Div>| {
          let cb = on_close_from_state.clone();
          this.on_mouse_down(MouseButton::Left, move |_, window, cx| cb(window, cx))
        },
      );

    // `deferred` at priority 2 paints above popovers (priority 1).
    // Use an anchored root so the overlay is positioned in window coordinates
    // instead of inheriting the host element's local layout offset.
    let overlay = deferred(anchored().child(backdrop)).with_priority(2);

    // Zero-size host element — doesn't affect layout but carries the overlay.
    div().id(self.id).size_0().child(overlay).into_any_element()
  }
}
