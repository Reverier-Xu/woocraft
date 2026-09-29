//! Style utilities and trait extensions for layout, typography, and styling.
//!
//! Provides PixelsExt for Pixels conversions, helper functions for flex layouts
//! (h_flex, v_flex), and style-related traits. Font configuration lives in
//! [`super::fonts`].
//!
//! # Rem-based sizing
//!
//! All non-border sizes (spacing, gaps, dimensions, radii, font sizes, icon
//! sizes) are expressed in `rem`, so changing the window root font size scales
//! the whole UI — including text metrics — together. Values were normalized
//! from the previous pixel-based design at the default `1 rem = 16 px`
//! baseline, so the appearance is unchanged at that root size.
//!
//! Deliberate exceptions that stay in pixels:
//! - border widths, because hairlines must remain device-crisp;
//! - box shadows, because GPUI's [`BoxShadow`] stores pixels;
//! - runtime geometry measured from text shaping, canvas paths, drag positions
//!   and device-pixel snapping (editor viewports, terminal cells, plot/chart
//!   coordinates). Those already scale indirectly when the font size derived
//!   from `rem` changes.

use gpui::{
  AbsoluteLength, BoxShadow, Corners, DefiniteLength, Div, Edges, Hsla, Pixels, Refineable, Rems,
  StyleRefinement, Styled, div, point, rems,
};
use serde::{Deserialize, Serialize};

use super::fonts::default_font;

/// Trait for converting [`Pixels`] to primitive float values.
///
/// Provides convenience methods to convert GPUI Pixels to f32 or f64 for
/// mathematical operations.
pub trait PixelsExt {
  /// Converts Pixels to f32.
  fn as_f32(&self) -> f32;

  /// Converts Pixels to f64.
  fn as_f64(&self) -> f64;
}

impl PixelsExt for Pixels {
  fn as_f32(&self) -> f32 {
    f32::from(*self)
  }

  fn as_f64(&self) -> f64 {
    f64::from(*self)
  }
}

/// Arithmetic helpers for [`Rems`] that GPUI does not provide.
///
/// GPUI only implements `Rems * Pixels`. Design code frequently needs to scale
/// a rem length by a scalar factor, so this trait exposes an explicit
/// [`RemsExt::scale`] method (operator overloads for foreign types are not
/// possible due to the orphan rule).
pub trait RemsExt {
  /// Multiplies this rem length by a scalar `factor`.
  fn scale(self, factor: f32) -> Rems;

  /// Returns the smaller of the two rem lengths.
  fn min(self, other: Rems) -> Rems;

  /// Returns the larger of the two rem lengths.
  fn max(self, other: Rems) -> Rems;
}

impl RemsExt for Rems {
  #[inline]
  fn scale(self, factor: f32) -> Rems {
    rems(self.0 * factor)
  }

  #[inline]
  fn min(self, other: Rems) -> Rems {
    if other.0 < self.0 { other } else { self }
  }

  #[inline]
  fn max(self, other: Rems) -> Rems {
    if other.0 > self.0 { other } else { self }
  }
}

/// Returns a default-styled `Div` in horizontal flex layout mode.
///
/// Shortcut for `div().h_flex()`. Children are laid out left-to-right.
/// Font is set to the default locale-appropriate font (with CJK fallbacks as
/// needed).
#[inline(always)]
pub fn h_flex() -> Div {
  div().h_flex().font(default_font())
}

/// Returns a default-styled `Div` in vertical flex layout mode.
///
/// Shortcut for `div().v_flex()`. Children are laid out top-to-bottom.
/// Font is set to the default locale-appropriate font (with CJK fallbacks as
/// needed).
#[inline(always)]
pub fn v_flex() -> Div {
  div().v_flex().font(default_font())
}

/// Create a [`BoxShadow`] like CSS.
#[inline(always)]
pub fn box_shadow(
  x: impl Into<Pixels>, y: impl Into<Pixels>, blur: impl Into<Pixels>, spread: impl Into<Pixels>,
  color: Hsla,
) -> BoxShadow {
  BoxShadow {
    offset: point(x.into(), y.into()),
    blur_radius: blur.into(),
    spread_radius: spread.into(),
    color,
    inset: false,
  }
}

macro_rules! font_weight {
  ($fn:ident, $const:ident) => {
    #[inline]
    fn $fn(self) -> Self {
      self.font_weight(gpui::FontWeight::$const)
    }
  };
}

/// Extends [`gpui::Styled`] with common style helpers.
pub trait StyledExt: Styled + Sized {
  fn refine_style(mut self, style: &StyleRefinement) -> Self {
    self.style().refine(style);
    self
  }

  #[inline(always)]
  fn h_flex(self) -> Self {
    self.flex().flex_row().items_center()
  }

  #[inline(always)]
  fn v_flex(self) -> Self {
    self.flex().flex_col()
  }

  fn paddings<L>(self, paddings: impl Into<Edges<L>>) -> Self
  where
    L: Into<DefiniteLength> + Clone + Default + std::fmt::Debug + PartialEq, {
    let paddings = paddings.into();
    self
      .pt(paddings.top.into())
      .pb(paddings.bottom.into())
      .pl(paddings.left.into())
      .pr(paddings.right.into())
  }

  fn margins<L>(self, margins: impl Into<Edges<L>>) -> Self
  where
    L: Into<DefiniteLength> + Clone + Default + std::fmt::Debug + PartialEq, {
    let margins = margins.into();
    self
      .mt(margins.top.into())
      .mb(margins.bottom.into())
      .ml(margins.left.into())
      .mr(margins.right.into())
  }

  font_weight!(font_thin, THIN);
  font_weight!(font_extralight, EXTRA_LIGHT);
  font_weight!(font_light, LIGHT);
  font_weight!(font_normal, NORMAL);
  font_weight!(font_medium, MEDIUM);
  font_weight!(font_semibold, SEMIBOLD);
  font_weight!(font_bold, BOLD);
  font_weight!(font_extrabold, EXTRA_BOLD);
  font_weight!(font_black, BLACK);

  fn corner_radius<L>(self, radius: Corners<L>) -> Self
  where
    L: Into<AbsoluteLength> + Clone + std::fmt::Debug + Default + PartialEq, {
    self
      .rounded_tl(radius.top_left)
      .rounded_tr(radius.top_right)
      .rounded_bl(radius.bottom_left)
      .rounded_br(radius.bottom_right)
  }

  #[inline(always)]
  fn center(self) -> Self {
    self.items_center().justify_center()
  }

  #[inline(always)]
  fn v_center(self) -> Self {
    self.justify_center()
  }

  #[inline(always)]
  fn h_center(self) -> Self {
    self.items_center()
  }
}

impl<E: Styled> StyledExt for E {}

/// Component sizing scale ordered `Small < Medium < Large`; the derived
/// [`Ord`] matches the variant order, which [`Size::max`]/[`Size::min`] rely
/// on.
#[derive(Clone, Default, Copy, PartialEq, Eq, PartialOrd, Ord, Debug, Deserialize, Serialize)]
pub enum Size {
  Small,
  #[default]
  Medium,
  Large,
}

impl Size {
  pub fn as_str(&self) -> &'static str {
    match self {
      Size::Small => "sm",
      Size::Medium => "md",
      Size::Large => "lg",
    }
  }

  pub fn parse(size: &str) -> Self {
    match size.to_lowercase().as_str() {
      "sm" | "small" => Size::Small,
      "md" | "medium" => Size::Medium,
      "lg" | "large" => Size::Large,
      _ => Size::Medium,
    }
  }

  /// Font size for this size step.
  ///
  /// Returns a length in ems relative to this tier's font size.
  ///
  /// GPUI has no native `em` unit, so component-internal metrics are computed
  /// as multiples of [`Size::text_size`] in Rust. With the normalized scale
  /// (`text_size` = 0.75 / 1 / 1.25 rem) `1em` is exactly the tier font size
  /// and `2em` is the target single-line component height, so an em reference
  /// frame keeps every internal gap/padding proportional to the component's
  /// own text.
  #[inline]
  pub fn em(&self, factor: f32) -> Rems {
    rems(self.text_size().0 * factor)
  }

  /// Font size for this size step (the em base).
  ///
  /// Small: 0.75rem (12px), Medium: 1rem (16px), Large: 1.25rem (20px) at
  /// the default `1rem = 16px` baseline.
  #[inline]
  pub fn text_size(&self) -> Rems {
    match self {
      Size::Small => rems(0.75),
      Size::Medium => rems(1.),
      Size::Large => rems(1.25),
    }
  }

  #[inline]
  pub fn icon_size(&self) -> Rems {
    self.text_size()
  }

  /// Target single-line border-box height: `2em`.
  #[inline]
  pub fn component_height(&self) -> Rems {
    self.em(2.)
  }

  #[inline]
  pub fn container_height(&self) -> Rems {
    match self {
      Size::Small => rems(2.),
      Size::Medium => rems(2.5),
      Size::Large => rems(3.),
    }
  }

  #[inline]
  pub fn multiline_container_height(&self, lines: u32) -> Rems {
    let lines = lines.max(1) as f32;
    rems(self.container_height().0 * lines)
  }

  #[inline]
  pub fn component_radius(&self) -> Rems {
    self.em(0.25)
  }

  #[inline]
  pub fn container_radius(&self) -> Rems {
    match self {
      Size::Small => rems(0.25),
      Size::Medium => rems(0.375),
      Size::Large => rems(0.5),
    }
  }

  /// Horizontal (left/right) container padding.
  ///
  /// Containers are aligned on the absolute rem grid (independent of the inner
  /// font), so both axes use a flat 0.25rem.
  #[inline]
  pub fn container_px(&self) -> Rems {
    rems(0.25)
  }

  /// Vertical (top/bottom) container padding.
  ///
  /// Flat 0.25rem like the horizontal axis: a container closes on
  /// `container_height` because its inner content is a full-size component
  /// (`2em` = `component_height`), so `2em + 2 × 0.25rem = 2.5rem` at Medium.
  #[inline]
  pub fn container_py(&self) -> Rems {
    rems(0.25)
  }

  #[inline]
  pub fn table_row_height(&self) -> Rems {
    self.component_height()
  }

  #[inline]
  pub fn table_cell_padding(&self) -> Edges<Rems> {
    let padding = self.container_px();
    Edges {
      top: self.container_py(),
      bottom: self.container_py(),
      left: padding,
      right: padding,
    }
  }

  pub fn smaller(&self) -> Self {
    match self {
      Size::Small => Size::Small,
      Size::Medium => Size::Small,
      Size::Large => Size::Medium,
    }
  }

  pub fn larger(&self) -> Self {
    match self {
      Size::Small => Size::Medium,
      Size::Medium => Size::Large,
      Size::Large => Size::Large,
    }
  }

  /// Returns the larger of the two sizes (`Small < Medium < Large`).
  pub fn max(&self, other: Self) -> Self {
    if other > *self { other } else { *self }
  }

  /// Returns the smaller of the two sizes (`Small < Medium < Large`).
  pub fn min(&self, other: Self) -> Self {
    if other < *self { other } else { *self }
  }

  /// Horizontal component padding: `0.5em`.
  #[inline]
  pub fn component_px(&self) -> Rems {
    self.em(0.5)
  }

  /// Vertical component padding: `0.5em - 1px`.
  ///
  /// Chosen so a single-line component with a 1px border closes on
  /// `component_height` (`2em`): `1em + 2*(0.5em - 1px) + 2*1px = 2em`. Height
  /// is never fixed, so multi-line content simply grows.
  #[inline]
  pub fn component_py(&self) -> Rems {
    rems(self.em(0.5).0 - 0.0625)
  }

  #[inline]
  pub fn component_padding(&self) -> Edges<Rems> {
    let px = self.component_px();
    Edges {
      top: self.component_py(),
      bottom: self.component_py(),
      left: px,
      right: px,
    }
  }

  #[inline]
  pub fn container_padding(&self) -> Edges<Rems> {
    let px = self.container_px();
    Edges {
      top: self.container_py(),
      bottom: self.container_py(),
      left: px,
      right: px,
    }
  }

  /// Spacing between inner components in a Container (container-level).
  ///
  /// Flat 0.25rem so containers align on the absolute grid.
  #[inline]
  pub fn container_gap(&self) -> Rems {
    rems(0.25)
  }

  /// Spacing between inner elements in a component (component-level): `0.5em`.
  #[inline]
  pub fn component_gap(&self) -> Rems {
    self.em(0.5)
  }

  /// Height system for interactive tracks/sliders: `1.25em`.
  #[inline]
  pub fn track_height(&self) -> Rems {
    self.em(1.25)
  }

  /// Slider thumb size: `1em` (equals the tier font size).
  #[inline]
  pub fn thumb_size(&self) -> Rems {
    self.text_size()
  }

  /// Track/slider thickness: `0.125em`.
  #[inline]
  pub fn track_thickness(&self) -> Rems {
    self.em(0.125)
  }

  /// Badge dot size: `0.375em`.
  #[inline]
  pub fn badge_dot_size(&self) -> Rems {
    self.em(0.375)
  }

  /// Circular progress bar diameter (based on container height)
  #[inline]
  pub fn circle_diameter(&self) -> Rems {
    self.container_height()
  }
}

pub trait Selectable: Sized {
  fn selected(self, selected: bool) -> Self;
  fn is_selected(&self) -> bool;

  fn secondary_selected(self, _: bool) -> Self {
    self
  }
}

pub trait Disableable {
  fn disabled(self, disabled: bool) -> Self;
}

pub trait Sizable: Sized {
  fn with_size(self, size: impl Into<Size>) -> Self;

  #[inline(always)]
  fn small(self) -> Self {
    self.with_size(Size::Small)
  }

  #[inline(always)]
  fn medium(self) -> Self {
    self.with_size(Size::Medium)
  }

  #[inline(always)]
  fn large(self) -> Self {
    self.with_size(Size::Large)
  }
}

pub trait Collapsible {
  fn collapsed(self, collapsed: bool) -> Self;
  fn is_collapsed(&self) -> bool;
}

pub trait StyleSized<T: Styled> {
  fn component_size(self, size: Size) -> Self;
  fn component_pl(self, size: Size) -> Self;
  fn component_pr(self, size: Size) -> Self;
  fn component_px(self, size: Size) -> Self;
  fn component_py(self, size: Size) -> Self;
  fn component_h(self, size: Size) -> Self;
  fn component_min_h(self, size: Size) -> Self;
  fn component_rounded(self, size: Size) -> Self;
  fn container_h(self, size: Size) -> Self;
  fn container_min_h(self, size: Size) -> Self;
  fn container_multiline_h(self, size: Size, lines: u32) -> Self;
  fn container_rounded(self, size: Size) -> Self;
  fn container_px(self, size: Size) -> Self;
  fn container_py(self, size: Size) -> Self;
  fn container_size(self, size: Size) -> Self;
  fn container_gap(self, size: Size) -> Self;
  fn container_gap_x(self, size: Size) -> Self;
  fn container_gap_y(self, size: Size) -> Self;
  fn component_gap(self, size: Size) -> Self;
  fn list_size(self, size: Size) -> Self;
  fn list_px(self, size: Size) -> Self;
  fn list_py(self, size: Size) -> Self;
  fn size_with(self, size: Size) -> Self;
  fn component_padding(self, size: Size) -> Self;
  fn container_padding(self, size: Size) -> Self;
}

impl<T: Styled> StyleSized<T> for T {
  #[inline]
  fn component_size(self, size: Size) -> Self {
    // No fixed height: the computed `component_py` plus the content determine
    // the height, so multi-line content can grow. `line_height` is pinned to
    // `1em` so a single line closes exactly on `component_height`.
    self
      .component_px(size)
      .component_py(size)
      .component_rounded(size)
      .line_height(size.text_size())
  }

  #[inline]
  fn component_pl(self, size: Size) -> Self {
    self.pl(size.component_px())
  }

  #[inline]
  fn component_pr(self, size: Size) -> Self {
    self.pr(size.component_px())
  }

  #[inline]
  fn component_px(self, size: Size) -> Self {
    self.px(size.component_px())
  }

  #[inline]
  fn component_py(self, size: Size) -> Self {
    self.py(size.component_py())
  }

  #[inline]
  fn component_h(self, size: Size) -> Self {
    self.h(size.component_height())
  }

  #[inline]
  fn component_min_h(self, size: Size) -> Self {
    self.min_h(size.component_height())
  }

  #[inline]
  fn component_rounded(self, size: Size) -> Self {
    self.rounded(size.component_radius())
  }

  #[inline]
  fn container_h(self, size: Size) -> Self {
    self.h(size.container_height())
  }

  #[inline]
  fn container_min_h(self, size: Size) -> Self {
    self.min_h(size.container_height())
  }

  #[inline]
  fn container_multiline_h(self, size: Size, lines: u32) -> Self {
    self.h(size.multiline_container_height(lines))
  }

  #[inline]
  fn container_rounded(self, size: Size) -> Self {
    self.rounded(size.container_radius())
  }

  #[inline]
  fn container_px(self, size: Size) -> Self {
    self.px(size.container_px())
  }

  #[inline]
  fn container_py(self, size: Size) -> Self {
    self.py(size.container_py())
  }

  #[inline]
  fn container_size(self, size: Size) -> Self {
    self
      .container_px(size)
      .container_py(size)
      .container_min_h(size)
  }

  #[inline]
  fn container_gap(self, size: Size) -> Self {
    self.gap(size.container_gap())
  }

  #[inline]
  fn container_gap_x(self, size: Size) -> Self {
    self.gap_x(size.container_gap())
  }

  #[inline]
  fn container_gap_y(self, size: Size) -> Self {
    self.gap_y(size.container_gap())
  }

  #[inline]
  fn component_gap(self, size: Size) -> Self {
    self.gap(size.component_gap())
  }

  #[inline]
  fn list_size(self, size: Size) -> Self {
    self
      .list_px(size)
      .list_py(size)
      .container_h(size)
      .text_size(size.text_size())
  }

  #[inline]
  fn list_px(self, size: Size) -> Self {
    self.px(size.component_px())
  }

  #[inline]
  fn list_py(self, size: Size) -> Self {
    self.py(size.component_py())
  }

  #[inline]
  fn size_with(self, size: Size) -> Self {
    self.size(size.icon_size())
  }

  #[inline]
  fn component_padding(self, size: Size) -> Self {
    let padding = size.component_padding();
    self
      .pt(padding.top)
      .pb(padding.bottom)
      .pl(padding.left)
      .pr(padding.right)
  }

  #[inline]
  fn container_padding(self, size: Size) -> Self {
    let padding = size.container_padding();
    self
      .pt(padding.top)
      .pb(padding.bottom)
      .pl(padding.left)
      .pr(padding.right)
  }
}

use crate::base::theme::Theme;

pub trait CardStyle: Styled + Sized {
  /// Bordered card surface. Reserved for components/popovers that must read as
  /// a raised panel; layout containers use [`CardStyle::container_style`].
  #[inline]
  fn card_style(self, theme: &Theme) -> Self {
    self
      .bg(theme.card)
      .text_color(theme.card_foreground)
      .border_1()
      .border_color(theme.border)
      .rounded(theme.radius_container)
  }

  /// Borderless container surface. Layout containers carry no border per the
  /// design rules; add an explicit 1px `Divider` element when separation is
  /// needed.
  #[inline]
  fn container_style(self, theme: &Theme) -> Self {
    self
      .bg(theme.card)
      .text_color(theme.card_foreground)
      .rounded(theme.radius_container)
  }

  /// Floating popover surface: borderless + shadow.
  #[inline]
  fn popover_style(self, theme: &Theme) -> Self {
    self.container_style(theme).shadow_sm()
  }

  #[inline]
  fn tooltip_style(self, theme: &Theme) -> Self {
    self.popover_style(theme).rounded(theme.radius)
  }
}

impl<E: Styled> CardStyle for E {}

#[cfg(test)]
mod tests {
  use gpui::rems;

  use super::Size;

  #[test]
  fn test_size_max_min() {
    assert_eq!(Size::Small.min(Size::Medium), Size::Small);
    assert_eq!(Size::Medium.min(Size::Large), Size::Medium);
    assert_eq!(Size::Large.min(Size::Small), Size::Small);
    assert_eq!(Size::Medium.min(Size::Medium), Size::Medium);

    assert_eq!(Size::Small.max(Size::Medium), Size::Medium);
    assert_eq!(Size::Medium.max(Size::Large), Size::Large);
    assert_eq!(Size::Large.max(Size::Small), Size::Large);
    assert_eq!(Size::Medium.max(Size::Medium), Size::Medium);
  }

  #[test]
  fn test_size_as_str() {
    assert_eq!(Size::Small.as_str(), "sm");
    assert_eq!(Size::Medium.as_str(), "md");
    assert_eq!(Size::Large.as_str(), "lg");
  }

  #[test]
  fn test_size_from_str() {
    assert_eq!(Size::parse("sm"), Size::Small);
    assert_eq!(Size::parse("small"), Size::Small);
    assert_eq!(Size::parse("md"), Size::Medium);
    assert_eq!(Size::parse("medium"), Size::Medium);
    assert_eq!(Size::parse("lg"), Size::Large);
    assert_eq!(Size::parse("large"), Size::Large);
    assert_eq!(Size::parse("unknown"), Size::Medium);

    assert_eq!(Size::parse("xs"), Size::Medium);
    assert_eq!(Size::parse("xsmall"), Size::Medium);

    assert_eq!(Size::parse("SMALL"), Size::Small);
    assert_eq!(Size::parse("Md"), Size::Medium);
  }

  #[test]
  fn test_size_metrics() {
    assert_eq!(Size::Small.text_size(), rems(0.75));
    assert_eq!(Size::Medium.text_size(), rems(1.));
    assert_eq!(Size::Large.text_size(), rems(1.25));

    assert_eq!(Size::Small.icon_size(), rems(0.75));
    assert_eq!(Size::Medium.icon_size(), rems(1.));
    assert_eq!(Size::Large.icon_size(), rems(1.25));

    assert_eq!(Size::Small.component_height(), rems(1.5));
    assert_eq!(Size::Medium.component_height(), rems(2.));
    assert_eq!(Size::Large.component_height(), rems(2.5));

    assert_eq!(Size::Small.container_height(), rems(2.));
    assert_eq!(Size::Medium.container_height(), rems(2.5));
    assert_eq!(Size::Large.container_height(), rems(3.));

    assert_eq!(Size::Medium.multiline_container_height(1), rems(2.5));
    assert_eq!(Size::Medium.multiline_container_height(2), rems(5.));

    assert_eq!(Size::Small.component_radius(), rems(0.1875));
    assert_eq!(Size::Medium.component_radius(), rems(0.25));
    assert_eq!(Size::Large.component_radius(), rems(0.3125));

    assert_eq!(Size::Small.container_radius(), rems(0.25));
    assert_eq!(Size::Medium.container_radius(), rems(0.375));
    assert_eq!(Size::Large.container_radius(), rems(0.5));

    assert_eq!(Size::Small.component_px(), rems(0.375));
    assert_eq!(Size::Medium.component_px(), rems(0.5));
    assert_eq!(Size::Large.component_px(), rems(0.625));

    assert_eq!(Size::Small.component_py(), rems(0.3125));
    assert_eq!(Size::Medium.component_py(), rems(0.4375));
    assert_eq!(Size::Large.component_py(), rems(0.5625));

    let padding = Size::Medium.component_padding();
    assert_eq!(padding.top, rems(0.4375));
    assert_eq!(padding.bottom, rems(0.4375));
    assert_eq!(padding.left, rems(0.5));
    assert_eq!(padding.right, rems(0.5));
  }
}
