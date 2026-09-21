//! modal dialog with themed backdrop, surface, and parts.
//!
//! this is a styled wrapper over [`gpui_base::Dialog`], which owns the full
//! behavior surface: focus trapping, Escape/confirm keyboard actions,
//! backdrop dismissal, veto-able decision callbacks, imperative
//! open/close through a [`DialogHandle`], and reason-tracked open-change
//! notifications. the wrapper contributes the design system's defaults — a
//! scrim inset by the window's client-decoration margins, a centered card
//! surface with a bounded width and a scrolling body, and themed
//! [`DialogTitle`]/[`DialogDescription`]/[`DialogClose`] parts — while every
//! behavioral setter forwards one-to-one to base. mount-time animation stays
//! with the application: the dialog unmounts with its `open` state, so there
//! is no persistent node to sample a fade from.
//!
//! ```rust,ignore
//! use woocraft::{Button, Dialog, DialogHandle, DialogTitle, DialogDescription};
//!
//! let handle = DialogHandle::new(false);
//! Dialog::new(cx)
//!     .handle(handle)
//!     .child(DialogTitle::new().child("delete project?"))
//!     .child(DialogDescription::new().child("this cannot be undone"))
//!     .child(Button::new("confirm").label("delete"));
//! ```

use gpui::{
  Action, AnyElement, App, ClickEvent, FocusHandle, InteractiveElement as _, IntoElement,
  ParentElement, Pixels, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled,
  Window, WindowControlArea, black, div, prelude::FluentBuilder as _, px, rems,
};
use gpui_base::{Dialog as BaseDialog, StyledExt as _, box_shadow};
pub use gpui_base::{DialogChangeReason, DialogHandle, DialogTrigger};

use super::{title_bar::TITLE_BAR_HEIGHT, window_border::window_paddings};
use crate::{
  icon::{Icon, IconName},
  theme::{ActiveTheme, with_alpha},
  v_flex,
  widgets::{
    button::{Button, ButtonVariants as _},
    divider::Divider,
    icon_label::IconLabel,
  },
};

/// the shared titlebar chrome of modal surfaces: `0.25rem` padding and
/// gap, the leading content on the left, trailing controls right-aligned.
/// a layout block, not a container — it owns its padding and expects the
/// parent stack to add none.
pub(crate) fn title_row(
  left: impl IntoElement, actions: Vec<AnyElement>, weight: gpui::FontWeight,
) -> impl IntoElement {
  div()
    .flex()
    .flex_row()
    .w_full()
    .items_center()
    .p(rems(0.25))
    .gap(rems(0.25))
    .child(
      div()
        .flex_1()
        .min_w_0()
        .font_weight(weight)
        .child(left.into_any_element()),
    )
    .when(!actions.is_empty(), |this| {
      this.child(
        div()
          .flex()
          .flex_row()
          .flex_shrink_0()
          .items_center()
          .gap(rems(0.25))
          .children(actions),
      )
    })
}

/// scrim applied between the viewport content and a modal surface.
///
/// every modal layer shares one wash so stacked modality reads as one depth
/// step, not two.
pub(crate) fn scrim() -> gpui::Hsla {
  with_alpha(black(), 0.4)
}

/// dispatches a dialog action as though the control's own focus node held
/// focus, so the enclosing dialog receives it whatever actually holds focus
/// at that moment — a native web view or an always-on-top window would
/// otherwise leave the control inert. this mirrors the anchor trick inside
/// `gpui_base::DialogClose`, adapted so the visible control stays a themed
/// [`Button`] with working keyboard activation.
#[derive(Clone)]
pub(crate) struct DispatchAnchor {
  handle: FocusHandle,
}

impl DispatchAnchor {
  pub(crate) fn new(cx: &mut App) -> Self {
    // a fresh handle per anchor instance: stacked dialogs each carry their
    // own, so a dispatch never resolves into a sibling dialog's path.
    Self {
      handle: cx.focus_handle(),
    }
  }

  /// a zero-size, out-of-flow node that sits inside the dialog's dispatch
  /// path without entering the tab order or taking pointer hits.
  pub(crate) fn element(&self) -> impl IntoElement {
    div().absolute().size_0().track_focus(&self.handle)
  }

  pub(crate) fn dispatch(&self, action: &dyn Action, window: &mut Window, cx: &mut App) {
    self.handle.dispatch_action(action, window, cx);
  }
}

/// geometry of the centered modal card, in window coordinates.
pub(crate) struct ModalGeometry {
  pub(crate) left: Pixels,
  pub(crate) top: Pixels,
  pub(crate) width: Pixels,
  pub(crate) max_height: Pixels,
}

/// resolves the card box for a modal surface: centered horizontally inside
/// the window's content area (the viewport minus client-decoration margins),
/// a tenth of the content height from the top unless overridden, never wider
/// than the area minus a `1rem` margin on each side, and never taller than
/// the space left below.
pub(crate) fn modal_geometry(
  window: &Window, width: Option<Pixels>, margin_top: Option<Pixels>,
) -> ModalGeometry {
  let paddings = window_paddings(window);
  let viewport = window.viewport_size();
  let area_w = viewport.width - paddings.left - paddings.right;
  let area_h = viewport.height - paddings.top - paddings.bottom;
  let margin = rems(1.).to_pixels(window.rem_size());
  let top = margin_top.unwrap_or(area_h / 10.);
  let width = width
    .unwrap_or_else(|| rems(28.).to_pixels(window.rem_size()))
    .min((area_w - margin * 2.).max(px(0.)));
  ModalGeometry {
    left: paddings.left + (area_w - width) / 2.,
    top: paddings.top + top,
    width,
    max_height: (area_h - top - margin).max(px(0.)),
  }
}

/// the themed modal card around dialog content: themed surface tokens,
/// absolute placement from [`modal_geometry`], and a body that scrolls
/// instead of outgrowing the window.
pub(crate) fn modal_card(
  geometry: &ModalGeometry, max_width: Option<Pixels>, style: &StyleRefinement, key: usize,
  children: Vec<AnyElement>, window: &mut Window, cx: &mut App,
) -> impl IntoElement {
  let (font_family, text_size, card, card_foreground, border_color, border_width, radius) = {
    let theme = cx.theme();
    (
      theme.font_family.clone(),
      theme.font_size,
      theme.card,
      theme.card_foreground,
      theme.border,
      theme.border_width,
      theme.radius_lg,
    )
  };
  let shadow_blur = rems(1.).to_pixels(window.rem_size());
  let shadow_offset = rems(0.5).to_pixels(window.rem_size());

  v_flex()
    .debug_selector(|| format!("woocraft-dialog-popup-{key}"))
    .absolute()
    .occlude()
    .left(geometry.left)
    .top(geometry.top)
    .w(geometry.width)
    .max_h(geometry.max_height)
    .when_some(max_width, |this, max_width| this.max_w(max_width))
    .overflow_hidden()
    .font_family(font_family)
    .text_size(text_size)
    .text_color(card_foreground)
    .bg(card)
    .rounded(radius)
    .border(border_width)
    .border_color(border_color)
    .shadow(vec![box_shadow(
      px(0.),
      shadow_offset,
      shadow_blur,
      px(0.),
      with_alpha(black(), 0.2),
    )])
    .refine_style(style)
    .child(
      // the body stack holds no padding and no gap: title, content, and
      // action blocks each own their 0.25rem chrome. scrolling needs a
      // stateful element; the layer key keeps every stacked dialog's
      // scroll state distinct.
      div()
        .id(("woocraft-dialog-body", key))
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .children(children),
    )
}

/// dialog element styled by the woocraft design system.
#[derive(IntoElement)]
pub struct Dialog {
  base: BaseDialog,
  backdrop: Option<AnyElement>,
  popup: Option<AnyElement>,
  width: Option<Pixels>,
  max_width: Option<Pixels>,
  margin_top: Option<Pixels>,
  dismiss_below_y: Option<Pixels>,
  layer_ix: usize,
  overlay_visible: bool,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Dialog {
  /// creates a closed dialog. it renders nothing until opened through
  /// `.open(true)` or a [`DialogHandle`].
  pub fn new(cx: &mut App) -> Self {
    Self {
      base: BaseDialog::new(cx).open(false),
      backdrop: None,
      popup: None,
      width: None,
      max_width: None,
      margin_top: None,
      dismiss_below_y: None,
      layer_ix: 0,
      overlay_visible: true,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// pins the open state.
  pub fn open(mut self, open: bool) -> Self {
    self.base = self.base.open(open);
    self
  }

  /// attaches a handle for imperative open/close and state reads.
  pub fn handle(mut self, handle: DialogHandle) -> Self {
    self.base = self.base.handle(handle);
    self
  }

  /// subscribes to open-state transitions with the change reason.
  pub fn on_open_change(
    mut self, handler: impl Fn(bool, DialogChangeReason, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_open_change(handler);
    self
  }

  /// replaces the themed scrim with a custom backdrop element.
  pub fn backdrop(mut self, element: impl IntoElement) -> Self {
    self.backdrop = Some(element.into_any_element());
    self
  }

  /// replaces the themed card surface with a custom popup element.
  pub fn popup(mut self, element: impl IntoElement) -> Self {
    self.popup = Some(element.into_any_element());
    self
  }

  /// sets the card width; the default is `28rem`, clamped so the card
  /// always leaves a `1rem` margin on each side of the content area.
  pub fn width(mut self, width: impl Into<Pixels>) -> Self {
    self.width = Some(width.into());
    self
  }

  /// caps the card width on top of the viewport clamp.
  pub fn max_w(mut self, max_width: impl Into<Pixels>) -> Self {
    self.max_width = Some(max_width.into());
    self
  }

  /// sets the card's offset from the top of the content area; the default
  /// is a tenth of the content height.
  pub fn margin_top(mut self, margin_top: impl Into<Pixels>) -> Self {
    self.margin_top = Some(margin_top.into());
    self
  }

  /// keeps the dialog open when Escape is pressed.
  pub fn close_on_escape(mut self, value: bool) -> Self {
    self.base = self.base.close_on_escape(value);
    self
  }

  /// keeps the dialog open when the backdrop is pressed.
  pub fn close_on_backdrop_press(mut self, value: bool) -> Self {
    self.base = self.base.close_on_backdrop_press(value);
    self
  }

  /// ignores backdrop presses below `value`. the default is
  /// [`TITLE_BAR_HEIGHT`](crate::widgets::title_bar::TITLE_BAR_HEIGHT), so
  /// the title bar never doubles as a close button; pass `px(0.)` when the
  /// window has no title bar.
  pub fn dismiss_below_y(mut self, value: Pixels) -> Self {
    self.dismiss_below_y = Some(value);
    self
  }

  /// vetoes or allows the confirm decision; the dialog closes on `true`.
  pub fn on_ok(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
  ) -> Self {
    self.base = self.base.on_ok(handler);
    self
  }

  /// vetoes or allows the cancel decision (Escape, backdrop, close part);
  /// the dialog closes on `true`.
  pub fn on_cancel(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) -> bool + 'static,
  ) -> Self {
    self.base = self.base.on_cancel(handler);
    self
  }

  /// runs after every close, whatever the reason.
  pub fn on_close(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_close(handler);
    self
  }

  #[doc(hidden)]
  pub fn layer(mut self, index: usize, topmost: bool) -> Self {
    self.layer_ix = index;
    self.base = self.base.layer(index, topmost);
    self
  }

  /// dims the backdrop; the dialog stack clears this on every layer but the
  /// topmost so stacked modals share one depth step. the backdrop element
  /// stays mounted either way to intercept pointer input.
  #[doc(hidden)]
  pub fn overlay_visible(mut self, visible: bool) -> Self {
    self.overlay_visible = visible;
    self
  }

  #[doc(hidden)]
  pub fn focus_handle(mut self, value: FocusHandle) -> Self {
    self.base = self.base.focus_handle(value);
    self
  }

  #[doc(hidden)]
  pub fn request_close(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
    self.base = self.base.request_close(handler);
    self
  }
}

impl ParentElement for Dialog {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for Dialog {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Dialog {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let geometry = modal_geometry(window, self.width, self.margin_top);
    let dismiss_below_y = self
      .dismiss_below_y
      .unwrap_or_else(|| TITLE_BAR_HEIGHT.to_pixels(window.rem_size()));
    let max_width = self.max_width;
    let style = self.style;

    let mut base = self.base.dismiss_below_y(dismiss_below_y);
    let overlay_visible = self.overlay_visible;
    let backdrop = match self.backdrop {
      Some(element) => element,
      None => {
        let paddings = window_paddings(window);
        let viewport = window.viewport_size();
        // the scrim stops at the client-decoration margins so it never
        // bleeds into the shadow band, and it keeps the window draggable
        // where the title bar sits underneath.
        div()
          .absolute()
          .top(paddings.top)
          .left(paddings.left)
          .w(viewport.width - paddings.left - paddings.right)
          .h(viewport.height - paddings.top - paddings.bottom)
          .window_control_area(WindowControlArea::Drag)
          .when(overlay_visible, |this| this.bg(scrim()))
          .into_any_element()
      }
    };
    base = base.backdrop(backdrop);
    match self.popup {
      Some(popup) => base = base.popup(popup),
      None => {
        let children = self.children;
        base = base.popup(modal_card(
          &geometry,
          max_width,
          &style,
          self.layer_ix,
          children,
          window,
          cx,
        ));
      }
    }
    base
  }
}

/// themed dialog titlebar: the window title bar's contract inside the
/// modal card. an [`IconLabel`] row (default icon [`IconName::AddCircle`],
/// semibold title) on the left, trailing controls right-aligned through
/// [`DialogTitle::action`], and a hairline divider separating the block
/// from the content below. the block owns its `0.25rem` chrome; the parent
/// stack adds none.
#[derive(IntoElement)]
pub struct DialogTitle {
  style: StyleRefinement,
  icon: Option<Icon>,
  actions: Vec<AnyElement>,
  children: Vec<AnyElement>,
}

impl DialogTitle {
  /// creates an empty titlebar.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      icon: None,
      actions: Vec::new(),
      children: Vec::new(),
    }
  }

  /// overrides the leading icon; the default is [`IconName::AddCircle`],
  /// matching the window title bar's fallback.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  /// adds a trailing control, right-aligned in insertion order — typically
  /// the [`DialogClose`] part.
  pub fn action(mut self, action: impl IntoElement) -> Self {
    self.actions.push(action.into_any_element());
    self
  }
}

impl Default for DialogTitle {
  fn default() -> Self {
    Self::new()
  }
}

impl ParentElement for DialogTitle {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for DialogTitle {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for DialogTitle {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let icon = self.icon.unwrap_or_else(|| Icon::new(IconName::AddCircle));
    div()
      .flex()
      .flex_col()
      .w_full()
      .child(title_row(
        IconLabel::new("woocraft-dialog-title")
          .icon(icon)
          .w_full()
          .px(rems(0.))
          .py(rems(0.))
          .children(self.children),
        self.actions,
        gpui::FontWeight::SEMIBOLD,
      ))
      .child(Divider::horizontal())
      .refine_style(&self.style)
  }
}

/// themed dialog body copy rendered in the muted foreground.
#[derive(IntoElement)]
pub struct DialogDescription {
  style: StyleRefinement,
  children: Vec<AnyElement>,
}

impl DialogDescription {
  /// creates an empty description.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      children: Vec::new(),
    }
  }
}

impl Default for DialogDescription {
  fn default() -> Self {
    Self::new()
  }
}

impl ParentElement for DialogDescription {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for DialogDescription {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for DialogDescription {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let (font_family, text_size, muted_foreground) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.muted_foreground,
      )
    };
    div()
      // a pure-text content block: it owns the roomier 0.5rem chrome.
      .p(rems(0.5))
      .font_family(font_family)
      .text_size(text_size)
      .text_color(muted_foreground)
      .children(self.children)
      .refine_style(&self.style)
  }
}

/// themed close control: a flat icon button that dispatches the dialog
/// cancel action through a [`DispatchAnchor`], closing the nearest dialog
/// whatever holds focus at that moment.
#[derive(IntoElement)]
pub struct DialogClose {
  style: StyleRefinement,
}

impl DialogClose {
  /// creates the themed close button.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
    }
  }
}

impl Default for DialogClose {
  fn default() -> Self {
    Self::new()
  }
}

impl Styled for DialogClose {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for DialogClose {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let anchor = DispatchAnchor::new(cx);
    let dispatch = anchor.clone();
    div()
      .child(anchor.element())
      .child(
        Button::new("dialog-close")
          .icon(Icon::new(IconName::Dismiss))
          .flat()
          .on_click(move |_, window, cx| {
            dispatch.dispatch(&gpui_base::actions::Cancel, window, cx)
          }),
      )
      .refine_style(&self.style)
  }
}

#[cfg(test)]
mod tests {
  use gpui::{
    Context, InteractiveElement as _, IntoElement, ParentElement, Render, Styled, div, px,
  };

  use super::{Dialog, DialogDescription, DialogTitle, modal_geometry};

  #[test]
  fn dialog_parts_default_construct() {
    let _title = DialogTitle::default();
    let _description = DialogDescription::default();
  }

  struct Host {
    // outer: whether to mount a dialog at all; inner: an optional pinned
    // open state — `Some(None)` mounts the dialog with defaults.
    dialog: Option<Option<bool>>,
  }

  impl Render for Host {
    fn render(&mut self, _: &mut gpui::Window, cx: &mut Context<Self>) -> impl IntoElement {
      div()
        .debug_selector(|| "host-root".into())
        .size_full()
        .children(self.dialog.map(|open| {
          let dialog = Dialog::new(cx).child("content");
          let dialog = match open {
            Some(open) => dialog.open(open),
            None => dialog,
          };
          dialog.into_any_element()
        }))
    }
  }

  #[gpui::test]
  fn the_dialog_is_closed_until_opened(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, _| Host { dialog: Some(None) });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
      cx.debug_bounds("woocraft-dialog-popup-0").is_none(),
      "a fresh dialog renders nothing"
    );
  }

  #[gpui::test]
  fn the_card_centers_and_bounds_itself(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, _| Host {
      dialog: Some(Some(true)),
    });
    cx.update(|window, cx| window.draw(cx).clear(cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));

    let (viewport, rem) = cx.update(|window, _| (window.viewport_size(), window.rem_size()));
    assert!(
      cx.debug_bounds("host-root").is_some(),
      "the host root paints"
    );
    let bounds = cx
      .debug_bounds("woocraft-dialog-popup-0")
      .expect("an open dialog renders its card");

    let expected_width = gpui::rems(28.).to_pixels(rem);
    assert_eq!(bounds.size.width, expected_width);
    assert_eq!(
      bounds.origin.x,
      (viewport.width - expected_width) / 2.,
      "the card is horizontally centered"
    );
    assert_eq!(
      bounds.origin.y,
      viewport.height / 10.,
      "the card sits a tenth of the content height from the top"
    );
    assert!(
      bounds.size.height <= viewport.height - bounds.origin.y - rem,
      "the card never outgrows the window"
    );
  }

  #[gpui::test]
  fn modal_geometry_clamps_to_the_content_area(cx: &mut gpui::TestAppContext) {
    cx.update(|cx| crate::init(cx).unwrap());
    let (_view, cx) = cx.add_window_view(|_, _| Host { dialog: None });
    cx.update(|window, _| {
      let viewport = window.viewport_size();
      let rem = window.rem_size();
      let margin = gpui::rems(1.).to_pixels(rem);

      // an over-wide request collapses to the content area minus margins.
      let geometry = modal_geometry(window, Some(px(100_000.)), None);
      assert_eq!(geometry.width, viewport.width - margin * 2.);
      assert_eq!(geometry.left, margin);

      // an over-deep margin_top still leaves the card inside the window.
      let geometry = modal_geometry(window, None, Some(viewport.height));
      assert_eq!(geometry.max_height, px(0.));
    });
  }
}
