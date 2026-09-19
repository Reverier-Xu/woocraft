//! linear progress as a labeled row with a hairline fill along the bottom
//! edge.
//!
//! this is a styled wrapper over [`gpui_base::Progress`], which owns the
//! controlled value and every accessibility concern — the progress
//! indicator role, the 0–100 numeric range, and the accessible name. the
//! wrapper adds the design-system vocabulary: button-height geometry with
//! the icon-plus-label row on the left, the percentage on the right, and a
//! `0.125rem` hairline along the bottom edge (muted track, primary fill)
//! that slides while indeterminate.

use gpui::{
  Animation, AnimationExt as _, AnyElement, App, ElementId, IntoElement, ParentElement,
  Refineable as _, RenderOnce, SharedString, StyleRefinement, Styled, Window, div, linear,
  prelude::FluentBuilder as _, relative, rems,
};
use gpui_base::Progress as BaseProgress;

use crate::{ActiveTheme, Icon, IconLabel, theme::duration};

/// linear progress element styled by the woocraft design system.
///
/// constructed through the builder pattern:
///
/// ```rust,ignore
/// use woocraft::{Icon, IconName, Progress};
///
/// Progress::new("upload")
///   .icon(Icon::new(IconName::ArrowUpload))
///   .label("uploading log")
///   .value(40.)
///   .accessibility_label("uploading attachment");
/// ```
#[derive(IntoElement)]
pub struct Progress {
  id: ElementId,
  base: BaseProgress,
  value: f32,
  indeterminate: bool,
  icon: Option<Icon>,
  label: Option<SharedString>,
  style: StyleRefinement,
}

impl Progress {
  /// creates a progress bar at 0% with a unique element identifier.
  pub fn new(id: impl Into<ElementId>) -> Self {
    let id = id.into();
    Self {
      base: BaseProgress::new(id.clone()),
      id,
      value: 0.,
      indeterminate: false,
      icon: None,
      label: None,
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

  /// sets the leading icon of the label row.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// sets the text label of the label row.
  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  /// sets the name exposed to accessibility clients.
  pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
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
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.base.extend(elements);
  }
}

impl RenderOnce for Progress {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let theme = cx.theme();
    let (primary, muted, muted_foreground) = (theme.primary, theme.muted, theme.muted_foreground);
    let value = self.value;
    let indeterminate = self.indeterminate;

    // the hairline hugs the bottom edge: muted track, primary fill. while
    // indeterminate a 40%-wide segment slides across the track and the
    // percentage readout is dropped.
    let fill: AnyElement = if indeterminate {
      div()
        .absolute()
        .top_0()
        .bottom_0()
        .w(relative(0.4))
        .rounded_full()
        .bg(primary)
        .with_animation(
          (self.id.clone(), "indeterminate"),
          Animation::new(duration::INDETERMINATE)
            .repeat()
            .with_easing(linear),
          move |this, delta| this.left(relative(-0.4 + 1.4 * delta)),
        )
        .into_any_element()
    } else {
      div()
        .h_full()
        .w(relative(value / 100.))
        .rounded_full()
        .bg(primary)
        .into_any_element()
    };

    // the label row reuses the icon-label vocabulary, stripped of its own
    // padding so the component padding alone rules the inset.
    let label: Option<AnyElement> = (self.icon.is_some() || self.label.is_some()).then(|| {
      let mut row = IconLabel::new((self.id.clone(), "label"));
      if let Some(icon) = self.icon {
        row = row.icon(icon);
      }
      if let Some(label) = self.label {
        row = row.label(label);
      }
      row.px(rems(0.)).py(rems(0.)).into_any_element()
    });

    let percent = SharedString::from(format!("{:.0}%", value));

    let mut base = self.base;

    // button geometry: 2rem tall, 0.5rem side padding, and the bottom
    // padding compensates for the hairline thickness so the label row sits
    // exactly in the vertical center, like a button's content.
    base = base
      .relative()
      .h(rems(2.))
      .w_full()
      .child(
        div()
          .mx(rems(0.5))
          .mt(rems(0.5))
          .h(rems(1.))
          .flex()
          .items_center()
          .gap(rems(0.5))
          .min_w_0()
          .when_some(label, |this, label| this.child(label))
          .when(!indeterminate, |this| {
            this.child(
              div()
                .ml_auto()
                .flex_none()
                .text_color(muted_foreground)
                .child(percent),
            )
          }),
      )
      .child(
        div()
          .absolute()
          .bottom_0()
          .left_0()
          .right_0()
          .h(rems(0.125))
          .overflow_hidden()
          .rounded_full()
          .bg(muted)
          .child(fill),
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
  fn icon_and_label_default_to_none() {
    let progress = Progress::new("test");
    assert!(progress.icon.is_none());
    assert!(progress.label.is_none());

    let progress = progress.label("loading").value(40.);
    assert_eq!(progress.label.as_ref().map(|l| l.as_ref()), Some("loading"));
  }
}
