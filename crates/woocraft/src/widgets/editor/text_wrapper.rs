use std::ops::Range;

use gpui::{
  App, Font, Half, Pixels, Point, ShapedLine, SharedString, Size, TextAlign, TextRun, Window,
  point, px, size,
};
use ropey::Rope;
use smallvec::SmallVec;

use super::{
  rope_ext::RopeExt,
  state::{LastLayout, WhitespaceIndicators},
};

/// A line with soft wrapped lines info.
#[derive(Debug, Clone)]
pub(super) struct LineItem {
  /// The original line text, without end `\n`.
  line: Rope,
  /// The soft wrapped lines relative byte range (0..line.len) of this line
  /// (Include first line).
  ///
  /// Not contains the line end `\n`.
  pub(super) wrapped_lines: Vec<Range<usize>>,
}

impl LineItem {
  /// Get the bytes length of this line.
  #[inline]
  pub(super) fn len(&self) -> usize {
    self.line.len()
  }

  /// Get number of soft wrapped lines of this line (include the first line).
  #[inline]
  pub(super) fn lines_len(&self) -> usize {
    self.wrapped_lines.len()
  }

  /// Get the height of this line item with given line height.
  #[allow(dead_code)]
  pub(super) fn height(&self, line_height: Pixels) -> Pixels {
    self.lines_len() as f32 * line_height
  }
}

#[derive(Debug, Default)]
pub(super) struct LongestRow {
  /// The 0-based row index.
  pub row: usize,
  /// The bytes length of the longest line.
  pub len: usize,
}

/// Used to prepare the text with soft wrap to be get lines to displayed in the
/// Editor.
///
/// After use lines to calculate the scroll size of the Editor.
pub(super) struct TextWrapper {
  text: Rope,
  /// Total wrapped lines (Inlucde the first line), value is start and end index
  /// of the line.
  soft_lines: usize,
  font: Font,
  font_size: Pixels,
  /// If is none, it means the text is not wrapped
  wrap_width: Option<Pixels>,
  /// The longest (row, bytes len) in characters, used to calculate the
  /// horizontal scroll width.
  pub(super) longest_row: LongestRow,
  /// The lines by split \n
  pub(super) lines: Vec<LineItem>,
  display_row_starts: Vec<usize>,

  _initialized: bool,
}

#[allow(unused)]
impl TextWrapper {
  pub(super) fn new(font: Font, font_size: Pixels, wrap_width: Option<Pixels>) -> Self {
    Self {
      text: Rope::new(),
      font,
      font_size,
      wrap_width,
      soft_lines: 0,
      longest_row: LongestRow::default(),
      lines: Vec::new(),
      display_row_starts: Vec::new(),
      _initialized: false,
    }
  }

  #[inline]
  pub(super) fn set_default_text(&mut self, text: &Rope) {
    self.text = text.clone();
  }

  /// Get the total number of lines including wrapped lines.
  #[inline]
  pub(super) fn len(&self) -> usize {
    self.soft_lines
  }

  pub(crate) fn display_row_to_line_row(&self, display_row: usize) -> Option<(usize, usize)> {
    if self.lines.is_empty() {
      return Some((0, 0));
    }

    if self.display_row_starts.len() != self.lines.len() {
      let mut start = 0;
      let display_row = display_row.min(
        self
          .lines
          .iter()
          .map(LineItem::lines_len)
          .sum::<usize>()
          .saturating_sub(1),
      );
      for (ix, line) in self.lines.iter().enumerate() {
        let end = start + line.lines_len();
        if display_row < end {
          return Some((ix, display_row.saturating_sub(start)));
        }
        start = end;
      }

      return Some((self.lines.len().saturating_sub(1), 0));
    }

    let display_row = display_row.min(self.soft_lines.saturating_sub(1));
    let ix = match self.display_row_starts.binary_search(&display_row) {
      Ok(ix) => ix,
      Err(0) => 0,
      Err(ix) => ix.saturating_sub(1),
    };
    let local_row = display_row.saturating_sub(self.display_row_starts[ix]);
    Some((ix, local_row))
  }

  /// Get the line item by row index.
  #[inline]
  pub(super) fn line(&self, row: usize) -> Option<&LineItem> {
    self.lines.get(row)
  }

  pub(super) fn sync(
    &mut self, text: &Rope, font: Font, font_size: Pixels, wrap_width: Option<Pixels>,
    window: &Window, cx: &mut App,
  ) -> bool {
    let text_changed = self.text != *text;
    let font_changed = !self.font.eq(&font) || self.font_size != font_size;
    let wrap_width_changed = self.wrap_width != wrap_width;

    if !self._initialized || text_changed || font_changed || wrap_width_changed {
      self._initialized = true;
      self.font = font;
      self.font_size = font_size;
      self.wrap_width = wrap_width;
      self.update_all(text, window, cx);
      return true;
    }

    false
  }

  pub(super) fn set_wrap_width(
    &mut self, wrap_width: Option<Pixels>, window: &Window, cx: &mut App,
  ) {
    if wrap_width == self.wrap_width {
      return;
    }

    self.wrap_width = wrap_width;
    self.update_all(&self.text.clone(), window, cx);
  }

  pub(super) fn set_font(&mut self, font: Font, font_size: Pixels, window: &Window, cx: &mut App) {
    if self.font.eq(&font) && self.font_size == font_size {
      return;
    }

    self.font = font;
    self.font_size = font_size;
    self.update_all(&self.text.clone(), window, cx);
  }

  pub(super) fn prepare_if_need(&mut self, text: &Rope, window: &Window, cx: &mut App) {
    if self._initialized {
      return;
    }
    self._initialized = true;
    self.update_all(text, window, cx);
  }

  /// Update the text wrapper and recalculate the wrapped lines.
  ///
  /// If the `text` is the same as the current text, do nothing.
  ///
  /// - `changed_text`: The text [`Rope`] that has changed.
  /// - `range`: The `selected_range` before change.
  /// - `new_text`: The inserted text.
  /// - `force`: Whether to force the update, if false, the update will be
  ///   skipped if the text is the same.
  /// - `cx`: The application context.
  pub(super) fn update(
    &mut self, changed_text: &Rope, range: &Range<usize>, new_text: &Rope, window: &Window,
    cx: &mut App,
  ) {
    let font = self.font.clone();
    let font_size = self.font_size;
    self._update(
      changed_text,
      range,
      new_text,
      &mut |line_str, wrap_width| {
        let shaped_line = window.text_system().shape_line(
          SharedString::from(line_str.to_string()),
          font_size,
          &[TextRun {
            len: line_str.len(),
            font: font.clone(),
            color: gpui::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
          }],
          None,
        );
        break_all_ranges(line_str, &shaped_line, wrap_width)
      },
    );
  }

  fn _update<F>(
    &mut self, changed_text: &Rope, range: &Range<usize>, new_text: &Rope, wrap_line: &mut F,
  ) where
    F: FnMut(&str, Pixels) -> Vec<Range<usize>>, {
    // Remove the old changed lines.
    let start_row = self.text.offset_to_point(range.start).row;
    let start_row = start_row.min(self.lines.len().saturating_sub(1));
    let end_row = self.text.offset_to_point(range.end).row;
    let end_row = end_row.min(self.lines.len().saturating_sub(1));
    let rows_range = start_row..=end_row;

    if rows_range.contains(&self.longest_row.row) {
      self.longest_row = LongestRow::default();
    }

    let mut longest_row_ix = self.longest_row.row;
    let mut longest_row_len = self.longest_row.len;

    // To add the new lines.
    let new_start_row = changed_text.offset_to_point(range.start).row;
    let new_start_offset = changed_text.line_start_offset(new_start_row);
    let new_end_row = changed_text
      .offset_to_point(range.start + new_text.len())
      .row;
    let new_end_offset = changed_text.line_end_offset(new_end_row);
    let new_range = new_start_offset..new_end_offset;

    let mut new_lines = vec![];
    let wrap_width = self.wrap_width;

    // line not contains `\n`.
    for (ix, line) in Rope::from(changed_text.slice(new_range))
      .iter_lines()
      .enumerate()
    {
      let line_str = line.to_string();
      let mut wrapped_lines = vec![];
      let mut prev_boundary_ix = 0;

      if line_str.len() > longest_row_len {
        longest_row_ix = new_start_row + ix;
        longest_row_len = line_str.len();
      }

      // If wrap_width is Pixels::MAX, skip wrapping to disable word wrap
      if let Some(wrap_width) = wrap_width {
        wrapped_lines = wrap_line(&line_str, wrap_width);
        prev_boundary_ix = wrapped_lines.last().map(|range| range.end).unwrap_or(0);
      }

      // Add the remaining tail when wrapping did not already cover the full line.
      if wrapped_lines.is_empty() || prev_boundary_ix < line.len() {
        wrapped_lines.push(prev_boundary_ix..line.len());
      }

      new_lines.push(LineItem {
        line: Rope::from(line),
        wrapped_lines,
      });
    }

    if self.lines.is_empty() {
      self.lines = new_lines;
    } else {
      self.lines.splice(rows_range, new_lines);
    }

    self.text = changed_text.clone();
    self.display_row_starts.clear();
    self.soft_lines = 0;
    for line in &self.lines {
      self.display_row_starts.push(self.soft_lines);
      self.soft_lines += line.lines_len();
    }
    self.longest_row = LongestRow {
      row: longest_row_ix,
      len: longest_row_len,
    }
  }

  /// Update the text wrapper and recalculate the wrapped lines.
  ///
  /// If the `text` is the same as the current text, do nothing.
  fn update_all(&mut self, text: &Rope, window: &Window, cx: &mut App) {
    self.update(text, &(0..text.len()), text, window, cx);
  }

  /// Return display point (with soft wrap) from the given byte offset in the
  /// text.
  ///
  /// Panics if the `offset` is out of bounds.
  pub(crate) fn offset_to_display_point(&self, offset: usize) -> DisplayPoint {
    let row = self.text.offset_to_point(offset).row;
    let start = self.text.line_start_offset(row);
    let line = &self.lines[row];

    let wrapped_row = self
      .display_row_starts
      .get(row)
      .copied()
      .unwrap_or_else(|| self.lines.iter().take(row).map(LineItem::lines_len).sum());

    let local_offset = offset.saturating_sub(start);
    for (ix, range) in line.wrapped_lines.iter().enumerate() {
      if range.contains(&local_offset) {
        return DisplayPoint::new(
          wrapped_row + ix,
          ix,
          local_offset.saturating_sub(range.start),
        );
      }
    }

    // Otherwise return the eof of the line.
    let last_range = line.wrapped_lines.last().unwrap_or(&(0..0));
    let ix = line.lines_len().saturating_sub(1);
    DisplayPoint::new(wrapped_row + ix, ix, last_range.len())
  }

  /// Return byte offset in the text from the given display point (with soft
  /// wrap).
  ///
  /// Panics if the `point.row` is out of bounds.
  pub(crate) fn display_point_to_offset(&self, point: DisplayPoint) -> usize {
    if let Some((row, local_row)) = self.display_row_to_line_row(point.row) {
      let line_start = self.text.line_start_offset(row);
      let line = &self.lines[row];
      if let Some(range) = line.wrapped_lines.get(local_row) {
        return line_start + (range.start + point.column).min(range.end);
      }

      return line_start + line.len();
    }

    self.text.len()
  }

  pub(crate) fn display_point_to_point(&self, point: DisplayPoint) -> tree_sitter::Point {
    let offset = self.display_point_to_offset(point);
    self.text.offset_to_point(offset)
  }

  pub(crate) fn point_to_display_point(&self, point: tree_sitter::Point) -> DisplayPoint {
    let offset = self.text.point_to_offset(point);
    self.offset_to_display_point(offset)
  }
}

pub(crate) fn break_all_ranges(
  line_str: &str, shaped_line: &ShapedLine, wrap_width: Pixels,
) -> Vec<Range<usize>> {
  if line_str.is_empty() {
    return std::iter::once(0..0).collect();
  }

  if wrap_width <= px(0.) {
    return std::iter::once(0..line_str.len()).collect();
  }

  let mut ranges = Vec::new();
  let mut start = 0;

  for (ix, ch) in line_str.char_indices() {
    let next = ix + ch.len_utf8();
    let width = shaped_line.x_for_index(next) - shaped_line.x_for_index(start);
    if width > wrap_width {
      let break_at = if ix > start { ix } else { next };
      ranges.push(start..break_at);
      start = break_at;
    }
  }

  if start < line_str.len() {
    ranges.push(start..line_str.len());
  }

  if ranges.is_empty() {
    ranges.push(0..0);
  }

  ranges
}

/// The actually display point in the text.
///
/// This is usually used to describe the
/// position in the text with `soft-wrap` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayPoint {
  /// The 0-based soft wrapped row index in the text.
  pub row: usize,
  /// The 0-based row index in local line (include first line).
  ///
  /// This value only valid when return from
  /// [`TextWrapper::offset_to_display_point`], otherwise it will be ignored.
  pub local_row: usize,
  /// The 0-based column byte index in the display line (with soft wrap).
  pub column: usize,
}

impl DisplayPoint {
  pub fn new(row: usize, local_row: usize, column: usize) -> Self {
    Self {
      row,
      local_row,
      column,
    }
  }
}

/// The layout info of a line with soft wrapped lines.
pub(crate) struct LineLayout {
  /// Total bytes length of the logical line.
  len: usize,
  /// Byte offset in the logical line where the visible wrapped slice starts.
  visible_offset: usize,
  /// Total bytes length covered by the visible wrapped slice.
  visible_len: usize,
  /// The soft wrapped lines of this line (Include the first line).
  pub(crate) wrapped_lines: SmallVec<[ShapedLine; 1]>,
  pub(crate) longest_width: Pixels,
  pub(crate) whitespace_indicators: Option<WhitespaceIndicators>,
  /// Whitespace indicators: (line_index, x_position, is_tab)
  pub(crate) whitespace_chars: Vec<(usize, Pixels, bool)>,
}

impl LineLayout {
  pub(crate) fn new() -> Self {
    Self {
      len: 0,
      visible_offset: 0,
      visible_len: 0,
      longest_width: px(0.),
      wrapped_lines: SmallVec::new(),
      whitespace_chars: Vec::new(),
      whitespace_indicators: None,
    }
  }

  #[allow(dead_code)]
  pub(crate) fn lines(mut self, wrapped_lines: SmallVec<[ShapedLine; 1]>) -> Self {
    self.set_wrapped_lines(wrapped_lines);
    self
  }

  pub(crate) fn visible_slice(
    mut self, line_len: usize, visible_offset: usize, wrapped_lines: SmallVec<[ShapedLine; 1]>,
  ) -> Self {
    self.set_visible_slice(line_len, visible_offset, wrapped_lines);
    self
  }

  #[cfg_attr(not(test), allow(dead_code))]
  pub(crate) fn set_wrapped_lines(&mut self, wrapped_lines: SmallVec<[ShapedLine; 1]>) {
    let len = wrapped_lines.iter().map(|l| l.len).sum();
    self.set_visible_slice(len, 0, wrapped_lines);
  }

  pub(crate) fn set_visible_slice(
    &mut self, line_len: usize, visible_offset: usize, wrapped_lines: SmallVec<[ShapedLine; 1]>,
  ) {
    self.len = line_len;
    self.visible_offset = visible_offset;
    self.visible_len = wrapped_lines.iter().map(|l| l.len).sum();
    let width = wrapped_lines
      .iter()
      .map(|l| l.width)
      .max()
      .unwrap_or_default();
    self.longest_width = width;
    self.wrapped_lines = wrapped_lines;
  }

  pub(crate) fn with_whitespaces(mut self, indicators: Option<WhitespaceIndicators>) -> Self {
    self.whitespace_indicators = indicators;
    let Some(indicators) = self.whitespace_indicators.as_ref() else {
      return self;
    };

    let space_indicator_offset = indicators.space.width.half();

    for (line_index, wrapped_line) in self.wrapped_lines.iter().enumerate() {
      for (relative_offset, c) in wrapped_line.text.char_indices() {
        if matches!(c, ' ' | '\t') {
          let is_tab = c == '\t';
          let start_x = wrapped_line.x_for_index(relative_offset);
          let end_x = wrapped_line.x_for_index(relative_offset + c.len_utf8());
          // Center the indicator in the actual character's space
          let x_position = if c == ' ' {
            (start_x + end_x).half() - space_indicator_offset
          } else {
            start_x
          };

          self.whitespace_chars.push((line_index, x_position, is_tab));
        }
      }
    }
    self
  }

  #[inline]
  #[cfg_attr(not(test), allow(dead_code))]
  pub(super) fn len(&self) -> usize {
    self.len
  }

  pub(super) fn first_visible_position(&self, last_layout: &LastLayout) -> Point<Pixels> {
    self
      .position_for_index(self.visible_offset, last_layout)
      .unwrap_or_default()
  }

  pub(super) fn last_visible_position(&self, last_layout: &LastLayout) -> Point<Pixels> {
    self
      .position_for_index(self.visible_offset + self.visible_len, last_layout)
      .unwrap_or_else(|| self.first_visible_position(last_layout))
  }

  /// Get the position (x, y) for the given index in this line layout.
  ///
  /// - The `offset` is a local byte index in this line layout.
  /// - The return value is relative to the top-left corner of this line layout,
  ///   start from (0, 0)
  pub(crate) fn position_for_index(
    &self, offset: usize, last_layout: &LastLayout,
  ) -> Option<Point<Pixels>> {
    if offset < self.visible_offset {
      return None;
    }

    let offset = offset - self.visible_offset;
    let mut acc_len = 0;
    let mut offset_y = px(0.);

    let x_offset = last_layout.alignment_offset(self.longest_width);

    for (i, line) in self.wrapped_lines.iter().enumerate() {
      let is_last = i + 1 == self.wrapped_lines.len();
      let line_len = if is_last { line.len + 1 } else { line.len };

      let range = acc_len..(acc_len + line_len);
      if range.contains(&offset) {
        let x = line.x_for_index(offset.saturating_sub(acc_len)) + x_offset;
        return Some(point(x, offset_y));
      }
      acc_len += line_len;
      offset_y += last_layout.line_height;
    }

    None
  }

  /// Get the index for the given position (x, y) in this line layout.
  ///
  /// The `pos` is relative to the top-left corner of this line layout, start
  /// from (0, 0) The return value is a local byte index in this line layout,
  /// start from 0.
  pub(super) fn closest_index_for_position(
    &self, pos: Point<Pixels>, last_layout: &LastLayout,
  ) -> Option<usize> {
    let mut offset = 0;
    let mut line_top = px(0.);
    let x_offset = last_layout.alignment_offset(self.longest_width);
    for (i, line) in self.wrapped_lines.iter().enumerate() {
      let is_last = i + 1 == self.wrapped_lines.len();
      let line_bottom = line_top + last_layout.line_height;
      if pos.y >= line_top && pos.y < line_bottom {
        let mut ix = line.closest_index_for_x(pos.x - x_offset);
        if !is_last && ix == line.text.len() {
          // For soft wrap line, we can't put the cursor at the end of the line.
          let c_len = line.text.chars().last().map(|c| c.len_utf8()).unwrap_or(0);
          ix = ix.saturating_sub(c_len);
        }
        return Some(self.visible_offset + offset + ix);
      }

      offset += line.text.len();
      line_top = line_bottom;
    }

    None
  }

  pub(super) fn index_for_position(
    &self, pos: Point<Pixels>, last_layout: &LastLayout,
  ) -> Option<usize> {
    let mut offset = 0;
    let mut line_top = px(0.);
    let x_offset = last_layout.alignment_offset(self.longest_width);
    for line in self.wrapped_lines.iter() {
      let line_bottom = line_top + last_layout.line_height;
      if pos.y >= line_top && pos.y < line_bottom {
        let ix = line.index_for_x(pos.x - x_offset)?;
        return Some(self.visible_offset + offset + ix);
      }

      offset += line.text.len();
      line_top = line_bottom;
    }

    None
  }

  pub(super) fn size(&self, line_height: Pixels) -> Size<Pixels> {
    size(self.longest_width, self.wrapped_lines.len() * line_height)
  }

  pub(super) fn paint(
    &self, pos: Point<Pixels>, line_height: Pixels, _text_align: TextAlign,
    _align_width: Option<Pixels>, window: &mut Window, cx: &mut App,
  ) {
    for (ix, line) in self.wrapped_lines.iter().enumerate() {
      _ = line.paint(
        pos + point(px(0.), ix * line_height),
        line_height,
        window,
        cx,
      );
    }

    // Paint whitespace indicators
    if let Some(indicators) = self.whitespace_indicators.as_ref() {
      for (line_index, x_position, is_tab) in &self.whitespace_chars {
        let invisible = if *is_tab {
          indicators.tab.clone()
        } else {
          indicators.space.clone()
        };

        let origin = point(
          pos.x + *x_position,
          pos.y + *line_index as f32 * line_height,
        );

        _ = invisible.paint(origin, line_height, window, cx);
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use gpui::{FontFeatures, FontStyle, FontWeight, px};

  use super::*;

  #[test]
  fn test_update() {
    let font = gpui::Font {
      family: "Arial".into(),
      weight: FontWeight::default(),
      style: FontStyle::Normal,
      features: FontFeatures::default(),
      fallbacks: None,
    };

    let mut wrapper = TextWrapper::new(font, px(14.), None);
    let mut text =
      Rope::from("Hello, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。");

    fn fake_wrap_line(_line: &str, _wrap_width: Pixels) -> Vec<Range<usize>> {
      vec![]
    }

    #[track_caller]
    fn assert_wrapper_lines(text: &Rope, wrapper: &TextWrapper, expected_lines: &[&[&str]]) {
      let mut actual_lines = vec![];
      let mut offset = 0;
      for line in wrapper.lines.iter() {
        actual_lines.push(
          line
            .wrapped_lines
            .iter()
            .map(|range| text.slice(offset + range.start..offset + range.end))
            .collect::<Vec<_>>(),
        );
        // +1 \n
        offset += line.len() + 1;
      }
      assert_eq!(actual_lines, expected_lines);
    }

    wrapper._update(&text, &(0..text.len()), &text, &mut fake_wrap_line);
    assert_eq!(wrapper.lines.len(), 4);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["Hello, 世界!\r"],
        &["This is second line."],
        &["This is third line."],
        &["这里是第 4 行。"],
      ],
    );

    // Add a new text to end
    let range = text.len()..text.len();
    let new_text = "New text";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "Hello, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。New text"
    );
    assert_eq!(wrapper.lines.len(), 4);
    assert_eq!(wrapper.lines.len(), 4);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["Hello, 世界!\r"],
        &["This is second line."],
        &["This is third line."],
        &["这里是第 4 行。New text"],
      ],
    );

    // Replace first line `Hello` to `AAA`
    let range = 0..5;
    let new_text = "AAA";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "AAA, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。New text"
    );
    assert_eq!(wrapper.lines.len(), 4);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["AAA, 世界!\r"],
        &["This is second line."],
        &["This is third line."],
        &["这里是第 4 行。New text"],
      ],
    );

    // Remove the second line
    let start_offset = text.line_start_offset(1);
    let end_offset = text.line_end_offset(1);
    let range = start_offset..end_offset + 1;
    text.replace(range.clone(), "");
    wrapper._update(&text, &range, &Rope::from(""), &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "AAA, 世界!\r\nThis is third line.\n这里是第 4 行。New text"
    );
    assert_eq!(wrapper.lines.len(), 3);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["AAA, 世界!\r"],
        &["This is third line."],
        &["这里是第 4 行。New text"],
      ],
    );

    // Replace the first 2 lines to "This is a new line."
    let range = text.line_start_offset(0)..text.line_end_offset(1) + 1;
    let new_text = "This is a new line.\nThis is new line 2.\n";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "This is a new line.\nThis is new line 2.\n这里是第 4 行。New text"
    );
    assert_eq!(wrapper.lines.len(), 3);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["This is a new line."],
        &["This is new line 2."],
        &["这里是第 4 行。New text"],
      ],
    );

    // Add a new line at the end
    let range = text.len()..text.len();
    let new_text = "\nThis is a new line at the end.";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "This is a new line.\nThis is new line 2.\n这里是第 4 行。New text\nThis is a new line at the end."
    );
    assert_eq!(wrapper.lines.len(), 4);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["This is a new line."],
        &["This is new line 2."],
        &["这里是第 4 行。New text"],
        &["This is a new line at the end."],
      ],
    );

    // Add a new line at the beginning
    let range = 0..0;
    let new_text = "This is a new line at the beginning.\n";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "This is a new line at the beginning.\nThis is a new line.\nThis is new line 2.\n这里是第 4 行。New text\nThis is a new line at the end."
    );
    assert_eq!(wrapper.lines.len(), 5);
    assert_wrapper_lines(
      &text,
      &wrapper,
      &[
        &["This is a new line at the beginning."],
        &["This is a new line."],
        &["This is new line 2."],
        &["这里是第 4 行。New text"],
        &["This is a new line at the end."],
      ],
    );

    // Remove all to at least one line in `lines`.
    let range = 0..text.len();
    let new_text = "";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &Rope::from(new_text), &mut fake_wrap_line);
    assert_eq!(text.to_string(), "");
    assert_eq!(wrapper.lines.len(), 1);
    assert_eq!(wrapper.lines[0].wrapped_lines, vec![0..0]);

    // Test update_all
    let range = 0..text.len();
    let new_text = "This is a full text.\nThis is a second line.";
    text.replace(range.clone(), new_text);
    wrapper._update(&text, &range, &text, &mut fake_wrap_line);
    assert_eq!(
      text.to_string(),
      "This is a full text.\nThis is a second line."
    );
    assert_eq!(wrapper.lines.len(), 2);
  }

  #[test]
  fn test_line_layout() {
    let mut line_layout = LineLayout::new();

    let line1 = ShapedLine::default().with_len(100);
    let line2 = ShapedLine::default().with_len(50);
    let wrapped_lines = smallvec::smallvec![line1, line2];
    line_layout.set_wrapped_lines(wrapped_lines);
    assert_eq!(line_layout.len(), 150);
    assert_eq!(line_layout.wrapped_lines.len(), 2);
  }

  #[test]
  fn test_offset_to_display_point() {
    let font = gpui::Font {
      family: "Arial".into(),
      weight: FontWeight::default(),
      style: FontStyle::Normal,
      features: FontFeatures::default(),
      fallbacks: None,
    };

    let mut wrapper = TextWrapper::new(font, px(14.), None);
    wrapper.text =
      Rope::from("Hello, 世界!\r\nThis is second line.\nThis is third line.\n这里是第 4 行。");
    wrapper.lines = vec![
      // range: 0..15
      LineItem {
        line: Rope::from("Hello, 世界!\r"),
        #[allow(clippy::single_range_in_vec_init)]
        wrapped_lines: vec![0..15],
      },
      // range: 16..36
      LineItem {
        line: Rope::from("This is second line."),
        wrapped_lines: vec![0..10, 10..20],
      },
      // range: 37..56
      LineItem {
        line: Rope::from("This is third line."),
        wrapped_lines: vec![0..9, 9..15, 15..20],
      },
      // range: 57..79
      LineItem {
        line: Rope::from("这里是第 4 行。"),
        #[allow(clippy::single_range_in_vec_init)]
        wrapped_lines: vec![0..22],
      },
    ];

    assert_eq!(
      wrapper.offset_to_display_point(12),
      DisplayPoint::new(0, 0, 12)
    );
    assert_eq!(
      wrapper.offset_to_display_point(15),
      DisplayPoint::new(0, 0, 15)
    );

    assert_eq!(
      wrapper.offset_to_display_point(16),
      DisplayPoint::new(1, 0, 0)
    );
    assert_eq!(
      wrapper.offset_to_display_point(21),
      DisplayPoint::new(1, 0, 5)
    );
    assert_eq!(
      wrapper.offset_to_display_point(27),
      DisplayPoint::new(2, 1, 1)
    );
    assert_eq!(
      wrapper.offset_to_display_point(37),
      DisplayPoint::new(3, 0, 0)
    );
    assert_eq!(
      wrapper.offset_to_display_point(54),
      DisplayPoint::new(5, 2, 2)
    );
    assert_eq!(
      wrapper.offset_to_display_point(59),
      DisplayPoint::new(6, 0, 2)
    );

    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(6, 0, 2)),
      59
    );
    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(5, 2, 2)),
      54
    );
    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(3, 0, 0)),
      37
    );
    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(2, 1, 1)),
      27
    );
    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(1, 0, 5)),
      21
    );
    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(1, 0, 0)),
      16
    );
    assert_eq!(
      wrapper.display_point_to_offset(DisplayPoint::new(0, 0, 15)),
      15
    );
  }

  #[test]
  fn test_display_row_to_line_row() {
    let font = gpui::Font {
      family: "Arial".into(),
      weight: FontWeight::default(),
      style: FontStyle::Normal,
      features: FontFeatures::default(),
      fallbacks: None,
    };

    let mut wrapper = TextWrapper::new(font, px(14.), None);
    wrapper.lines = vec![
      LineItem {
        line: Rope::from("alpha"),
        wrapped_lines: vec![0..2, 2..5],
      },
      LineItem {
        line: Rope::from("beta"),
        wrapped_lines: std::iter::once(0..4).collect(),
      },
      LineItem {
        line: Rope::from("gamma"),
        wrapped_lines: vec![0..1, 1..3, 3..5],
      },
    ];
    wrapper.soft_lines = wrapper.lines.iter().map(LineItem::lines_len).sum();

    assert_eq!(wrapper.display_row_to_line_row(0), Some((0, 0)));
    assert_eq!(wrapper.display_row_to_line_row(1), Some((0, 1)));
    assert_eq!(wrapper.display_row_to_line_row(2), Some((1, 0)));
    assert_eq!(wrapper.display_row_to_line_row(3), Some((2, 0)));
    assert_eq!(wrapper.display_row_to_line_row(5), Some((2, 2)));
  }

  #[test]
  fn test_empty_line_only_uses_one_wrapped_row() {
    let font = gpui::Font {
      family: "Arial".into(),
      weight: FontWeight::default(),
      style: FontStyle::Normal,
      features: FontFeatures::default(),
      fallbacks: None,
    };

    let mut wrapper = TextWrapper::new(font, px(14.), Some(px(80.)));
    let text = Rope::from("alpha\n\nbeta");

    fn fake_wrap_line(_line: &str, _wrap_width: Pixels) -> Vec<Range<usize>> {
      vec![]
    }

    wrapper._update(&text, &(0..text.len()), &text, &mut fake_wrap_line);

    assert_eq!(wrapper.lines.len(), 3);
    assert_eq!(wrapper.lines[1].wrapped_lines, vec![0..0]);
    assert_eq!(wrapper.lines[1].lines_len(), 1);
    assert_eq!(wrapper.soft_lines, 3);
  }
}
