use std::ops::Range;

use gpui::{
  App, HighlightStyle, IntoElement, ParentElement, RenderOnce, SharedString, StyleRefinement,
  Styled, StyledText, Window, div, prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, StyledExt};

const MASKED: &str = "•";

#[derive(Clone)]
pub enum HighlightsMatch {
  Prefix(SharedString),
  Full(SharedString),
}

impl HighlightsMatch {
  pub fn as_str(&self) -> &str {
    match self {
      Self::Prefix(s) => s.as_str(),
      Self::Full(s) => s.as_str(),
    }
  }

  pub fn is_prefix(&self) -> bool {
    matches!(self, Self::Prefix(_))
  }
}

impl From<&str> for HighlightsMatch {
  fn from(value: &str) -> Self {
    Self::Full(value.to_string().into())
  }
}

impl From<String> for HighlightsMatch {
  fn from(value: String) -> Self {
    Self::Full(value.into())
  }
}

impl From<SharedString> for HighlightsMatch {
  fn from(value: SharedString) -> Self {
    Self::Full(value)
  }
}

#[derive(IntoElement)]
pub struct Label {
  style: StyleRefinement,
  label: SharedString,
  secondary: Option<SharedString>,
  masked: bool,
  highlights_text: Option<HighlightsMatch>,
  cached_full_text: SharedString,
}

impl Label {
  pub fn new(label: impl Into<SharedString>) -> Self {
    let label: SharedString = label.into();
    let cached_full_text = label.clone();
    Self {
      style: StyleRefinement::default(),
      label,
      secondary: None,
      masked: false,
      highlights_text: None,
      cached_full_text,
    }
  }

  pub fn secondary(mut self, secondary: impl Into<SharedString>) -> Self {
    let secondary: SharedString = secondary.into();
    self.cached_full_text = format!("{} {}", self.label, secondary).into();
    self.secondary = Some(secondary);
    self
  }

  pub fn masked(mut self, masked: bool) -> Self {
    self.masked = masked;
    self
  }

  pub fn highlights(mut self, text: impl Into<HighlightsMatch>) -> Self {
    self.highlights_text = Some(text.into());
    self
  }

  fn full_text(&self) -> &SharedString {
    &self.cached_full_text
  }

  fn highlight_ranges(&self) -> Vec<Range<usize>> {
    let full_text = self.full_text();
    let full_text_str = full_text.as_ref();
    let mut ranges = Vec::new();

    if self.secondary.is_some() {
      ranges.push(0..self.label.len());
      ranges.push(self.label.len()..full_text_str.len());
    }

    if let Some(matched) = &self.highlights_text {
      let matched_str = matched.as_str();
      if !matched_str.is_empty() {
        let search_lower = matched_str.to_lowercase();
        let full_text_lower = full_text_str.to_lowercase();

        if matched.is_prefix() {
          if full_text_lower.starts_with(&search_lower) {
            ranges.push(0..matched_str.len());
          }
        } else {
          let mut search_start = 0;
          while let Some(pos) = full_text_lower[search_start..].find(&search_lower) {
            let match_start = search_start + pos;
            let match_end = match_start + matched_str.len();
            if match_end <= full_text_str.len() {
              ranges.push(match_start..match_end);
            }

            search_start = match_start + 1;
            while search_start < full_text_str.len()
              && !full_text_str.is_char_boundary(search_start)
            {
              search_start += 1;
            }
            if search_start >= full_text_str.len() {
              break;
            }
          }
        }
      }
    }

    ranges
  }

  fn measure_highlights(&self, cx: &mut App) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
    let ranges = self.highlight_ranges();
    if ranges.is_empty() {
      return None;
    }

    let mut highlights = Vec::new();
    let mut added = 0;

    if self.secondary.is_some() {
      highlights.push((ranges[0].clone(), HighlightStyle::default()));
      highlights.push((
        ranges[1].clone(),
        HighlightStyle {
          color: Some(cx.theme().muted_foreground),
          ..Default::default()
        },
      ));
      added = 2;
    }

    for range in ranges.iter().skip(added) {
      highlights.push((
        range.clone(),
        HighlightStyle {
          color: Some(cx.theme().primary),
          ..Default::default()
        },
      ));
    }

    Some(gpui::combine_highlights(vec![], highlights).collect())
  }
}

impl_styled!(Label);

impl RenderOnce for Label {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let full_text = self.cached_full_text.clone();
    let mut text = full_text;

    if self.masked {
      text = MASKED.repeat(text.chars().count()).into();
    }

    div()
      .line_height(gpui::relative(1.25))
      .text_color(cx.theme().foreground)
      .refine_style(&self.style)
      .child(
        StyledText::new(&text).when_some(self.measure_highlights(cx), |this, hl| {
          this.with_highlights(hl)
        }),
      )
  }
}
