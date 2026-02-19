use gpui::{
  AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
  div, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme, Icon, Sizable, Size, StyleSized, StyledExt, h_flex};

#[derive(Default, Clone)]
enum BadgeVariant {
  #[default]
  Number,
  Dot,
  Icon(Box<Icon>),
}

#[derive(IntoElement)]
pub struct Badge {
  style: StyleRefinement,
  count: usize,
  max: usize,
  variant: BadgeVariant,
  children: Vec<AnyElement>,
  color: Option<Hsla>,
  size: Size,
}

impl Default for Badge {
  fn default() -> Self {
    Self::new()
  }
}

impl Badge {
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      count: 0,
      max: 99,
      variant: BadgeVariant::default(),
      children: Vec::new(),
      color: None,
      size: Size::default(),
    }
  }

  pub fn dot(mut self) -> Self {
    self.variant = BadgeVariant::Dot;
    self
  }

  pub fn count(mut self, count: usize) -> Self {
    self.count = count;
    self
  }

  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.variant = BadgeVariant::Icon(Box::new(icon.into()));
    self
  }

  pub fn max(mut self, max: usize) -> Self {
    self.max = max;
    self
  }

  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }
}

impl ParentElement for Badge {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl Sizable for Badge {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for Badge {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for Badge {
  fn render(self, _window: &mut Window, cx: &mut App) -> impl IntoElement {
    let visible = match self.variant {
      BadgeVariant::Number => self.count > 0,
      BadgeVariant::Dot | BadgeVariant::Icon(_) => true,
    };

    let badge_color = self.color.unwrap_or(cx.theme().danger);
    let text_color = cx.theme().primary_foreground;

    // Badge Medium = Small Input Size
    let size = self.size.smaller();

    let overlay = match self.variant {
      BadgeVariant::Dot => h_flex()
        .absolute()
        .justify_center()
        .items_center()
        .rounded_full()
        .bg(badge_color)
        .text_color(text_color)
        .text_xs()
        .top_0()
        .right_0()
        .size(size.badge_dot_size())
        .into_any_element(),
      BadgeVariant::Number => {
        let count = if self.count > self.max {
          format!("{}+", self.max)
        } else {
          self.count.to_string()
        };

        h_flex()
          .absolute()
          .justify_center()
          .items_center()
          .rounded_full()
          .bg(badge_color)
          .text_color(text_color)
          .text_xs()
          .top(-px(5.0))
          .right(-px(6.0))
          .input_padding(size)
          .min_w(size.component_height())
          .line_height(px(12.0))
          .child(count)
          .into_any_element()
      }
      BadgeVariant::Icon(icon) => h_flex()
        .absolute()
        .justify_center()
        .items_center()
        .rounded_full()
        .bg(badge_color)
        .text_color(text_color)
        .right_0()
        .bottom_0()
        .size(size.component_height())
        .border_1()
        .border_color(cx.theme().background)
        .child((*icon).with_size(size))
        .into_any_element(),
    };

    div()
      .relative()
      .refine_style(&self.style)
      .children(self.children)
      .when(visible, |this| this.child(overlay))
  }
}
