use std::rc::Rc;

use gpui::{
  Anchor, AnyElement, App, Bounds, Deferred, DismissEvent, ElementId, Entity, EventEmitter,
  FocusHandle, Focusable, InteractiveElement as _, IntoElement, MouseButton, ParentElement, Pixels,
  Point, Render, RenderOnce, Stateful, StyleRefinement, Styled, Subscription, Window, anchored,
  deferred, div, point, prelude::FluentBuilder as _, px, size,
};

use crate::{
  ActiveTheme, CardStyle, Easing, ElementExt, Presence, Selectable, Size, StyleSized, StyledExt,
  Transition,
  actions::{Cancel, POPOVER_CONTEXT},
  duration, h_flex,
};

type PopoverTriggerBuilder = Box<dyn FnOnce(bool, &Window, &App) -> AnyElement + 'static>;
type PopoverContentBuilder = Rc<
  dyn Fn(&mut PopoverState, &mut Window, &mut gpui::Context<PopoverState>) -> AnyElement + 'static,
>;
type OpenChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// Distance, in pixels, the popover content travels while entering.
///
/// Small enough to read as the content settling into place rather than flying
/// in; the direction is always away from the trigger.
const ENTER_SLIDE: f32 = 6.0;

#[derive(IntoElement)]
pub struct Popover {
  id: ElementId,
  style: StyleRefinement,
  anchor: Anchor,
  default_open: bool,
  open: Option<bool>,
  tracked_focus_handle: Option<FocusHandle>,
  trigger: Option<PopoverTriggerBuilder>,
  content: Option<PopoverContentBuilder>,
  children: Vec<AnyElement>,
  trigger_style: Option<StyleRefinement>,
  mouse_button: MouseButton,
  position_at_mouse: bool,
  overlay_closable: bool,
  on_open_change: Option<OpenChangeHandler>,
  size: Size,
}

impl Popover {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      anchor: Anchor::BottomLeft,
      default_open: false,
      open: None,
      tracked_focus_handle: None,
      trigger: None,
      content: None,
      children: vec![],
      trigger_style: None,
      mouse_button: MouseButton::Left,
      position_at_mouse: false,
      overlay_closable: true,
      on_open_change: None,
      size: Size::Medium,
    }
  }

  /// The side of the trigger the popover opens toward, default
  /// [`Anchor::BottomLeft`] (a dropdown).
  ///
  /// The anchor is a preference, not a guarantee: when the requested side has
  /// no room, the popover opens on the opposite side instead, and when neither
  /// side fits it is shifted to stay inside the window.
  pub fn anchor(mut self, anchor: Anchor) -> Self {
    self.anchor = anchor;
    self
  }

  pub fn mouse_button(mut self, mouse_button: MouseButton) -> Self {
    self.mouse_button = mouse_button;
    self
  }

  pub fn position_at_mouse(mut self, enabled: bool) -> Self {
    self.position_at_mouse = enabled;
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

  /// Trigger built from the popover's open state.
  ///
  /// For triggers with no selected state of their own: the closure receives
  /// `is_open` and returns the element to render.
  pub fn trigger_with(
    mut self, trigger: impl FnOnce(bool, &Window, &App) -> AnyElement + 'static,
  ) -> Self {
    self.trigger = Some(Box::new(trigger));
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

  pub fn track_focus(mut self, handle: &FocusHandle) -> Self {
    self.tracked_focus_handle = Some(handle.clone());
    self
  }

  /// Where the popover box goes relative to its trigger.
  ///
  /// `anchor` names the trigger corner the popover attaches to, so the popover
  /// ends up on that side of the trigger: `Top*` above, `Bottom*` below, and
  /// the two centre anchors beside it. The popover is positioned by the edge
  /// facing the trigger, which is what keeps the two boxes flush instead of
  /// overlapping — the previous version shifted the trigger corner by the
  /// trigger's own height, so every popover covered its trigger.
  ///
  /// Returns the popover corner that lands on the trigger corner, and that
  /// corner's position. This is the *requested* side;
  /// [`Self::resolve_placement`] may flip it.
  pub(crate) fn side_placement(anchor: Anchor, trigger: Bounds<Pixels>) -> (Anchor, Point<Pixels>) {
    let corner = match anchor {
      Anchor::TopLeft => Anchor::BottomLeft,
      Anchor::TopCenter => Anchor::BottomCenter,
      Anchor::TopRight => Anchor::BottomRight,
      Anchor::BottomLeft => Anchor::TopLeft,
      Anchor::BottomCenter => Anchor::TopCenter,
      Anchor::BottomRight => Anchor::TopRight,
      Anchor::LeftCenter => Anchor::RightCenter,
      Anchor::RightCenter => Anchor::LeftCenter,
    };
    (corner, trigger.corner(anchor))
  }

  /// The opposite side of the trigger: `Top*` ↔ `Bottom*`, `Left*` ↔ `Right*`.
  fn opposite_side(anchor: Anchor) -> Anchor {
    match anchor {
      Anchor::TopLeft => Anchor::BottomLeft,
      Anchor::TopCenter => Anchor::BottomCenter,
      Anchor::TopRight => Anchor::BottomRight,
      Anchor::BottomLeft => Anchor::TopLeft,
      Anchor::BottomCenter => Anchor::TopCenter,
      Anchor::BottomRight => Anchor::TopRight,
      Anchor::LeftCenter => Anchor::RightCenter,
      Anchor::RightCenter => Anchor::LeftCenter,
    }
  }

  /// Picks the side the popover actually opens on, and where to put it.
  ///
  /// Only the axis the anchor names decides the side: a `Bottom*` popover flips
  /// above only when there is no vertical room below, and only then if there is
  /// room above. Running out of room on the *other* axis never flips it — that
  /// would leave the popover touching the trigger at a single corner — it
  /// slides along that axis instead. So the popover always keeps one edge
  /// coincident with the trigger's edge; sliding is the last resort, used only
  /// when neither side of the chosen axis fits.
  ///
  /// `content` is the popover's measured size, `None` before its first layout.
  pub(crate) fn resolve_placement(
    anchor: Anchor, trigger: Bounds<Pixels>, content: Option<gpui::Size<Pixels>>,
    viewport: gpui::Size<Pixels>,
  ) -> (Anchor, Anchor, Point<Pixels>) {
    let requested = Self::side_placement(anchor, trigger);
    let opposite_anchor = Self::opposite_side(anchor);
    let opposite = Self::side_placement(opposite_anchor, trigger);

    let Some(content) = content else {
      // No measurement yet: trust the request and let the next frame correct
      // it.
      let (corner, position) = requested;
      return (anchor, corner, position);
    };

    // The axis the anchor places the popover along.
    let vertical = matches!(
      anchor,
      Anchor::TopLeft
        | Anchor::TopCenter
        | Anchor::TopRight
        | Anchor::BottomLeft
        | Anchor::BottomCenter
        | Anchor::BottomRight
    );

    let has_room = |(corner, position): (Anchor, Point<Pixels>)| {
      let bounds = Bounds::from_anchor_and_size(corner, position, content);
      if vertical {
        bounds.origin.y >= Pixels::ZERO && bounds.origin.y + content.height <= viewport.height
      } else {
        bounds.origin.x >= Pixels::ZERO && bounds.origin.x + content.width <= viewport.width
      }
    };

    let (side, (corner, position)) = if has_room(requested) {
      (anchor, requested)
    } else if has_room(opposite) {
      (opposite_anchor, opposite)
    } else {
      (anchor, requested)
    };

    // Slide the box back inside the viewport, on whichever axis overflows. With
    // one side fitting, this only moves along the cross axis, so the shared
    // edge with the trigger survives.
    let bounds = Bounds::from_anchor_and_size(corner, position, content);
    let origin = Point {
      x: bounds.origin.x.clamp(
        Pixels::ZERO,
        (viewport.width - content.width).max(Pixels::ZERO),
      ),
      y: bounds.origin.y.clamp(
        Pixels::ZERO,
        (viewport.height - content.height).max(Pixels::ZERO),
      ),
    };
    (side, corner, position + (origin - bounds.origin))
  }

  /// Entrance offset for a popover anchored on `anchor`'s side.
  ///
  /// The content starts a few pixels toward the trigger and settles away from
  /// it, so the motion reads as growing out of the trigger edge.
  fn enter_slide(anchor: Anchor) -> Point<f32> {
    match anchor {
      Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => point(0., ENTER_SLIDE),
      Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => point(0., -ENTER_SLIDE),
      Anchor::LeftCenter => point(ENTER_SLIDE, 0.),
      Anchor::RightCenter => point(-ENTER_SLIDE, 0.),
    }
  }

  fn render_popover<E>(corner: Anchor, position: Point<Pixels>, content: E) -> Deferred
  where
    E: IntoElement + 'static, {
    // The side was already chosen by `resolve_placement`; snapping is only a
    // safety net for a viewport that shrank under a settled popover.
    deferred(
      anchored()
        .snap_to_window_with_margin(px(8.))
        .anchor(corner)
        .position(position)
        .child(div().relative().child(content)),
    )
    .with_priority(1)
  }

  fn render_popover_content(
    state: &Entity<PopoverState>, anchor: Anchor, size: Size, _: &mut Window, cx: &mut App,
  ) -> Stateful<gpui::Div> {
    let state_for_size = state.clone();
    h_flex()
      .id("content")
      .on_prepaint(move |bounds, window, cx| {
        let measured = bounds.size;
        let changed = state_for_size.update(cx, |state, _| {
          let changed = state.content_size != Some(measured);
          state.content_size = Some(measured);
          changed
        });
        // The first frame of an open has no measurement, so the placement it
        // used may be the wrong side. Ask for one more frame to correct it
        // even when the entrance animation is disabled.
        if changed {
          window.request_animation_frame();
        }
      })
      .occlude()
      .tab_group()
      .popover_style(cx.theme())
      .container_padding(size)
      .map(|this| match anchor {
        Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => this.top(size.container_py()),
        Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => {
          this.bottom(size.container_py())
        }
        Anchor::LeftCenter => this.left(size.container_px()),
        Anchor::RightCenter => this.right(size.container_px()),
      })
  }
}

impl_parent_element!(Popover);
impl_sizable!(Popover);
impl_styled!(Popover);

pub struct PopoverState {
  focus_handle: FocusHandle,
  tracked_focus_handle: Option<FocusHandle>,
  trigger_bounds: Bounds<Pixels>,
  /// The popover card's size as of the last frame it was laid out.
  ///
  /// The placement decision needs it to know whether a side has room, and the
  /// size is only known after layout — so the first open frame places the
  /// popover on the requested side and the next frame corrects it. The entrance
  /// animation is still running at that point, which is what hides the
  /// correction.
  content_size: Option<gpui::Size<Pixels>>,
  open: bool,
  on_open_change: Option<OpenChangeHandler>,
  dismiss_subscription: Option<Subscription>,
}

impl PopoverState {
  pub fn new(default_open: bool, cx: &mut App) -> Self {
    Self {
      focus_handle: cx.focus_handle(),
      tracked_focus_handle: None,
      trigger_bounds: Bounds::default(),
      content_size: None,
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
      focus_handle.focus(window, cx);

      self.dismiss_subscription =
        Some(
          window.subscribe(&cx.entity(), cx, move |_, _: &DismissEvent, window, cx| {
            state.update(cx, |state, cx| {
              state.dismiss(window, cx);
            });
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
      // Write back only what actually changed; `Popover::render` runs on every
      // parent re-render, so unconditional per-frame rewrites here would be
      // pure churn.
      if tracked_focus_handle != state.tracked_focus_handle {
        state.tracked_focus_handle = tracked_focus_handle;
      }
      let on_open_changed = match (&self.on_open_change, &state.on_open_change) {
        (Some(new), Some(old)) => !Rc::ptr_eq(new, old),
        (None, None) => false,
        _ => true,
      };
      if on_open_changed {
        state.on_open_change = self.on_open_change.clone();
      }
      if let Some(forced) = force_open
        && state.open != forced
      {
        state.open = forced;
      }
    });

    let open = state.read(cx).open;
    let focus_handle = state.read(cx).focus_handle.clone();
    let trigger_bounds = state.read(cx).trigger_bounds;
    let position_at_mouse = self.position_at_mouse;

    let Some(trigger) = self.trigger else {
      return div().id("empty");
    };

    let parent_view_id = window.current_view();

    // The measurement lives on the trigger's own box: the outer box also holds
    // the deferred overlay in its layout, so measuring it would report the
    // popover's own space instead of the trigger's.
    let trigger_el = div()
      .on_prepaint({
        let state = state.clone();
        move |bounds, _, cx| {
          state.update(cx, |state, _| {
            if !(position_at_mouse && state.open) {
              state.trigger_bounds = bounds;
            }
          });
        }
      })
      .child((trigger)(open, window, cx))
      .when_some(self.trigger_style.clone(), |this, trigger_style| {
        this.refine_style(&trigger_style)
      });

    let el = div()
      .id(self.id.clone())
      .child(trigger_el)
      .on_mouse_down(self.mouse_button, {
        let state = state.clone();
        move |event, window, cx| {
          cx.stop_propagation();
          state.update(cx, |state, cx| {
            state.open = open;
            if position_at_mouse {
              state.trigger_bounds = Bounds::new(event.position, size(px(0.), px(0.)));
            }
            state.toggle_open(window, cx);
          });
          cx.notify(parent_view_id);
        }
      });

    if !open {
      return el;
    }

    // Enter animation: fade in and slide a few pixels toward the anchor. The
    // content subtree only exists while open, so its keyed presence state is
    // dropped on close and every open replays the entrance. Exit animations
    // are intentionally not installed — they would require keeping the
    // overlay mounted through an exit phase, which the deferred overlay
    // lifecycle does not support today.
    let enter = Presence::new((self.id.clone(), "enter"), true)
      .transition(Transition::new(duration::POPOVER_ENTER).easing(Easing::EaseOut))
      .sample(window, cx)
      .progress;
    let content_size = state.read(cx).content_size;
    let (side, corner, position) = Self::resolve_placement(
      self.anchor,
      trigger_bounds,
      content_size,
      window.viewport_size(),
    );
    let slide = Self::enter_slide(side);

    let popover_content = Self::render_popover_content(&state, side, self.size, window, cx)
      .track_focus(&focus_handle)
      .key_context(POPOVER_CONTEXT)
      .opacity(enter)
      .relative()
      .top(px(slide.y * (1.0 - enter)))
      .left(px(slide.x * (1.0 - enter)))
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

    el.child(Self::render_popover(corner, position, popover_content))
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, ops::Deref as _, rc::Rc};

  use gpui::{
    Anchor, Bounds, IntoElement, ParentElement as _, Point, Size, Styled as _, TestAppContext,
    VisualTestContext, div, point, px, size,
  };

  use super::Popover;
  use crate::{ElementExt as _, h_flex};

  /// A 240×60 viewport with a 100×30 trigger sitting in it, in the shape the
  /// callers use: `(trigger origin, trigger size, viewport)`.
  fn scene(
    trigger: Point<gpui::Pixels>, viewport: Size<gpui::Pixels>,
  ) -> (Bounds<gpui::Pixels>, Size<gpui::Pixels>) {
    (Bounds::new(trigger, size(px(100.), px(30.))), viewport)
  }

  fn resolve(
    anchor: Anchor, trigger: Bounds<gpui::Pixels>, content: Size<gpui::Pixels>,
    viewport: Size<gpui::Pixels>,
  ) -> (Anchor, Bounds<gpui::Pixels>) {
    let (side, corner, position) =
      Popover::resolve_placement(anchor, trigger, Some(content), viewport);
    (
      side,
      Bounds::from_anchor_and_size(corner, position, content),
    )
  }

  /// The popover sits flush against the trigger edge on the requested side.
  #[test]
  fn popover_opens_on_the_requested_side() {
    let (trigger, viewport) = scene(point(px(100.), px(100.)), size(px(240.), px(300.)));
    let content = size(px(120.), px(80.));

    let (side, bounds) = resolve(Anchor::BottomLeft, trigger, content, viewport);
    assert_eq!(side, Anchor::BottomLeft);
    assert_eq!(bounds.origin, point(px(100.), px(130.)));

    let (side, bounds) = resolve(Anchor::TopLeft, trigger, content, viewport);
    assert_eq!(side, Anchor::TopLeft);
    assert_eq!(bounds.origin, point(px(100.), px(20.)));
  }

  /// No room on the requested side, room on the other: flip.
  #[test]
  fn popover_flips_when_the_requested_side_has_no_room() {
    // Trigger near the bottom edge, room above.
    let (trigger, viewport) = scene(point(px(100.), px(250.)), size(px(240.), px(300.)));
    let content = size(px(120.), px(80.));

    let (side, bounds) = resolve(Anchor::BottomLeft, trigger, content, viewport);
    assert_eq!(side, Anchor::TopLeft, "should flip above the trigger");
    assert_eq!(
      bounds.origin,
      point(px(100.), px(170.)),
      "flipped popover sits flush on the trigger's top edge"
    );
  }

  /// Neither side fits: stay on the requested side and slide back inside.
  #[test]
  fn popover_shifts_when_no_side_fits() {
    // A popover taller than the room on either side of a centred trigger.
    let (trigger, viewport) = scene(point(px(100.), px(90.)), size(px(240.), px(200.)));
    let content = size(px(120.), px(180.));

    let (side, bounds) = resolve(Anchor::BottomLeft, trigger, content, viewport);
    assert_eq!(side, Anchor::BottomLeft, "no flip when neither side fits");
    assert_eq!(bounds.origin.y, px(20.), "slid up to stay inside");
    assert_eq!(bounds.top(), px(20.));
    assert_eq!(bounds.bottom(), px(200.));
    assert_eq!(bounds.origin.x, px(100.), "horizontal position unchanged");
  }

  /// Running out of room sideways slides the popover, it never flips it: the
  /// edge shared with the trigger has to survive.
  #[test]
  fn sideways_overflow_slides_instead_of_flipping() {
    // A wide popover on a trigger near the right edge, with plenty of room
    // below: it stays below and slides left.
    let (trigger, viewport) = scene(point(px(200.), px(100.)), size(px(240.), px(300.)));
    let content = size(px(120.), px(60.));

    let (side, bounds) = resolve(Anchor::BottomLeft, trigger, content, viewport);
    assert_eq!(
      side,
      Anchor::BottomLeft,
      "no vertical flip for an x overflow"
    );
    assert_eq!(
      bounds.top(),
      px(130.),
      "top edge still on the trigger bottom"
    );
    assert_eq!(bounds.right(), px(240.), "slid left to stay inside");
  }

  /// The horizontal edge follows the requested corner and slides only when it
  /// would leave the viewport.
  #[test]
  fn horizontal_edge_follows_the_anchor() {
    let (trigger, viewport) = scene(point(px(20.), px(100.)), size(px(240.), px(300.)));
    let content = size(px(120.), px(60.));

    let (_, bounds) = resolve(Anchor::BottomRight, trigger, content, viewport);
    assert_eq!(bounds.right(), px(120.), "right edges line up");

    // The same popover on the left edge slides right instead of leaving it.
    let (trigger, viewport) = scene(point(px(0.), px(100.)), size(px(240.), px(300.)));
    let (_, bounds) = resolve(Anchor::BottomRight, trigger, content, viewport);
    assert_eq!(bounds.left(), px(0.), "slid to stay inside");
  }

  /// A trigger inside a full-height row, the shape that used to open menus a
  /// whole trigger-height away from their button.
  struct EdgeProbe {
    trigger: Rc<Cell<Bounds<gpui::Pixels>>>,
    content: Rc<Cell<Bounds<gpui::Pixels>>>,
  }

  impl gpui::Render for EdgeProbe {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
      let content_bounds = self.content.clone();
      let trigger_bounds = self.trigger.clone();
      h_flex().size_full().child(
        Popover::new("probe")
          .open(true)
          .anchor(Anchor::BottomLeft)
          .trigger_with(move |_, _, _| {
            let trigger_bounds = trigger_bounds.clone();
            div()
              .w(px(120.))
              .h(px(40.))
              .on_prepaint(move |bounds, _, _| trigger_bounds.set(bounds))
              .into_any_element()
          })
          .content(move |_, _, _| {
            let content_bounds = content_bounds.clone();
            div()
              .w(px(100.))
              .h(px(80.))
              .on_prepaint(move |bounds, _, _| content_bounds.set(bounds))
          }),
      )
    }
  }

  /// The card starts on the trigger's edge. Measuring the box around the
  /// trigger instead of the trigger itself put the menu a whole trigger-height
  /// below its button, which reads as a detached menu.
  #[gpui::test]
  fn popover_hugs_its_trigger(cx: &mut TestAppContext) {
    cx.set_global(crate::Theme::default());
    let trigger = Rc::new(Cell::new(Bounds::default()));
    let content = Rc::new(Cell::new(Bounds::default()));
    let probe = EdgeProbe {
      trigger: trigger.clone(),
      content: content.clone(),
    };
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| probe);

    let mut visual = VisualTestContext::from_window(*window.deref(), cx);
    visual.run_until_parked();
    // Bounds are captured in prepaint, so readings land a frame later.
    visual.update(|_, cx| cx.refresh_windows());
    visual.run_until_parked();

    let gap = content.get().top() - trigger.get().bottom();
    assert!(
      gap.abs() <= px(8.),
      "the card sits on the trigger's bottom edge, not a trigger-height away \
       (trigger={:?} content={:?})",
      trigger.get(),
      content.get()
    );
    let inset = content.get().left() - trigger.get().left();
    assert!(
      (px(0.)..=px(8.)).contains(&inset),
      "the card's left edge lines up with the trigger's, inset only by its own \
       padding (inset={inset:?})"
    );
  }
}
