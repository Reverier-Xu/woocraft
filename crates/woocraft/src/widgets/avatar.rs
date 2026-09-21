//! avatar displaying a person's image with initials or icon fallback.
//!
//! this is a styled wrapper over the [`gpui_base::Avatar`] slots. the
//! wrapper adds the design-system vocabulary: a `2rem` circle, initials
//! extracted from a name, a deterministic accent hue derived from that
//! name, and a placeholder icon when neither image nor name is set. image
//! load failure rendering is the application's concern, matching the base
//! slot model.

use gpui::{
  App, Hsla, ImageSource, InteractiveElement, IntoElement, ParentElement as _, Refineable as _,
  RenderOnce, SharedString, StyleRefinement, Styled, Window, div, rems,
};
use gpui_base::{Avatar as BaseAvatar, AvatarFallback, AvatarImage};

use crate::{
  ActiveTheme, Icon, IconName,
  theme::{opacity, with_alpha},
};

/// extracts up to two uppercase leading characters from a display name.
///
/// multi-word names use the first letter of the first two words
/// ("Jason Lee" → "JL"); single-word names use the first two letters
/// ("huacnlee" → "HU").
pub(crate) fn extract_initials(name: &str) -> String {
  let mut initials: String = name
    .split(' ')
    .filter_map(|word| word.chars().next())
    .take(2)
    .collect();

  if initials.chars().count() == 1 {
    initials = name.chars().take(2).collect();
  }

  initials.to_uppercase()
}

/// derives a stable accent hue in `[0, 360)` from a display name so the
/// same person keeps the same avatar color across renders and across
/// builds — fnv-1a, because `DefaultHasher` makes no cross-version
/// stability guarantee.
fn hue_for_name(name: &str) -> f32 {
  const FNV_OFFSET: u64 = 0xCBF2_9CE4_8422_2325;
  const FNV_PRIME: u64 = 0x0000_0100_0000_01B3;
  let mut hash = FNV_OFFSET;
  for byte in name.as_bytes() {
    hash ^= u64::from(*byte);
    hash = hash.wrapping_mul(FNV_PRIME);
  }
  (hash % 360) as f32
}

/// user avatar element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Avatar;
///
/// Avatar::new().name("alice johnson");
/// Avatar::new().src("https://example.com/alice.png");
/// ```
#[derive(IntoElement)]
pub struct Avatar {
  base: BaseAvatar,
  src: Option<ImageSource>,
  name: Option<SharedString>,
  initials: SharedString,
  placeholder: Option<Icon>,
  bg_color: Option<Hsla>,
  style: StyleRefinement,
}

impl Avatar {
  /// creates a new, empty avatar showing the default placeholder icon.
  pub fn new() -> Self {
    Self {
      base: BaseAvatar::new(),
      src: None,
      name: None,
      initials: SharedString::default(),
      placeholder: None,
      bg_color: None,
      style: StyleRefinement::default(),
    }
  }

  /// sets the avatar image source. when set, the image replaces the
  /// initials or placeholder.
  pub fn src(mut self, src: impl Into<ImageSource>) -> Self {
    self.src = Some(src.into());
    self
  }

  /// sets the display name. initials are extracted from it and, unless an
  /// explicit [`Avatar::bg_color`] is set, the fallback accent hue is
  /// derived from it.
  pub fn name(mut self, name: impl Into<SharedString>) -> Self {
    let name: SharedString = name.into();
    self.initials = SharedString::from(extract_initials(&name));
    self.name = Some(name);
    self
  }

  /// sets the icon shown when neither image nor name is set. the default
  /// is [`IconName::Person`].
  pub fn placeholder(mut self, icon: impl Into<Icon>) -> Self {
    self.placeholder = Some(icon.into());
    self
  }

  /// explicitly sets the accent color behind initials or the placeholder.
  pub fn bg_color(mut self, color: Hsla) -> Self {
    self.bg_color = Some(color);
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
  fn interactivity(&mut self) -> &mut gpui::Interactivity {
    self.base.interactivity()
  }
}

impl RenderOnce for Avatar {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();

    let accent = self.bg_color.unwrap_or_else(|| match &self.name {
      Some(name) => theme.color_for_hue(hue_for_name(name)),
      None => theme.primary,
    });

    let border_width = theme.border_width;
    let mut base = self.base;

    // the accent wash covers the whole circle and the ring sits on top;
    // children are inset by the border width so a full-bleed image never
    // paints over the ring.
    base = base
      .size(rems(2.))
      .flex_none()
      .flex()
      .items_center()
      .justify_center()
      .overflow_hidden()
      .rounded_full()
      .p(border_width)
      .border(border_width)
      .border_color(theme.border)
      .bg(with_alpha(accent, opacity::MUTED))
      .text_size(theme.font_size)
      .font_family(theme.font_family.clone())
      .text_color(accent);

    base = match self.src {
      Some(src) => base.image(AvatarImage::new(src).size_full()),
      None => {
        let content: gpui::AnyElement = match &self.name {
          // the initials inherit the base text size: the design system
          // admits no smaller in-library type.
          Some(_) => div().child(self.initials.clone()).into_any_element(),
          None => self
            .placeholder
            .unwrap_or_else(|| Icon::new(IconName::Person))
            .into_any_element(),
        };
        base.fallback(AvatarFallback::new().child(content))
      }
    };

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base
  }
}

#[cfg(test)]
mod tests {
  use super::{Avatar, extract_initials, hue_for_name};

  #[test]
  fn initials_use_the_first_two_words() {
    assert_eq!(extract_initials("Jason Lee"), "JL");
    assert_eq!(extract_initials("Foo Bar Dar"), "FB");
  }

  #[test]
  fn single_word_names_use_the_first_two_letters() {
    assert_eq!(extract_initials("huacnlee"), "HU");
  }

  #[test]
  fn single_letter_names_duplicate_the_letter() {
    assert_eq!(extract_initials("q"), "Q");
  }

  #[test]
  fn name_hue_is_stable_across_calls() {
    assert_eq!(hue_for_name("alice"), hue_for_name("alice"));
    assert!((0.0..360.0).contains(&hue_for_name("bob")));
  }

  #[test]
  fn name_setter_extracts_initials() {
    let avatar = Avatar::new().name("Jason Lee");
    assert_eq!(avatar.initials, "JL");
  }
}
