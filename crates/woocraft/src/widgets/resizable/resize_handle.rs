use std::{cell::Cell, rc::Rc};

use gpui::{
  AnyElement, App, Axis, DispatchPhase, Element, ElementId, Entity, GlobalElementId,
  InteractiveElement, IntoElement, MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, Point,
  Render, StatefulInteractiveElement, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::ActiveTheme as _;

pub(crate) const HANDLE_SIZE: Pixels = px(1.);
pub(crate) const HANDLE_HIT_PADDING: Pixels = px(3.);

type DragHandler<E> = dyn Fn(&Point<Pixels>, &mut Window, &mut App) -> Entity<E>;

pub(crate) fn resize_handle<T: 'static, E: 'static + Render>(
  id: impl Into<ElementId>, axis: Axis,
) -> ResizeHandle<T, E> {
  ResizeHandle::new(id, axis)
}

pub(crate) struct ResizeHandle<T: 'static, E: 'static + Render> {
  id: ElementId,
  axis: Axis,
  drag_value: Option<Rc<T>>,
  on_drag: Option<Rc<DragHandler<E>>>,
}

impl<T: 'static, E: 'static + Render> ResizeHandle<T, E> {
  fn new(id: impl Into<ElementId>, axis: Axis) -> Self {
    Self {
      id: id.into(),
      on_drag: None,
      drag_value: None,
      axis,
    }
  }

  pub(crate) fn on_drag(
    mut self, value: T,
    f: impl Fn(Rc<T>, &Point<Pixels>, &mut Window, &mut App) -> Entity<E> + 'static,
  ) -> Self {
    let value = Rc::new(value);
    self.drag_value = Some(value.clone());
    self.on_drag = Some(Rc::new(move |p, window, cx| {
      f(value.clone(), p, window, cx)
    }));
    self
  }
}

#[derive(Default, Debug, Clone)]
struct ResizeHandleState {
  active: Cell<bool>,
}

impl ResizeHandleState {
  fn set_active(&self, active: bool) {
    self.active.set(active);
  }

  fn is_active(&self) -> bool {
    self.active.get()
  }
}

impl<T: 'static, E: 'static + Render> IntoElement for ResizeHandle<T, E> {
  type Element = ResizeHandle<T, E>;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl<T: 'static, E: 'static + Render> Element for ResizeHandle<T, E> {
  type RequestLayoutState = AnyElement;
  type PrepaintState = ();

  fn id(&self) -> Option<ElementId> {
    Some(self.id.clone())
  }

  fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, id: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    window: &mut Window, cx: &mut App,
  ) -> (gpui::LayoutId, Self::RequestLayoutState) {
    let axis = self.axis;

    window.with_element_state(id.unwrap(), |state, window| {
      let state = state.unwrap_or(ResizeHandleState::default());

      let bg_color = if state.is_active() {
        cx.theme().primary
      } else {
        cx.theme().border
      };

      let handle_total = HANDLE_SIZE + HANDLE_HIT_PADDING * 2.;

      // The visual 1px line is centered inside the expanded hit area.
      let visual_line = div()
        .absolute()
        .when(matches!(axis, Axis::Horizontal), |this| {
          this
            .top_0()
            .bottom_0()
            .left(HANDLE_HIT_PADDING)
            .w(HANDLE_SIZE)
            .bg(bg_color)
            .group_hover("handle", |e| e.bg(bg_color))
        })
        .when(matches!(axis, Axis::Vertical), |this| {
          this
            .left_0()
            .right_0()
            .top(HANDLE_HIT_PADDING)
            .h(HANDLE_SIZE)
            .bg(bg_color)
            .group_hover("handle", |e| e.bg(bg_color))
        });

      let mut el = div()
        .id(self.id.clone())
        .occlude()
        .relative()
        .flex_shrink_0()
        .group("handle")
        .when_some(self.on_drag.clone(), |this, on_drag| {
          this.on_drag(
            self.drag_value.clone().unwrap(),
            move |_, position, window, cx| on_drag(&position, window, cx),
          )
        })
        .when(matches!(axis, Axis::Horizontal), |this| {
          this
            .cursor_col_resize()
            .h_full()
            .w(handle_total)
            .ml(-HANDLE_HIT_PADDING)
            .mr(-HANDLE_HIT_PADDING)
        })
        .when(matches!(axis, Axis::Vertical), |this| {
          this
            .cursor_row_resize()
            .w_full()
            .h(handle_total)
            .mt(-HANDLE_HIT_PADDING)
            .mb(-HANDLE_HIT_PADDING)
        })
        .child(visual_line)
        .into_any_element();

      let layout_id = el.request_layout(window, cx);
      ((layout_id, el), state)
    })
  }

  fn prepaint(
    &mut self, _: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    _: gpui::Bounds<Pixels>, request_layout: &mut Self::RequestLayoutState, window: &mut Window,
    cx: &mut App,
  ) -> Self::PrepaintState {
    request_layout.prepaint(window, cx);
  }

  fn paint(
    &mut self, id: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    bounds: gpui::Bounds<Pixels>, request_layout: &mut Self::RequestLayoutState,
    _: &mut Self::PrepaintState, window: &mut Window, cx: &mut App,
  ) {
    request_layout.paint(window, cx);

    window.with_element_state(id.unwrap(), |state: Option<ResizeHandleState>, window| {
      let state = state.unwrap_or_default();

      window.on_mouse_event({
        let state = state.clone();
        move |ev: &MouseDownEvent, phase, window, _| {
          if bounds.contains(&ev.position) && phase.bubble() {
            state.set_active(true);
            window.refresh();
          }
        }
      });

      window.on_mouse_event({
        let state = state.clone();
        move |_: &MouseUpEvent, phase, window, _| {
          if phase != DispatchPhase::Bubble {
            return;
          }
          if state.is_active() {
            state.set_active(false);
            window.refresh();
          }
        }
      });

      ((), state)
    });
  }
}
