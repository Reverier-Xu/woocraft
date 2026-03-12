use std::{char, ops::Range};

use gpui::{Context, Window};
use ropey::Rope;
use sum_tree::Bias;

use super::{rope_ext::RopeExt as _, state::InputState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharType {
  Word,
  Whitespace,
  Newline,
  Other,
}

fn is_word_char(c: char) -> bool {
  matches!(c, '_')
    || c.is_ascii_alphanumeric()
    || matches!(c, '\u{00C0}'..='\u{00FF}')
    || matches!(c, '\u{0100}'..='\u{017F}')
    || matches!(c, '\u{0180}'..='\u{024F}')
    || matches!(c, '\u{0400}'..='\u{04FF}')
    || matches!(c, '\u{1E00}'..='\u{1EFF}')
    || matches!(c, '\u{0300}'..='\u{036F}')
}

impl From<char> for CharType {
  fn from(c: char) -> Self {
    match c {
      c if is_word_char(c) => CharType::Word,
      c if c == '\n' || c == '\r' => CharType::Newline,
      c if c.is_whitespace() => CharType::Whitespace,
      _ => CharType::Other,
    }
  }
}

impl CharType {
  fn is_connectable(self, c: char) -> bool {
    let other = CharType::from(c);
    matches!(
      (self, other),
      (CharType::Word, CharType::Word) | (CharType::Whitespace, CharType::Whitespace)
    )
  }
}

impl InputState {
  pub(super) fn select_word(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
    let Some(range) = TextSelector::word_range(&self.text, offset) else {
      return;
    };

    self.selected_range = (range.start..range.end).into();
    self.selected_word_range = Some(self.selected_range);
    cx.notify()
  }

  pub(super) fn select_line(&mut self, offset: usize, _: &mut Window, cx: &mut Context<Self>) {
    let range = TextSelector::line_range(&self.text, offset);
    self.selected_range = (range.start..range.end).into();
    self.selected_word_range = None;
    cx.notify()
  }
}

struct TextSelector;
impl TextSelector {
  pub fn line_range(text: &Rope, offset: usize) -> Range<usize> {
    let offset = text.clip_offset(offset, Bias::Left);
    let row = text.offset_to_point(offset).row;
    let start = text.line_start_offset(row);
    let end = text.line_end_offset(row);

    start..end
  }

  pub fn word_range(text: &Rope, offset: usize) -> Option<Range<usize>> {
    let offset = text.clip_offset(offset, Bias::Left);
    let char = text.char_at(offset)?;

    let char_type = CharType::from(char);
    let mut start = offset;
    let mut end = offset + char.len_utf8();
    let prev_chars = text.chars_at(start).reversed().take(128);
    let next_chars = text.chars_at(end).take(128);

    for ch in prev_chars {
      if char_type.is_connectable(ch) {
        start -= ch.len_utf8();
      } else {
        break;
      }
    }

    for ch in next_chars {
      if char_type.is_connectable(ch) {
        end += ch.len_utf8();
      } else {
        break;
      }
    }

    Some(start..end)
  }
}

#[cfg(test)]
mod tests {
  use ropey::Rope;

  use super::TextSelector;

  #[test]
  fn word_range_groups_unicode_words_and_combining_marks() {
    let text = "alpha na\u{0308}ive пример";
    let rope = Rope::from(text);

    let naive_range = TextSelector::word_range(&rope, text.find("\u{0308}").unwrap()).unwrap();
    assert_eq!(&text[naive_range], "na\u{0308}ive");

    let cyrillic_offset = text.find("мер").unwrap();
    let cyrillic_range = TextSelector::word_range(&rope, cyrillic_offset).unwrap();
    assert_eq!(&text[cyrillic_range], "пример");
  }

  #[test]
  fn word_range_groups_whitespace_runs() {
    let text = "foo   bar";
    let rope = Rope::from(text);
    let range = TextSelector::word_range(&rope, text.find("   ").unwrap() + 1).unwrap();

    assert_eq!(&text[range], "   ");
  }

  #[test]
  fn word_range_keeps_punctuation_and_newlines_isolated() {
    let text = "foo,\nbar";
    let rope = Rope::from(text);

    let comma = text.find(',').unwrap();
    let comma_range = TextSelector::word_range(&rope, comma).unwrap();
    assert_eq!(&text[comma_range], ",");

    let newline = text.find('\n').unwrap();
    let newline_range = TextSelector::word_range(&rope, newline).unwrap();
    assert_eq!(&text[newline_range], "\n");
  }

  #[test]
  fn word_range_returns_none_past_the_end() {
    let rope = Rope::from("foo");

    assert_eq!(TextSelector::word_range(&rope, rope.len()), None);
  }

  #[test]
  fn line_range_clips_offsets_to_the_current_line() {
    let text = "one\n中文\nthree";
    let rope = Rope::from(text);

    let middle_of_zhong = text.find("中文").unwrap() + 1;
    let middle_range = TextSelector::line_range(&rope, middle_of_zhong);
    assert_eq!(&text[middle_range], "中文");

    let tail_range = TextSelector::line_range(&rope, usize::MAX);
    assert_eq!(&text[tail_range], "three");
  }
}
