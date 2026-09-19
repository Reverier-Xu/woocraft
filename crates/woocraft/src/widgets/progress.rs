//! linear progress bar showing completion as a percentage.
//!
//! this is a styled wrapper over [`gpui_base::Progress`], which owns the
//! controlled value and every accessibility concern — the progress
//! indicator role, the 0–100 numeric range, and the accessible name. the
//! wrapper adds the design-system vocabulary: a `0.5rem` tall rounded
//! track with a primary indicator, an optional accent color, and an
//! indeterminate slide that honors the system reduce-motion preference.

use gpui::{
  Animation, AnimationExt as _, App, Hsla, IntoElement, ParentElement, Refineable as _, RenderOnce,
  StyleRefinement, Styled, Window, div, linear, relative, rems,
};
use gpui_base::Progress as BaseProgress;

use crate::{
  ActiveTheme,
  theme::{duration, opacity, with_alpha},
};

/// linear progress element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::Progress;
///
/// Progress::new("upload")
///   .value(40.)
///   .accessibility_label("uploading attachment");
/// ```
#[derive(IntoElement)]
pub struct Progress {
  id: gpui::ElementId,
  base: BaseProgress,
  value: f32,
  indeterminate: bool,
  color: Option<Hsla>,
  style: StyleRefinement,
}

impl Progress {
  /// creates a progress bar at 0% with a unique element identifier.
  pub fn new(id: impl Into<gpui::ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseProgress::new(id.clone()),
      id,
      value: 0.,
      indeterminate: false,
      color: None,
      style: StyleRefinement::default(),
    }
  }

  /// sets the controlled percentage value, clamped to `0..=100`.
  pub fn value(mut self, value: f32) -> Self {
    self.value = value.clamp(0., 100.);
    self.base = self.base.value(value);
    self
  }

  /// switches to the indeterminate presentation.
  pub fn indeterminate(mut self, indeterminate: bool) -> Self {
    self.indeterminate = indeterminate;
    self.base = self.base.indeterminate(indeterminate);
    self
  }

  /// sets the accent color of the indicator. defaults to the theme
  /// primary.
  pub fn color(mut self, color: Hsla) -> Self {
    self.color = Some(color);
    self
  }

  /// sets the name exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<gpui::SharedString>) -> Self {
    self.base = self.base.accessibility_label(label);
    self
  }
}

impl Styled for Progress {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for Progress {
  fn extend(&mut self, elements: impl IntoIterator<Item = gpui::AnyElement>) {
    self.base.extend(elements);
  }
}

impl RenderOnce for Progress {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let value = self.value;
    let indeterminate = self.indeterminate;
    let accent = self.color.unwrap_or(theme.primary);
    let track_bg = with_alpha(theme.foreground, opacity::MUTED);
    let reduce_motion = cx.reduce_motion();

    // the track clips a 40%-wide indicator that slides across the full
    // width while indeterminate; reduce motion freezes it in place.
    let indicator: gpui::AnyElement = if indeterminate {
      let slide = div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(0.4))
        .rounded_full()
        .bg(accent);
      if reduce_motion {
        slide.left(relative(0.3)).into_any_element()
      } else {
        slide
          .with_animation(
            (self.id.clone(), "indeterminate"),
            Animation::new(duration::INDETERMINATE)
              .repeat()
              .with_easing(linear),
            move |this, delta| this.left(relative(-0.4 + 1.4 * delta)),
          )
          .into_any_element()
      }
    } else {
      div()
        .h_full()
        .w(relative(value / 100.))
        .rounded_full()
        .bg(accent)
        .into_any_element()
    };

    let mut base = self.base;

    base = base.w_full().child(
      div()
        .relative()
        .h(rems(0.5))
        .w_full()
        .overflow_hidden()
        .rounded_full()
        .bg(track_bg)
        .child(indicator),
    );

    // caller refinements win over the theme defaults.
    base.style().refine(&self.style);

    base
  }
}

#[cfg(test)]
mod tests {
  use super::Progress;

  #[test]
  fn value_clamps_to_the_unit_range() {
    assert_eq!(Progress::new("test").value(120.).value, 100.);
    assert_eq!(Progress::new("test").value(-5.).value, 0.);
    assert_eq!(Progress::new("test").value(42.).value, 42.);
  }

  #[test]
  fn indeterminate_flag_is_stored() {
    assert!(Progress::new("test").indeterminate(true).indeterminate);
  }

  #[test]
  fn color_defaults_to_none_until_set() {
    let progress = Progress::new("test");
    assert!(progress.color.is_none());
  }
}
