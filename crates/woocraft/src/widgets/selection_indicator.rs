//! Private indicator rendering shared by [`Checkbox`](crate::Checkbox) and
//! [`Radio`](crate::Radio).
//!
//! Both widgets draw the same control: a boxed indicator, an optional label and
//! children, focus tracking, hover/active feedback and an activation handler.
//! They differ only in the center mark and in the value reported on
//! activation, so both are rendered by [`SelectionIndicator`].

use std::{rc::Rc, time::Duration};

use gpui::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, MouseButton,
  ParentElement, RenderOnce, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{
  ActiveTheme, Easing, Icon, IconName, Sizable, Size, StyleSized, StyledExt, Transition, h_flex,
  transition,
};

pub(super) type SelectionClickHandler = Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>;

/// Selection indicator with its label, shared by [`Checkbox`](crate::Checkbox)
/// and [`Radio`](crate::Radio).
///
/// The caller supplies the state-specific parts: the center mark (a check or
/// dash for a checkbox, none for a radio), the transition duration token and
/// the value handed to `on_click` on activation.
#[derive(IntoElement)]
pub(super) struct SelectionIndicator {
  pub(super) id: ElementId,
  pub(super) style: StyleRefinement,
  pub(super) label: Option<AnyElement>,
  pub(super) children: Vec<AnyElement>,
  /// Whether the indicator is filled with the primary color.
  pub(super) marked: bool,
  /// Mark drawn over the indicator fill; `None` leaves the box empty.
  pub(super) mark: Option<IconName>,
  pub(super) disabled: bool,
  pub(super) size: Size,
  pub(super) tab_stop: bool,
  pub(super) tab_index: isize,
  /// Duration of the fill, border and mark transitions.
  pub(super) toggle_duration: Duration,
  /// State reported to `on_click` when the user activates the indicator.
  pub(super) activation_value: bool,
  pub(super) on_click: Option<SelectionClickHandler>,
}

impl RenderOnce for SelectionIndicator {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let Self {
      id,
      style,
      label,
      children,
      marked,
      mark,
      disabled,
      size,
      tab_stop,
      tab_index,
      toggle_duration,
      activation_value,
      on_click,
    } = self;

    let focus_handle = window
      .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone();

    // Fill and border cross-fade to the primary color and the mark fades in on
    // top, all interruptible and reduced motion aware. Disabled controls
    // resolve without animation.
    let policy = Transition::new(if disabled {
      Duration::ZERO
    } else {
      toggle_duration
    })
    .easing(Easing::EaseOut);
    let fill = transition(
      (id.clone(), "fill"),
      if marked {
        cx.theme().primary
      } else {
        cx.theme().background
      },
      policy.clone(),
      window,
      cx,
    );
    let border = transition(
      (id.clone(), "border"),
      if marked {
        cx.theme().primary
      } else {
        cx.theme().input
      },
      policy.clone(),
      window,
      cx,
    );
    let mark_opacity = mark.map(|mark| {
      let opacity = transition(
        (id.clone(), "check"),
        if marked { 1.0 } else { 0.0 },
        policy,
        window,
        cx,
      );
      (mark, opacity)
    });

    h_flex()
      .id(id)
      .items_center()
      .component_gap(size)
      .text_color(if disabled {
        cx.theme().muted_foreground
      } else {
        cx.theme().foreground
      })
      .child(
        div()
          .flex_none()
          .size(size.component_height() * 0.5)
          .rounded(size.component_radius())
          .border_1()
          .border_color(border)
          .bg(fill)
          .child(
            h_flex()
              .size_full()
              .items_center()
              .justify_center()
              .border(size.em(0.125))
              .rounded(size.component_radius())
              .border_color(cx.theme().background)
              .when_some(mark_opacity, |this, (mark, opacity)| {
                this.child(
                  Icon::new(mark)
                    .with_size(size.smaller())
                    .text_color(cx.theme().background)
                    .opacity(opacity),
                )
              }),
          ),
      )
      .when_some(label, |this, label| this.child(label))
      .children(children)
      .when(!disabled, |this| {
        this.track_focus(&focus_handle.tab_stop(tab_stop).tab_index(tab_index))
      })
      .when(!disabled, |this| {
        this
          .cursor_pointer()
          .hover(|this| this.opacity(0.9))
          .active(|this| this.opacity(0.8))
      })
      .on_mouse_down(MouseButton::Left, |_, window, _| {
        window.prevent_default();
      })
      .when_some(on_click.filter(|_| !disabled), |this, on_click| {
        this
          .on_click(move |_event: &ClickEvent, window, cx| on_click(&activation_value, window, cx))
      })
      .refine_style(&style)
  }
}
