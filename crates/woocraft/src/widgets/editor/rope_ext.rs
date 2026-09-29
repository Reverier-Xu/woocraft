use std::ops::Range;

use gpui_sum_tree::Bias;
use lsp_types::Position;
use ropey::{LineType, Rope, RopeSlice};
use tree_sitter::Point;

/// An iterator over the lines of a `Rope`.
pub struct RopeLines<'a> {
  rope: &'a Rope,
  row: usize,
  end_row: usize,
}

impl<'a> RopeLines<'a> {
  /// Create a new `RopeLines` iterator.
  pub fn new(rope: &'a Rope) -> Self {
    let end_row = rope.lines_len();
    Self {
      row: 0,
      end_row,
      rope,
    }
  }
}
impl<'a> Iterator for RopeLines<'a> {
  type Item = RopeSlice<'a>;

  #[inline]
  fn next(&mut self) -> Option<Self::Item> {
    if self.row >= self.end_row {
      return None;
    }

    let line = self.rope.slice_line(self.row);
    self.row += 1;
    Some(line)
  }

  #[inline]
  fn nth(&mut self, n: usize) -> Option<Self::Item> {
    self.row = self.row.saturating_add(n);
    self.next()
  }

  #[inline]
  fn size_hint(&self) -> (usize, Option<usize>) {
    let len = self.end_row - self.row;
    (len, Some(len))
  }
}

impl std::iter::ExactSizeIterator for RopeLines<'_> {}
impl std::iter::FusedIterator for RopeLines<'_> {}

/// An extension trait for [`Rope`] to provide additional utility methods.
pub trait RopeExt {
  /// Start offset of the line at the given row (0-based) index.
  ///
  /// # Example
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  ///
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// assert_eq!(rope.line_start_offset(0), 0);
  /// assert_eq!(rope.line_start_offset(1), 6);
  /// ```
  fn line_start_offset(&self, row: usize) -> usize;

  /// Line the end offset (including `\n`) of the line at the given row
  /// (0-based) index.
  ///
  /// Return the end of the rope if the row is out of bounds.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// assert_eq!(rope.line_end_offset(0), 5); // "Hello\n"
  /// assert_eq!(rope.line_end_offset(1), 12); // "World\r\n"
  /// ```
  fn line_end_offset(&self, row: usize) -> usize;

  /// Return a line slice at the given row (0-based) index. including `\r` if
  /// present, but not `\n`.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// assert_eq!(rope.slice_line(0).to_string(), "Hello");
  /// assert_eq!(rope.slice_line(1).to_string(), "World\r");
  /// assert_eq!(rope.slice_line(2).to_string(), "This is a test 中文");
  /// assert_eq!(rope.slice_line(6).to_string(), ""); // out of bounds
  /// ```
  fn slice_line(&self, row: usize) -> RopeSlice<'_>;

  /// Return a slice of rows in the given range (0-based, end exclusive).
  ///
  /// If the range is out of bounds, it will be clamped to the valid range.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// assert_eq!(rope.slice_lines(0..2).to_string(), "Hello\nWorld\r");
  /// assert_eq!(rope.slice_lines(1..3).to_string(), "World\r\nThis is a test 中文");
  /// assert_eq!(rope.slice_lines(2..5).to_string(), "This is a test 中文\nRope");
  /// assert_eq!(rope.slice_lines(3..10).to_string(), "Rope");
  /// assert_eq!(rope.slice_lines(5..10).to_string(), ""); // out of bounds
  /// ```
  fn slice_lines(&self, rows_range: Range<usize>) -> RopeSlice<'_>;

  /// Return an iterator over all lines in the rope.
  ///
  /// Each line slice includes `\r` if present, but not `\n`.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// let lines: Vec<_> = rope.iter_lines().map(|r| r.to_string()).collect();
  /// assert_eq!(lines, vec!["Hello", "World\r", "This is a test 中文", "Rope"]);
  /// ```
  fn iter_lines(&self) -> RopeLines<'_>;

  /// Return the number of lines in the rope.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// assert_eq!(rope.lines_len(), 4);
  /// ```
  fn lines_len(&self) -> usize;

  /// Return the length of the row (0-based) in characters, including `\r` if
  /// present, but not `\n`.
  ///
  /// If the row is out of bounds, return 0.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// assert_eq!(rope.line_len(0), 5); // "Hello"
  /// assert_eq!(rope.line_len(1), 6); // "World\r"
  /// assert_eq!(rope.line_len(2), 21); // "This is a test 中文"
  /// assert_eq!(rope.line_len(4), 0); // out of bounds
  /// ```
  fn line_len(&self, row: usize) -> usize;

  /// Replace the text in the given byte range with new text.
  ///
  /// # Panics
  ///
  /// - If the range is not on char boundary.
  /// - If the range is out of bounds.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let mut rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
  /// rope.replace(6..11, "Universe");
  /// assert_eq!(rope.to_string(), "Hello\nUniverse\r\nThis is a test 中文\nRope");
  /// ```
  fn replace(&mut self, range: Range<usize>, new_text: &str);

  /// Get char at the given offset (byte).
  ///
  /// - If the offset is in the middle of a multi-byte character will panic.
  /// - If the offset is out of bounds, return None.
  fn char_at(&self, offset: usize) -> Option<char>;

  /// Get the byte offset from the given line, column [`Position`] (0-based).
  ///
  /// The column is in characters.
  fn position_to_offset(&self, line_col: &Position) -> usize;

  /// Get the line, column [`Position`] (0-based) from the given byte offset.
  ///
  /// The column is in characters.
  fn offset_to_position(&self, offset: usize) -> Position;

  /// Get point (row, column) from the given byte offset.
  ///
  /// The column is in bytes.
  fn offset_to_point(&self, offset: usize) -> Point;

  /// Get byte offset from the given point (row, column).
  ///
  /// The column is 0-based in bytes.
  fn point_to_offset(&self, point: Point) -> usize;

  /// Get the word byte range at the given byte offset (0-based).
  fn word_range(&self, offset: usize) -> Option<Range<usize>>;

  /// Convert offset in UTF-16 to byte offset (0-based).
  ///
  /// Runs in O(log N) time.
  fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize;

  /// Convert byte offset (0-based) to offset in UTF-16.
  ///
  /// Runs in O(log N) time.
  fn offset_to_offset_utf16(&self, offset: usize) -> usize;

  /// Get a clipped offset (avoid in a char boundary).
  ///
  /// - If Bias::Left and inside the char boundary, return the ix - 1;
  /// - If Bias::Right and in inside char boundary, return the ix + 1;
  /// - Otherwise return the ix.
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// use gpui_sum_tree::Bias;
  ///
  /// let rope = Rope::from("Hello 中文🎉 test\nRope");
  /// assert_eq!(rope.clip_offset(5, Bias::Left), 5);
  /// // Inside multi-byte character '中' (3 bytes)
  /// assert_eq!(rope.clip_offset(7, Bias::Left), 6);
  /// assert_eq!(rope.clip_offset(7, Bias::Right), 9);
  /// ```
  fn clip_offset(&self, offset: usize, bias: Bias) -> usize;

  /// Convert offset in characters to byte offset (0-based).
  ///
  /// Run in O(n) time.
  ///
  /// # Example
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("a 中文🎉 test\nRope");
  /// assert_eq!(rope.char_index_to_offset(0), 0);
  /// assert_eq!(rope.char_index_to_offset(1), 1);
  /// assert_eq!(rope.char_index_to_offset(3), "a 中".len());
  /// assert_eq!(rope.char_index_to_offset(5), "a 中文🎉".len());
  /// ```
  fn char_index_to_offset(&self, char_index: usize) -> usize;

  /// Convert byte offset (0-based) to offset in characters.
  ///
  /// Run in O(n) time.
  ///
  /// # Example
  ///
  /// ```
  /// use woocraft::{Rope, RopeExt};
  /// let rope = Rope::from("a 中文🎉 test\nRope");
  /// assert_eq!(rope.offset_to_char_index(0), 0);
  /// assert_eq!(rope.offset_to_char_index(1), 1);
  /// assert_eq!(rope.offset_to_char_index(3), 3);
  /// assert_eq!(rope.offset_to_char_index(4), 3);
  /// ```
  fn offset_to_char_index(&self, offset: usize) -> usize;
}

impl RopeExt for Rope {
  fn slice_line(&self, row: usize) -> RopeSlice<'_> {
    let total_lines = self.lines_len();
    if row >= total_lines {
      return self.slice(0..0);
    }

    let line = self.line(row, LineType::LF);
    if line.len() > 0 {
      let line_end = line.len() - 1;
      if line.is_char_boundary(line_end) && line.char(line_end) == '\n' {
        return line.slice(..line_end);
      }
    }

    line
  }

  fn slice_lines(&self, rows_range: Range<usize>) -> RopeSlice<'_> {
    let start = self.line_start_offset(rows_range.start);
    let end = self.line_end_offset(rows_range.end.saturating_sub(1));
    self.slice(start..end)
  }

  fn iter_lines(&self) -> RopeLines<'_> {
    RopeLines::new(self)
  }

  fn line_len(&self, row: usize) -> usize {
    self.slice_line(row).len()
  }

  fn line_start_offset(&self, row: usize) -> usize {
    self.point_to_offset(Point::new(row, 0))
  }

  fn offset_to_point(&self, offset: usize) -> Point {
    let offset = self.clip_offset(offset, Bias::Left);
    let row = self.byte_to_line_idx(offset, LineType::LF);
    let line_start = self.line_to_byte_idx(row, LineType::LF);
    let column = offset.saturating_sub(line_start);
    Point::new(row, column)
  }

  fn point_to_offset(&self, point: Point) -> usize {
    if point.row >= self.lines_len() {
      return self.len();
    }

    let line_start = self.line_to_byte_idx(point.row, LineType::LF);
    line_start + point.column
  }

  fn position_to_offset(&self, pos: &Position) -> usize {
    let line = self.slice_line(pos.line as usize);
    self.line_start_offset(pos.line as usize)
      + line
        .chars()
        .take(pos.character as usize)
        .map(|c| c.len_utf8())
        .sum::<usize>()
  }

  fn offset_to_position(&self, offset: usize) -> Position {
    let point = self.offset_to_point(offset);
    let line = self.slice_line(point.row);
    let offset = line.utf16_to_byte_idx(line.byte_to_utf16_idx(point.column));
    let character = line.slice(..offset).chars().count();
    Position::new(point.row as u32, character as u32)
  }

  fn line_end_offset(&self, row: usize) -> usize {
    if row > self.lines_len() {
      return self.len();
    }

    self.line_start_offset(row) + self.line_len(row)
  }

  fn lines_len(&self) -> usize {
    self.len_lines(LineType::LF)
  }

  fn char_at(&self, offset: usize) -> Option<char> {
    if offset > self.len() {
      return None;
    }

    self.get_char(offset).ok()
  }

  fn word_range(&self, offset: usize) -> Option<Range<usize>> {
    if offset >= self.len() {
      return None;
    }

    let mut left = String::new();
    let offset = self.clip_offset(offset, Bias::Left);
    for c in self.chars_at(offset).reversed() {
      if c.is_alphanumeric() || c == '_' {
        left.insert(0, c);
      } else {
        break;
      }
    }
    let start = offset.saturating_sub(left.len());

    let right = self
      .chars_at(offset)
      .take_while(|c| c.is_alphanumeric() || *c == '_')
      .collect::<String>();

    let end = offset + right.len();

    if start == end { None } else { Some(start..end) }
  }

  #[inline]
  fn offset_utf16_to_offset(&self, offset_utf16: usize) -> usize {
    if offset_utf16 > self.len_utf16() {
      return self.len();
    }

    self.utf16_to_byte_idx(offset_utf16)
  }

  #[inline]
  fn offset_to_offset_utf16(&self, offset: usize) -> usize {
    if offset > self.len() {
      return self.len_utf16();
    }

    self.byte_to_utf16_idx(offset)
  }

  fn replace(&mut self, range: Range<usize>, new_text: &str) {
    let range = self.clip_offset(range.start, Bias::Left)..self.clip_offset(range.end, Bias::Right);
    self.remove(range.clone());
    self.insert(range.start, new_text);
  }

  fn clip_offset(&self, offset: usize, bias: Bias) -> usize {
    if offset > self.len() {
      return self.len();
    }

    if self.is_char_boundary(offset) {
      return offset;
    }

    if bias == Bias::Left {
      self.floor_char_boundary(offset)
    } else {
      self.ceil_char_boundary(offset)
    }
  }

  fn char_index_to_offset(&self, char_offset: usize) -> usize {
    self.chars().take(char_offset).map(|c| c.len_utf8()).sum()
  }

  fn offset_to_char_index(&self, offset: usize) -> usize {
    let offset = self.clip_offset(offset, Bias::Right);
    self.slice(..offset).chars().count()
  }
}

/// Length in bytes of the longest common prefix of two ropes.
///
/// Compares chunk-by-chunk (effectively `memcmp` speed), so a shared prefix
/// costs one pass over the identical bytes and stops at the first differing
/// byte. The returned length is always a char boundary in both ropes, because
/// both byte strings start with the same bytes.
fn common_prefix_len(a: &Rope, b: &Rope) -> usize {
  let mut a_chunks = a.chunks();
  let mut b_chunks = b.chunks();
  let mut a_chunk = a_chunks.next().unwrap_or("");
  let mut b_chunk = b_chunks.next().unwrap_or("");
  let mut prefix = 0;

  loop {
    if a_chunk.is_empty() {
      a_chunk = a_chunks.next().unwrap_or("");
    }
    if b_chunk.is_empty() {
      b_chunk = b_chunks.next().unwrap_or("");
    }
    if a_chunk.is_empty() || b_chunk.is_empty() {
      return prefix;
    }

    let limit = a_chunk.len().min(b_chunk.len());
    let matched = a_chunk.as_bytes()[..limit]
      .iter()
      .zip(b_chunk.as_bytes()[..limit].iter())
      .take_while(|(x, y)| x == y)
      .count();
    prefix += matched;
    if matched < limit {
      return prefix;
    }

    a_chunk = &a_chunk[limit..];
    b_chunk = &b_chunk[limit..];
  }
}

/// Length in bytes of the longest common suffix of two ropes, considering at
/// most `limit` bytes.
///
/// Walks the rope trees from the end with chunk cursors, because ropey's
/// chunk iterator is not double-ended.
fn common_suffix_len(a: &Rope, b: &Rope, limit: usize) -> usize {
  if limit == 0 {
    return 0;
  }

  let mut a_cursor = a.chunk_cursor();
  while a_cursor.next() {}
  let mut b_cursor = b.chunk_cursor();
  while b_cursor.next() {}

  let mut suffix = 0;
  loop {
    let a_chunk = a_cursor.chunk();
    let b_chunk = b_cursor.chunk();
    let limit_here = a_chunk.len().min(b_chunk.len()).min(limit - suffix);

    let matched = a_chunk.as_bytes()[a_chunk.len() - limit_here..]
      .iter()
      .rev()
      .zip(
        b_chunk.as_bytes()[b_chunk.len() - limit_here..]
          .iter()
          .rev(),
      )
      .take_while(|(x, y)| x == y)
      .count();
    suffix += matched;
    if matched < limit_here {
      return suffix;
    }
    if suffix >= limit {
      return suffix;
    }

    // Both chunks fully matched; step to the previous chunks. Running out
    // of chunks on either side means everything compared equal so far.
    if !a_cursor.prev() || !b_cursor.prev() {
      return suffix;
    }
  }
}

/// Derive the minimal single edit (range in `old` + inserted text) that turns
/// `old` into `new`, or `None` when the ropes are equal.
///
/// This lets the editor re-wrap only the changed region of an externally
/// mutated document (e.g. a log backend appending batches of lines) instead
/// of rebuilding the whole layout. The comparison cost is proportional to the
/// changed region plus one `memcmp`-speed pass over the shared prefix — for
/// append-only logs the prefix is the whole previous document, which is still
/// far cheaper than re-wrapping every line.
pub(crate) fn rope_diff_edit(old: &Rope, new: &Rope) -> Option<(Range<usize>, String)> {
  let min_len = old.len().min(new.len());
  let raw_prefix = common_prefix_len(old, new);
  // The suffix must not reach into the common prefix.
  let raw_suffix = common_suffix_len(old, new, min_len - raw_prefix);

  // The divergence points can land inside a multi-byte character. Snap them
  // to char boundaries that are valid in BOTH ropes: within the identical
  // prefix/suffix regions both ropes share the same byte content, so walking
  // a few bytes left/right finds a shared boundary (UTF-8 guarantees one
  // within 4 bytes).
  let mut prefix = raw_prefix;
  while prefix > 0 && !(old.is_char_boundary(prefix) && new.is_char_boundary(prefix)) {
    prefix -= 1;
  }

  let mut suffix = raw_suffix;
  while suffix < min_len - prefix
    && !(old.is_char_boundary(old.len() - suffix) && new.is_char_boundary(new.len() - suffix))
  {
    suffix += 1;
  }

  let old_range = prefix..old.len() - suffix;
  let inserted_len = new.len() - suffix - prefix;
  if old_range.is_empty() && inserted_len == 0 {
    return None;
  }

  let inserted = new.slice(prefix..new.len() - suffix).to_string();
  Some((old_range, inserted))
}

#[cfg(test)]
mod tests {
  use gpui_sum_tree::Bias;
  use ropey::Rope;
  use tree_sitter::Point;

  use super::rope_diff_edit;
  use crate::widgets::editor::{Position, RopeExt};

  #[test]
  fn test_slice_line() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    assert_eq!(rope.slice_line(0).to_string(), "Hello");
    assert_eq!(rope.slice_line(1).to_string(), "World\r");
    assert_eq!(rope.slice_line(2).to_string(), "This is a test 中文");
    assert_eq!(rope.slice_line(3).to_string(), "Rope");

    // over bounds
    assert_eq!(rope.slice_line(6).to_string(), "");

    // only have \r end
    let rope = Rope::from("Hello\r");
    assert_eq!(rope.slice_line(0).to_string(), "Hello\r");
    assert_eq!(rope.slice_line(1).to_string(), "");
  }

  #[test]
  fn test_lines_len() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    assert_eq!(rope.lines_len(), 4);
    let rope = Rope::from("");
    assert_eq!(rope.lines_len(), 1);
    let rope = Rope::from("Single line");
    assert_eq!(rope.lines_len(), 1);

    // only have \r end
    let rope = Rope::from("Hello\r");
    assert_eq!(rope.lines_len(), 1);
  }

  #[test]
  fn test_lines() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope\r");
    let lines: Vec<_> = rope.iter_lines().map(|r| r.to_string()).collect();
    assert_eq!(
      lines,
      vec!["Hello", "World\r", "This is a test 中文", "Rope\r"]
    );
  }

  #[test]
  fn test_eq() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    assert!(rope.eq(&Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope")));
    assert!(!rope.eq(&Rope::from("Hello\nWorld")));

    let rope1 = rope.clone();
    assert!(rope.eq(&rope1));
  }

  #[test]
  fn test_iter_lines() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    let lines: Vec<_> = rope
      .iter_lines()
      .skip(1)
      .take(2)
      .map(|r| r.to_string())
      .collect();
    assert_eq!(lines, vec!["World\r", "This is a test 中文"]);
  }

  #[test]
  fn test_line_start_end_offset() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    assert_eq!(rope.line_start_offset(0), 0);
    assert_eq!(rope.line_end_offset(0), 5);

    assert_eq!(rope.line_start_offset(1), 6);
    assert_eq!(rope.line_end_offset(1), 12);

    assert_eq!(rope.line_start_offset(2), 13);
    assert_eq!(rope.line_end_offset(2), 34);

    assert_eq!(rope.line_start_offset(3), 35);
    assert_eq!(rope.line_end_offset(3), 39);

    assert_eq!(rope.line_start_offset(4), 39);
    assert_eq!(rope.line_end_offset(4), 39);
  }

  #[test]
  fn test_line_column() {
    let rope = Rope::from("a 中文🎉 test\nRope");
    assert_eq!(rope.position_to_offset(&Position::new(0, 3)), "a 中".len());
    assert_eq!(
      rope.position_to_offset(&Position::new(0, 5)),
      "a 中文🎉".len()
    );
    assert_eq!(
      rope.position_to_offset(&Position::new(1, 1)),
      "a 中文🎉 test\nR".len()
    );

    assert_eq!(
      rope.offset_to_position("a 中文🎉 test\nR".len()),
      Position::new(1, 1)
    );
    assert_eq!(
      rope.offset_to_position("a 中文🎉".len()),
      Position::new(0, 5)
    );
  }

  #[test]
  fn test_offset_to_point() {
    let rope = Rope::from("a 中文🎉 test\nRope");
    assert_eq!(rope.offset_to_point(0), Point::new(0, 0));
    assert_eq!(rope.offset_to_point(1), Point::new(0, 1));
    assert_eq!(rope.offset_to_point("a 中".len()), Point::new(0, 5));
    assert_eq!(rope.offset_to_point("a 中文🎉".len()), Point::new(0, 12));
    assert_eq!(
      rope.offset_to_point("a 中文🎉 test\nR".len()),
      Point::new(1, 1)
    );
  }

  #[test]
  fn test_point_to_offset() {
    let rope = Rope::from("a 中文🎉 test\nRope");
    assert_eq!(rope.point_to_offset(Point::new(0, 0)), 0);
    assert_eq!(rope.point_to_offset(Point::new(0, 1)), 1);
    assert_eq!(rope.point_to_offset(Point::new(0, 5)), "a 中".len());
    assert_eq!(rope.point_to_offset(Point::new(0, 12)), "a 中文🎉".len());
    assert_eq!(
      rope.point_to_offset(Point::new(1, 1)),
      "a 中文🎉 test\nR".len()
    );
  }

  #[test]
  fn test_char_at() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文🎉\nRope");
    assert_eq!(rope.char_at(0), Some('H'));
    assert_eq!(rope.char_at(5), Some('\n'));
    assert_eq!(rope.char_at(13), Some('T'));
    assert_eq!(rope.char_at(28), Some('中'));
    assert_eq!(rope.char_at(34), Some('🎉'));
    assert_eq!(rope.char_at(38), Some('\n'));
    assert_eq!(rope.char_at(50), None);
  }

  #[test]
  fn test_word_range() {
    let rope = Rope::from("Hello\nWorld\r\nThis is a test 中文 世界\nRope");
    assert_eq!(rope.word_range(0), Some(0..5));
    assert_eq!(rope.word_range(6), Some(6..11));
    assert_eq!(rope.word_range(8), Some(6..11));
    assert_eq!(rope.word_range(11), Some(6..11));
    assert_eq!(rope.word_range(12), None);
    assert_eq!(rope.word_range(13), Some(13..17));
    assert_eq!(rope.word_range(31), Some(28..34));
    assert_eq!(rope.word_range(35), Some(35..41));
    assert_eq!(rope.word_range(38), Some(35..41));
    assert_eq!(rope.word_range(42), Some(42..46));
    assert_eq!(rope.word_range(45), Some(42..46));
  }

  #[test]
  fn test_offset_utf16_conversion() {
    let rope = Rope::from("hello 中文🎉 test\nRope");
    assert_eq!(rope.offset_to_offset_utf16("hello".len()), 5);
    assert_eq!(rope.offset_to_offset_utf16("hello 中".len()), 7);
    assert_eq!(rope.offset_to_offset_utf16("hello 中文".len()), 8);
    assert_eq!(rope.offset_to_offset_utf16("hello 中文🎉".len()), 10);
    assert_eq!(rope.offset_to_offset_utf16(100), 20);

    assert_eq!(rope.offset_utf16_to_offset(5), "hello".len());
    assert_eq!(rope.offset_utf16_to_offset(7), "hello 中".len());
    assert_eq!(rope.offset_utf16_to_offset(8), "hello 中文".len());
    assert_eq!(rope.offset_utf16_to_offset(10), "hello 中文🎉".len());
    assert_eq!(rope.offset_utf16_to_offset(100), rope.len());
  }

  #[test]
  fn test_replace() {
    let mut rope = Rope::from("Hello\nWorld\r\nThis is a test 中文\nRope");
    rope.replace(6..11, "Universe");
    assert_eq!(
      rope.to_string(),
      "Hello\nUniverse\r\nThis is a test 中文\nRope"
    );

    rope.replace(0..5, "Hi");
    assert_eq!(
      rope.to_string(),
      "Hi\nUniverse\r\nThis is a test 中文\nRope"
    );

    rope.replace(rope.len() - 4..rope.len(), "String");
    assert_eq!(
      rope.to_string(),
      "Hi\nUniverse\r\nThis is a test 中文\nString"
    );

    // Test for not on a char boundary
    let mut rope = Rope::from("中文");
    rope.replace(0..1, "New");
    // autocorrect-disable
    assert_eq!(rope.to_string(), "New文");
    let mut rope = Rope::from("中文");
    rope.replace(0..2, "New");
    assert_eq!(rope.to_string(), "New文");
    let mut rope = Rope::from("中文");
    rope.replace(0..3, "New");
    assert_eq!(rope.to_string(), "New文");
    // autocorrect-enable
    let mut rope = Rope::from("中文");
    rope.replace(1..4, "New");
    assert_eq!(rope.to_string(), "New");
  }

  #[test]
  fn test_clip_offset() {
    let rope = Rope::from("Hello 中文🎉 test\nRope");
    // Inside multi-byte character '中' (3 bytes)
    assert_eq!(rope.clip_offset(5, Bias::Left), 5);
    assert_eq!(rope.clip_offset(7, Bias::Left), 6);
    assert_eq!(rope.clip_offset(7, Bias::Right), 9);
    assert_eq!(rope.clip_offset(9, Bias::Left), 9);

    // Inside multi-byte character '🎉' (4 bytes)
    assert_eq!(rope.clip_offset(13, Bias::Left), 12);
    assert_eq!(rope.clip_offset(13, Bias::Right), 16);
    assert_eq!(rope.clip_offset(16, Bias::Left), 16);

    // At character boundary
    assert_eq!(rope.clip_offset(5, Bias::Left), 5);
    assert_eq!(rope.clip_offset(5, Bias::Right), 5);

    // Out of bounds
    assert_eq!(rope.clip_offset(26, Bias::Left), 26);
    assert_eq!(rope.clip_offset(100, Bias::Left), 26);
  }

  #[test]
  fn test_char_index_to_offset() {
    let rope = Rope::from("a 中文🎉 test\nRope");
    assert_eq!(rope.char_index_to_offset(0), 0);
    assert_eq!(rope.char_index_to_offset(1), 1);
    assert_eq!(rope.char_index_to_offset(3), "a 中".len());
    assert_eq!(rope.char_index_to_offset(5), "a 中文🎉".len());
    assert_eq!(rope.char_index_to_offset(6), "a 中文🎉 ".len());

    assert_eq!(rope.offset_to_char_index(0), 0);
    assert_eq!(rope.offset_to_char_index(1), 1);
    assert_eq!(rope.offset_to_char_index(3), 3);
    assert_eq!(rope.offset_to_char_index(4), 3);
    assert_eq!(rope.offset_to_char_index(5), 3);
    assert_eq!(rope.offset_to_char_index(6), 4);
    assert_eq!(rope.offset_to_char_index(10), 5);
  }

  #[test]
  fn test_rope_diff_edit_append() {
    let old = Rope::from("one\ntwo\nthree\n");
    let new = Rope::from("one\ntwo\nthree\nfour\nfive\n");

    let (range, inserted) = rope_diff_edit(&old, &new).expect("append is an edit");
    assert_eq!(range, old.len()..old.len());
    assert_eq!(inserted, "four\nfive\n");
  }

  #[test]
  fn test_rope_diff_edit_append_to_partial_last_line() {
    let old = Rope::from("one\ntwo\nthre");
    let new = Rope::from("one\ntwo\nthree\nfour");

    let (range, inserted) = rope_diff_edit(&old, &new).expect("extending the last line is an edit");
    assert_eq!(range, old.len()..old.len());
    assert_eq!(inserted, "e\nfour");
  }

  #[test]
  fn test_rope_diff_edit_middle_change() {
    let old = Rope::from("one\ntypo\nthree\n");
    let new = Rope::from("one\nfine\nthree\n");

    let (range, inserted) = rope_diff_edit(&old, &new).expect("middle change is an edit");
    assert_eq!(range, "one\n".len().."one\ntypo".len());
    assert_eq!(inserted, "fine");
  }

  #[test]
  fn test_rope_diff_edit_shrink() {
    let old = Rope::from("a\nb\nc\nd\ne\nf\n");
    let new = Rope::from("a\nb\nc\n");

    let (range, inserted) = rope_diff_edit(&old, &new).expect("truncation is an edit");
    assert_eq!(range, new.len()..old.len());
    assert_eq!(inserted, "");
  }

  #[test]
  fn test_rope_diff_edit_equal_and_replaced() {
    assert_eq!(rope_diff_edit(&Rope::from(""), &Rope::from("")), None);
    assert_eq!(
      rope_diff_edit(&Rope::from("same"), &Rope::from("same")),
      None
    );

    let (range, inserted) = rope_diff_edit(&Rope::from(""), &Rope::from("hello")).unwrap();
    assert_eq!(range, 0..0);
    assert_eq!(inserted, "hello");

    let (range, inserted) = rope_diff_edit(&Rope::from("hello"), &Rope::from("")).unwrap();
    assert_eq!(range, 0..5);
    assert_eq!(inserted, "");
  }

  #[test]
  fn test_rope_diff_edit_multibyte_boundaries() {
    // Divergence inside multi-byte characters must still produce char
    // boundary offsets, so the derived slice ranges never panic.
    let old = Rope::from("前缀相同 中间不同 后缀相同");
    let new = Rope::from("前缀相同 俩间不同 后缀相同");

    let (range, inserted) = rope_diff_edit(&old, &new).expect("multibyte change is an edit");
    assert_eq!(&old.slice(range.clone()).to_string(), "中");
    assert_eq!(inserted, "俩");

    // Fully disjoint multibyte texts.
    let (range, inserted) = rope_diff_edit(&Rope::from("中文"), &Rope::from("英文")).unwrap();
    assert_eq!(range, 0.."中".len());
    assert_eq!(inserted, "英");
  }

  #[test]
  fn test_rope_diff_edit_large_append_is_line_aligned() {
    let old = Rope::from(
      (0..1000)
        .map(|i| format!("log line {i}\n"))
        .collect::<String>(),
    );
    let mut text = old.to_string();
    text.push_str(&"appended batch\n".repeat(50));
    let new = Rope::from(text);

    let (range, inserted) = rope_diff_edit(&old, &new).expect("batch append is an edit");
    assert_eq!(range, old.len()..old.len());
    assert_eq!(inserted, "appended batch\n".repeat(50));
  }
}
