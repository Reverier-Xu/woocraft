use gpui::{
  BoxShadow, Corners, DefiniteLength, Div, Edges, Font, FontFallbacks, FontFeatures, Hsla, Pixels,
  Refineable, StyleRefinement, Styled, div, point, px,
};
use serde::{Deserialize, Serialize};

/// A trait for converting [`Pixels`] to primitive float values.
pub trait PixelsExt {
  fn as_f32(&self) -> f32;
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

pub fn default_font() -> Font {
  let locale = crate::locale();
  let cjk_fallbacks = if locale.starts_with("zh-hans") {
    vec![
      "Sarasa Mono SC".into(),
      "Source Han Sans CN".into(),
      "Source Han Sans SC".into(),
      "Noto Sans CJK SC".into(),
      "PingFang SC".into(),
      "Microsoft YaHei".into(),
      "Hiragino Sans GB".into(),
      "WenQuanYi Micro Hei".into(),
    ]
  } else if locale.starts_with("zh-hant") {
    vec![
      "PingFang TC".into(),
      "Heiti TC".into(),
      "Source Han Sans TC".into(),
      "Noto Sans CJK TC".into(),
      "Microsoft JhengHei".into(),
    ]
  } else if locale.starts_with("ja") {
    vec![
      "Hiragino Kaku Gothic ProN".into(),
      "Yu Gothic".into(),
      "Meiryo".into(),
      "Noto Sans CJK JP".into(),
      "Source Han Sans JP".into(),
    ]
  } else {
    vec![
      "Source Han Sans CN".into(),
      "Source Han Sans SC".into(),
      "Noto Sans CJK SC".into(),
      "PingFang SC".into(),
      "Microsoft YaHei".into(),
      "Hiragino Sans GB".into(),
      "WenQuanYi Micro Hei".into(),
    ]
  };

  Font {
    family: crate::DEFAULT_FONT_FAMILY.into(),
    weight: gpui::FontWeight::NORMAL,
    style: gpui::FontStyle::Normal,
    features: FontFeatures::default(),
    fallbacks: Some(FontFallbacks::from_fonts(cjk_fallbacks)),
  }
}

/// Returns a `Div` as horizontal flex layout.
#[inline(always)]
pub fn h_flex() -> Div {
  div().h_flex().font(default_font())
}

/// Returns a `Div` as vertical flex layout.
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
    L: Into<DefiniteLength> + Clone + Default + std::fmt::Debug + PartialEq,
  {
    let paddings = paddings.into();
    self
      .pt(paddings.top.into())
      .pb(paddings.bottom.into())
      .pl(paddings.left.into())
      .pr(paddings.right.into())
  }

  fn margins<L>(self, margins: impl Into<Edges<L>>) -> Self
  where
    L: Into<DefiniteLength> + Clone + Default + std::fmt::Debug + PartialEq,
  {
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

  fn corner_radius(self, radius: Corners<Pixels>) -> Self {
    self
      .rounded_tl(radius.top_left)
      .rounded_tr(radius.top_right)
      .rounded_bl(radius.bottom_left)
      .rounded_br(radius.bottom_right)
  }
}

impl<E: Styled> StyledExt for E {}

#[derive(Clone, Default, Copy, PartialEq, Eq, Debug, Deserialize, Serialize)]
pub enum Size {
  Small,
  #[default]
  Medium,
  Large,
}

impl Size {
  fn as_f32(&self) -> f32 {
    match self {
      Size::Small => 1.,
      Size::Medium => 2.,
      Size::Large => 3.,
    }
  }

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

  #[inline]
  pub fn text_size(&self) -> Pixels {
    match self {
      Size::Small => px(14.),
      Size::Medium => px(16.),
      Size::Large => px(18.),
    }
  }

  #[inline]
  pub fn icon_size(&self) -> Pixels {
    self.text_size()
  }

  #[inline]
  pub fn component_height(&self) -> Pixels {
    match self {
      Size::Small => px(24.),
      Size::Medium => px(32.),
      Size::Large => px(40.),
    }
  }

  #[inline]
  pub fn container_height(&self) -> Pixels {
    match self {
      Size::Small => px(32.),
      Size::Medium => px(40.),
      Size::Large => px(48.),
    }
  }

  #[inline]
  pub fn multiline_container_height(&self, lines: u32) -> Pixels {
    let lines = lines.max(1) as f32;
    self.container_height() * lines
  }

  #[inline]
  pub fn component_radius(&self) -> Pixels {
    match self {
      Size::Small => px(2.),
      Size::Medium => px(4.),
      Size::Large => px(6.),
    }
  }

  #[inline]
  pub fn container_radius(&self) -> Pixels {
    match self {
      Size::Small => px(4.),
      Size::Medium => px(6.),
      Size::Large => px(8.),
    }
  }

  #[inline]
  pub fn container_px(&self) -> Pixels {
    match self {
      Size::Small => px(2.),
      Size::Medium => px(4.),
      Size::Large => px(6.),
    }
  }

  #[inline]
  pub fn container_py(&self) -> Pixels {
    match self {
      Size::Small => px(2.),
      Size::Medium => px(4.),
      Size::Large => px(6.),
    }
  }

  #[inline]
  pub fn table_row_height(&self) -> Pixels {
    self.component_height()
  }

  #[inline]
  pub fn table_cell_padding(&self) -> Edges<Pixels> {
    let padding = self.input_px();
    Edges {
      top: self.input_py(),
      bottom: self.input_py(),
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

  pub fn max(&self, other: Self) -> Self {
    if self.as_f32() < other.as_f32() {
      *self
    } else {
      other
    }
  }

  pub fn min(&self, other: Self) -> Self {
    if self.as_f32() > other.as_f32() {
      *self
    } else {
      other
    }
  }

  pub fn input_px(&self) -> Pixels {
    match self {
      Self::Small => px(4.),
      Self::Medium => px(8.),
      Self::Large => px(12.),
    }
  }

  pub fn input_py(&self) -> Pixels {
    match self {
      Size::Small => px(2.),
      Size::Medium => px(4.),
      Size::Large => px(6.),
    }
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
  fn large(self) -> Self {
    self.with_size(Size::Large)
  }
}

pub trait Collapsible {
  fn collapsed(self, collapsed: bool) -> Self;
  fn is_collapsed(&self) -> bool;
}

pub trait StyleSized<T: Styled> {
  fn input_text_size(self, size: Size) -> Self;
  fn input_size(self, size: Size) -> Self;
  fn input_pl(self, size: Size) -> Self;
  fn input_pr(self, size: Size) -> Self;
  fn input_px(self, size: Size) -> Self;
  fn input_py(self, size: Size) -> Self;
  fn input_h(self, size: Size) -> Self;
  fn input_rounded(self, size: Size) -> Self;
  fn container_h(self, size: Size) -> Self;
  fn container_min_h(self, size: Size) -> Self;
  fn container_multiline_h(self, size: Size, lines: u32) -> Self;
  fn container_rounded(self, size: Size) -> Self;
  fn container_px(self, size: Size) -> Self;
  fn container_py(self, size: Size) -> Self;
  fn container_size(self, size: Size) -> Self;
  fn list_size(self, size: Size) -> Self;
  fn list_px(self, size: Size) -> Self;
  fn list_py(self, size: Size) -> Self;
  fn size_with(self, size: Size) -> Self;
  fn table_cell_size(self, size: Size) -> Self;
  fn button_text_size(self, size: Size) -> Self;
}

impl<T: Styled> StyleSized<T> for T {
  #[inline]
  fn input_text_size(self, size: Size) -> Self {
    self.text_size(size.text_size())
  }

  #[inline]
  fn input_size(self, size: Size) -> Self {
    self
      .input_px(size)
      .input_py(size)
      .input_h(size)
      .input_rounded(size)
  }

  #[inline]
  fn input_pl(self, size: Size) -> Self {
    self.pl(size.input_px())
  }

  #[inline]
  fn input_pr(self, size: Size) -> Self {
    self.pr(size.input_px())
  }

  #[inline]
  fn input_px(self, size: Size) -> Self {
    self.px(size.input_px())
  }

  #[inline]
  fn input_py(self, size: Size) -> Self {
    self.py(size.input_py())
  }

  #[inline]
  fn input_h(self, size: Size) -> Self {
    self.h(size.component_height())
  }

  #[inline]
  fn input_rounded(self, size: Size) -> Self {
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
  fn list_size(self, size: Size) -> Self {
    self
      .list_px(size)
      .list_py(size)
      .container_h(size)
      .input_text_size(size)
  }

  #[inline]
  fn list_px(self, size: Size) -> Self {
    self.px(size.input_px())
  }

  #[inline]
  fn list_py(self, size: Size) -> Self {
    self.py(size.input_py())
  }

  #[inline]
  fn size_with(self, size: Size) -> Self {
    self.size(size.icon_size())
  }

  #[inline]
  fn table_cell_size(self, size: Size) -> Self {
    let padding = size.table_cell_padding();
    self
      .input_text_size(size)
      .pl(padding.left)
      .pr(padding.right)
      .pt(padding.top)
      .pb(padding.bottom)
  }

  fn button_text_size(self, size: Size) -> Self {
    self.input_text_size(size)
  }
}

#[cfg(test)]
mod tests {
  use gpui::px;

  use super::Size;

  #[test]
  fn test_size_max_min() {
    assert_eq!(Size::Small.min(Size::Medium), Size::Medium);
    assert_eq!(Size::Medium.min(Size::Large), Size::Large);
    assert_eq!(Size::Large.min(Size::Small), Size::Large);

    assert_eq!(Size::Small.max(Size::Medium), Size::Small);
    assert_eq!(Size::Medium.max(Size::Large), Size::Medium);
    assert_eq!(Size::Large.max(Size::Small), Size::Small);
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
    assert_eq!(Size::Small.text_size(), px(14.));
    assert_eq!(Size::Medium.text_size(), px(16.));
    assert_eq!(Size::Large.text_size(), px(18.));

    assert_eq!(Size::Small.icon_size(), px(14.));
    assert_eq!(Size::Medium.icon_size(), px(16.));
    assert_eq!(Size::Large.icon_size(), px(18.));

    assert_eq!(Size::Small.component_height(), px(24.));
    assert_eq!(Size::Medium.component_height(), px(32.));
    assert_eq!(Size::Large.component_height(), px(40.));

    assert_eq!(Size::Small.container_height(), px(32.));
    assert_eq!(Size::Medium.container_height(), px(40.));
    assert_eq!(Size::Large.container_height(), px(48.));

    assert_eq!(Size::Medium.multiline_container_height(1), px(40.));
    assert_eq!(Size::Medium.multiline_container_height(2), px(80.));

    assert_eq!(Size::Small.component_radius(), px(2.));
    assert_eq!(Size::Medium.component_radius(), px(4.));
    assert_eq!(Size::Large.component_radius(), px(6.));

    assert_eq!(Size::Small.container_radius(), px(4.));
    assert_eq!(Size::Medium.container_radius(), px(6.));
    assert_eq!(Size::Large.container_radius(), px(8.));

    assert_eq!(Size::Small.input_px(), px(4.));
    assert_eq!(Size::Medium.input_px(), px(8.));
    assert_eq!(Size::Large.input_px(), px(12.));

    assert_eq!(Size::Small.input_py(), px(2.));
    assert_eq!(Size::Medium.input_py(), px(4.));
    assert_eq!(Size::Large.input_py(), px(6.));

    let padding = Size::Medium.table_cell_padding();
    assert_eq!(padding.top, px(4.));
    assert_eq!(padding.bottom, px(4.));
    assert_eq!(padding.left, px(8.));
    assert_eq!(padding.right, px(8.));
  }
}
