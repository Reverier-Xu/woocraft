use gpui::{
  App, Div, Hsla, ImageSource, InteractiveElement, Interactivity, IntoElement, ParentElement as _,
  RenderOnce, SharedString, StyleRefinement, Styled, Window, div, img, prelude::FluentBuilder,
};

use crate::{ActiveTheme, Icon, IconName, Sizable, Size, StyledExt};

fn extract_text_initials(text: &str) -> String {
  let mut result = text
    .split(' ')
    .flat_map(|word| word.chars().next().map(|c| c.to_string()))
    .take(2)
    .collect::<Vec<String>>()
    .join("");

  if result.len() == 1 {
    result = text.chars().take(2).collect::<String>();
  }

  result.to_uppercase()
}

#[derive(IntoElement)]
pub struct Avatar {
  base: Div,
  style: StyleRefinement,
  src: Option<ImageSource>,
  name: Option<SharedString>,
  short_name: SharedString,
  placeholder: Icon,
  size: Size,
  bg_color: Option<Hsla>,
}

impl Avatar {
  pub fn new() -> Self {
    Self {
      base: div(),
      style: StyleRefinement::default(),
      src: None,
      name: None,
      short_name: SharedString::default(),
      placeholder: Icon::new(IconName::Person),
      size: Size::Medium,
      bg_color: None,
    }
  }

  pub fn src(mut self, source: impl Into<ImageSource>) -> Self {
    self.src = Some(source.into());
    self
  }

  pub fn name(mut self, name: impl Into<SharedString>) -> Self {
    let name: SharedString = name.into();
    let short: SharedString = extract_text_initials(&name).into();

    self.name = Some(name);
    self.short_name = short;
    self
  }

  pub fn placeholder(mut self, icon: impl Into<Icon>) -> Self {
    self.placeholder = icon.into();
    self
  }

  pub fn bg_color(mut self, color: Hsla) -> Self {
    self.bg_color = Some(color);
    self
  }
}

impl Sizable for Avatar {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Default for Avatar {
  fn default() -> Self {
    Self::new()
  }
}

impl Styled for Avatar {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl InteractiveElement for Avatar {
  fn interactivity(&mut self) -> &mut Interactivity {
    self.base.interactivity()
  }
}

impl RenderOnce for Avatar {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let corner_radii = self.style.corner_radii.clone();
    let inner_style = gpui::StyleRefinement {
      corner_radii,
      ..Default::default()
    };

    const COLOR_COUNT: usize = 4;

    fn get_color_for_name(name: &str, cx: &mut App) -> Hsla {
      let hash = gpui::hash(&name) as usize;
      let color_index = hash % COLOR_COUNT;

      match color_index {
        0 => cx.theme().primary,
        1 => cx.theme().success,
        2 => cx.theme().warning,
        _ => cx.theme().danger,
      }
    }

    const BG_OPACITY: f32 = 0.2;

    self
      .base
      .size(self.size.component_height())
      .flex()
      .items_center()
      .justify_center()
      .flex_shrink_0()
      .rounded_full()
      .overflow_hidden()
      .text_color(cx.theme().background)
      .border_1()
      .border_color(cx.theme().background)
      .when(self.name.is_none() && self.src.is_none(), |this| {
        this
          .text_size(self.size.component_height() * 0.6)
          .child(self.placeholder.clone())
      })
      .map(|this| match self.src {
        None => this.when(self.name.is_some(), |this| {
          let color = self
            .bg_color
            .unwrap_or_else(|| get_color_for_name(&self.short_name, cx));

          this.bg(color.opacity(BG_OPACITY)).text_color(color).child(
            div()
              .text_size(self.size.text_size())
              .child(self.short_name.clone()),
          )
        }),
        Some(src) => this.child(
          img(src)
            .size(self.size.component_height())
            .rounded_full()
            .refine_style(&inner_style),
        ),
      })
      .refine_style(&self.style)
  }
}

impl Clone for Avatar {
  fn clone(&self) -> Self {
    Self {
      base: div(),
      style: self.style.clone(),
      src: self.src.clone(),
      name: self.name.clone(),
      short_name: self.short_name.clone(),
      placeholder: self.placeholder.clone(),
      size: self.size,
      bg_color: self.bg_color,
    }
  }
}

#[derive(IntoElement)]
pub struct AvatarGroup {
  base: Div,
  style: StyleRefinement,
  avatars: Vec<Avatar>,
  size: Size,
  limit: usize,
  ellipsis: bool,
}

impl AvatarGroup {
  pub fn new() -> Self {
    Self {
      base: div(),
      style: StyleRefinement::default(),
      avatars: Vec::new(),
      size: Size::default(),
      limit: 3,
      ellipsis: false,
    }
  }

  pub fn child(mut self, avatar: Avatar) -> Self {
    self.avatars.push(avatar);
    self
  }

  pub fn children(mut self, avatars: impl IntoIterator<Item = Avatar>) -> Self {
    self.avatars.extend(avatars);
    self
  }

  pub fn limit(mut self, limit: usize) -> Self {
    self.limit = limit;
    self
  }

  pub fn ellipsis(mut self) -> Self {
    self.ellipsis = true;
    self
  }
}

impl Default for AvatarGroup {
  fn default() -> Self {
    Self::new()
  }
}

impl Sizable for AvatarGroup {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for AvatarGroup {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl InteractiveElement for AvatarGroup {
  fn interactivity(&mut self) -> &mut Interactivity {
    self.base.interactivity()
  }
}

impl RenderOnce for AvatarGroup {
  fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl IntoElement {
    let item_ml = -self.size.component_height() * 0.3;
    let avatars_len = self.avatars.len();

    let show_ellipsis = self.ellipsis && avatars_len > self.limit;
    let visible_count = if show_ellipsis {
      self.limit.saturating_sub(1)
    } else {
      self.limit
    };

    let avatar_size = self.size.component_height();
    let total_width = if visible_count > 0 {
      avatar_size + (avatar_size + item_ml) * (visible_count - 1) as f32
    } else {
      avatar_size
    };

    self
      .base
      .h_flex()
      .items_center()
      .w(total_width)
      .refine_style(&self.style)
      .children(
        self
          .avatars
          .iter()
          .take(visible_count)
          .enumerate()
          .map(|(ix, item)| {
            item
              .clone()
              .with_size(self.size)
              .when(ix > 0, |this| this.ml(item_ml))
          }),
      )
      .when(show_ellipsis, |this| {
        this.child(
          Avatar::new()
            .name("+")
            .bg_color(cx.theme().muted)
            .text_color(cx.theme().muted_foreground)
            .with_size(self.size)
            .ml(item_ml),
        )
      })
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_avatar_text_initials() {
    assert_eq!(extract_text_initials("Jason Lee"), "JL".to_string());
    assert_eq!(extract_text_initials("Foo Bar Dar"), "FB".to_string());
    assert_eq!(extract_text_initials("huacnlee"), "HU".to_string());
  }

  #[test]
  fn test_avatar_builder() {
    let avatar = Avatar::new()
      .name("Jason Lee")
      .placeholder(Icon::new(IconName::Person))
      .large();

    assert_eq!(avatar.name, Some(SharedString::from("Jason Lee")));
    assert_eq!(avatar.short_name, SharedString::from("JL"));
    assert_eq!(avatar.size, Size::Large);
  }

  #[test]
  fn test_avatar_group_builder() {
    let group = AvatarGroup::new()
      .child(Avatar::new().name("Alice"))
      .child(Avatar::new().name("Bob"))
      .child(Avatar::new().name("Charlie"))
      .child(Avatar::new().name("David"))
      .large()
      .limit(3)
      .ellipsis();

    assert_eq!(group.avatars.len(), 4);
    assert_eq!(group.size, Size::Large);
    assert_eq!(group.limit, 3);
    assert!(group.ellipsis);
  }
}
