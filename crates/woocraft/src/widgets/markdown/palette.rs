//! Colors, inline styling and inline code boxes.
//!
//! [`MarkdownPalette`] resolves the active [`Theme`](crate::Theme) into the
//! handful of colors the renderer needs, so block rendering never reads theme
//! fields directly. [`InlineRender`] turns a run of parsed inlines into one
//! text string plus the highlight runs and clickable ranges over it, which is
//! what keeps a paragraph a single shaped line (and therefore keeps line
//! breaking correct) while still supporting bold, links and inline code.

/// Renders inline content into a text element. Links become interactive text
/// so clicks map back to their URL.
/// Inline code span geometry, painted behind a single text element.
///
/// The paragraph stays **one** `StyledText` so gpui's own line breaker
/// (`icu_segmenter` / UAX #14) handles word wrapping: splitting a paragraph
/// into several flex items makes breaks only possible between items, which is
/// what put commas at the start of lines. A code span therefore cannot be a
/// real box, so its background, border and radius are painted from the text
/// layout's per-index positions instead.
pub(super) struct CodeBoxes {
  pub(super) layout: gpui::TextLayout,
  pub(super) ranges: Vec<Range<usize>>,
  pub(super) fill: Hsla,
  pub(super) border: Hsla,
  pub(super) radius: Rems,
  pub(super) line_height: Pixels,
}

impl CodeBoxes {
  pub(super) fn paint(&self, window: &mut Window) {
    let rem_size = window.rem_size();
    let pad_x = inline_code_pad_x(rem_size);
    let radius = self.radius.to_pixels(rem_size);
    // `position_for_index` yields absolute window coordinates already, so no
    // element origin is added here (adding it again is what offset the boxes
    // by the element's own position).
    let (ascent, descent) = (self.ascent(), self.descent());
    let glyph_height = ascent + descent;

    // A span that wraps is painted as one box per line segment, so a code span
    // broken across lines keeps its background instead of losing it entirely.
    let lines = self.layout.line_layouts();
    let mut line_tops = Vec::with_capacity(lines.len());
    let mut y = self.layout.bounds().origin.y;
    for _ in &lines {
      line_tops.push(y);
      y += self.line_height;
    }

    for range in &self.ranges {
      let Some(start) = self.layout.position_for_index(range.start) else {
        continue;
      };
      let Some(end) = self.layout.position_for_index(range.end) else {
        continue;
      };

      let height = glyph_height;
      let top_of = |glyph_top: Pixels| glyph_top + (self.line_height - height) * 0.5;

      if start.y == end.y {
        self.paint_box(
          window,
          start.x - pad_x,
          end.x + pad_x,
          top_of(start.y),
          height,
          radius,
        );
        continue;
      }

      // Wrapped: one box per visual line, clamped to that line's horizontal
      // extent. The first line runs from the span start to the line's right
      // edge, middle lines are full width, and the last line runs from the left
      // edge to the span end.
      let origin_x = self.layout.bounds().origin.x;
      let line_index_of = |glyph_top: Pixels| {
        line_tops
          .iter()
          .position(|top| (*top - glyph_top).abs() < self.line_height * 0.5)
      };
      let (Some(first), Some(last)) = (line_index_of(start.y), line_index_of(end.y)) else {
        continue;
      };

      for index in first..=last {
        let line_top = line_tops[index];
        let line_right = lines[index]
          .wrap_width
          .map(|width| origin_x + width)
          .unwrap_or(end.x);

        let left = if index == first { start.x } else { origin_x };
        let right = if index == last { end.x } else { line_right };

        if right > left {
          self.paint_box(
            window,
            left - pad_x,
            right + pad_x,
            top_of(line_top),
            height,
            radius,
          );
        }
      }
    }
  }

  /// Paints one code box spanning `left..right` at `top`.
  fn paint_box(
    &self, window: &mut Window, left: Pixels, right: Pixels, top: Pixels, height: Pixels,
    radius: Pixels,
  ) {
    let bounds = Bounds::new(point(left, top), size(right - left, height));
    window.paint_quad(fill(bounds, self.fill).corner_radii(radius));
    window.paint_quad(outline(bounds, self.border, BorderStyle::Solid).corner_radii(radius));
  }

  fn ascent(&self) -> Pixels {
    self
      .layout
      .line_layouts()
      .first()
      .map(|line| line.unwrapped_layout.ascent)
      .unwrap_or(self.line_height)
  }

  fn descent(&self) -> Pixels {
    self
      .layout
      .line_layouts()
      .first()
      .map(|line| line.unwrapped_layout.descent)
      .unwrap_or_default()
  }
}

use std::ops::Range;

use gpui::{
  BorderStyle, Bounds, FontWeight, HighlightStyle, Hsla, Pixels, Rems, SharedString,
  StrikethroughStyle, UnderlineStyle, Window, fill, outline, point, px, rems, size,
};

use super::{
  element::inline_code_pad_x,
  parser::{MarkdownBlockQuoteKind, MarkdownInline},
};
use crate::{HighlightTheme, TableThemeExt as _, Theme, default_font};

pub(super) struct MarkdownPalette {
  pub(super) foreground: Hsla,
  pub(super) muted_foreground: Hsla,
  pub(super) link: Hsla,
  pub(super) border: Hsla,
  pub(super) inline_code_background: Hsla,
  /// Neutral fill for placeholders (images, missing assets).
  pub(super) placeholder_background: Hsla,
  pub(super) code_block_background: Hsla,
  pub(super) quote_background: Hsla,
  /// Header fill, from [`TableThemeExt::table_head`] so markdown tables
  /// match the data table component.
  pub(super) table_header_background: Hsla,
  pub(super) info: Hsla,
  pub(super) success: Hsla,
  pub(super) warning: Hsla,
  pub(super) danger: Hsla,
  pub(super) syntax: HighlightTheme,
  pub(super) mono_family: SharedString,
  pub(super) radius: Rems,
  /// Border color of inputs and code surfaces, matching [`crate::Editor`].
  pub(super) input_border: Hsla,
  /// Row stripe of alternating table rows, matching [`TableThemeExt`].
  pub(super) table_stripe: Hsla,
  /// Header text color, from [`TableThemeExt::table_head_foreground`].
  pub(super) table_head_foreground: Hsla,
  /// Selection highlight, from the theme's own selection color.
  pub(super) selection: Hsla,
  /// Color of list markers: the theme primary at 80% opacity, so bullets and
  /// numbers read as structure without competing with body text.
  pub(super) list_marker: Hsla,
  /// Marker color of the item under the pointer, at full opacity.
  pub(super) list_marker_hovered: Hsla,
  /// Line height used by [`crate::Editor`] for code.
  pub(super) code_line_height: Rems,
}

impl MarkdownPalette {
  pub(super) fn new(theme: &Theme) -> Self {
    Self {
      foreground: theme.foreground,
      muted_foreground: theme.muted_foreground,
      link: theme.primary,
      border: theme.border,
      inline_code_background: theme.foreground.opacity(0.03),
      placeholder_background: theme.foreground.opacity(0.03),
      code_block_background: theme.editor_background,
      // A quote is a neutral surface, not an accent one: `muted` is a tinted
      // emphasis fill and reads far too loud behind body text. A few percent of
      // the foreground keeps it theme-aware but almost invisible.
      quote_background: theme.foreground.opacity(0.03),
      table_header_background: theme.table_head(),
      info: theme.info,
      success: theme.success,
      warning: theme.warning,
      danger: theme.danger,
      syntax: HighlightTheme::from_theme(theme),
      mono_family: default_font().family,
      radius: theme.radius,
      input_border: theme.input,
      table_stripe: theme.table_even(),
      table_head_foreground: theme.table_head_foreground(),
      selection: theme.selection,
      list_marker: theme.primary.opacity(0.8),
      list_marker_hovered: theme.primary,
      code_line_height: rems(1.5),
    }
  }

  pub(super) fn quote_accent(&self, kind: Option<MarkdownBlockQuoteKind>) -> Hsla {
    match kind {
      Some(MarkdownBlockQuoteKind::Note) => self.info,
      Some(MarkdownBlockQuoteKind::Tip) => self.success,
      Some(MarkdownBlockQuoteKind::Important) => self.link,
      Some(MarkdownBlockQuoteKind::Warning) => self.warning,
      Some(MarkdownBlockQuoteKind::Caution) => self.danger,
      None => self.border,
    }
  }
}

/// Inline styles accumulated while flattening a markdown inline tree into a
/// single text line with styled runs.
#[derive(Clone, Copy, Default)]
struct InlineStyle {
  strong: bool,
  emphasis: bool,
  strikethrough: bool,
  link: bool,
  code: bool,
}

impl InlineStyle {
  fn highlight(self, palette: &MarkdownPalette) -> Option<HighlightStyle> {
    if !(self.strong || self.emphasis || self.strikethrough || self.link || self.code) {
      return None;
    }

    let mut style = HighlightStyle::default();

    if self.code {
      style.color = Some(palette.foreground);
    }

    if self.strong {
      style.font_weight = Some(FontWeight::BOLD);
    }

    if self.emphasis {
      style.font_style = Some(gpui::FontStyle::Italic);
    }

    if self.strikethrough {
      style.strikethrough = Some(StrikethroughStyle {
        thickness: px(1.),
        color: Some(palette.muted_foreground),
      });
    }

    if self.link {
      style.color = Some(palette.link);
      style.underline = Some(UnderlineStyle {
        thickness: px(1.),
        color: Some(palette.link),
        wavy: false,
      });
    }

    // An inline code span's box (fill, border, radius, padding) is painted by
    // `render_inlines`: a highlight background covers exactly the glyph
    // advance at full line height, so it cannot have padding, a border or
    // rounded corners. Only the text color is applied here.

    Some(style)
  }
}

/// Flat inline text with its style runs, link ranges and inline code ranges.
#[derive(Default)]
pub(super) struct InlineRender {
  pub(super) text: String,
  pub(super) highlights: Vec<(Range<usize>, HighlightStyle)>,
  /// Byte ranges of inline code spans, painted with a rounded background by
  /// the renderer.
  pub(super) code_ranges: Vec<Range<usize>>,
  pub(super) links: Vec<(Range<usize>, SharedString)>,
}

impl InlineRender {
  pub(super) fn build(inlines: &[MarkdownInline], palette: &MarkdownPalette) -> Self {
    let mut render = Self::default();
    render.push_inlines(inlines, InlineStyle::default(), palette);
    render.highlights = merge_highlights(std::mem::take(&mut render.highlights));
    render
  }

  fn push_inlines(
    &mut self, inlines: &[MarkdownInline], style: InlineStyle, palette: &MarkdownPalette,
  ) {
    for inline in inlines {
      match inline {
        MarkdownInline::Text(text) => self.push_text(text, style, palette),
        MarkdownInline::Code(code) => {
          let start = self.text.len();
          self.push_text(
            code,
            InlineStyle {
              code: true,
              ..style
            },
            palette,
          );
          let end = self.text.len();
          if start < end {
            self.code_ranges.push(start..end);
          }
        }
        MarkdownInline::Emphasis(inlines) => self.push_inlines(
          inlines,
          InlineStyle {
            emphasis: true,
            ..style
          },
          palette,
        ),
        MarkdownInline::Strong(inlines) => self.push_inlines(
          inlines,
          InlineStyle {
            strong: true,
            ..style
          },
          palette,
        ),
        MarkdownInline::Strikethrough(inlines) => self.push_inlines(
          inlines,
          InlineStyle {
            strikethrough: true,
            ..style
          },
          palette,
        ),
        MarkdownInline::Link { url, inlines, .. } => {
          let start = self.text.len();
          if inlines.is_empty() {
            self.push_text(
              url,
              InlineStyle {
                link: true,
                ..style
              },
              palette,
            );
          } else {
            self.push_inlines(
              inlines,
              InlineStyle {
                link: true,
                ..style
              },
              palette,
            );
          }
          let end = self.text.len();
          if start < end {
            self.links.push((start..end, url.clone()));
          }
        }
        MarkdownInline::Image { url, alt, .. } => {
          let start = self.text.len();
          if alt.is_empty() {
            self.push_text(
              url,
              InlineStyle {
                link: true,
                ..style
              },
              palette,
            );
          } else {
            self.push_inlines(
              alt,
              InlineStyle {
                link: true,
                ..style
              },
              palette,
            );
          }
          let end = self.text.len();
          if start < end {
            self.links.push((start..end, url.clone()));
          }
        }
        // A soft break is rendered as a space, so wrapped prose flows as one
        // line instead of preserving the source line width.
        MarkdownInline::SoftBreak => self.push_text(" ", style, palette),
        MarkdownInline::HardBreak => self.text.push('\n'),
        MarkdownInline::FootnoteReference(label) => {
          let text = format!("[^{label}]");
          let start = self.text.len();
          self.text.push_str(&text);
          let end = self.text.len();
          self.highlights.push((
            start..end,
            HighlightStyle {
              color: Some(palette.link),
              font_weight: Some(FontWeight::SEMIBOLD),
              ..Default::default()
            },
          ));
        }
        // Raw inline HTML is not rendered.
        MarkdownInline::Html(_) => {}
      }
    }
  }

  fn push_text(&mut self, text: &str, style: InlineStyle, palette: &MarkdownPalette) {
    if text.is_empty() {
      return;
    }

    let start = self.text.len();
    self.text.push_str(text);
    let end = self.text.len();

    if let Some(highlight) = style.highlight(palette) {
      self.highlights.push((start..end, highlight));
    }
  }
}

/// Merges adjacent highlights that share the same style, keeping the run list
/// small for long paragraphs with uniform styling.
fn merge_highlights(
  highlights: Vec<(Range<usize>, HighlightStyle)>,
) -> Vec<(Range<usize>, HighlightStyle)> {
  let mut merged: Vec<(Range<usize>, HighlightStyle)> = Vec::with_capacity(highlights.len());

  for (range, style) in highlights {
    if range.is_empty() {
      continue;
    }

    match merged.last_mut() {
      Some((previous, previous_style))
        if previous.end == range.start && *previous_style == style =>
      {
        previous.end = range.end;
      }
      _ => merged.push((range, style)),
    }
  }

  merged
}

#[cfg(test)]
mod tests {
  use gpui::FontWeight;

  use super::*;
  use crate::{Theme, widgets::markdown::element::highlighted_runs};

  fn palette() -> MarkdownPalette {
    MarkdownPalette::new(&Theme::default())
  }

  fn text(value: &str) -> MarkdownInline {
    MarkdownInline::Text(value.into())
  }

  #[test]
  fn inline_runs_track_ranges_links_and_code() {
    let built = InlineRender::build(
      &[
        text("a "),
        MarkdownInline::Strong(vec![text("b")]),
        MarkdownInline::Link {
          url: "https://example.com".into(),
          title: None,
          inlines: vec![text("c")],
        },
        MarkdownInline::Code("d".into()),
      ],
      &palette(),
    );

    // Padding is painted geometrically, so the text itself is unpadded.
    assert_eq!(built.text, "a bcd");
    assert_eq!(built.links.len(), 1);
    assert_eq!(built.links[0].0, 3..4);
    assert_eq!(built.links[0].1, "https://example.com");
    // The code run covers just the code text, without padding.
    assert_eq!(built.code_ranges, vec![4..5]);
    assert!(
      built
        .highlights
        .iter()
        .any(|(range, style)| { *range == (2..3) && style.font_weight == Some(FontWeight::BOLD) })
    );
    assert!(
      built
        .highlights
        .iter()
        .any(|(range, style)| { *range == (3..4) && style.underline.is_some() })
    );
  }

  #[test]
  fn soft_break_renders_as_a_space() {
    let built = InlineRender::build(
      &[text("a"), MarkdownInline::SoftBreak, text("b")],
      &palette(),
    );
    assert_eq!(built.text, "a b");
  }

  #[test]
  fn footnote_references_are_highlighted_and_not_links() {
    let built = InlineRender::build(&[MarkdownInline::FootnoteReference("1".into())], &palette());
    assert_eq!(built.text, "[^1]");
    assert!(built.links.is_empty());
    assert_eq!(built.highlights.len(), 1);
  }

  #[test]
  fn adjacent_highlights_are_merged() {
    let inlines = vec![MarkdownInline::Strong(vec![text("a"), text("b")])];
    let built = InlineRender::build(&inlines, &palette());
    let strong = InlineStyle {
      strong: true,
      ..Default::default()
    }
    .highlight(&palette())
    .unwrap();

    assert_eq!(built.text, "ab");
    assert_eq!(built.highlights, vec![(0..2, strong)]);
  }

  #[test]
  fn unknown_language_has_no_highlighting() {
    let runs = highlighted_runs("not-a-language", "let x = 1;", &palette().syntax);
    assert!(runs.is_empty());
  }

  #[cfg(feature = "tree-sitter-languages")]
  #[test]
  fn known_language_is_highlighted() {
    let runs = highlighted_runs("rust", "fn main() {}", &palette().syntax);
    assert!(!runs.is_empty());
  }
}
