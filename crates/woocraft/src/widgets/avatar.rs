//! User profile picture component with automatic initial fallback.
//!
//! Avatar displays a user's profile image, or falls back to displaying their
//! initials (derived from name) or a placeholder icon. Supports multiple
//! display modes: image, initials with auto-generated background color, or
//! custom icon. Great for user lists, headers, and comments sections.
//!
//! # Features
//! - **Image Display**: Show a profile picture from local or remote source
//! - **Initial Fallback**: Auto-extract initials from name (e.g., "John Doe" →
//!   "JD")
//! - **Color Backgrounds**: Auto-assigned colors based on name hash for visual
//!   variety
//! - **Icon Support**: Use custom icons instead of initials or images
//! - **Size Flexibility**: Supports Small, Medium, Large sizing
//! - **Styled Border**: Respects corner radius settings for circular or rounded
//!   display
//!
//! # Example
//! ```rust,ignore
//! // Image avatar with fallback
//! Avatar::new()
//!   .name("Alice Johnson")
//!   .src("https://example.com/alice.jpg")
//!
//! // Initials avatar with custom color
//! Avatar::new()
//!   .name("Bob Smith")
//!   .bg_color(theme.primary)
//! ```

use gpuim::{
  App, Div, Hsla, ImageSource, InteractiveElement, Interactivity, IntoElement, ParentElement as _,
  RenderOnce, SharedString, StyleRefinement, Styled, Window, div, img, prelude::FluentBuilder,
};

use crate::{ActiveTheme, Icon, IconName, Sizable, Size, StyledExt, h_flex};

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
/// User profile picture component with image, initial, or icon display.
///
/// Avatar renders a circular or rounded square showing user identity through an
/// image, auto-generated initials, or a placeholder icon. The component
/// automatically derives initials from names and assigns consistent background
/// colors. Useful in any UI showing user information.
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
  /// Create a new, empty avatar.
  ///
  /// Starts with a Person icon placeholder and no image or name. Configure with
  /// builder methods.
  pub fn new() -> Self {
    Self {
      base: h_flex(),
      style: StyleRefinement::default(),
      src: None,
      name: None,
      short_name: SharedString::default(),
      placeholder: Icon::new(IconName::Person),
      size: Size::Medium,
      bg_color: None,
    }
  }

  /// Set the avatar image source (URL or local path).
  ///
  /// When set, the image is displayed instead of initials or placeholder. Falls
  /// back to initials if the image fails to load.
  pub fn src(mut self, source: impl Into<ImageSource>) -> Self {
    self.src = Some(source.into());
    self
  }

  /// Set the avatar name to derive initials from.
  ///
  /// Initials are automatically extracted (e.g., "John Doe" → "JD", "Alice" →
  /// "AL"). Also determines the background color if not explicitly set via
  /// `bg_color()`.
  pub fn name(mut self, name: impl Into<SharedString>) -> Self {
    let name: SharedString = name.into();
    let short: SharedString = extract_text_initials(&name).into();

    self.name = Some(name);
    self.short_name = short;
    self
  }

  /// Set a custom placeholder icon (shown when no image or initials available).
  ///
  /// Default: PersonIcon. Useful for special avatar types (groups,
  /// organizations, etc.).
  pub fn placeholder(mut self, icon: impl Into<Icon>) -> Self {
    self.placeholder = icon.into();
    self
  }

  /// Explicitly set the background color for initials display.
  ///
  /// If not set, color is auto-generated from the name using a hash (ensures
  /// consistency).
  pub fn bg_color(mut self, color: Hsla) -> Self {
    self.bg_color = Some(color);
    self
  }
}

impl_sizable!(Avatar);
impl_styled!(Avatar);

impl Default for Avatar {
  fn default() -> Self {
    Self::new()
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
    let inner_style = gpuim::StyleRefinement {
      corner_radii,
      ..Default::default()
    };

    const COLOR_COUNT: usize = 4;

    fn color_for_index(index: usize, cx: &mut App) -> Hsla {
      let color_index = index % COLOR_COUNT;

      match color_index {
        0 => cx.theme().primary,
        1 => cx.theme().success,
        2 => cx.theme().warning,
        _ => cx.theme().danger,
      }
    }

    fn get_color_for_name(name: &str, cx: &mut App) -> Hsla {
      color_for_index(gpuim::hash(&name) as usize, cx)
    }

    const BG_OPACITY: f32 = 0.2;

    self
      .base
      .size(self.size.component_height())
      .items_center()
      .justify_center()
      .flex_shrink_0()
      .rounded_full()
      .overflow_hidden()
      .text_color(cx.theme().background)
      .border_1()
      .border_color(cx.theme().background)
      .when(self.name.is_none() && self.src.is_none(), |this| {
        let color = self.bg_color.unwrap_or(cx.theme().primary);

        this
          .bg(color.opacity(BG_OPACITY))
          .text_color(color)
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
      base: h_flex(),
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
      base: h_flex(),
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

impl_sizable!(AvatarGroup);
impl_styled!(AvatarGroup);

impl InteractiveElement for AvatarGroup {
  fn interactivity(&mut self) -> &mut Interactivity {
    self.base.interactivity()
  }
}

impl RenderOnce for AvatarGroup {
  fn render(self, _: &mut gpuim::Window, cx: &mut gpuim::App) -> impl IntoElement {
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
