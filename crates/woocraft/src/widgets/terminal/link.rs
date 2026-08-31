//! Link detection plumbing for the terminal view.
//!
//! Detection is a host concern: hosts register [`LinkProvider`]s (typically
//! regex-based URL matchers), and the view owns the interaction pipeline —
//! hit-testing, hover cursor, annotation, and click activation. This mirrors
//! xterm.js' `registerLinkProvider` split: providers answer *what* is a link,
//! the view answers *how* links behave.
//!
//! Mouse priority is fixed and never overridden by providers:
//!
//! 1. **TUI mouse mode** (`SGR_MOUSE`/`MOUSE_DRAG`/`MOUSE_MOTION`): events are
//!    reported to the application, unless the user holds shift.
//! 2. **Link activation**: a click on a link that never turned into a drag.
//! 3. **Selection**: everything else.

use std::{borrow::Cow, ops::Range, sync::Arc};

use woocraft_terminal::{CellFlags, Content, IndexedCell, Point as GridPoint};

/// A contiguous link on one viewport row.
///
/// `start` is inclusive and `end` exclusive, both in columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkSpan {
  pub start: usize,
  pub end: usize,
  pub uri: Arc<str>,
}

impl LinkSpan {
  pub fn new(start: usize, end: usize, uri: impl Into<Arc<str>>) -> Self {
    Self {
      start,
      end,
      uri: uri.into(),
    }
  }
}

/// The text and cells of one viewport row, handed to providers.
pub struct LineContext<'a> {
  /// The row's cells, wide-character spacers included.
  pub cells: &'a [IndexedCell],
  /// The row's plain text (spacers removed, zero-width characters kept).
  pub text: Cow<'a, str>,
}

impl LineContext<'_> {
  /// The cell at `column`, if the row extends that far.
  pub fn cell(&self, column: usize) -> Option<&woocraft_terminal::Cell> {
    self.cells.get(column).map(|indexed| &indexed.cell)
  }
}

/// Detects links on a terminal line.
///
/// Providers are called per visible row during hover and, when annotation is
/// enabled, per frame while painting. Keep them cheap; cache internally if
/// the detection is expensive.
pub trait LinkProvider: Send + Sync {
  fn links_for_line(&self, line: &LineContext<'_>) -> Vec<LinkSpan>;
}

/// A resolved link on the grid, in cell coordinates: the same space as
/// [`woocraft_terminal::Content::cells`] points and mouse grid points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridLink {
  pub range: Range<GridPoint>,
  pub uri: Arc<str>,
}

impl GridLink {
  pub fn contains(&self, point: GridPoint) -> bool {
    self.range.start <= point && point < self.range.end
  }
}

/// The built-in provider for OSC 8 hyperlinks: consecutive cells carrying the
/// same hyperlink URI are merged into one span. The renderer already
/// underlines these cells; this provider adds hover and click behavior.
pub struct Osc8LinkProvider;

impl LinkProvider for Osc8LinkProvider {
  fn links_for_line(&self, line: &LineContext<'_>) -> Vec<LinkSpan> {
    let mut spans = Vec::new();
    let mut current: Option<(usize, usize, Arc<str>)> = None; // (start, end, uri)
    for (column, indexed) in line.cells.iter().enumerate() {
      if indexed
        .cell
        .flags
        .contains(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
      {
        continue;
      }
      let link = indexed.cell.hyperlink.clone();
      match (&mut current, link) {
        (Some((_, end, uri)), Some(new_link)) if *uri == new_link => {
          *end = column + 1;
        }
        (Some((start, end, uri)), new_link) => {
          spans.push(LinkSpan::new(*start, *end, uri.clone()));
          current = new_link.map(|new_link| (column, column + 1, new_link));
        }
        (None, Some(new_link)) => {
          current = Some((column, column + 1, new_link));
        }
        (None, None) => {}
      }
    }
    if let Some((start, end, uri)) = current {
      spans.push(LinkSpan::new(start, end, uri));
    }
    spans
  }
}

/// Runs all `providers` against one viewport row and converts the resulting
/// column spans into [`GridLink`]s.
///
/// The resulting ranges are in *cell coordinates* — the same space as
/// [`Content::cells`] points and the mouse mapper's [`GridPoint`]s — so that
/// hit-testing needs no conversion. `row` is the viewport row (`0` is the
/// topmost visible line).
pub fn links_for_row(
  content: &Content, row: usize, providers: &[Arc<dyn LinkProvider>],
) -> Vec<GridLink> {
  let Some(cells) = content.row(row) else {
    return Vec::new();
  };
  let line = row as i32 - content.display_offset as i32;
  let text = content
    .line_text(row)
    .map(Cow::Owned)
    .unwrap_or_else(|| Cow::Borrowed(""));
  if text.trim().is_empty() && cells.iter().all(|cell| cell.cell.hyperlink.is_none()) {
    return Vec::new();
  }
  let context = LineContext { cells, text };
  providers
    .iter()
    .flat_map(|provider| provider.links_for_line(&context))
    .filter(|span| span.end > span.start)
    .map(|span| GridLink {
      range: Range {
        start: GridPoint::new(line, span.start),
        end: GridPoint::new(line, span.end),
      },
      uri: span.uri,
    })
    .collect()
}

/// Runs the built-in OSC 8 provider plus all registered providers.
pub fn all_links_for_row(
  content: &Content, row: usize, providers: &[Arc<dyn LinkProvider>],
) -> Vec<GridLink> {
  let mut all = Vec::with_capacity(providers.len() + 1);
  all.push(Arc::new(Osc8LinkProvider) as Arc<dyn LinkProvider>);
  all.extend(providers.iter().cloned());
  links_for_row(content, row, &all)
}

/// Finds the link containing `point`, which must be in cell coordinates
/// (the same space the mouse mapper produces).
pub fn link_at(
  content: &Content, point: GridPoint, providers: &[Arc<dyn LinkProvider>],
) -> Option<GridLink> {
  let row = point.line + content.display_offset as i32;
  if row < 0 {
    return None;
  }
  all_links_for_row(content, row as usize, providers)
    .into_iter()
    .find(|link| link.contains(point))
}

#[cfg(test)]
mod tests {
  use woocraft_terminal::{Cell, IndexedCell};

  use super::*;

  fn content_from_rows(rows: &[&str]) -> Content {
    let mut content = Content::empty();
    content.columns = rows.first().map(|row| row.len()).unwrap_or(0);
    content.screen_lines = rows.len();
    let mut cells = Vec::new();
    for (row, text) in rows.iter().enumerate() {
      for (column, c) in text.chars().enumerate() {
        cells.push(IndexedCell {
          point: GridPoint::new(row as i32, column),
          cell: Cell {
            c,
            ..Cell::default()
          },
        });
      }
    }
    content.cells = cells;
    content
  }

  struct UrlProvider;

  impl LinkProvider for UrlProvider {
    fn links_for_line(&self, line: &LineContext<'_>) -> Vec<LinkSpan> {
      // Minimal stand-in for a regex matcher: finds "http" prefixes.
      line
        .text
        .match_indices("http")
        .map(|(start, _)| LinkSpan::new(start, line.text.len(), "https://example.com"))
        .collect()
    }
  }

  #[test]
  fn provider_spans_become_grid_links() {
    let content = content_from_rows(&["see http://x.dev now", "plain line"]);
    let links = links_for_row(&content, 0, &[Arc::new(UrlProvider)]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, GridPoint::new(0, 4));
    assert_eq!(links[0].range.end, GridPoint::new(0, 20));
    assert!(links[0].contains(GridPoint::new(0, 4)));
    assert!(links[0].contains(GridPoint::new(0, 19)));
    assert!(!links[0].contains(GridPoint::new(0, 20)));
    assert!(!links[0].contains(GridPoint::new(1, 5)));

    assert!(links_for_row(&content, 1, &[Arc::new(UrlProvider)]).is_empty());
    assert!(links_for_row(&content, 2, &[Arc::new(UrlProvider)]).is_empty());
  }

  #[test]
  fn link_at_hits_registered_providers() {
    let content = content_from_rows(&["visit http://x.dev ok"]);
    assert!(link_at(&content, GridPoint::new(0, 6), &[Arc::new(UrlProvider)]).is_some());
    assert!(link_at(&content, GridPoint::new(0, 2), &[Arc::new(UrlProvider)]).is_none());
    assert!(link_at(&content, GridPoint::new(-1, 0), &[Arc::new(UrlProvider)]).is_none());
  }

  #[test]
  fn osc8_cells_merge_into_spans() {
    let mut content = content_from_rows(&["a hyperlink b"]);
    let uri: Arc<str> = Arc::from("https://zed.dev");
    for point in [1, 2, 3, 4, 5, 6, 7, 8] {
      content.cells[point].cell.hyperlink = Some(uri.clone());
    }
    let links = all_links_for_row(&content, 0, &[]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, GridPoint::new(0, 1));
    assert_eq!(links[0].range.end, GridPoint::new(0, 9));
    assert_eq!(&*links[0].uri, "https://zed.dev");
    assert!(link_at(&content, GridPoint::new(0, 8), &[]).is_some());
    assert!(link_at(&content, GridPoint::new(0, 0), &[]).is_none());
  }

  #[test]
  fn osc8_adjacent_distinct_uris_split() {
    let mut content = content_from_rows(&["abcdef"]);
    let a: Arc<str> = Arc::from("https://a");
    let b: Arc<str> = Arc::from("https://b");
    for point in 0..3 {
      content.cells[point].cell.hyperlink = Some(a.clone());
    }
    for point in 3..6 {
      content.cells[point].cell.hyperlink = Some(b.clone());
    }
    let links = all_links_for_row(&content, 0, &[]);
    assert_eq!(links.len(), 2);
    assert_eq!(links[0].uri, a);
    assert_eq!(links[1].uri, b);
  }

  #[test]
  fn blank_rows_are_skipped_cheaply() {
    let content = content_from_rows(&["", "   "]);
    assert!(all_links_for_row(&content, 0, &[Arc::new(UrlProvider)]).is_empty());
    assert!(all_links_for_row(&content, 1, &[Arc::new(UrlProvider)]).is_empty());
  }
}
