//! Styled label component for categorization and filtering.
//!
//! Tag displays short text content with a colored background and border,
//! typically used to label categories, tags, statuses, or filter options.
//! Supports six semantic color variants plus fully custom color settings and
//! an outline (hollow) presentation.

use gpui::{
  AnyElement, App, Hsla, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce,
  StyleRefinement, Styled, Window, div, px, rems,
};

use crate::{ActiveTheme, base::StyledExt};

/// Color variant for the tag/label component.
///
/// Determines the background, foreground, and border colors of the tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TagVariant {
  Primary,
  #[default]
  Secondary,
  Danger,
  Success,
  Warning,
  Info,
  Custom,
}

#[derive(IntoElement)]
/// Styled label/chip component for categorization and status indication.
///
/// Tag renders as a colored pill-shaped label with an optional border.
/// Supports both filled and outline (hollow) display modes.
pub struct Tag {
  style: StyleRefinement,
  variant: TagVariant,
  outline: bool,
  rounded: Option<Pixels>,
  custom_bg: Option<Hsla>,
  custom_fg: Option<Hsla>,
  custom_border: Option<Hsla>,
  children: Vec<AnyElement>,
}

impl Default for Tag {
  fn default() -> Self {
    Self::new()
  }
}

impl Tag {
  /// Creates a new tag with the Secondary variant (default neutral colors).
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      variant: TagVariant::default(),
      outline: false,
      rounded: None,
      custom_bg: None,
      custom_fg: None,
      custom_border: None,
      children: Vec::new(),
    }
  }

  /// Primary variant (theme primary color).
  pub fn primary() -> Self {
    Self::new().with_variant(TagVariant::Primary)
  }

  /// Secondary variant (muted/subdued colors, default).
  pub fn secondary() -> Self {
    Self::new().with_variant(TagVariant::Secondary)
  }

  /// Danger variant (red/error color).
  pub fn danger() -> Self {
    Self::new().with_variant(TagVariant::Danger)
  }

  /// Success variant (green/positive color).
  pub fn success() -> Self {
    Self::new().with_variant(TagVariant::Success)
  }

  /// Warning variant (orange/yellow caution color).
  pub fn warning() -> Self {
    Self::new().with_variant(TagVariant::Warning)
  }

  /// Info variant (informational ring color).
  pub fn info() -> Self {
    Self::new().with_variant(TagVariant::Info)
  }

  /// Creates a custom tag with explicit background, foreground, and border
  /// colors.
  pub fn custom(bg: Hsla, fg: Hsla, border: Hsla) -> Self {
    Self::new()
      .with_variant(TagVariant::Custom)
      .custom_bg(bg)
      .custom_fg(fg)
      .custom_border(border)
  }

  /// Sets the color variant.
  pub fn with_variant(mut self, variant: TagVariant) -> Self {
    self.variant = variant;
    self
  }

  /// Switches to outline mode (transparent background, colored border and
  /// text).
  pub fn outline(mut self) -> Self {
    self.outline = true;
    self
  }

  /// Sets the corner radius. Defaults to the theme radius.
  pub fn rounded(mut self, radius: impl Into<Pixels>) -> Self {
    self.rounded = Some(radius.into());
    self
  }

  /// Sets a fully rounded (pill) style.
  pub fn rounded_full(mut self) -> Self {
    self.rounded = Some(px(999.0));
    self
  }

  /// Sets a custom background color (only for the Custom variant).
  pub fn custom_bg(mut self, color: Hsla) -> Self {
    self.custom_bg = Some(color);
    self
  }

  /// Sets a custom foreground/text color (only for the Custom variant).
  pub fn custom_fg(mut self, color: Hsla) -> Self {
    self.custom_fg = Some(color);
    self
  }

  /// Sets a custom border color (only for the Custom variant).
  pub fn custom_border(mut self, color: Hsla) -> Self {
    self.custom_border = Some(color);
    self
  }
}

impl Styled for Tag {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Tag {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for Tag {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let (default_bg, default_fg, default_border) = match self.variant {
      TagVariant::Primary => (theme.primary, theme.primary_foreground, theme.primary),
      TagVariant::Secondary => (theme.muted, theme.foreground, theme.border),
      TagVariant::Danger => (theme.danger, theme.primary_foreground, theme.danger),
      TagVariant::Success => (theme.success, theme.primary_foreground, theme.success),
      TagVariant::Warning => (theme.warning, theme.primary_foreground, theme.warning),
      TagVariant::Info => (theme.ring, theme.primary_foreground, theme.ring),
      TagVariant::Custom => (
        self.custom_bg.unwrap_or(theme.muted),
        self.custom_fg.unwrap_or(theme.foreground),
        self.custom_border.unwrap_or(theme.border),
      ),
    };

    let bg = if self.outline {
      Hsla::transparent_black()
    } else {
      default_bg
    };
    let fg = if self.outline {
      default_border
    } else {
      default_fg
    };
    let rounded = self
      .rounded
      .unwrap_or(theme.radius.to_pixels(window.rem_size()));
    // same padding contract as Button: 0.5rem all sides, inset by the border
    // width so the outline never changes the geometry.
    let pad = rems(0.5).to_pixels(window.rem_size()) - theme.border_width;

    div()
      .flex()
      .items_center()
      .border(theme.border_width)
      .line_height(gpui::relative(1.0))
      .p(pad)
      .bg(bg)
      .text_color(fg)
      .border_color(default_border)
      .rounded(rounded)
      .hover(|this| this.opacity(0.9))
      .refine_style(&self.style)
      .children(self.children)
  }
}
