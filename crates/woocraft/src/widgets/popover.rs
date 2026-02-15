use std::rc::Rc;

use gpui::{
  AnyElement, App, Bounds, Deferred, DismissEvent, ElementId, EventEmitter, FocusHandle, Focusable,
  Half, InteractiveElement as _, IntoElement, MouseButton, ParentElement, Pixels, Point, Render,
  RenderOnce, Stateful, StyleRefinement, Styled, Subscription, Window, anchored, deferred, div,
  point, prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Anchor, ElementExt, Selectable, StyledExt,
  actions::{Cancel, POPOVER_CONTEXT},
  h_flex,
};

#[derive(IntoElement)]
pub struct Popover {
  id: ElementId,
  style: StyleRefinement,
  anchor: Anchor,
  default_open: bool,
  open: Option<bool>,
  tracked_focus_handle: Option<FocusHandle>,
  trigger: Option<Box<dyn FnOnce(bool, &Window, &App) -> AnyElement + 'static>>,
  content: Option<
    Rc<
      dyn Fn(&mut PopoverState, &mut Window, &mut gpui::Context<PopoverState>) -> AnyElement
        + 'static,
    >,
  >,
  children: Vec<AnyElement>,
  trigger_style: Option<StyleRefinement>,
  mouse_button: MouseButton,
  appearance: bool,
  overlay_closable: bool,
  on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
}

impl Popover {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      anchor: Anchor::TopLeft,
      default_open: false,
      open: None,
      tracked_focus_handle: None,
      trigger: None,
      content: None,
      children: vec![],
      trigger_style: None,
      mouse_button: MouseButton::Left,
      appearance: true,
      overlay_closable: true,
      on_open_change: None,
    }
  }

  pub fn anchor(mut self, anchor: impl Into<Anchor>) -> Self {
    self.anchor = anchor.into();
    self
  }

  pub fn mouse_button(mut self, mouse_button: MouseButton) -> Self {
    self.mouse_button = mouse_button;
    self
  }

  pub fn trigger<T>(mut self, trigger: T) -> Self
  where
    T: Selectable + IntoElement + 'static, {
    self.trigger = Some(Box::new(|is_open, _, _| {
      let selected = trigger.is_selected();
      trigger.selected(selected || is_open).into_any_element()
    }));
    self
  }

  pub fn default_open(mut self, open: bool) -> Self {
    self.default_open = open;
    self
  }

  pub fn open(mut self, open: bool) -> Self {
    self.open = Some(open);
    self
  }

  pub fn on_open_change<F>(mut self, callback: F) -> Self
  where
    F: Fn(&bool, &mut Window, &mut App) + 'static, {
    self.on_open_change = Some(Rc::new(callback));
    self
  }

  pub fn trigger_style(mut self, style: StyleRefinement) -> Self {
    self.trigger_style = Some(style);
    self
  }

  pub fn overlay_closable(mut self, closable: bool) -> Self {
    self.overlay_closable = closable;
    self
  }

  pub fn content<F, E>(mut self, content: F) -> Self
  where
    E: IntoElement,
    F: Fn(&mut PopoverState, &mut Window, &mut gpui::Context<PopoverState>) -> E + 'static, {
    self.content = Some(Rc::new(move |state, window, cx| {
      content(state, window, cx).into_any_element()
    }));
    self
  }

  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
    self.tracked_focus_handle = Some(handle.clone());
    self
  }

  fn resolved_corner(anchor: Anchor, trigger_bounds: Bounds<Pixels>) -> Point<Pixels> {
    let offset = if anchor.is_center() {
      point(trigger_bounds.size.width.half(), px(0.))
    } else {
      Point::default()
    };

    trigger_bounds.corner(anchor.swap_vertical().into())
      + offset
      + Point {
        x: px(0.),
        y: -trigger_bounds.size.height,
      }
  }

  fn render_popover<E>(
    anchor: Anchor, trigger_bounds: Bounds<Pixels>, content: E, _: &mut Window, _: &mut App,
  ) -> Deferred
  where
    E: IntoElement + 'static, {
    deferred(
      anchored()
        .snap_to_window_with_margin(px(8.))
        .anchor(anchor.into())
        .position(Self::resolved_corner(anchor, trigger_bounds))
        .child(div().relative().child(content)),
    )
    .with_priority(1)
  }

  fn render_popover_content(
    anchor: Anchor, appearance: bool, _: &mut Window, cx: &mut App,
  ) -> Stateful<gpui::Div> {
    h_flex()
      .id("content")
      .occlude()
      .tab_group()
      .when(appearance, |this| {
        this
          .bg(cx.theme().popover)
          .text_color(cx.theme().popover_foreground)
          .border_1()
          .shadow_sm()
          .border_color(cx.theme().border)
          .rounded(cx.theme().radius_container)
          .p_2()
      })
      .map(|this| match anchor {
        Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => this.top_1(),
        Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => this.bottom_1(),
      })
  }
}

impl ParentElement for Popover {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for Popover {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

pub struct PopoverState {
  focus_handle: FocusHandle,
  tracked_focus_handle: Option<FocusHandle>,
  trigger_bounds: Bounds<Pixels>,
  open: bool,
  on_open_change: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
  dismiss_subscription: Option<Subscription>,
}

impl PopoverState {
  pub fn new(default_open: bool, cx: &mut App) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      tracked_focus_handle: None,
      trigger_bounds: Bounds::default(),
      open: default_open,
      on_open_change: None,
      dismiss_subscription: None,
    }
  }

  pub fn is_open(&self) -> bool {
    self.open
  }

  pub fn dismiss(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
    if self.open {
      self.toggle_open(window, cx);
    }
  }

  fn on_action_cancel(&mut self, _: &Cancel, window: &mut Window, cx: &mut gpui::Context<Self>) {
    self.dismiss(window, cx);
  }

  pub fn show(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
    if !self.open {
      self.toggle_open(window, cx);
    }
  }

  fn toggle_open(&mut self, window: &mut Window, cx: &mut gpui::Context<Self>) {
    self.open = !self.open;

    if self.open {
      let state = cx.entity();
      let focus_handle = self
        .tracked_focus_handle
        .clone()
        .unwrap_or_else(|| self.focus_handle.clone());
      focus_handle.focus(window);

      self.dismiss_subscription =
        Some(
          window.subscribe(&cx.entity(), cx, move |_, _: &DismissEvent, window, cx| {
            state.update(cx, |state, cx| {
              state.dismiss(window, cx);
            });
            window.refresh();
          }),
        );
    } else {
      self.dismiss_subscription = None;
    }

    if let Some(callback) = self.on_open_change.as_ref() {
      callback(&self.open, window, cx);
    }

    cx.notify();
  }
}

impl Focusable for PopoverState {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for PopoverState {
  fn render(&mut self, _: &mut Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
    div()
  }
}

impl EventEmitter<DismissEvent> for PopoverState {}

impl RenderOnce for Popover {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let force_open = self.open;
    let default_open = self.default_open;
    let tracked_focus_handle = self.tracked_focus_handle.clone();
    let state = window.use_keyed_state(self.id.clone(), cx, |_, cx| {
      PopoverState::new(default_open, cx)
    });

    state.update(cx, |state, _| {
      if let Some(tracked_focus_handle) = tracked_focus_handle {
        state.tracked_focus_handle = Some(tracked_focus_handle);
      }
      state.on_open_change = self.on_open_change.clone();
      if let Some(force_open) = force_open {
        state.open = force_open;
      }
    });

    let open = state.read(cx).open;
    let focus_handle = state.read(cx).focus_handle.clone();
    let trigger_bounds = state.read(cx).trigger_bounds;

    let Some(trigger) = self.trigger else {
      return div().id("empty");
    };

    let parent_view_id = window.current_view();

    let trigger_el = div()
      .child((trigger)(open, window, cx))
      .when_some(self.trigger_style.clone(), |this, trigger_style| {
        this.refine_style(&trigger_style)
      });

    let el = div()
      .id(self.id)
      .child(trigger_el)
      .on_mouse_down(self.mouse_button, {
        let state = state.clone();
        move |_, window, cx| {
          cx.stop_propagation();
          state.update(cx, |state, cx| {
            state.open = open;
            state.toggle_open(window, cx);
          });
          cx.notify(parent_view_id);
        }
      })
      .on_prepaint({
        let state = state.clone();
        move |bounds, _, cx| {
          state.update(cx, |state, _| {
            state.trigger_bounds = bounds;
          });
        }
      });

    if !open {
      return el;
    }

    let popover_content = Self::render_popover_content(self.anchor, self.appearance, window, cx)
      .track_focus(&focus_handle)
      .key_context(POPOVER_CONTEXT)
      .on_action(window.listener_for(&state, PopoverState::on_action_cancel))
      .when_some(self.content, |this, content| {
        this.child(state.update(cx, |state, cx| (content)(state, window, cx)))
      })
      .children(self.children)
      .when(self.overlay_closable, |this| {
        this.on_mouse_down_out({
          let state = state.clone();
          move |_, window, cx| {
            state.update(cx, |state, cx| {
              state.dismiss(window, cx);
            });
            cx.notify(parent_view_id);
          }
        })
      })
      .refine_style(&self.style);

    el.child(Self::render_popover(
      self.anchor,
      trigger_bounds,
      popover_content,
      window,
      cx,
    ))
  }
}
