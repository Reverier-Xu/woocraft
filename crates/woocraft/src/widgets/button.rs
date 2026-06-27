//! Interactive button component with support for multiple visual variants,
//! sizes, and states.
//!
//! Buttons are fundamental interactive elements used to trigger actions or
//! navigate between views. This implementation provides a rich feature set
//! including variant styling, loading states, icons, tooltips, and
//! accessibility support via keyboard navigation.
//!
//! # Features
//! - **8 Visual Variants**: Primary, Success, Warning, Info, Default, Link,
//!   Flat, Danger
//! - **Size Variations**: Small, Medium (default), Large for different UI
//!   contexts
//! - **State Management**: Supports disabled, loading, and selected states
//! - **Icon Support**: Optional leading icon or loading spinner with animation
//! - **Customization**: Outline style, border corner control, width expansion
//! - **Interactivity**: Click handlers, hover callbacks, tooltips
//! - **Accessibility**: Keyboard tab navigation with configurable tab stop and
//!   index
//!
//! # Example
//! ```rust,ignore
//! use woocraft::{Button, Size};
//!
//! // Primary button with click handler
//! Button::new("submit_btn")
//!   .label("Submit")
//!   .primary()
//!   .on_click(|_event, _window, _cx| {
//!     println!("Form submitted!");
//!   })
//!
//! // Danger button with outline style
//! Button::new("delete_btn")
//!   .label("Delete")
//!   .danger()
//!   .outline(true)
//!
//! // Loading state button
//! Button::new("save_btn")
//!   .label("Saving...")
//!   .loading(true)
//!   .disabled(true)
//!
//! // Icon-only button
//! let icon = Icon::new(IconName::Copy);
//! Button::new("copy_btn")
//!   .icon(icon)
//!   .info()
//! ```
//!
//! # Performance Notes
//! Button rendering is optimized for rapid state updates. The component
//! recomputes styling on variant or state changes but caches icon animations.
//! For buttons in large lists (100+), consider using `virtual_list` to only
//! render visible buttons.

use std::rc::Rc;

use gpui::{
  AnimationExt as _, AnyElement, AnyView, App, ClickEvent, Corners, ElementId, Hsla,
  InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, Transformation, Window, percentage,
  prelude::FluentBuilder, px,
};

use crate::{
  ActiveTheme, ColorExt, Icon, IconName, InteractionColors, Sizable, Size, StyleSized, StyledExt,
  h_flex, opacity, spinner_animation,
};

type ButtonClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ButtonHoverHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;
type TooltipBuilder = Rc<dyn Fn(&mut Window, &mut App) -> AnyView>;

/// Semantic button style variants with color and behavior differences.
///
/// Each variant has a specific purpose and conveys meaning through color:
/// - **Primary**: Main call-to-action button (blue/primary color)
/// - **Success**: Indicates a positive action or confirmation (green)
/// - **Warning**: Alerts user to potential risks or important actions
///   (yellow/orange)
/// - **Info**: Provides information or secondary action (info color)
/// - **Default**: Standard neutral button with border (gray)
/// - **Link**: Text-only link-style button, no background
/// - **Flat**: Minimal flat button with transparent background
/// - **Danger**: Destructive actions like delete or remove (red)
///
/// All variants respect the current theme and disabled state, automatically
/// adjusting opacity when disabled.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ButtonVariant {
  /// Primary action button. Use for main CTAs like "Submit", "Save", "Confirm".
  Primary,
  /// Success/positive action button. Use for "Create", "Publish", "Approve".
  Success,
  /// Warning button. Use for less critical alerts like "Change", "Restart".
  Warning,
  /// Info button for secondary information or support actions.
  Info,
  /// Default neutral button. Use when variant is not semantically important.
  #[default]
  Default,
  /// Link-style button with no background. Use for inline actions or
  /// navigation.
  Link,
  /// Flat minimal button with transparent background. Use in dense UI layouts.
  Flat,
  /// Danger/destructive button. Use only for delete, remove, or other
  /// irreversible actions.
  Danger,
}

/// Border radius style for button corners.
///
/// Provides preset radius values that follow the design system's border radius
/// scale, or allows custom pixel-based radius. The radius is applied to all
/// corners by default, but can be customized per-corner using
/// `border_corners()`.
#[derive(Default, Clone, Copy, Debug)]
pub enum ButtonRounded {
  /// No rounding (sharp corners).
  None,
  /// Small radius (theme.radius / 2.0).
  Small,
  /// Medium radius (theme.radius). Default.
  #[default]
  Medium,
  /// Large radius (theme.radius_container).
  Large,
  /// Custom pixel-based radius.
  Size(Pixels),
}

impl From<Pixels> for ButtonRounded {
  fn from(value: Pixels) -> Self {
    Self::Size(value)
  }
}

/// Convenience trait for types that support button variant styling.
///
/// This trait provides shorthand methods for setting button variants without
/// explicitly constructing `ButtonVariant` enum values. Implemented by both
/// `Button` and other button-like components.
pub trait ButtonVariants: Sized {
  /// Set the button variant directly.
  fn with_variant(self, variant: ButtonVariant) -> Self;

  /// Set button to Primary variant (main call-to-action).
  fn primary(self) -> Self {
    self.with_variant(ButtonVariant::Primary)
  }

  /// Set button to Success variant (positive actions).
  fn success(self) -> Self {
    self.with_variant(ButtonVariant::Success)
  }

  /// Set button to Warning variant (important but less critical actions).
  fn warning(self) -> Self {
    self.with_variant(ButtonVariant::Warning)
  }

  /// Set button to Info variant (secondary information).
  fn info(self) -> Self {
    self.with_variant(ButtonVariant::Info)
  }

  /// Set button to Default variant (neutral style).
  fn default(self) -> Self {
    self.with_variant(ButtonVariant::Default)
  }

  /// Set button to Flat variant (minimal transparent style).
  fn flat(self) -> Self {
    self.with_variant(ButtonVariant::Flat)
  }

  /// Set button to Link variant (text-only link style).
  fn link(self) -> Self {
    self.with_variant(ButtonVariant::Link)
  }

  /// Set button to Danger variant (destructive actions).
  fn danger(self) -> Self {
    self.with_variant(ButtonVariant::Danger)
  }
}

/// Interactive button element with configurable styling, state, and event
/// handling.
///
/// `Button` is typically constructed using the builder pattern via
/// `Button::new()`, then configured using chainable methods. The component
/// automatically handles styling changes based on variant, size, disabled
/// state, and hover/click interactions.
///
/// # Builder Example
/// ```rust,ignore
/// Button::new("btn_id")
///   .label("Click Me")
///   .primary()
///   .on_click(|event, window, cx| {
///     // Handle click
///   })
/// ```
///
/// # States
/// The button supports several interactive states:
/// - **default**: Normal state, responds to clicks and hover
/// - **disabled**: Cannot be clicked, grayed out appearance
/// - **loading**: Shows spinner icon, click handler disabled
/// - **selected**: Highlighted state for toggle-like usage
///
/// # Styling
/// Style is determined by the combination of variant, size, and outline flag:
/// - `variant`: Controls semantic color and styling (8 options)
/// - `size`: Controls padding, height, text size, and gap (via `Sizable` trait)
/// - `outline`: Renders empty background with colored border instead of solid
///   background
#[derive(IntoElement)]
pub struct Button {
  id: ElementId,
  label: Option<SharedString>,
  icon: Option<Icon>,
  children: Vec<AnyElement>,
  style: StyleRefinement,
  variant: ButtonVariant,
  size: Size,
  disabled: bool,
  selected: bool,
  rounded: ButtonRounded,
  border_corners: Corners<bool>,
  expanded: bool,
  tab_stop: bool,
  tab_index: isize,
  outline: bool,
  loading: bool,
  loading_icon: Option<Icon>,
  on_click: Option<ButtonClickHandler>,
  on_hover: Option<ButtonHoverHandler>,
  tooltip_builder: Option<TooltipBuilder>,
}

impl Button {
  /// Create a new button with the given unique identifier.
  ///
  /// The button starts with default configuration: no label, Default variant,
  /// Medium size, and no attached handlers. Use builder methods to customize
  /// the button.
  ///
  /// # Arguments
  /// * `id` - Unique identifier for the button within its window. Used for
  ///   state management, focus handling, and accessibility.
  ///
  /// # Example
  /// ```rust,ignore
  /// let button = Button::new("confirm_btn");
  /// ```
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      label: None,
      icon: None,
      children: Vec::new(),
      style: StyleRefinement::default(),
      variant: ButtonVariant::default(),
      size: Size::Medium,
      disabled: false,
      selected: false,
      rounded: ButtonRounded::default(),
      border_corners: Corners::all(true),
      expanded: false,
      tab_stop: true,
      tab_index: 0,
      outline: false,
      loading: false,
      loading_icon: None,
      on_click: None,
      on_hover: None,
      tooltip_builder: None,
    }
  }

  /// Set the button's label text.
  ///
  /// The label is the primary text content displayed in the button. Can be
  /// combined with `icon()` to display both icon and text. If both `label`
  /// and `icon` are set, they are displayed horizontally centered with
  /// spacing determined by the button's size.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// Set the button's leading icon.
  ///
  /// The icon is displayed before the label text. When no label is set and this
  /// is the only content, the button becomes square (icon-only). Icon size is
  /// determined by the button's size setting via the `Sizable` trait.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// Attach a click event handler to the button.
  ///
  /// The handler is called when the user clicks the button, unless the button
  /// is disabled or loading. The handler receives the `ClickEvent`, mutable
  /// window and app context.
  ///
  /// # Arguments
  /// * `handler` - Closure with signature `(event: &ClickEvent, window: &mut
  ///   Window, cx: &mut App)`
  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Rc::new(handler));
    self
  }

  /// Attach a hover state change handler.
  ///
  /// The handler is called when the user hovers over or leaves the button.
  /// Receives a boolean indicating whether the mouse is currently over the
  /// button. Disabled and loading buttons do not trigger hover events.
  ///
  /// # Arguments
  /// * `handler` - Closure with signature `(hovered: &bool, window: &mut
  ///   Window, cx: &mut App)`
  pub fn on_hover(mut self, handler: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
    self.on_hover = Some(Rc::new(handler));
    self
  }

  /// Set whether the button should render in outline style.
  ///
  /// When `true`, the button renders with a transparent background and colored
  /// border instead of a solid background. Outline style is commonly used for
  /// secondary actions or to reduce visual prominence. Default: `false`.
  pub fn outline(mut self, outline: bool) -> Self {
    self.outline = outline;
    self
  }

  /// Set the border radius style for all corners.
  ///
  /// Applies the same border radius to all four corners. Use `border_corners()`
  /// to customize per-corner. Default: `ButtonRounded::Medium`.
  pub fn rounded(mut self, rounded: impl Into<ButtonRounded>) -> Self {
    self.rounded = rounded.into();
    self
  }

  /// Control which corners are rounded individually.
  ///
  /// Allows fine-grained control over individual corner rounding, useful for
  /// buttons in button groups where only outer corners should be rounded.
  /// Order: top-left, top-right, bottom-left, bottom-right. Default: all
  /// `true`.
  pub fn border_corners(mut self, corners: impl Into<Corners<bool>>) -> Self {
    self.border_corners = corners.into();
    self
  }

  /// Set whether the button should expand to fill available width.
  ///
  /// When `true`, the button grows to fill its parent's width and aligns
  /// content to the left. Useful for full-width buttons in forms or dialogs.
  /// Default: `false`.
  pub fn expand(mut self, expanded: bool) -> Self {
    self.expanded = expanded;
    self
  }

  /// Enable or disable the button as a tab stop in keyboard navigation.
  ///
  /// When `true`, the button can receive focus via Tab key. When `false`, the
  /// button is skipped during keyboard navigation. Default: `true`.
  pub fn tab_stop(mut self, tab_stop: bool) -> Self {
    self.tab_stop = tab_stop;
    self
  }

  /// Set the button's tab index for keyboard navigation order.
  ///
  /// Controls the keyboard navigation order. Buttons with higher tab index are
  /// focused after those with lower index. Default: `0`.
  pub fn tab_index(mut self, tab_index: isize) -> Self {
    self.tab_index = tab_index;
    self
  }

  /// Set the button's loading state.
  ///
  /// When `true`, displays a loading spinner icon and disables click
  /// interactions. Useful for async operations like form submission. The
  /// spinner animates automatically. See `loading_icon()` to customize the
  /// spinner appearance. Default: `false`.
  pub fn loading(mut self, loading: bool) -> Self {
    self.loading = loading;
    self
  }

  /// Set a custom loading spinner icon.
  ///
  /// Customizes the icon displayed when `loading()` is `true`. If not set,
  /// defaults to a standard spinner. The icon is automatically animated with
  /// rotation. Only used if `loading()` is set to `true`.
  pub fn loading_icon(mut self, icon: Icon) -> Self {
    self.loading_icon = Some(icon);
    self
  }

  /// Set the button's tooltip.
  ///
  /// The tooltip builder function is called when the user hovers over the
  /// button and should return an `AnyView` containing the tooltip content.
  /// Use for brief help text or shortened labels. The tooltip appears with
  /// delay and is automatically positioned.
  pub fn tooltip(mut self, builder: impl Fn(&mut Window, &mut App) -> AnyView + 'static) -> Self {
    self.tooltip_builder = Some(Rc::new(builder));
    self
  }

  /// Get the button's unique element identifier.
  pub fn element_id(&self) -> ElementId {
    self.id.clone()
  }

  fn clickable(&self) -> bool {
    !self.disabled && !self.loading && self.on_click.is_some()
  }

  fn hoverable(&self) -> bool {
    !self.disabled && !self.loading
  }
}

impl ButtonVariants for Button {
  fn with_variant(mut self, variant: ButtonVariant) -> Self {
    self.variant = variant;
    self
  }
}

impl_disableable!(Button);
impl_selectable!(Button);
impl_sizable!(Button);
impl_styled!(Button);
impl_parent_element!(Button);

struct ButtonColors {
  bg: Hsla,
  fg: Hsla,
  border: Hsla,
  hover_bg: Hsla,
  active_bg: Hsla,
  selected_bg: Hsla,
  selected_fg: Hsla,
  selected_border: Hsla,
  is_flat: bool,
}

fn compute_button_colors(
  variant: ButtonVariant, outline: bool, disabled: bool, theme: &crate::Theme,
) -> ButtonColors {
  let is_flat = matches!(variant, ButtonVariant::Flat | ButtonVariant::Link);

  let interaction_colors = match variant {
    ButtonVariant::Primary => InteractionColors::solid(theme.primary, theme.primary_foreground),
    ButtonVariant::Success => InteractionColors::solid(theme.success, theme.primary_foreground),
    ButtonVariant::Warning => InteractionColors::solid(theme.warning, theme.primary_foreground),
    ButtonVariant::Info => InteractionColors::solid(theme.ring, theme.primary_foreground),
    ButtonVariant::Danger => InteractionColors::solid(theme.danger, theme.primary_foreground),
    ButtonVariant::Default => {
      InteractionColors::transparent(theme.foreground).with_border(theme.border)
    }
    ButtonVariant::Link => InteractionColors::transparent(theme.primary),
    ButtonVariant::Flat => InteractionColors::transparent(theme.foreground),
  };

  let transparent = Hsla::transparent_black();
  let (bg, fg, border) = if outline {
    if variant == ButtonVariant::Default {
      (transparent, theme.foreground, theme.foreground)
    } else {
      (
        transparent,
        interaction_colors.base,
        interaction_colors.base,
      )
    }
  } else {
    (
      interaction_colors.base,
      interaction_colors.foreground,
      interaction_colors.border,
    )
  };

  let background_hover = theme.background.darken(0.04);
  let background_active = theme.background.darken(0.08);

  let hover_bg = if outline {
    background_hover
  } else {
    interaction_colors.hover
  };
  let active_bg = if outline {
    background_active
  } else {
    interaction_colors.active
  };

  let selected_bg = if is_flat {
    theme.foreground.opacity(opacity::transparent::ACTIVE)
  } else if outline {
    background_hover
  } else {
    active_bg
  };
  let selected_fg = if is_flat {
    theme.primary
  } else if outline {
    if variant == ButtonVariant::Default {
      theme.foreground
    } else {
      interaction_colors.base
    }
  } else {
    fg
  };
  let selected_border = if is_flat {
    border
  } else if outline {
    if variant == ButtonVariant::Default {
      theme.foreground
    } else {
      interaction_colors.base
    }
  } else {
    border
  };

  let (bg, fg, border, hover_bg, active_bg, selected_bg, selected_fg, selected_border) = if disabled
  {
    (
      bg.opacity(opacity::DISABLED),
      fg.opacity(opacity::DISABLED),
      border.opacity(opacity::DISABLED),
      hover_bg.opacity(opacity::DISABLED),
      active_bg.opacity(opacity::DISABLED),
      selected_bg.opacity(opacity::DISABLED),
      selected_fg.opacity(opacity::DISABLED),
      selected_border.opacity(opacity::DISABLED),
    )
  } else {
    (
      bg,
      fg,
      border,
      hover_bg,
      active_bg,
      selected_bg,
      selected_fg,
      selected_border,
    )
  };

  let (bg, border) = if disabled && matches!(variant, ButtonVariant::Link | ButtonVariant::Default)
  {
    (theme.foreground.opacity(0.1), transparent)
  } else {
    (bg, border)
  };

  let (bg, border, hover_bg, active_bg, selected_bg) = if disabled && variant == ButtonVariant::Flat
  {
    (
      transparent,
      transparent,
      theme.foreground.opacity(0.05),
      theme.foreground.opacity(0.05),
      theme.foreground.opacity(0.05),
    )
  } else {
    (bg, border, hover_bg, active_bg, selected_bg)
  };

  ButtonColors {
    bg,
    fg,
    border,
    hover_bg,
    active_bg,
    selected_bg,
    selected_fg,
    selected_border,
    is_flat,
  }
}

impl RenderOnce for Button {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let colors = compute_button_colors(self.variant, self.outline, self.disabled, theme);
    let ButtonColors {
      bg,
      fg,
      border,
      hover_bg,
      active_bg,
      selected_bg,
      selected_fg,
      selected_border,
      is_flat,
    } = colors;

    let has_only_icon = self.label.is_none() && self.children.is_empty() && self.icon.is_some();
    let clickable = self.clickable();
    let hoverable = self.hoverable();
    let icon = if self.loading {
      self.loading_icon.or(Some(Icon::new(IconName::SpinnerIos)))
    } else {
      self.icon
    };

    let content = h_flex()
      .items_center()
      .justify_center()
      .component_gap(self.size)
      .when(self.expanded, |this| this.flex_1().w_full().justify_start())
      .when_some(icon, |this, icon| {
        let icon = icon.with_size(self.size);
        if self.loading {
          this.child(icon.with_animation(
            "loading-spin",
            spinner_animation(),
            |this: Icon, delta| this.transform(Transformation::rotate(percentage(delta))),
          ))
        } else {
          this.child(icon)
        }
      })
      .when_some(self.label, |this, label| this.child(label))
      .children(self.children)
      .text_size(self.size.text_size());

    let radius = match self.rounded {
      ButtonRounded::None => px(0.0),
      ButtonRounded::Small => theme.radius / 2.0,
      ButtonRounded::Medium => theme.radius,
      ButtonRounded::Large => theme.radius_container,
      ButtonRounded::Size(radius) => radius,
    };

    let focus_handle = window
      .use_keyed_state(self.id.clone(), cx, |_, cx| cx.focus_handle())
      .read(cx)
      .clone();

    h_flex()
      .id(self.id)
      .justify_center()
      .when(!is_flat, |this| this.border_1())
      .bg(bg)
      .border_color(border)
      .text_color(fg)
      .component_size(self.size)
      .rounded_tl(if self.border_corners.top_left {
        radius
      } else {
        px(0.0)
      })
      .rounded_tr(if self.border_corners.top_right {
        radius
      } else {
        px(0.0)
      })
      .rounded_bl(if self.border_corners.bottom_left {
        radius
      } else {
        px(0.0)
      })
      .rounded_br(if self.border_corners.bottom_right {
        radius
      } else {
        px(0.0)
      })
      .when(self.selected, |this| {
        this
          .bg(selected_bg)
          .text_color(selected_fg)
          .border_color(selected_border)
      })
      .when(!has_only_icon, |this| {
        this
          .px(self.size.component_px())
          .min_w(self.size.component_height())
      })
      .when(has_only_icon, |this| {
        this
          .size(self.size.component_height())
          .p_0()
          .flex_shrink_0()
      })
      .when(self.expanded, |this| this.w_full())
      .when(!self.disabled, |this| {
        this.track_focus(
          &focus_handle
            .tab_stop(self.tab_stop)
            .tab_index(self.tab_index),
        )
      })
      .child(content)
      .when(hoverable && !self.disabled, |this| {
        this
          .cursor_pointer()
          .hover(move |this| this.bg(hover_bg).border_color(border))
          .active(move |this| this.bg(active_bg).border_color(border))
      })
      .when(self.disabled, |this| this.cursor_not_allowed())
      .when_some(self.on_hover, |this, on_hover| {
        this.on_hover(move |hovered, window, cx| on_hover(hovered, window, cx))
      })
      .when_some(self.on_click.filter(|_| clickable), |this, on_click| {
        this.on_click(move |event, window, cx| on_click(event, window, cx))
      })
      .when_some(self.tooltip_builder, |this, tooltip_builder| {
        this.tooltip(move |window, cx| tooltip_builder(window, cx))
      })
      .refine_style(&self.style)
  }
}

impl From<Button> for AnyElement {
  fn from(value: Button) -> Self {
    value.into_any_element()
  }
}
