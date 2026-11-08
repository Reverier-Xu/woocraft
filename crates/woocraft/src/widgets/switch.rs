//! Toggle switch component for boolean state selection.
//!
//! Switch provides an animated toggle between on/off states. Smoother and more
//! touch-friendly than checkbox for binary choices. Animated sliding thumb
//! indicates state change. Supports 4 color variants (Primary, Success,
//! Warning, Danger) and optional label text. Common in settings panels, feature
//! toggles, and mobile-style UI.
//!
//! # Features
//! - **Animated Toggle**: Smooth sliding animation when toggled
//! - **Two-State**: On/off boolean with clear visual indication
//! - **Variants**: Color variants (Primary, Success, Warning, Danger)
//! - **Optional Label**: Display text label next to switch
//! - **Click & Keyboard**: Click to toggle or press Space
//! - **Disabled State**: Prevent toggling and dim appearance
//! - **Size Variants**: Small, Medium, Large sizing
//!
//! # Example
//! ```rust,ignore
//! Switch::new("dark_mode")
//!   .checked(true)
//!   .label("Enable Dark Mode")
//!   .primary()
//!   .on_click(|enabled, _window, _cx| {
//!     println!("Dark mode: {}", enabled);
//!   })
//! ```

use std::{rc::Rc, time::Duration};

use gpui::{
  AnyElement, App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement,
  RenderOnce, SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{
  ActiveTheme, ColorExt, Easing, Size, StyleSized, StyledExt, Transition, duration, h_flex,
  opacity, transition,
};

type SwitchClickHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

/// Color variant for switch control.
///
/// Determines the color of the switch thumb when in the "on" state.
/// - Primary (default): Uses theme primary color (usually blue)
/// - Success: Green, for positive/enabled states
/// - Warning: Yellow/orange, for caution
/// - Danger: Red, for destructive or critical toggles
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SwitchVariant {
  /// Primary color. Default.
  #[default]
  Primary,
  /// Success/positive color (green).
  Success,
  /// Warning/caution color (yellow/orange).
  Warning,
  /// Danger/critical color (red).
  Danger,
}

/// Convenience trait for types that support switch variant styling.
///
/// Provides shorthand methods for setting switch color variants without
/// explicitly constructing `SwitchVariant` enum values.
pub trait SwitchVariants: Sized {
  /// Set the switch variant directly.
  fn with_variant(self, variant: SwitchVariant) -> Self;

  /// Set switch to Primary variant (default, blue).
  fn primary(self) -> Self {
    self.with_variant(SwitchVariant::Primary)
  }

  /// Set switch to Success variant (green).
  fn success(self) -> Self {
    self.with_variant(SwitchVariant::Success)
  }

  /// Set switch to Warning variant (yellow/orange).
  fn warning(self) -> Self {
    self.with_variant(SwitchVariant::Warning)
  }

  /// Set switch to Danger variant (red).
  fn danger(self) -> Self {
    self.with_variant(SwitchVariant::Danger)
  }
}

#[derive(IntoElement)]
/// Animated toggle switch for boolean on/off selection.
///
/// Switch renders as a rounded pill-shaped control with an animated sliding
/// thumb. Toggled by clicking or pressing Space. Uses smooth animation to
/// indicate state change. Great for settings, feature toggles, and any binary
/// on/off configuration.
pub struct Switch {
  id: ElementId,
  style: StyleRefinement,
  checked: bool,
  disabled: bool,
  label: Option<SharedString>,
  on_click: Option<SwitchClickHandler>,
  size: Size,
  variant: SwitchVariant,
}

impl Switch {
  /// Creates a new switch in the "off" (false) state.
  ///
  /// The `id` is required to maintain state and keyboard focus across renders.
  /// Default size is Medium with Primary color variant.
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      checked: false,
      disabled: false,
      label: None,
      on_click: None,
      size: Size::Medium,
      variant: SwitchVariant::Primary,
    }
  }

  /// Sets the initial checked state (on/off).
  ///
  /// Pass `true` for the "on" state (thumb slides right, colored active color),
  /// or `false` for the "off" state (thumb stays left, muted color).
  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  /// Sets an optional text label to display next to the switch.
  ///
  /// The label appears to the right of the toggle pill, typically used to
  /// describe what the switch controls (e.g., "Enable notifications").
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// Sets a callback handler invoked when the user clicks the switch.
  ///
  /// The handler receives the new boolean state after toggle.
  /// Called with `true` when switched on, `false` when switched off.
  pub fn on_click<F>(mut self, handler: F) -> Self
  where
    F: Fn(&bool, &mut Window, &mut App) + 'static, {
    self.on_click = Some(Rc::new(handler));
    self
  }
}

impl SwitchVariants for Switch {
  fn with_variant(mut self, variant: SwitchVariant) -> Self {
    self.variant = variant;
    self
  }
}

impl_styled!(Switch);
impl_sizable!(Switch);
impl_disableable!(Switch);

impl RenderOnce for Switch {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let id = self.id.clone();

    // Motion policy: a keyed target-value transition replaces the previous
    // settle-timer scheme. Interrupting a toggle mid-flight reverses from the
    // sampled value instead of snapping back to the animation's start, and
    // reduced motion resolves the target immediately without frames.
    let policy = Transition::new(if self.disabled {
      Duration::ZERO
    } else {
      duration::SWITCH_TOGGLE
    })
    .easing(Easing::EaseOut);

    let rem_size = window.rem_size();
    let track_h = self.size.track_height().to_pixels(rem_size);
    let track_w = track_h * 1.5;
    let track_thickness = self.size.track_thickness().to_pixels(rem_size);
    let track_radius = self.size.component_radius();
    // Thumb: a borderless text-colored pill, 0.25em × 1em at rest, growing to
    // 0.5em wide and brightening on hover. Its center always stays 0.5em
    // away from the track ends.
    let thumb_h = self.size.thumb_size().to_pixels(rem_size);
    let thumb_w_rest = self.size.em(0.25).to_pixels(rem_size);
    let thumb_w_hover = self.size.em(0.5).to_pixels(rem_size);
    let thumb_inset = self.size.em(0.5).to_pixels(rem_size);
    let thumb_radius = self.size.em(0.125);

    let track_bg = cx.theme().muted;
    let active_color = match self.variant {
      SwitchVariant::Primary => cx.theme().primary,
      SwitchVariant::Success => cx.theme().success,
      SwitchVariant::Warning => cx.theme().warning,
      SwitchVariant::Danger => cx.theme().danger,
    };

    // Hover grows and brightens the thumb through the same interruptible
    // target-value transition scheme as the toggle; the hover flag itself is
    // keyed element state fed by the track's hover listener.
    let hover_state = window.use_keyed_state((id.clone(), "thumb-hovered"), cx, |_, _| false);
    let hover_policy = Transition::new(if self.disabled {
      Duration::ZERO
    } else {
      duration::THUMB_HOVER
    })
    .easing(Easing::EaseOut);
    let hover_t = transition(
      (id.clone(), "thumb-hover"),
      if *hover_state.read(cx) { 1.0 } else { 0.0 },
      hover_policy,
      window,
      cx,
    );
    let thumb_w = thumb_w_rest + (thumb_w_hover - thumb_w_rest) * hover_t;
    let thumb_opacity = 0.6 + 0.2 * hover_t;

    // One normalized progress drives the thumb slide and the filled track.
    let progress = transition(
      (id.clone(), "progress"),
      if checked { 1.0 } else { 0.0 },
      policy,
      window,
      cx,
    );

    let thumb_x = thumb_inset + (track_w - thumb_inset * 2.) * progress - thumb_w / 2.;
    let filled_w = (track_w - thumb_inset) * progress;

    let filled_track: AnyElement = div()
      .absolute()
      .left_0()
      .top((track_h - track_thickness) / 2.0)
      .h(track_thickness)
      .w(filled_w)
      .rounded_full()
      .bg(if self.disabled {
        active_color.opacity(opacity::DISABLED)
      } else {
        active_color
      })
      .into_any_element();

    let thumb: AnyElement = div()
      .absolute()
      .top((track_h - thumb_h) / 2.0)
      .left(thumb_x)
      .w(thumb_w)
      .h(thumb_h)
      .rounded(thumb_radius)
      .bg(cx.theme().foreground.opacity(thumb_opacity))
      .when(self.disabled, |this| this.opacity(0.7))
      .into_any_element();

    h_flex()
      .id(id.clone())
      .h(track_h)
      .items_center()
      .component_gap(self.size)
      .child(
        div()
          .id((id, "track"))
          .relative()
          .w(track_w)
          .h(track_h)
          .rounded(track_radius)
          .child(
            div()
              .absolute()
              .left_0()
              .right_0()
              .top((track_h - track_thickness) / 2.0)
              .h(track_thickness)
              .rounded_full()
              .bg(if self.disabled {
                track_bg.opacity(opacity::DISABLED)
              } else {
                track_bg
              }),
          )
          .child(filled_track)
          .child(thumb)
          .when(!self.disabled, |this| {
            this
              .cursor_pointer()
              .on_hover({
                let hover_state = hover_state.clone();
                move |hovered, _, cx| {
                  hover_state.update(cx, |state, cx| {
                    if *state != *hovered {
                      *state = *hovered;
                      cx.notify();
                    }
                  });
                }
              })
              .on_click({
                let on_click = self.on_click.clone();
                move |_: &ClickEvent, window, cx| {
                  if let Some(on_click) = on_click.as_ref() {
                    on_click(&!checked, window, cx);
                  }
                }
              })
          }),
      )
      .when_some(self.label, |this, label| {
        this.child(
          div()
            .text_size(self.size.text_size())
            .text_color(if self.disabled {
              cx.theme().muted_foreground
            } else {
              cx.theme().foreground
            })
            .child(label),
        )
      })
      .refine_style(&self.style)
  }
}
