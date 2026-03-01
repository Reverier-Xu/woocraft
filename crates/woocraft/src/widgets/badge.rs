//! Notification badge component for displaying counts, status dots, or icons.
//!
//! Badge is a small overlay element typically positioned on top of buttons or
//! list items to indicate notifications, unread counts, or status. Supports
//! three display modes: numeric count (with optional max), simple dot, or
//! custom icon. Commonly placed in the top-right corner of avatars or buttons.
//!
//! # Features
//! - **Numeric Display**: Show unread/notification count (auto-capped at max
//!   value like "99+")
//! - **Simple Dot**: Minimal notification indicator (no number)
//! - **Icon Mode**: Display custom status icon instead of count
//! - **Color Control**: Customizable background color (default: danger/red)
//! - **Size Variants**: Small, Medium, Large sizing options
//! - **Visibility Control**: Automatically hides when count is zero (for number
//!   mode)
//!
//! # Example
//! ```rust,ignore
//! // Notification count badge
//! Avatar::new().child(
//!   Badge::new()
//!     .count(5)
//!     .max(99)
//! )
//!
//! // Status dot
//! Icon::new(IconName::User).child(
//!   Badge::new().dot()
//! )
//! ```

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
/// Small notification or status indicator badge.
///
/// Badge is an overlay component positioned on top of other UI elements
/// (buttons, avatars, icons) to show notifications or status. Displays as a
/// colored circle with a count, simple dot, or icon. Designed to be nested as a
/// child of other elements.
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
  /// Create a new badge in numeric count mode.
  ///
  /// Displays a count (with optional maximum). Default count is 0 (hidden).
  /// Use `count()` to set the number to display.
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

  /// Switch badge to simple dot mode (no number).
  ///
  /// Shows a small colored circle instead of a count. Useful for online status
  /// or simple presence indicators. Builder method.
  pub fn dot(mut self) -> Self {
    self.variant = BadgeVariant::Dot;
    self
  }

  /// Set the notification count to display.
  ///
  /// The count is capped at the maximum value set via `max()`. Zero hides the
  /// badge in numeric mode.
  pub fn count(mut self, count: usize) -> Self {
    self.count = count;
    self
  }

  /// Switch badge to icon mode, displaying a custom icon instead of count.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.variant = BadgeVariant::Icon(Box::new(icon.into()));
    self
  }

  /// Set the maximum value for count display (e.g., "99+" when count > 99).
  ///
  /// Default: 99. Affects how large counts are formatted ("123" becomes "99+").
  pub fn max(mut self, max: usize) -> Self {
    self.max = max;
    self
  }

  /// Set the badge background color.
  ///
  /// Default: danger (red). Common values: primary, success, warning, danger.
  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }
}

impl_parent_element!(Badge);
impl_sizable!(Badge);
impl_styled!(Badge);

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
          .component_padding(size)
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
