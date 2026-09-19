//! Notification badge component for displaying counts, status dots, or icons.
//!
//! Badge is a small overlay element typically positioned on top of buttons or
//! list items to indicate notifications, unread counts, or status. Supports
//! three display modes: numeric count (with optional max), simple dot, or
//! custom icon. Commonly placed in the top-right corner of avatars or buttons.
//!
//! # Example
//! ```rust,ignore
//! // notification count badge
//! div().relative().child(
//!   Badge::new().count(5).max(99)
//! )
//!
//! // status dot
//! Badge::new().dot()
//! ```

use gpui::{
  AnyElement, App, Hsla, IntoElement, ParentElement, RenderOnce, StyleRefinement, Styled, Window,
  div, prelude::FluentBuilder as _, rems,
};

use crate::{
  ActiveTheme, Icon,
  base::{StyledExt, h_flex},
};

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
/// Badge is an overlay component positioned on top of other UI elements to
/// show notifications or status. Displays as a colored circle with a count, a
/// simple dot, or an icon; nest it inside a `relative` container.
pub struct Badge {
  style: StyleRefinement,
  count: usize,
  max: usize,
  variant: BadgeVariant,
  children: Vec<AnyElement>,
  color: Option<Hsla>,
}

impl Default for Badge {
  fn default() -> Self {
    Self::new()
  }
}

impl Badge {
  /// Creates a badge in numeric count mode. Zero hides the badge.
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      count: 0,
      max: 99,
      variant: BadgeVariant::default(),
      children: Vec::new(),
      color: None,
    }
  }

  /// Switches to dot mode: a small colored circle, no number.
  pub fn dot(mut self) -> Self {
    self.variant = BadgeVariant::Dot;
    self
  }

  /// Sets the notification count. Capped at `max`; zero hides the badge.
  pub fn count(mut self, count: usize) -> Self {
    self.count = count;
    self
  }

  /// Switches to icon mode, showing a custom icon instead of a count.
  pub fn icon(mut self, icon: impl Into<Icon>) -> Self {
    self.variant = BadgeVariant::Icon(Box::new(icon.into()));
    self
  }

  /// Sets the cap for count display (`"99+"` when count > 99). default 99.
  pub fn max(mut self, max: usize) -> Self {
    self.max = max;
    self
  }

  /// Sets the badge fill color. Defaults to the theme danger color.
  pub fn color(mut self, color: impl Into<Hsla>) -> Self {
    self.color = Some(color.into());
    self
  }
}

impl Styled for Badge {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Badge {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
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

    // icon-mode badges hang on the bottom-right corner; all other modes on
    // the top-right corner.
    let bottom_right_anchor = matches!(self.variant, BadgeVariant::Icon(_));

    let overlay = match self.variant {
      BadgeVariant::Dot => h_flex()
        .justify_center()
        .items_center()
        .rounded_full()
        .bg(badge_color)
        .text_color(text_color)
        .size(rems(0.625))
        .into_any_element(),
      BadgeVariant::Number => {
        let count = if self.count > self.max {
          format!("{}+", self.max)
        } else {
          self.count.to_string()
        };

        h_flex()
          .justify_center()
          .items_center()
          .rounded_full()
          .bg(badge_color)
          .text_color(text_color)
          .h(rems(1.))
          .min_w(rems(1.))
          .px(rems(0.25))
          .line_height(rems(1.))
          .child(count)
          .into_any_element()
      }
      BadgeVariant::Icon(icon) => h_flex()
        .justify_center()
        .items_center()
        .rounded_full()
        .bg(badge_color)
        .text_color(text_color)
        .size(rems(1.25))
        .border_1()
        .border_color(cx.theme().background)
        .child(*icon)
        .into_any_element(),
    };

    // the badge is centered on the host's top-right corner (bottom-right for
    // icon mode): its center then sits on the 45° diagonal from the host
    // center, regardless of host or badge size. the anchor is a zero-size
    // absolutely-positioned flex box, so the centering needs no per-variant
    // pixel math.
    div()
      .relative()
      .refine_style(&self.style)
      .children(self.children)
      .when(visible, |this| {
        // explicit zero size: an absolute div with auto size would shrink-wrap
        // the overlay and the flex centering would become a no-op.
        let anchor = if bottom_right_anchor {
          div().absolute().bottom_0().right_0().size(rems(0.))
        } else {
          div().absolute().top_0().right_0().size(rems(0.))
        };
        this.child(anchor.flex().items_center().justify_center().child(overlay))
      })
  }
}
