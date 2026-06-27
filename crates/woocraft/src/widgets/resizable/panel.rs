use std::{
  ops::{Deref, Range},
  rc::Rc,
};

use gpui::{
  Along, AnyElement, App, AppContext, Axis, Bounds, Context, Element, ElementId, Empty, Entity,
  EventEmitter, InteractiveElement as _, IntoElement, IsZero as _, MouseMoveEvent, MouseUpEvent,
  ParentElement, Pixels, Render, RenderOnce, Style, Styled, Window, div, prelude::FluentBuilder,
  px,
};

use super::{PANEL_MIN_SIZE, ResizableState, resizable_panel, resize_handle};
use crate::{ElementExt, h_flex, v_flex};

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
    state.update(cx, |state, cx| {
      state.sync_panels_count(self.axis, panels_count, cx);
      state.apply_pending_resize(cx);
    });
    let resize_handles = {
      let display_sizes = state.read(cx).display_sizes();
      let mut offset = px(0.);

      display_sizes
        .into_iter()
        .take(panels_count.saturating_sub(1))
        .enumerate()
        .map(|(ix, size)| {
          offset += size;

          match self.axis {
            Axis::Horizontal => div()
              .absolute()
              .left(offset)
              .top_0()
              .bottom_0()
              .w(px(0.))
              .child(
                resize_handle(("resizable-handle", ix), self.axis).on_drag(DragPanel, {
                  let state = state.clone();
                  move |drag_panel, _, _, cx| {
                    cx.stop_propagation();
                    state.update(cx, |state, _| {
                      state.resizing_panel_ix = Some(ix);
                      state.last_resize_position = None;
                    });
                    cx.new(|_| drag_panel.deref().clone())
                  }
                }),
              )
              .into_any_element(),
            Axis::Vertical => div()
              .absolute()
              .top(offset)
              .left_0()
              .right_0()
              .h(px(0.))
              .child(
                resize_handle(("resizable-handle", ix), self.axis).on_drag(DragPanel, {
                  let state = state.clone();
                  move |drag_panel, _, _, cx| {
                    cx.stop_propagation();
                    state.update(cx, |state, _| {
                      state.resizing_panel_ix = Some(ix);
                      state.last_resize_position = None;
                    });
                    cx.new(|_| drag_panel.deref().clone())
                  }
                }),
              )
              .into_any_element(),
          }
        })
        .collect::<Vec<_>>()
    };

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
          .map(|(ix, mut panel)| {
            panel.panel_ix = ix;
            panel.axis = self.axis;
            panel.state = Some(state.clone());
            panel
          }),
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
      .children(resize_handles)
  }
}

#[derive(IntoElement)]
pub struct ResizablePanel {
  axis: Axis,
  panel_ix: usize,
  state: Option<Entity<ResizableState>>,
  initial_size: Option<Pixels>,
  size_range: Range<Pixels>,
  children: Vec<AnyElement>,
  visible: bool,
}

impl ResizablePanel {
  pub(super) fn new() -> Self {
    Self {
      panel_ix: 0,
      initial_size: None,
      state: None,
      size_range: PANEL_MIN_SIZE..Pixels::MAX,
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
    self.size_range = range.into();
    self
  }
}

impl ParentElement for ResizablePanel {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for ResizablePanel {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    if !self.visible {
      return div().id(("resizable-panel", self.panel_ix));
    }

    let state = self
      .state
      .expect("BUG: The `state` in ResizablePanel should be present.");
    let display_size = state.read(cx).display_size(self.panel_ix);
    let size_range = self.size_range.clone();
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
        move |bounds, _, cx| {
          state.update(cx, |state, cx| {
            state.update_panel_size(self.panel_ix, bounds, self.size_range, cx)
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

        state.update(cx, |state, cx| {
          if state.last_resize_position == Some(e.position) {
            return;
          }
          state.last_resize_position = Some(e.position);

          let panel = state.panels.get(ix).expect("BUG: invalid panel index");

          match axis {
            Axis::Horizontal => {
              state.resize_panel(ix, e.position.x - panel.bounds.left(), window, cx)
            }
            Axis::Vertical => {
              state.resize_panel(ix, e.position.y - panel.bounds.top(), window, cx);
            }
          }
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
