//! Attaching a [`PopupMenu`] to an element.
//!
//! One entry point for both menu flavours — an anchored dropdown and a
//! right-click context menu — over one menu widget. There is deliberately no
//! separate "dropdown" type: where the menu opens is a knob on the returned
//! [`MenuPopover`], not a second verb.
//!
//! The menu is an overlay *inside* the wrapped element rather than a box around
//! it: wrapping would put a second layout box between a panel and its content,
//! and any content sized as a percentage of its parent (`h_full`, `size_full`)
//! then has no definite height to resolve against and collapses.

use std::{cell::RefCell, panic::Location, rc::Rc};

use gpui::{
  Anchor, AnyElement, App, Bounds, Context, DismissEvent, Element, ElementId, Entity, Focusable,
  GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId, IntoElement, MouseButton,
  MouseDownEvent, ParentElement, Pixels, Point, Styled, Subscription, Window, anchored, deferred,
  prelude::FluentBuilder, px,
};

use super::popup_menu::PopupMenu;
use crate::{ActiveTheme as _, Button, CardStyle as _, Popover, StyleSized as _, Theme, v_flex};

pub(crate) type MenuBuilderFn =
  dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu;

/// Attaches a menu to an element.
///
/// One verb. Where the menu opens is a knob on the returned [`MenuPopover`]:
/// `.anchor(..)` picks the side and `.at_pointer()` turns it into a right-click
/// menu at the cursor. Elements with their own identity key the menu by it (see
/// the inherent `Button::menu`); everything else keys it by the call site.
pub trait MenuExt: ParentElement + Styled + IntoElement + 'static + Sized {
  /// Builds the menu this element opens.
  #[track_caller]
  fn menu(
    self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> MenuPopover<Self> {
    let caller = Location::caller();
    MenuPopover::new(ElementId::CodeLocation(*caller), self, f)
  }
}

impl<T: ParentElement + Styled + IntoElement + 'static> MenuExt for T {}

/// An element with a menu attached to it.
pub struct MenuPopover<T: ParentElement + Styled + IntoElement + 'static> {
  id: ElementId,
  anchor: Anchor,
  /// Open at the pointer on right-click instead of below the element.
  at_pointer: bool,
  trigger: Option<T>,
  builder: Rc<MenuBuilderFn>,
  /// The element's style lives here while the trigger is taken; kept so
  /// `Styled` stays usable after construction.
  style: gpui::StyleRefinement,
}

impl<T: ParentElement + Styled + IntoElement + 'static> MenuPopover<T> {
  fn new(
    id: ElementId, trigger: T,
    builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    Self {
      id,
      // A dropdown hangs off the trigger's bottom-left corner.
      anchor: Anchor::BottomLeft,
      at_pointer: false,
      trigger: Some(trigger),
      builder: Rc::new(builder),
      style: gpui::StyleRefinement::default(),
    }
  }

  /// Opens the menu on the given side of the trigger instead of below it.
  pub fn anchor(mut self, anchor: Anchor) -> Self {
    self.anchor = anchor;
    self
  }

  /// Opens the menu at the pointer on right-click instead of below the trigger.
  ///
  /// Right-clicking again moves the open menu to the new position rather than
  /// closing it.
  pub fn at_pointer(mut self) -> Self {
    self.at_pointer = true;
    self
  }

  fn with_state<R>(
    &mut self, id: &GlobalElementId, window: &mut Window, cx: &mut App,
    f: impl FnOnce(&mut Self, &mut MenuPopoverState, &mut Window, &mut App) -> R,
  ) -> R {
    window.with_optional_element_state::<MenuPopoverState, _>(Some(id), |element_state, window| {
      let mut element_state = element_state.unwrap().unwrap_or_default();
      let result = f(self, &mut element_state, window, cx);
      (result, Some(element_state))
    })
  }
}

impl<T: ParentElement + Styled + IntoElement + 'static> ParentElement for MenuPopover<T> {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    if let Some(trigger) = &mut self.trigger {
      trigger.extend(elements);
    }
  }
}

impl<T: ParentElement + Styled + IntoElement + 'static> Styled for MenuPopover<T> {
  fn style(&mut self) -> &mut gpui::StyleRefinement {
    if let Some(trigger) = &mut self.trigger {
      trigger.style()
    } else {
      &mut self.style
    }
  }
}

impl<T: ParentElement + Styled + IntoElement + 'static> IntoElement for MenuPopover<T> {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

/// Everything the element needs to survive between frames.
#[derive(Default)]
struct MenuShared {
  menu: Option<Entity<PopupMenu>>,
  open: bool,
  position: Point<Pixels>,
  /// The trigger's own bounds as of the last frame it was laid out.
  trigger_bounds: Bounds<Pixels>,
  _subscription: Option<Subscription>,
}

/// Element state for [`MenuPopover`].
#[derive(Default)]
pub struct MenuPopoverState {
  element: Option<AnyElement>,
  shared: Rc<RefCell<MenuShared>>,
  /// Whether the menu is on screen; used in prepaint to block hover on the
  /// elements underneath it.
  open: bool,
}

impl<T: ParentElement + Styled + IntoElement + 'static> Element for MenuPopover<T> {
  type RequestLayoutState = MenuPopoverState;
  type PrepaintState = Hitbox;

  fn id(&self) -> Option<ElementId> {
    Some(self.id.clone())
  }

  fn source_location(&self) -> Option<&'static Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window,
    cx: &mut App,
  ) -> (gpui::LayoutId, Self::RequestLayoutState) {
    let anchor = self.anchor;
    let at_pointer = self.at_pointer;

    self.with_state(
      id.unwrap(),
      window,
      cx,
      |this, state: &mut MenuPopoverState, window, cx| {
        let (open, menu_view, trigger_bounds, position) = {
          let shared = state.shared.borrow();
          (
            shared.open,
            shared.menu.clone(),
            shared.trigger_bounds,
            shared.position,
          )
        };
        let has_items = menu_view
          .as_ref()
          .map(|menu| !menu.read(cx).is_empty())
          .unwrap_or(false);

        // Place the menu on the requested side of the trigger, flipping to the
        // other side only when the requested one has no room. A pointer menu is
        // already pinned to the cursor.
        let position = if at_pointer {
          position
        } else if has_items {
          let menu_size = menu_view.as_ref().map(|menu| menu.read(cx).bounds.size);
          Popover::resolve_placement(anchor, trigger_bounds, menu_size, window.viewport_size()).2
        } else {
          position
        };

        let mut overlay = None;
        if open && has_items {
          overlay = Some(
            deferred(
              anchored()
                .position(position)
                .snap_to_window_with_margin(px(8.))
                .anchor(anchor_of_placement(anchor))
                .when_some(menu_view, |this, menu| {
                  if !menu.focus_handle(cx).contains_focused(window, cx) {
                    menu.focus_handle(cx).focus(window, cx);
                  }
                  this.child(menu_card(cx, menu))
                }),
            )
            .with_priority(1)
            .into_any(),
          );
        }

        let mut element = this
          .trigger
          .take()
          .expect("the trigger is only taken once per layout")
          .children(overlay)
          .into_any_element();

        let layout_id = element.request_layout(window, cx);

        (
          layout_id,
          MenuPopoverState {
            element: Some(element),
            shared: state.shared.clone(),
            open: open && has_items,
          },
        )
      },
    )
  }

  fn prepaint(
    &mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>,
    request_layout: &mut Self::RequestLayoutState, window: &mut Window, cx: &mut App,
  ) -> Self::PrepaintState {
    if let Some(element) = &mut request_layout.element {
      element.prepaint(window, cx);
    }

    // The element's own bounds are the trigger's bounds: there is no wrapper in
    // between, so this is what the menu attaches to.
    request_layout.shared.borrow_mut().trigger_bounds = bounds;

    let behavior = if request_layout.open {
      HitboxBehavior::BlockMouse
    } else {
      HitboxBehavior::Normal
    };
    window.insert_hitbox(bounds, behavior)
  }

  fn paint(
    &mut self, id: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>,
    request_layout: &mut Self::RequestLayoutState, hitbox: &mut Hitbox, window: &mut Window,
    cx: &mut App,
  ) {
    if let Some(element) = &mut request_layout.element {
      element.paint(window, cx);
    }

    let builder = self.builder.clone();
    let at_pointer = self.at_pointer;
    let shared = request_layout.shared.clone();
    let hitbox = hitbox.clone();

    // One listener for both flavours: a right-click opens at the cursor, a
    // left-click toggles a dropdown on its anchor. It lives on the window
    // rather than on the element so inner widgets that swallow the event (tree
    // rows, table cells) cannot hide the menu.
    window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
      if !phase.bubble() || !hitbox.is_hovered(window) {
        return;
      }

      let pointer_menu = at_pointer && event.button == MouseButton::Right;
      let dropdown = !at_pointer && event.button == MouseButton::Left;

      if !(pointer_menu || dropdown) {
        return;
      }
      if dropdown && shared.borrow().open {
        close(&shared, window);
        return;
      }

      {
        let mut state = shared.borrow_mut();
        state.menu = None;
        state._subscription = None;
        if pointer_menu {
          state.position = event.position;
        }
        state.open = true;
      }

      let builder = builder.clone();
      let shared = shared.clone();
      // Build the menu next frame, so the menu entity is created outside of
      // event handling.
      window.defer(cx, move |window, cx| {
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
          builder(menu, window, cx)
        });
        let subscription = window.subscribe(&menu, cx, {
          let shared = shared.clone();
          move |_, _: &DismissEvent, window, _| close(&shared, window)
        });

        {
          let mut state = shared.borrow_mut();
          if menu.read(cx).is_empty() {
            state.open = false;
            state.menu = None;
          } else {
            state.menu = Some(menu);
            state._subscription = Some(subscription);
          }
        }
        window.refresh();
      });
    });

    let _ = id;
  }
}

fn close(shared: &Rc<RefCell<MenuShared>>, window: &mut Window) {
  {
    let mut state = shared.borrow_mut();
    state.open = false;
    state.menu = None;
    state._subscription = None;
  }
  window.refresh();
}

/// The corner the menu's `position` refers to, for a menu opened on `anchor`.
fn anchor_of_placement(anchor: Anchor) -> Anchor {
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

fn menu_card(cx: &App, menu: Entity<PopupMenu>) -> AnyElement {
  let theme: &Theme = cx.theme();
  v_flex()
    .popover_style(theme)
    .shadow_md()
    .container_padding(crate::Size::Medium)
    .child(menu)
    .into_any_element()
}

impl Button {
  /// Builds the menu this button opens, keyed by the button's own id.
  pub fn menu(
    self, f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> MenuPopover<Self> {
    MenuPopover::new(self.element_id(), self, f)
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::Cell, ops::Deref as _, rc::Rc};

  use gpui::{
    Anchor, Bounds, InteractiveElement as _, IntoElement, Modifiers, MouseButton,
    ParentElement as _, Styled as _, TestAppContext, VisualTestContext, div, point, px, size,
  };

  use super::MenuExt as _;
  use crate::{ElementExt as _, PopupMenuItem, h_flex};

  /// A 120×40 box that reports its own bounds and owns a right-click menu.
  struct Probe {
    at_pointer: bool,
    bounds: Rc<Cell<Bounds<gpui::Pixels>>>,
  }

  impl gpui::Render for Probe {
    fn render(&mut self, _: &mut gpui::Window, _: &mut gpui::Context<Self>) -> impl IntoElement {
      let bounds = self.bounds.clone();
      let trigger = div()
        .id("trigger")
        .w(px(120.))
        .h(px(40.))
        .on_prepaint(move |b, _, _| bounds.set(b))
        .menu(|menu, _, _| menu.item(PopupMenuItem::new("Item")));
      let trigger = if self.at_pointer {
        trigger.at_pointer()
      } else {
        trigger.anchor(Anchor::BottomLeft)
      };
      h_flex().size_full().items_start().child(trigger)
    }
  }

  fn open_probe(
    cx: &mut TestAppContext, at_pointer: bool,
  ) -> (VisualTestContext, Rc<Cell<Bounds<gpui::Pixels>>>) {
    cx.set_global(crate::Theme::default());
    let bounds = Rc::new(Cell::new(Bounds::default()));
    let probe = Probe {
      at_pointer,
      bounds: bounds.clone(),
    };
    let window = cx.open_window(size(px(400.), px(300.)), |_, _| probe);
    let visual = VisualTestContext::from_window(*window.deref(), cx);
    visual.run_until_parked();
    (visual, bounds)
  }

  /// Right-clicking opens the menu wherever the pointer is, even though the
  /// click never reaches the element itself as a bubble-phase handler.
  #[gpui::test]
  fn right_click_opens_the_menu_at_the_pointer(cx: &mut TestAppContext) {
    let (mut visual, _) = open_probe(cx, true);
    visual.simulate_mouse_down(
      point(px(20.), px(20.)),
      MouseButton::Right,
      Modifiers::none(),
    );
    visual.run_until_parked();

    let focused = visual.update(|window, cx| window.focused(cx).is_some());
    assert!(focused, "the menu takes focus, so it must be on screen");
  }

  /// A dropdown opens on its anchor: flush under the trigger.
  #[gpui::test]
  fn dropdown_opens_under_the_trigger(cx: &mut TestAppContext) {
    let (mut visual, bounds) = open_probe(cx, false);
    visual.simulate_mouse_down(
      point(px(20.), px(20.)),
      MouseButton::Left,
      Modifiers::none(),
    );
    visual.run_until_parked();
    visual.update(|_, cx| cx.refresh_windows());
    visual.run_until_parked();

    assert_eq!(bounds.get().size, size(px(120.), px(40.)));
    let focused = visual.update(|window, cx| window.focused(cx).is_some());
    assert!(focused, "the menu is on screen and focused");
  }
}
