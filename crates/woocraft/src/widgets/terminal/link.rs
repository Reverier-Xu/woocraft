//! Link detection plumbing for the terminal view.
//!
//! Detection is a host concern: hosts register [`LinkProvider`]s (typically
//! regex-based URL matchers), and the view owns the interaction pipeline —
//! hit-testing, hover cursor, annotation, and click activation. This mirrors
//! xterm.js' `registerLinkProvider` split: providers answer *what* is a link,
//! the view answers *how* links behave.
//!
//! # Coordinates
//!
//! Providers work on [`LineContext::text`] and report [`LinkSpan`]s as **byte
//! offsets into that text** — exactly what `str::find` / `match_indices`
//! return. The view converts offsets to grid columns via
//! [`LineContext::column_of_byte`], which follows the row's cells (including
//! `WIDE_CHAR` cells, which span two columns). Providers must never do their
//! own text-to-column math: one character can be two columns wide, so char
//! counts, byte counts, and columns all disagree as soon as CJK text shows up.
//!
//! [`GridLink`] ranges are in *cell coordinates* — the same space as
//! [`Content::cells`] points and the mouse mapper's [`GridPoint`]s.
//!
//! # Mouse priority
//!
//! Fixed and never overridden by providers:
//!
//! 1. **TUI mouse mode** (`SGR_MOUSE`/`MOUSE_DRAG`/`MOUSE_MOTION`): events are
//!    reported to the application, unless the user holds shift.
//! 2. **Link activation**: a click on a link that never turned into a drag.
//! 3. **Selection**: everything else.

use std::{borrow::Cow, ops::Range, sync::Arc};

use woocraft_terminal::{Cell, CellFlags, Content, IndexedCell, Point as GridPoint};

/// A contiguous link on one viewport row.
///
/// `start` (inclusive) and `end` (exclusive) are **byte offsets into
/// [`LineContext::text`]** — the values `str::find` and `match_indices`
/// produce. The view maps them to grid columns; see [`LineContext`].
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
///
/// The row's plain text is *not* aligned with grid columns: a `WIDE_CHAR` cell
/// contributes one character (two columns), and zero-width characters extend
/// the text without occupying cells. Use [`Self::column_of_byte`] to map
/// [`LinkSpan`] offsets (or any offset in `text`) back to columns; the
/// mapping is built cell-by-cell from the grid, never estimated.
pub struct LineContext<'a> {
  /// The row's cells, wide-character spacers included. Indexed by column.
  pub cells: &'a [IndexedCell],
  /// The row's plain text (spacers removed, zero-width characters kept).
  pub text: Cow<'a, str>,
  /// Byte offset → grid column boundary table, ascending, ending with a
  /// sentinel at `text.len()`.
  byte_columns: Vec<(usize, usize)>,
}

impl LineContext<'_> {
  /// Maps a byte offset in [`Self::text`] to the grid column of the character
  /// containing it. Offsets past the end of the text clamp to the column just
  /// after the last character (so a span's exclusive `end` maps to the column
  /// after the link, covering a wide character's second half).
  pub fn column_of_byte(&self, byte: usize) -> usize {
    // The table starts at (0, 0), so `index` is always >= 1 here.
    let index = self
      .byte_columns
      .partition_point(|&(offset, _)| offset <= byte);
    self.byte_columns[index - 1].1
  }

  /// The cell at `column`, if the row extends that far.
  pub fn cell(&self, column: usize) -> Option<&Cell> {
    self.cells.get(column).map(|indexed| &indexed.cell)
  }
}

/// Builds a [`LineContext`] from one row's cells. The byte→column table is
/// derived directly from the cell grid: each non-spacer cell advances the
/// column by its width (2 for `WIDE_CHAR`, 1 otherwise).
fn line_context(cells: &[IndexedCell]) -> LineContext<'_> {
  let mut text = String::with_capacity(cells.len());
  let mut byte_columns = Vec::with_capacity(cells.len() + 1);
  let mut column = 0;
  for indexed in cells {
    if indexed
      .cell
      .flags
      .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
    {
      continue;
    }
    byte_columns.push((text.len(), column));
    text.push(indexed.cell.c);
    text.extend(indexed.cell.zerowidth.iter().copied());
    column += if indexed.cell.flags.contains(CellFlags::WIDE_CHAR) {
      2
    } else {
      1
    };
  }
  // End sentinel: offsets at (or past) `text.len()` map past the last cell.
  byte_columns.push((text.len(), column));
  LineContext {
    cells,
    text: Cow::Owned(text),
    byte_columns,
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
/// [`Content::cells`] points and mouse grid points.
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

/// OSC 8 hyperlinks on one row: consecutive cells carrying the same hyperlink
/// URI merge into one link. The renderer already underlines these cells; the
/// view adds hover and click behavior. Computed from the cells directly, so
/// no text-offset mapping is involved.
fn osc8_links(cells: &[IndexedCell], line: i32) -> Vec<GridLink> {
  let mut links = Vec::new();
  let mut current: Option<(usize, usize, Arc<str>)> = None; // (start, end, uri)
  for (column, indexed) in cells.iter().enumerate() {
    // A trailing wide-char spacer is the second half of the preceding wide
    // character: while a span is open it belongs to it (so a click on either
    // half of the last glyph hits). A leading spacer starts a new wrapped
    // character and never extends a span.
    if indexed.cell.flags.contains(CellFlags::WIDE_CHAR_SPACER) {
      if let Some((_, end, _)) = &mut current {
        *end = column + 1;
      }
      continue;
    }
    if indexed
      .cell
      .flags
      .contains(CellFlags::LEADING_WIDE_CHAR_SPACER)
    {
      continue;
    }
    let link = indexed.cell.hyperlink.clone();
    match (&mut current, link) {
      (Some((_, end, uri)), Some(new_link)) if *uri == new_link => {
        *end = column + 1;
      }
      (Some((start, end, uri)), new_link) => {
        links.push(GridLink {
          range: Range {
            start: GridPoint::new(line, *start),
            end: GridPoint::new(line, *end),
          },
          uri: uri.clone(),
        });
        current = new_link.map(|new_link| (column, column + 1, new_link));
      }
      (None, Some(new_link)) => {
        current = Some((column, column + 1, new_link));
      }
      (None, None) => {}
    }
  }
  if let Some((start, end, uri)) = current {
    links.push(GridLink {
      range: Range {
        start: GridPoint::new(line, start),
        end: GridPoint::new(line, end),
      },
      uri,
    });
  }
  links
}

/// Runs all `providers` against one viewport row and converts the resulting
/// text-offset spans into [`GridLink`]s.
///
/// `row` is the viewport row (`0` is the topmost visible line); the resulting
/// ranges are in cell coordinates.
pub fn links_for_row(
  content: &Content, row: usize, providers: &[Arc<dyn LinkProvider>],
) -> Vec<GridLink> {
  let Some(cells) = content.row(row) else {
    return Vec::new();
  };
  let line = row as i32 - content.display_offset as i32;
  let context = line_context(cells);
  if context.text.trim().is_empty() && cells.iter().all(|cell| cell.cell.hyperlink.is_none()) {
    return Vec::new();
  }
  providers
    .iter()
    .flat_map(|provider| provider.links_for_line(&context))
    .filter(|span| span.end > span.start)
    .map(|span| {
      let start = context.column_of_byte(span.start);
      let end = context.column_of_byte(span.end);
      GridLink {
        range: Range {
          start: GridPoint::new(line, start),
          end: GridPoint::new(line, end),
        },
        uri: span.uri,
      }
    })
    .filter(|link| link.range.end.column > link.range.start.column)
    .collect()
}

/// The built-in OSC 8 links plus all registered providers' links for one row.
pub fn all_links_for_row(
  content: &Content, row: usize, providers: &[Arc<dyn LinkProvider>],
) -> Vec<GridLink> {
  let Some(cells) = content.row(row) else {
    return Vec::new();
  };
  let line = row as i32 - content.display_offset as i32;
  let mut all = osc8_links(cells, line);
  all.extend(links_for_row(content, row, providers));
  all
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
  use woocraft_terminal::{Cell, IndexedCell, SpawnOptions, TerminalBounds, TerminalSession};

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

  /// Builds cells from `(char, is_wide)` pairs, inserting wide-char spacers.
  fn content_from_widths(row: &[(char, bool)]) -> Content {
    let mut content = Content::empty();
    content.screen_lines = 1;
    let mut column = 0;
    for &(c, wide) in row {
      let mut flags = CellFlags::empty();
      if wide {
        flags.insert(CellFlags::WIDE_CHAR);
      }
      content.cells.push(IndexedCell {
        point: GridPoint::new(0, column),
        cell: Cell {
          c,
          flags,
          ..Cell::default()
        },
      });
      column += 1;
      if wide {
        content.cells.push(IndexedCell {
          point: GridPoint::new(0, column),
          cell: Cell {
            c: ' ',
            flags: CellFlags::WIDE_CHAR_SPACER,
            ..Cell::default()
          },
        });
        column += 1;
      }
    }
    content.columns = column;
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
  fn wide_chars_shift_text_offsets_to_columns() {
    // "未知的命令：https://x.dev" — six wide characters (each a 3-byte UTF-8
    // char) occupy twelve grid columns before the URL. A provider sees byte
    // offset 18 for "https"; the grid column is 12.
    let mut row: Vec<(char, bool)> = "未知的命令：".chars().map(|c| (c, true)).collect();
    row.extend("https://x.dev".chars().map(|c| (c, false)));
    let content = content_from_widths(&row);

    let links = links_for_row(&content, 0, &[Arc::new(UrlProvider)]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, GridPoint::new(0, 12));
    // The URL is 13 ASCII characters, so the link ends at column 25.
    assert_eq!(links[0].range.end, GridPoint::new(0, 25));
    assert!(links[0].contains(GridPoint::new(0, 12)));
    assert!(links[0].contains(GridPoint::new(0, 24)));
    assert!(!links[0].contains(GridPoint::new(0, 25)));

    // Hit-testing lands on the visible URL, not 6–12 columns to the left.
    assert!(link_at(&content, GridPoint::new(0, 12), &[Arc::new(UrlProvider)]).is_some());
    assert!(link_at(&content, GridPoint::new(0, 13), &[Arc::new(UrlProvider)]).is_some());
    assert!(link_at(&content, GridPoint::new(0, 0), &[Arc::new(UrlProvider)]).is_none());
    assert!(link_at(&content, GridPoint::new(0, 11), &[Arc::new(UrlProvider)]).is_none());
  }

  #[test]
  fn wide_chars_in_text_map_end_offsets_after_the_link() {
    // A wide character *inside* the row after the link must not truncate or
    // extend the link: the exclusive end maps past the last character.
    let row: Vec<(char, bool)> = "http://界.dev"
      .chars()
      .enumerate()
      .map(|(index, c)| (c, index == 7))
      .collect();
    let content = content_from_widths(&row);
    let context = line_context(&content.cells);
    assert_eq!(context.text, "http://界.dev");

    // '界' starts at column 7 and spans two columns (7..9); the final 'v'
    // sits at column 12, and the end sentinel maps to column 13.
    assert_eq!(context.column_of_byte(0), 0);
    assert_eq!(context.column_of_byte("http://".len()), 7);
    assert_eq!(context.column_of_byte(context.text.len()), 13);
  }

  #[test]
  fn osc8_links_use_cell_columns_directly() {
    let mut row: Vec<(char, bool)> = "未知的命令：".chars().map(|c| (c, true)).collect();
    row.extend("https://x.dev".chars().map(|c| (c, false)));
    let mut content = content_from_widths(&row);
    let uri: Arc<str> = Arc::from("https://x.dev");
    for cell in &mut content.cells[12..] {
      cell.cell.hyperlink = Some(uri.clone());
    }

    let links = all_links_for_row(&content, 0, &[]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, GridPoint::new(0, 12));
    assert_eq!(links[0].range.end, GridPoint::new(0, 25));
    assert!(link_at(&content, GridPoint::new(0, 12), &[]).is_some());
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

  /// End-to-end check against the real emulator: alacritty (via
  /// `unicode-width`) is the sole width authority — it decomposes ZWJ emoji
  /// sequences into wide cells with zero-width attachments, and the link
  /// mapping must follow that grid exactly, never re-measure.
  #[test]
  fn emulator_grid_is_the_width_authority_for_links() {
    let session = TerminalSession::spawn_display_only(
      SpawnOptions::default(),
      TerminalBounds::new(20.0, 8.0, 80, 4),
    );
    // "中文" + 👨‍👩‍👧 (ZWJ sequence, hyperlinked via OSC 8) + "链接".
    session.feed_display(
      concat!(
        "\u{4e2d}\u{6587}",
        "\x1b]8;;https://x.dev\x1b\\",
        "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}",
        "\x1b]8;;\x1b\\",
        "\u{94fe}\u{63a5}"
      )
      .as_bytes(),
    );
    let content = session.snapshot();

    // The grid: 中文 = columns 0..4, the three emoji = columns 4..10 (each
    // wide cell carrying the following ZWJ as a zero-width attachment),
    // 链接 = columns 10..14.
    let emoji_cell = content
      .cells
      .iter()
      .find(|cell| cell.cell.c == '\u{1f468}')
      .expect("base emoji cell");
    assert!(emoji_cell.cell.flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(emoji_cell.point.column, 4);
    assert!(emoji_cell.cell.zerowidth.contains(&'\u{200d}'));

    let links = all_links_for_row(&content, 0, &[]);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].range.start, GridPoint::new(0, 4));
    assert_eq!(links[0].range.end, GridPoint::new(0, 10));
    assert!(link_at(&content, GridPoint::new(0, 9), &[]).is_some());
    assert!(link_at(&content, GridPoint::new(0, 10), &[]).is_none());
  }
}
