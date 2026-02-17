use gpui::{
  AnyElement, App, Hsla, InteractiveElement as _, IntoElement, ParentElement, Pixels, RenderOnce,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme, Sizable, Size, StyledExt};

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

  pub fn primary() -> Self {
    Self::new().with_variant(TagVariant::Primary)
  }

  pub fn secondary() -> Self {
    Self::new().with_variant(TagVariant::Secondary)
  }

  pub fn danger() -> Self {
    Self::new().with_variant(TagVariant::Danger)
  }

  pub fn success() -> Self {
    Self::new().with_variant(TagVariant::Success)
  }

  pub fn warning() -> Self {
    Self::new().with_variant(TagVariant::Warning)
  }

  pub fn info() -> Self {
    Self::new().with_variant(TagVariant::Info)
  }

  pub fn custom(bg: Hsla, fg: Hsla, border: Hsla) -> Self {
    Self::new()
      .with_variant(TagVariant::Custom)
      .custom_bg(bg)
      .custom_fg(fg)
      .custom_border(border)
  }

  pub fn with_variant(mut self, variant: TagVariant) -> Self {
    self.variant = variant;
    self
  }

  pub fn outline(mut self) -> Self {
    self.outline = true;
    self
  }

  pub fn rounded(mut self, radius: impl Into<Pixels>) -> Self {
    self.rounded = Some(radius.into());
    self
  }

  pub fn rounded_full(mut self) -> Self {
    self.rounded = Some(px(999.0));
    self
  }

  pub fn custom_bg(mut self, color: Hsla) -> Self {
    self.custom_bg = Some(color);
    self
  }

  pub fn custom_fg(mut self, color: Hsla) -> Self {
    self.custom_fg = Some(color);
    self
  }

  pub fn custom_border(mut self, color: Hsla) -> Self {
    self.custom_border = Some(color);
    self
  }
}

impl Sizable for Tag {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl ParentElement for Tag {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Styled for Tag {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

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

    div()
      .flex()
      .items_center()
      .border_1()
      .line_height(gpui::relative(1.0))
      .text_xs()
      .map(|this| match self.size {
        Size::Small => this.px_1p5().py_0p5(),
        Size::Medium => this.px_2().py_1(),
        Size::Large => this.px_2p5().py_1(),
      })
      .bg(bg)
      .text_color(fg)
      .border_color(default_border)
      .rounded(rounded)
      .hover(|this| this.opacity(0.9))
      .refine_style(&self.style)
      .children(self.children)
  }
}
