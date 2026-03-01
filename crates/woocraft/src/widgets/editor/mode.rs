use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{App, SharedString};
use ropey::Rope;
use tree_sitter::InputEdit;

use super::{
  highlighter::{DiagnosticSet, SyntaxHighlighter},
  indent::TabSize,
  rope_ext::RopeExt as _,
  text_wrapper::TextWrapper,
};

#[derive(Clone)]
pub(crate) enum InputMode {
  /// A plain text input mode.
  PlainText { tab: TabSize, rows: usize },
  /// An auto grow input mode.
  AutoGrow {
    rows: usize,
    min_rows: usize,
    max_rows: usize,
  },
  /// A code editor input mode.
  CodeEditor {
    tab: TabSize,
    rows: usize,
    /// Show line number
    line_number: bool,
    language: SharedString,
    indent_guides: bool,
    highlighter: Rc<RefCell<Option<SyntaxHighlighter>>>,
    diagnostics: DiagnosticSet,
  },
}

impl Default for InputMode {
  fn default() -> Self {
    InputMode::plain_text()
  }
}

#[allow(unused)]
impl InputMode {
  /// Create a plain input mode with default settings.
  pub(super) fn plain_text() -> Self {
    InputMode::PlainText {
      tab: TabSize::default(),
      rows: 1,
    }
  }

  /// Create a code editor input mode with default settings.
  pub(super) fn code_editor(language: impl Into<SharedString>) -> Self {
    InputMode::CodeEditor {
      rows: 2,
      tab: TabSize::default(),
      language: language.into(),
      highlighter: Rc::new(RefCell::new(None)),
      line_number: true,
      indent_guides: true,
      diagnostics: DiagnosticSet::new(&Rope::new()),
    }
  }

  /// Create an auto grow input mode with given min and max rows.
  pub(super) fn auto_grow(min_rows: usize, max_rows: usize) -> Self {
    InputMode::AutoGrow {
      rows: min_rows,
      min_rows,
      max_rows,
    }
  }

  #[inline]
  pub(super) fn is_code_editor(&self) -> bool {
    matches!(self, InputMode::CodeEditor { .. })
  }

  #[inline]
  pub(super) fn is_auto_grow(&self) -> bool {
    matches!(self, InputMode::AutoGrow { .. })
  }

  #[inline]
  pub(super) fn is_multi_line(&self) -> bool {
    true
  }

  pub(super) fn set_rows(&mut self, new_rows: usize) {
    match self {
      InputMode::PlainText { rows, .. } => {
        *rows = new_rows;
      }
      InputMode::CodeEditor { rows, .. } => {
        *rows = new_rows;
      }
      InputMode::AutoGrow {
        rows,
        min_rows,
        max_rows,
      } => {
        *rows = new_rows.clamp(*min_rows, *max_rows);
      }
    }
  }

  pub(super) fn update_auto_grow(&mut self, text_wrapper: &TextWrapper) {
    let wrapped_lines = text_wrapper.len();
    self.set_rows(wrapped_lines);
  }

  /// At least 1 row be return.
  pub(super) fn rows(&self) -> usize {
    match self {
      InputMode::PlainText { rows, .. } => *rows,
      InputMode::CodeEditor { rows, .. } => *rows,
      InputMode::AutoGrow { rows, .. } => *rows,
    }
    .max(1)
  }

  /// At least 1 row be return.
  #[allow(unused)]
  pub(super) fn min_rows(&self) -> usize {
    match self {
      InputMode::AutoGrow { min_rows, .. } => *min_rows,
      _ => 1,
    }
    .max(1)
  }

  #[allow(unused)]
  pub(super) fn max_rows(&self) -> usize {
    match self {
      InputMode::AutoGrow { max_rows, .. } => *max_rows,
      _ => usize::MAX,
    }
  }

  /// Return false if the mode is not [`InputMode::CodeEditor`].
  #[allow(unused)]
  #[inline]
  pub(super) fn line_number(&self) -> bool {
    match self {
      InputMode::CodeEditor { line_number, .. } => *line_number,
      _ => false,
    }
  }

  pub(super) fn update_highlighter(
    &mut self, selected_range: &Range<usize>, text: &Rope, new_text: &str, force: bool,
    cx: &mut App,
  ) {
    if let InputMode::CodeEditor {
      language,
      highlighter,
      ..
    } = &self
    {
      if !force && highlighter.borrow().is_some() {
        return;
      }

      let mut highlighter = highlighter.borrow_mut();
      if highlighter.is_none() {
        let new_highlighter = SyntaxHighlighter::new(language);
        highlighter.replace(new_highlighter);
      }

      let Some(highlighter) = highlighter.as_mut() else {
        return;
      };

      // When full text changed, the selected_range may be out of bound (The before
      // version).
      let mut selected_range = selected_range.clone();
      selected_range.end = selected_range.end.min(text.len());

      // If insert a chart, this is 1.
      // If backspace or delete, this is -1.
      // If selected to delete, this is the length of the selected text.
      // let changed_len = new_text.len() as isize - selected_range.len() as isize;
      let changed_len = new_text.len() as isize - selected_range.len() as isize;
      let new_end = (selected_range.end as isize + changed_len) as usize;

      let start_pos = text.offset_to_point(selected_range.start);
      let old_end_pos = text.offset_to_point(selected_range.end);
      let new_end_pos = text.offset_to_point(new_end);

      let edit = InputEdit {
        start_byte: selected_range.start,
        old_end_byte: selected_range.end,
        new_end_byte: new_end,
        start_position: start_pos,
        old_end_position: old_end_pos,
        new_end_position: new_end_pos,
      };

      highlighter.update(Some(edit), text);
    }
  }

  #[allow(unused)]
  pub(super) fn diagnostics(&self) -> Option<&DiagnosticSet> {
    match self {
      InputMode::CodeEditor { diagnostics, .. } => Some(diagnostics),
      _ => None,
    }
  }

  pub(super) fn diagnostics_mut(&mut self) -> Option<&mut DiagnosticSet> {
    match self {
      InputMode::CodeEditor { diagnostics, .. } => Some(diagnostics),
      _ => None,
    }
  }
}

#[cfg(test)]
mod tests {
  use crate::widgets::editor::{TabSize, mode::InputMode};

  #[test]
  fn test_code_editor() {
    let mode = InputMode::code_editor("rust");
    assert!(mode.is_code_editor());
    assert!(mode.is_multi_line());
    assert!(mode.line_number());
    assert!(mode.has_indent_guides());
    assert_eq!(mode.max_rows(), usize::MAX);
    assert_eq!(mode.min_rows(), 1);
  }

  #[test]
  fn test_plain() {
    let mode = InputMode::PlainText {
      tab: TabSize::default(),
      rows: 5,
    };
    assert!(!mode.is_code_editor());
    assert!(mode.is_multi_line());
    assert!(!mode.line_number());
    assert_eq!(mode.rows(), 5);
    assert_eq!(mode.max_rows(), usize::MAX);
    assert_eq!(mode.min_rows(), 1);

    let mode = InputMode::plain_text();
    assert!(!mode.is_code_editor());
    assert!(mode.is_multi_line());
    assert!(!mode.line_number());
    assert_eq!(mode.max_rows(), usize::MAX);
    assert_eq!(mode.min_rows(), 1);
  }

  #[test]
  fn test_auto_grow() {
    let mut mode = InputMode::auto_grow(2, 5);
    assert!(!mode.is_code_editor());
    assert!(mode.is_multi_line());
    assert!(!mode.line_number());
    assert_eq!(mode.rows(), 2);
    assert_eq!(mode.max_rows(), 5);
    assert_eq!(mode.min_rows(), 2);

    mode.set_rows(4);
    assert_eq!(mode.rows(), 4);

    mode.set_rows(1);
    assert_eq!(mode.rows(), 2);

    mode.set_rows(10);
    assert_eq!(mode.rows(), 5);
  }
}
