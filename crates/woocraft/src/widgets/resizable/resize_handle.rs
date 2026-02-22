use std::{cell::Cell, rc::Rc};

use gpui::{
  AnyElement, App, Axis, Element, ElementId, Entity, GlobalElementId, InteractiveElement,
  IntoElement, MouseDownEvent, MouseUpEvent, ParentElement as _, Pixels, Point, Render,
  StatefulInteractiveElement, Styled as _, Window, div, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme as _, base::DockPlacement};

pub(crate) const HANDLE_SIZE: Pixels = px(1.);
const HANDLE_HIT_PADDING: Pixels = px(3.);

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
  placement: Option<DockPlacement>,
  on_drag: Option<Rc<DragHandler<E>>>,
}

impl<T: 'static, E: 'static + Render> ResizeHandle<T, E> {
  fn new(id: impl Into<ElementId>, axis: Axis) -> Self {
    Self {
      id: id.into(),
      on_drag: None,
      drag_value: None,
      placement: None,
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

  #[allow(dead_code)]
  pub(crate) fn placement(mut self, placement: DockPlacement) -> Self {
    self.placement = Some(placement);
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

fn expanded_hit_bounds(bounds: gpui::Bounds<Pixels>, axis: Axis) -> gpui::Bounds<Pixels> {
  let mut hit_bounds = bounds;
  match axis {
    Axis::Horizontal => {
      hit_bounds.origin.x -= HANDLE_HIT_PADDING;
      hit_bounds.size.width += HANDLE_HIT_PADDING * 2.;
    }
    Axis::Vertical => {
      hit_bounds.origin.y -= HANDLE_HIT_PADDING;
      hit_bounds.size.height += HANDLE_HIT_PADDING * 2.;
    }
  }
  hit_bounds
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

      let hit_area = div().absolute().map(|this| match self.placement {
        Some(DockPlacement::Left) => this
          .cursor_col_resize()
          .top_0()
          .bottom_0()
          .left(-HANDLE_HIT_PADDING)
          .right(-HANDLE_HIT_PADDING),
        _ => this
          .when(matches!(axis, Axis::Horizontal), |this| {
            this
              .cursor_col_resize()
              .top_0()
              .bottom_0()
              .left(-HANDLE_HIT_PADDING)
              .right(-HANDLE_HIT_PADDING)
          })
          .when(matches!(axis, Axis::Vertical), |this| {
            this
              .cursor_row_resize()
              .left_0()
              .right_0()
              .top(-HANDLE_HIT_PADDING)
              .bottom(-HANDLE_HIT_PADDING)
          }),
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
        .map(|this| match self.placement {
          Some(DockPlacement::Left) => this.cursor_col_resize().h_full().w(HANDLE_SIZE),
          _ => this
            .when(matches!(axis, Axis::Horizontal), |this| {
              this.cursor_col_resize().h_full().w(HANDLE_SIZE)
            })
            .when(matches!(axis, Axis::Vertical), |this| {
              this.cursor_row_resize().w_full().h(HANDLE_SIZE)
            }),
        })
        .child(
          div()
            .bg(bg_color)
            .group_hover("handle", |this| this.bg(bg_color))
            .size_full(),
        )
        .child(hit_area.into_any_element())
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
        let hit_axis = if matches!(self.placement, Some(DockPlacement::Left)) {
          Axis::Horizontal
        } else {
          self.axis
        };
        move |ev: &MouseDownEvent, phase, window, _| {
          let hit_bounds = expanded_hit_bounds(bounds, hit_axis);
          if hit_bounds.contains(&ev.position) && phase.bubble() {
            state.set_active(true);
            window.refresh();
          }
        }
      });

      window.on_mouse_event({
        let state = state.clone();
        move |_: &MouseUpEvent, _, window, _| {
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
