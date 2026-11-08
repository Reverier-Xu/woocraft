use std::{
  ops::{Deref, Range},
  rc::Rc,
};

use gpui::{
  Along, AnyElement, App, AppContext, Axis, Bounds, Context, Element, ElementId, Empty, Entity,
  EventEmitter, InteractiveElement as _, IntoElement, IsZero as _, MouseMoveEvent, MouseUpEvent,
  ParentElement, Pixels, Render, RenderOnce, Style, Styled, Window, div, prelude::FluentBuilder,
  px, rems,
};

use super::{ResizableState, resizable_panel, resize_handle};
use crate::{ActiveTheme as _, ElementExt, h_flex, v_flex};

type ResizeHandler = dyn Fn(&Entity<ResizableState>, &mut Window, &mut App);

pub enum ResizablePanelEvent {
  Resized,
}

#[derive(Clone)]
pub(crate) struct DragPanel;

impl Render for DragPanel {
  fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
    Empty
  }
}

#[derive(IntoElement)]
pub struct ResizablePanelGroup {
  id: ElementId,
  state: Option<Entity<ResizableState>>,
  axis: Axis,
  size: Option<Pixels>,
  children: Vec<ResizablePanel>,
  on_resize: Rc<ResizeHandler>,
}

impl ResizablePanelGroup {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      axis: Axis::Horizontal,
      children: vec![],
      state: None,
      size: None,
      on_resize: Rc::new(|_, _, _| {}),
    }
  }

  pub fn with_state(mut self, state: &Entity<ResizableState>) -> Self {
    self.state = Some(state.clone());
    self
  }

  pub fn axis(mut self, axis: Axis) -> Self {
    self.axis = axis;
    self
  }

  pub fn child(mut self, panel: impl Into<ResizablePanel>) -> Self {
    self.children.push(panel.into());
    self
  }

  pub fn children<I>(mut self, panels: impl IntoIterator<Item = I>) -> Self
  where
    I: Into<ResizablePanel>, {
    self.children = panels.into_iter().map(Into::into).collect();
    self
  }

  pub fn size(mut self, size: Pixels) -> Self {
    self.size = Some(size);
    self
  }

  pub fn on_resize(
    mut self, on_resize: impl Fn(&Entity<ResizableState>, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_resize = Rc::new(on_resize);
    self
  }
}

impl<T> From<T> for ResizablePanel
where
  T: Into<AnyElement>,
{
  fn from(value: T) -> Self {
    resizable_panel().child(value.into())
  }
}

impl From<ResizablePanelGroup> for ResizablePanel {
  fn from(value: ResizablePanelGroup) -> Self {
    resizable_panel().child(value)
  }
}

impl EventEmitter<ResizablePanelEvent> for ResizablePanelGroup {}

impl RenderOnce for ResizablePanelGroup {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self
      .state
      .unwrap_or(window.use_keyed_state(self.id.clone(), cx, |_, _| ResizableState::default()));

    let container = match self.axis {
      Axis::Horizontal => h_flex(),
      Axis::Vertical => v_flex(),
    };

    let panels_count = self.children.len();
    let rem_size = window.rem_size();
    state.update(cx, |state, cx| {
      state.set_rem_size(rem_size);
      state.sync_panels_count(self.axis, panels_count, cx);
      state.apply_pending_resize(cx);
    });
    let resizing_panel_ix = state.read(cx).resizing_panel_ix;

    container
      .id(self.id)
      .relative()
      .size_full()
      .when_some(self.size, |this, size| match self.axis {
        Axis::Horizontal => this.h(size),
        Axis::Vertical => this.w(size),
      })
      .children(
        self
          .children
          .into_iter()
          .enumerate()
          .flat_map(|(ix, mut panel)| {
            let mut items = Vec::with_capacity(2);
            if ix > 0 {
              // The 1px visual divider goes through flex layout, and the
              // resize handle hit area is nested inside it so both always
              // share the same position. Positioning the hit area with an
              // absolute offset derived from the render-time divider sizes
              // lagged a layout change (the sizes are corrected during
              // prepaint, after the handle element was built).
              let divider_ix = ix - 1;
              let drag_state = state.clone();
              items.push(
                div()
                  .debug_selector(move || format!("resizable-handle-{divider_ix}"))
                  .flex_shrink_0()
                  .when(matches!(self.axis, Axis::Horizontal), |this| {
                    this.w(px(1.)).h_full()
                  })
                  .when(matches!(self.axis, Axis::Vertical), |this| {
                    this.h(px(1.)).w_full()
                  })
                  .bg(if resizing_panel_ix == Some(divider_ix) {
                    cx.theme().primary
                  } else {
                    cx.theme().border
                  })
                  .child(
                    resize_handle(("resizable-handle", divider_ix), self.axis).on_drag(
                      DragPanel,
                      move |drag_panel, _, _, cx| {
                        cx.stop_propagation();
                        drag_state.update(cx, |state, cx| {
                          state.resizing_panel_ix = Some(divider_ix);
                          cx.notify();
                        });
                        cx.new(|_| drag_panel.deref().clone())
                      },
                    ),
                  )
                  .into_any_element(),
              );
            }
            panel.panel_ix = ix;
            panel.axis = self.axis;
            panel.state = Some(state.clone());
            items.push(panel.into_any_element());
            items
          })
          .collect::<Vec<_>>(),
      )
      .on_prepaint({
        let state = state.clone();
        move |bounds, _, cx| {
          state.update(cx, |state, cx| {
            let size_changed = state.bounds.size.along(self.axis) != bounds.size.along(self.axis);

            state.bounds = bounds;

            if size_changed {
              state.adjust_to_container_size(cx);
            }
          })
        }
      })
      .child(ResizePanelGroupElement {
        state: state.clone(),
        axis: self.axis,
        on_resize: self.on_resize.clone(),
      })
  }
}

#[derive(IntoElement)]
pub struct ResizablePanel {
  axis: Axis,
  panel_ix: usize,
  state: Option<Entity<ResizableState>>,
  initial_size: Option<Pixels>,
  size_range: Option<Range<Pixels>>,
  children: Vec<AnyElement>,
  visible: bool,
}

impl ResizablePanel {
  pub(super) fn new() -> Self {
    Self {
      panel_ix: 0,
      initial_size: None,
      state: None,
      size_range: None,
      axis: Axis::Horizontal,
      children: vec![],
      visible: true,
    }
  }

  pub fn visible(mut self, visible: bool) -> Self {
    self.visible = visible;
    self
  }

  pub fn size(mut self, size: impl Into<Pixels>) -> Self {
    self.initial_size = Some(size.into());
    self
  }

  pub fn size_range(mut self, range: impl Into<Range<Pixels>>) -> Self {
    self.size_range = Some(range.into());
    self
  }
}

impl ParentElement for ResizablePanel {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for ResizablePanel {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    if !self.visible {
      return div().id(("resizable-panel", self.panel_ix));
    }

    let state = self
      .state
      .expect("BUG: The `state` in ResizablePanel should be present.");
    let display_size = state.read(cx).display_size(self.panel_ix);
    let size_range = self
      .size_range
      .clone()
      .unwrap_or_else(|| rems(6.).to_pixels(window.rem_size())..Pixels::MAX);
    let content = div().flex_1().size_full().children(self.children);

    div()
      .id(("resizable-panel", self.panel_ix))
      .flex()
      .flex_grow(1.)
      .size_full()
      .relative()
      .when(matches!(self.axis, Axis::Vertical), |this| this.flex_col())
      .when(matches!(self.axis, Axis::Vertical), |this| {
        this.min_h(size_range.start).max_h(size_range.end)
      })
      .when(matches!(self.axis, Axis::Horizontal), |this| {
        this.min_w(size_range.start).max_w(size_range.end)
      })
      .when(self.initial_size.is_none(), |this| this.flex_shrink(1.))
      .when_some(self.initial_size, |this, initial_size| {
        this
          .when(display_size.is_none() && !initial_size.is_zero(), |this| {
            this.flex_none()
          })
          .flex_basis(initial_size)
      })
      .map(|this| match display_size {
        Some(size) => this.flex_basis(size.min(size_range.end).max(size_range.start)),
        None => this,
      })
      .on_prepaint({
        let state = state.clone();
        let panel_ix = self.panel_ix;
        let size_range = size_range.clone();
        move |bounds, _, cx| {
          state.update(cx, |state, cx| {
            state.update_panel_size(panel_ix, bounds, size_range, cx)
          })
        }
      })
      .child(content)
  }
}

struct ResizePanelGroupElement {
  state: Entity<ResizableState>,
  on_resize: Rc<ResizeHandler>,
  axis: Axis,
}

impl IntoElement for ResizePanelGroupElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for ResizePanelGroupElement {
  type RequestLayoutState = ();
  type PrepaintState = ();

  fn id(&self) -> Option<gpui::ElementId> {
    None
  }

  fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    window: &mut Window, cx: &mut App,
  ) -> (gpui::LayoutId, Self::RequestLayoutState) {
    (window.request_layout(Style::default(), None, cx), ())
  }

  fn prepaint(
    &mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    _: Bounds<Pixels>, _: &mut Self::RequestLayoutState, _: &mut Window, _: &mut App,
  ) {
  }

  fn paint(
    &mut self, _: Option<&gpui::GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    _: Bounds<Pixels>, _: &mut Self::RequestLayoutState, _: &mut Self::PrepaintState,
    window: &mut Window, cx: &mut App,
  ) {
    // Only keep the global resize listeners active while the user is actually
    // dragging a resize handle in this group.
    if self.state.read(cx).resizing_panel_ix.is_none() {
      return;
    }

    window.on_mouse_event({
      let state = self.state.clone();
      let axis = self.axis;
      move |e: &MouseMoveEvent, phase, window, cx| {
        if !phase.bubble() {
          return;
        }

        let current_ix = state.read(cx).resizing_panel_ix;
        let Some(ix) = current_ix else {
          return;
        };

        let state_read = state.read(cx);
        let panel = state_read.panels.get(ix).expect("BUG: invalid panel index");
        let new_size = match axis {
          Axis::Horizontal => e.position.x - panel.bounds.left(),
          Axis::Vertical => e.position.y - panel.bounds.top(),
        };
        if state_read.pending_resize == Some((ix, new_size)) {
          return;
        }

        state.update(cx, |state, cx| {
          state.resize_panel(ix, new_size, window, cx);
        })
      }
    });

    window.on_mouse_event({
      let state = self.state.clone();
      let on_resize = self.on_resize.clone();
      move |_: &MouseUpEvent, phase, window, cx| {
        let current_ix = state.read(cx).resizing_panel_ix;
        if current_ix.is_none() {
          return;
        }

        if phase.bubble() {
          state.update(cx, |state, cx| state.done_resizing(cx));
          on_resize(&state, window, cx);
        }
      }
    })
  }
}

#[cfg(test)]
mod tests {
  use gpui::{
    Context, Entity, IntoElement, Modifiers, MouseButton, Render, TestAppContext, Window, div,
    point, px,
  };

  use super::*;
  use crate::Theme;

  struct TestRoot {
    width: Entity<f32>,
    state: Entity<ResizableState>,
  }

  impl Render for TestRoot {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
      let w = *self.width.read(cx);
      div().w(px(w)).h_full().child(
        ResizablePanelGroup::new("group")
          .with_state(&self.state)
          .children([resizable_panel(), resizable_panel()]),
      )
    }
  }

  #[gpui::test]
  fn handle_hit_area_follows_container_resize(cx: &mut TestAppContext) {
    cx.set_global(Theme::default());
    let (root, cx) = cx.add_window_view(|_, cx| TestRoot {
      width: cx.new(|_| 800.0f32),
      state: cx.new(|_| ResizableState::default()),
    });
    cx.run_until_parked();

    let before = cx
      .debug_bounds("resizable-handle-0")
      .expect("handle before");
    println!("handle before: {before:?}");

    // Grow the container, mirroring the center when a side dock collapses.
    let width = cx.update(|_, cx| root.read(cx).width.clone());
    cx.update(|_, cx| {
      width.update(cx, |w, cx| {
        *w = 1200.0;
        cx.notify();
      });
    });
    cx.run_until_parked();

    let after = cx.debug_bounds("resizable-handle-0").expect("handle after");
    println!("handle after: {after:?}");

    // The two equal panels meet at ~600px; the hit area must be there.
    assert!(
      (after.origin.x - px(600.)).abs() < px(4.),
      "hit area at {:?}, expected near 600",
      after.origin.x
    );

    // The hit area still captures a drag anywhere across its 7px width,
    // including the 3px that reach over the following panel.
    for x in [598.0, 600.0, 602.0] {
      cx.simulate_mouse_down(
        point(px(x), px(300.)),
        MouseButton::Left,
        Modifiers::default(),
      );
      cx.simulate_mouse_move(
        point(px(x + 20.), px(300.)),
        MouseButton::Left,
        Modifiers::default(),
      );
      let resizing = cx.update(|_, cx| root.read(cx).state.read(cx).resizing_panel_ix);
      println!("drag from x={x}: resizing_panel_ix={resizing:?}");
      assert_eq!(resizing, Some(0), "drag from x={x} did not grab the handle");
      let state = cx.update(|_, cx| root.read(cx).state.clone());
      cx.update(|_, cx| {
        state.update(cx, |state, cx| {
          state.resizing_panel_ix = None;
          cx.notify();
        });
      });
      cx.simulate_mouse_up(
        point(px(x + 20.), px(300.)),
        MouseButton::Left,
        Modifiers::default(),
      );
      cx.run_until_parked();
    }
  }
}
