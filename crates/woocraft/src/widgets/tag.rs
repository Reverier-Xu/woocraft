//! Styled label component for categorization and filtering.
//!
//! Tag displays short text content with colored background and border,
//! typically used to label categories, tags, statuses, or filter options.
//! Supports 6 semantic color variants (Primary, Secondary, Danger, Success,
//! Warning, Info) plus fully custom color settings.
//!
//! # Features
//! - **Semantic variants**: Primary, Secondary, Danger, Success, Warning, Info
//! - **Custom colors**: Full control over background, foreground, and border
//!   colors
//! - **Outline mode**: Transparent background with colored border and text
//! - **Rounded corners**: Configurable radius or fully rounded (pill) style
//! - **Flexible size**: Inherits from Size trait (Small, Medium, Large)
//!
//! # Example
//! ```rust,ignore
//! use woocraft::Tag;
//!
//! let tag = Tag::success().rounded_full().child("Featured");
//! let custom_tag = Tag::custom(red, white, red).child("Critical");
//! ```

use gpuim::{
  AnyElement, App, Hsla, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce,
  StyleRefinement, Styled, Window, div, px,
};

use crate::{ActiveTheme, Size, StyleSized, StyledExt};

/// Color variant for the tag/label component.
///
/// Determines the background, foreground, and border colors of the tag.
/// - Primary: Theme primary color (typically blue)
/// - Secondary: Muted/subdued colors (default)
/// - Danger: Warning/error red color
/// - Success: Positive/success green color
/// - Warning: Caution/alert orange/yellow color
/// - Info: Informational ring color
/// - Custom: User-specified RGB(A) colors via custom_bg/custom_fg/custom_border
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
/// Tag renders as a colored pill-shaped label with optional border. Commonly
/// used in filtering interfaces, category lists, and status indicators.
/// Supports both filled and outline (hollow) display modes.
pub struct Tag {
  style: StyleRefinement,
  variant: TagVariant,
  outline: bool,
  size: Size,
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
  /// Creates a new tag with Secondary variant (default neutral colors).
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      variant: TagVariant::default(),
      outline: false,
      size: Size::default(),
      rounded: None,
      custom_bg: None,
      custom_fg: None,
      custom_border: None,
      children: Vec::new(),
    }
  }

  /// Creates a new Primary variant tag (theme primary color, typically blue).
  pub fn primary() -> Self {
    Self::new().with_variant(TagVariant::Primary)
  }

  /// Creates a new Secondary variant tag (muted/subdued colors, default).
  pub fn secondary() -> Self {
    Self::new().with_variant(TagVariant::Secondary)
  }

  /// Creates a new Danger variant tag (red/error color).
  pub fn danger() -> Self {
    Self::new().with_variant(TagVariant::Danger)
  }

  /// Creates a new Success variant tag (green/positive color).
  pub fn success() -> Self {
    Self::new().with_variant(TagVariant::Success)
  }

  /// Creates a new Warning variant tag (orange/yellow caution color).
  pub fn warning() -> Self {
    Self::new().with_variant(TagVariant::Warning)
  }

  /// Creates a new Info variant tag (blue/info ring color).
  pub fn info() -> Self {
    Self::new().with_variant(TagVariant::Info)
  }

  /// Creates a custom tag with specified background, foreground, and border
  /// colors.
  ///
  /// Use this to override theme colors with specific RGB(A) values for unique
  /// color combinations not covered by semantic variants.
  pub fn custom(bg: Hsla, fg: Hsla, border: Hsla) -> Self {
    Self::new()
      .with_variant(TagVariant::Custom)
      .custom_bg(bg)
      .custom_fg(fg)
      .custom_border(border)
  }

  /// Sets the color variant (Primary, Secondary, Danger, Success, Warning,
  /// Info, Custom).
  pub fn with_variant(mut self, variant: TagVariant) -> Self {
    self.variant = variant;
    self
  }

  /// Switches to outline mode (transparent background, colored border and
  /// text).
  ///
  /// When enabled, background becomes transparent and border/text use the
  /// variant color.
  pub fn outline(mut self) -> Self {
    self.outline = true;
    self
  }

  /// Sets the corner radius to a specific pixel value.
  ///
  /// Defaults to theme radius if not set.
  pub fn rounded(mut self, radius: impl Into<Pixels>) -> Self {
    self.rounded = Some(radius.into());
    self
  }

  /// Sets a fully rounded (pill) style with large border radius.
  pub fn rounded_full(mut self) -> Self {
    self.rounded = Some(px(999.0));
    self
  }

  /// Sets a custom background color (only for Custom variant).
  pub fn custom_bg(mut self, color: Hsla) -> Self {
    self.custom_bg = Some(color);
    self
  }

  /// Sets a custom foreground/text color (only for Custom variant).
  pub fn custom_fg(mut self, color: Hsla) -> Self {
    self.custom_fg = Some(color);
    self
  }

  /// Sets a custom border color (only for Custom variant).
  pub fn custom_border(mut self, color: Hsla) -> Self {
    self.custom_border = Some(color);
    self
  }
}

impl_sizable!(Tag);

impl ParentElement for Tag {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl_styled!(Tag);

impl RenderOnce for Tag {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let (default_bg, default_fg, default_border) = match self.variant {
      TagVariant::Primary => (
        cx.theme().primary,
        cx.theme().primary_foreground,
        cx.theme().primary,
      ),
      TagVariant::Secondary => (cx.theme().muted, cx.theme().foreground, cx.theme().border),
      TagVariant::Danger => (
        cx.theme().danger,
        cx.theme().primary_foreground,
        cx.theme().danger,
      ),
      TagVariant::Success => (
        cx.theme().success,
        cx.theme().primary_foreground,
        cx.theme().success,
      ),
      TagVariant::Warning => (
        cx.theme().warning,
        cx.theme().primary_foreground,
        cx.theme().warning,
      ),
      TagVariant::Info => (
        cx.theme().ring,
        cx.theme().primary_foreground,
        cx.theme().ring,
      ),
      TagVariant::Custom => (
        self.custom_bg.unwrap_or(cx.theme().muted),
        self.custom_fg.unwrap_or(cx.theme().foreground),
        self.custom_border.unwrap_or(cx.theme().border),
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
    let rounded = self.rounded.unwrap_or(cx.theme().radius);

    // Tag/Badge Medium = Small Input Size
    let size = self.size.smaller();

    div()
      .flex()
      .items_center()
      .border_1()
      .line_height(gpuim::relative(1.0))
      .text_xs()
      .component_padding(size)
      .bg(bg)
      .text_color(fg)
      .border_color(default_border)
      .rounded(rounded)
      .hover(|this| this.opacity(0.9))
      .refine_style(&self.style)
      .children(self.children)
  }
}
