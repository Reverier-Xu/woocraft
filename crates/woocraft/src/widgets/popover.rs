//! popover panel anchored to a trigger, styled by the woocraft design
//! system.
//!
//! this is a thin styled composition over [`gpui_base::Popover`], mirroring
//! the official gpui-kit component layer: the base owns the full behavior
//! surface (pointer or keyboard activation, controlled and uncontrolled
//! open state, outside-click and Escape dismissal, focus capture and
//! restoration, deferred paint above dialogs), and this wrapper contributes
//! exactly the themed content surface — popover colors, large radius, the
//! theme hairline border, one elevation shadow, base typography — under any
//! caller style refinements.
//!
//! the surface has no entrance motion here: gpui mounts the panel in the
//! same frame the state flips, so a fade would require keeping it mounted
//! past the close. the shared dropdown entrance helper lands later with
//! select / combobox / date_picker, which own their positioning.
//!
//! ```rust,ignore
//! use gpui::Anchor;
//! use woocraft::{Button, Popover, v_flex};
//!
//! Popover::new("export")
//!     .trigger(Button::new("export-trigger").label("Export…"))
//!     .anchor(Anchor::BottomRight)
//!     .content(|_, _, cx| {
//!         // the state handle arrives as an entity when controls inside
//!         // need to dismiss the panel; escape and outside clicks always do.
//!         v_flex().gap_2().child(Label::new("pick a format"))
//!     });
//! ```

use std::rc::Rc;

use gpui::{
  Anchor, AnyElement, App, Context, ElementId, IntoElement, MouseButton, ParentElement, RenderOnce,
  StyleRefinement, Styled, Window, black, px, rems,
};
pub use gpui_base::PopoverState;
use gpui_base::{Popover as BasePopover, Selectable, StyledExt as _, box_shadow};

use crate::{
  theme::{ActiveTheme, with_alpha},
  v_flex,
};

type OpenChangeHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type ContentBuilder =
  Box<dyn FnOnce(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> AnyElement>;

/// popover element styled by the woocraft design system.
///
/// open semantics follow the base popover: `open` pins the state (controlled),
/// `default_open` seeds it (uncontrolled), and every uncontrolled transition
/// announces through `on_open_change`.
#[derive(IntoElement)]
pub struct Popover {
  id: ElementId,
  default_open: bool,
  open: Option<bool>,
  tracked_focus: Option<gpui::FocusHandle>,
  trigger: Option<TriggerBuilder>,
  content: Option<ContentBuilder>,
  on_open_change: Option<OpenChangeHandler>,
  anchor: Anchor,
  mouse_button: MouseButton,
  overlay_closable: bool,
  appearance: bool,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

type TriggerBuilder = Box<dyn FnOnce(bool, &Window, &App) -> AnyElement>;

impl Popover {
  /// creates a closed popover with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      default_open: false,
      open: None,
      tracked_focus: None,
      trigger: None,
      content: None,
      on_open_change: None,
      anchor: Anchor::TopLeft,
      mouse_button: MouseButton::Left,
      overlay_closable: true,
      appearance: true,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets the anchor corner of the surface relative to the trigger.
  pub fn anchor(mut self, anchor: impl Into<Anchor>) -> Self {
    self.anchor = anchor.into();
    self
  }

  /// sets the pointer button that toggles the popover; left by default.
  pub fn mouse_button(mut self, mouse_button: MouseButton) -> Self {
    self.mouse_button = mouse_button;
    self
  }

  /// sets the trigger element; its selected state tracks the popover.
  pub fn trigger<T>(mut self, trigger: T) -> Self
  where
    T: Selectable + IntoElement + 'static, {
    self.trigger = Some(Box::new(move |is_open, _, _| {
      let selected = trigger.is_selected();
      trigger.selected(selected || is_open).into_any_element()
    }));
    self
  }

  /// supplies a raw trigger builder for higher-level presentation facades.
  #[doc(hidden)]
  pub fn trigger_with(
    mut self, trigger: impl FnOnce(bool, &Window, &App) -> AnyElement + 'static,
  ) -> Self {
    self.trigger = Some(Box::new(trigger));
    self
  }

  /// seeds the uncontrolled open state.
  pub fn default_open(mut self, open: bool) -> Self {
    self.default_open = open;
    self
  }

  /// pins the open state, turning the popover controlled.
  pub fn open(mut self, open: bool) -> Self {
    self.open = Some(open);
    self
  }

  /// moves focus into `handle` on open instead of the surface's own focus
  /// handle.
  pub fn track_focus(mut self, handle: &gpui::FocusHandle) -> Self {
    self.tracked_focus = Some(handle.clone());
    self
  }

  /// keeps the popover open when the pointer goes down outside it.
  pub fn overlay_closable(mut self, closable: bool) -> Self {
    self.overlay_closable = closable;
    self
  }

  /// strips the themed surface — no background, border, shadow, or padding
  /// — leaving a bare panel for callers that style their own surface, the
  /// way select and combobox dropdowns do. dismissal behavior is unchanged;
  /// use [`Popover::overlay_closable`] for that.
  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  /// subscribes to every open-state transition.
  pub fn on_open_change(
    mut self, callback: impl Fn(&bool, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_open_change = Some(Rc::new(callback));
    self
  }

  /// sets the content builder; it receives the popover state so controls
  /// inside the surface can dismiss the panel.
  pub fn content<F, E>(mut self, content: F) -> Self
  where
    E: IntoElement,
    F: FnOnce(&mut PopoverState, &mut Window, &mut Context<PopoverState>) -> E + 'static, {
    self.content = Some(Box::new(move |state, window, cx| {
      content(state, window, cx).into_any_element()
    }));
    self
  }
}

impl ParentElement for Popover {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    // children render inside the themed surface, after the content builder
    // output, exactly like the official component layer.
    self.children.extend(elements);
  }
}

impl Styled for Popover {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Popover {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    // theme values are copied out before any mutable borrow of `cx`; they
    // travel into the content closure, which base runs during its render.
    let (font_family, text_size, foreground, background, border_color, border_width, radius) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.font_size,
        theme.popover_foreground,
        theme.popover,
        theme.border,
        theme.border_width,
        theme.radius_lg,
      )
    };
    let shadow_offset = rems(0.25).to_pixels(window.rem_size());
    let shadow_blur = rems(1.).to_pixels(window.rem_size());
    let anchor = self.anchor;
    let appearance = self.appearance;
    let style = self.style;
    let children = self.children;
    let content = self.content;

    let mut base = BasePopover::new(self.id)
      .anchor(anchor)
      .mouse_button(self.mouse_button)
      .default_open(self.default_open)
      .overlay_closable(self.overlay_closable)
      .content(move |state, window, cx| {
        let built = content.map(|build| build(state, window, cx));
        let surface = v_flex();
        // the surface breathes 0.25rem off the trigger on the side facing
        // it, so the panel never hugs the control that opened it.
        let surface = match anchor {
          Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => surface.top(rems(0.25)),
          Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => {
            surface.bottom(rems(0.25))
          }
          Anchor::LeftCenter | Anchor::RightCenter => surface.top(rems(0.25)),
        };
        if !appearance {
          return surface
            .children(built)
            .children(children)
            .into_any_element();
        }
        surface
          .font_family(font_family)
          .text_size(text_size)
          .text_color(foreground)
          .bg(background)
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
          .p(rems(1.))
          .gap(rems(1.))
          .refine_style(&style)
          .children(built)
          .children(children)
          .into_any_element()
      });
    if let Some(trigger) = self.trigger {
      base = base.trigger_with(trigger);
    }
    if let Some(open) = self.open {
      base = base.open(open);
    }
    if let Some(handle) = self.tracked_focus {
      base = base.track_focus(&handle);
    }
    if let Some(on_open_change) = self.on_open_change {
      base = base.on_open_change(move |open, window, cx| on_open_change(open, window, cx));
    }
    base
  }
}
