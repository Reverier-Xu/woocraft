//! interactive button component with woocraft theme styling.
//!
//! this is a styled wrapper over [`gpui_base::Button`], which owns every
//! behavioral concern — interaction, focus, keyboard activation, and
//! accessibility. the wrapper adds the design-system vocabulary: visual
//! variants, outline styling, label/icon content helpers, and a loading
//! state. geometry follows the design norms: a single default size rendered
//! at `1rem` text, `2rem` tall for single-line content and square `2rem ×
//! 2rem` for icon-only content, with a `2rem` minimum width.

use std::f32::consts::TAU;

use gpui::{
  AnyElement, App, ClickEvent, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement,
  ParentElement, Radians, Refineable as _, RenderOnce, SharedString, StatefulInteractiveElement,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, rems,
};
use gpui_base::{
  Button as BaseButton, Disableable, RoleOverride, Selectable,
  motion::{
    Easing, IterationCount, Keyframe, Keyframes, MotionTransform, Timing, animate_keyframes,
  },
};

use crate::{
  ActiveTheme, Icon, IconName, Theme,
  theme::{duration, opacity},
};

/// visual variants of the [`Button`].
///
/// variant styling is derived from the active [`crate::Theme`] at render
/// time; the variant only selects which theme roles apply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ButtonVariant {
  /// neutral outlined style.
  #[default]
  Default,
  /// main call-to-action.
  Primary,
  /// positive actions.
  Success,
  /// important but less critical actions.
  Warning,
  /// secondary information.
  Info,
  /// minimal transparent style.
  Flat,
  /// text-only link style.
  Link,
  /// destructive actions.
  Danger,
}

/// sets the visual variant of a [`Button`].
pub trait ButtonVariants: Sized {
  /// sets the variant directly.
  fn with_variant(self, variant: ButtonVariant) -> Self;

  /// primary variant (main call-to-action).
  fn primary(self) -> Self {
    self.with_variant(ButtonVariant::Primary)
  }

  /// success variant (positive actions).
  fn success(self) -> Self {
    self.with_variant(ButtonVariant::Success)
  }

  /// warning variant (important but less critical actions).
  fn warning(self) -> Self {
    self.with_variant(ButtonVariant::Warning)
  }

  /// info variant (secondary information).
  fn info(self) -> Self {
    self.with_variant(ButtonVariant::Info)
  }

  /// default neutral variant.
  fn default(self) -> Self {
    self.with_variant(ButtonVariant::Default)
  }

  /// flat variant (minimal transparent style).
  fn flat(self) -> Self {
    self.with_variant(ButtonVariant::Flat)
  }

  /// link variant (text-only link style).
  fn link(self) -> Self {
    self.with_variant(ButtonVariant::Link)
  }

  /// danger variant (destructive actions).
  fn danger(self) -> Self {
    self.with_variant(ButtonVariant::Danger)
  }
}

/// resolved colors for one variant against the active theme.
struct VariantColors {
  bg: Hsla,
  fg: Hsla,
  border: Option<Hsla>,
  hover_bg: Hsla,
  active_bg: Hsla,
}

/// interactive button element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::{Button, ButtonVariants};
///
/// Button::new("submit")
///   .label("submit")
///   .primary()
///   .on_click(|_event, _window, _cx| {
///     println!("clicked");
///   });
/// ```
#[derive(IntoElement)]
pub struct Button {
  id: ElementId,
  base: BaseButton,
  variant: ButtonVariant,
  outline: bool,
  disabled: bool,
  loading: bool,
  loading_icon: Option<Icon>,
  icon: Option<Icon>,
  label: Option<SharedString>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
}

impl Button {
  /// creates a button with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseButton::new(id.clone()),
      id,
      variant: ButtonVariant::default(),
      outline: false,
      disabled: false,
      loading: false,
      loading_icon: None,
      icon: None,
      label: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
    }
  }

  /// sets whether the button ignores pointer and keyboard activation.
  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }

  /// sets the application-controlled selected presentation state.
  pub fn selected(mut self, selected: bool) -> Self {
    self.base = self.base.selected(selected);
    self
  }

  /// defines application-owned styles for the button's semantic states.
  pub fn styles(
    mut self, build: impl FnOnce(gpui_base::ButtonStyles) -> gpui_base::ButtonStyles,
  ) -> Self {
    self.base = self.base.styles(build);
    self
  }

  /// sets the label exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }

  /// overrides the accessibility role. the default is the button role.
  pub fn role(mut self, role: impl Into<RoleOverride>) -> Self {
    self.base = self.base.role(role);
    self
  }

  /// uses a caller-owned focus handle instead of creating keyed state.
  pub fn track_focus(mut self, focus_handle: &FocusHandle) -> Self {
    self.base = self.base.track_focus(focus_handle);
    self
  }

  /// sets the activation handler for pointer, enter, and space input.
  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.base = self.base.on_click(handler);
    self
  }

  /// sets the focus traversal index. the default is `0`.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.base = self.base.tab_index(tab_index);
    self
  }

  /// sets whether the button participates in keyboard focus traversal.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.base = self.base.tab_stop(tab_stop);
    self
  }

  /// sets whether pressing the button moves focus onto it.
  pub fn focusable(mut self, focusable: bool) -> Self {
    self.base = self.base.focusable(focusable);
    self
  }

  /// sets the visible text label of the button.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the leading icon of the button.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.icon = Some(icon.into());
    self
  }

  /// shows a spinning icon in place of the leading icon and disables
  /// activation while the action runs.
  pub fn loading(mut self, loading: bool) -> Self {
    self.loading = loading;
    self
  }

  /// overrides the icon shown while loading. defaults to a spinning
  /// [`IconName::SpinnerIos`].
  pub fn loading_icon(mut self, icon: impl Into<Icon>) -> Self {
    self.loading_icon = Some(icon.into());
    self
  }

  /// switches the button to its outlined presentation.
  pub fn outline(mut self, outline: bool) -> Self {
    self.outline = outline;
    self
  }

  /// returns the button's unique element identifier.
  pub fn element_id(&self) -> &ElementId {
    &self.id
  }
}

impl ButtonVariants for Button {
  fn with_variant(mut self, variant: ButtonVariant) -> Self {
    self.variant = variant;
    self
  }
}

impl Selectable for Button {
  fn selected(mut self, selected: bool) -> Self {
    self.base = self.base.selected(selected);
    self
  }

  fn is_selected(&self) -> bool {
    self.base.is_selected()
  }
}

impl Disableable for Button {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self.base = self.base.disabled(disabled);
    self
  }
}

impl Styled for Button {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Button {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

fn with_alpha(color: gpui::Hsla, alpha: f32) -> gpui::Hsla {
  gpui::Hsla {
    a: alpha.clamp(0.0, 1.0),
    ..color
  }
}

fn variant_colors(variant: ButtonVariant, outline: bool, theme: &Theme) -> VariantColors {
  let t = theme;

  let solid = |bg: Hsla, fg: Hsla| VariantColors {
    bg,
    fg,
    border: None,
    hover_bg: with_alpha(bg, opacity::solid::HOVER),
    active_bg: with_alpha(bg, opacity::solid::ACTIVE),
  };

  let ghost = |fg: Hsla| VariantColors {
    bg: with_alpha(fg, 0.0),
    fg,
    border: None,
    hover_bg: with_alpha(fg, opacity::transparent::HOVER),
    active_bg: with_alpha(fg, opacity::transparent::ACTIVE),
  };

  let mut colors = match variant {
    ButtonVariant::Primary => solid(t.primary, t.primary_foreground),
    ButtonVariant::Success => solid(t.success, t.primary_foreground),
    ButtonVariant::Warning => solid(t.warning, t.primary_foreground),
    ButtonVariant::Info => solid(t.ring, t.primary_foreground),
    ButtonVariant::Danger => solid(t.danger, t.primary_foreground),
    ButtonVariant::Default => {
      let mut colors = ghost(t.foreground);
      colors.border = Some(t.border);
      colors
    }
    ButtonVariant::Link => ghost(t.primary),
    ButtonVariant::Flat => ghost(t.foreground),
  };

  if outline
    && !matches!(
      variant,
      ButtonVariant::Default | ButtonVariant::Flat | ButtonVariant::Link
    )
  {
    // outlined colored buttons read as accent-colored text and border on the
    // page background — never on the solid variant foreground, which is
    // tuned against the solid fill and turns unreadable in dark mode.
    let accent = colors.bg;
    colors.bg = with_alpha(accent, 0.0);
    colors.fg = accent;
    colors.border = Some(accent);
    colors.hover_bg = with_alpha(accent, opacity::transparent::HOVER);
    colors.active_bg = with_alpha(accent, opacity::transparent::ACTIVE);
  }

  colors
}

impl RenderOnce for Button {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let colors = variant_colors(self.variant, self.outline, theme);
    let mut base = self.base;

    // horizontal and vertical padding are synced at 0.5rem so single-line
    // buttons are 2rem tall, icon-only buttons are square 2rem × 2rem, and
    // multi-line content still grows the height naturally. the border is
    // inset from the same padding so outlines never change the geometry.
    let pad = match colors.border {
      Some(_) => rems(0.5).to_pixels(window.rem_size()) - theme.border_width,
      None => rems(0.5).to_pixels(window.rem_size()),
    };

    let disabled = self.disabled || self.loading;
    let solid_family = !self.outline
      && matches!(
        self.variant,
        ButtonVariant::Primary
          | ButtonVariant::Success
          | ButtonVariant::Warning
          | ButtonVariant::Info
          | ButtonVariant::Danger
      );
    let (muted, muted_foreground, primary) = (theme.muted, theme.muted_foreground, theme.primary);
    let border_width = theme.border_width;

    base = base
      .min_w(rems(2.))
      .p(pad)
      .gap(rems(0.5))
      .rounded(theme.radius)
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .bg(colors.bg)
      .text_color(colors.fg)
      .hover(move |s| s.bg(colors.hover_bg))
      .active(move |s| s.bg(colors.active_bg))
      .when_some(colors.border, |this, border| {
        this.border(border_width).border_color(border)
      })
      .when(!disabled, |this| this.cursor_pointer())
      .when(disabled, |this| {
        this.cursor(gpui::CursorStyle::OperationNotAllowed)
      })
      .styles(|s| {
        s.disabled(|st| st.bg(muted).text_color(muted_foreground))
          .selected(|st| {
            if solid_family {
              st.bg(with_alpha(colors.bg, opacity::solid::ACTIVE))
            } else {
              st.bg(with_alpha(primary, opacity::transparent::ACTIVE))
                .text_color(primary)
            }
          })
      });

    // caller refinements win over the variant defaults.
    base.style().refine(&self.style);

    let mut children: Vec<AnyElement> = Vec::new();

    if self.loading {
      base = base.disabled(true);
      let spinner = self
        .loading_icon
        .unwrap_or_else(|| Icon::new(IconName::SpinnerIos));
      let spinner_id = ElementId::from(SharedString::from(format!("{:?}-loading", self.id)));

      // continuous 360° rotation driven by the gpui-base motion system on
      // the theme spinner duration.
      let keyframes = Keyframes::try_new([
        Keyframe::new(0.0, MotionTransform::identity()),
        Keyframe::new(
          1.0,
          MotionTransform {
            rotation_radians: TAU,
            ..MotionTransform::identity()
          },
        ),
      ])
      .ok();

      if let Some(keyframes) = keyframes {
        let timing = Timing::new(duration::SPINNER)
          .iterations(IterationCount::Infinite)
          .ease(Easing::Linear);
        let transform = animate_keyframes(spinner_id, &keyframes, timing, window, cx).value;
        children.push(
          div()
            .child(spinner.rotate(Radians(transform.rotation_radians)))
            .into_any_element(),
        );
      } else {
        children.push(spinner.into_any_element());
      }
    } else if let Some(icon) = self.icon {
      children.push(icon.into_any_element());
    }

    if let Some(label) = self.label {
      children.push(div().child(label).into_any_element());
    }

    children.extend(self.children);

    base.children(children)
  }
}

#[cfg(test)]
mod tests {
  use super::{Button, ButtonVariant, ButtonVariants};

  #[test]
  fn default_variant_is_neutral() {
    let button = Button::new("test");
    assert_eq!(button.variant, ButtonVariant::Default);
  }

  #[test]
  fn variant_setters_select_the_requested_variant() {
    let button = Button::new("test").primary();
    assert_eq!(button.variant, ButtonVariant::Primary);

    let button = Button::new("test").danger().warning();
    assert_eq!(button.variant, ButtonVariant::Warning);
  }
}
