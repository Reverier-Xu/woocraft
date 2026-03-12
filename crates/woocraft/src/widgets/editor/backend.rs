use std::{ops::Range, sync::Arc};

use gpui::{Entity, HighlightStyle, MouseButton, SharedString, Window};
use lsp_types::Position;
use ropey::Rope;

use super::{RopeExt as _, highlighter::HighlightTheme, state::InputState};
use crate::PopupMenu;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorPointerButton {
  Left,
  Right,
  Middle,
  Other,
}

impl From<MouseButton> for EditorPointerButton {
  fn from(value: MouseButton) -> Self {
    match value {
      MouseButton::Left => Self::Left,
      MouseButton::Right => Self::Right,
      MouseButton::Middle => Self::Middle,
      _ => Self::Other,
    }
  }
}

#[derive(Clone, Debug)]
pub enum EditorUserAction {
  MoveCursor {
    offset: u64,
  },
  Select {
    range: Range<u64>,
    reversed: bool,
  },
  Replace {
    range: Range<u64>,
    new_text: String,
    marked: bool,
    silent: bool,
  },
  MouseDown {
    offset: u64,
    button: EditorPointerButton,
    click_count: u8,
    shift: bool,
  },
  MouseUp {
    offset: u64,
    button: EditorPointerButton,
  },
  MouseMove {
    offset: u64,
  },
  Scroll {
    delta_x: f32,
    delta_y: f32,
  },
  Copy {
    range: Range<u64>,
  },
  Cut {
    range: Range<u64>,
  },
  Paste {
    range: Range<u64>,
    text: String,
  },
  UndoRequested,
  RedoRequested,
  ContextMenuRequested {
    offset: u64,
  },
}

#[derive(Clone, Debug)]
pub struct EditorBackendEditRequest {
  pub range: Range<u64>,
  pub new_text: String,
  pub marked: bool,
}

#[derive(Clone, Debug)]
pub struct EditorBackendEditResult {
  pub accepted: bool,
  pub selection: Option<Range<u64>>,
  pub cursor: Option<u64>,
}

impl Default for EditorBackendEditResult {
  fn default() -> Self {
    Self {
      accepted: true,
      selection: None,
      cursor: None,
    }
  }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EditorBackendCapabilities {
  pub editable: bool,
  pub custom_line_numbers: bool,
  pub custom_highlighter: bool,
}

impl Default for EditorBackendCapabilities {
  fn default() -> Self {
    Self {
      editable: true,
      custom_line_numbers: false,
      custom_highlighter: false,
    }
  }
}

impl EditorBackendCapabilities {
  pub const fn read_only() -> Self {
    Self {
      editable: false,
      custom_line_numbers: false,
      custom_highlighter: false,
    }
  }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorLine {
  pub row: u64,
  pub byte_range: Range<u64>,
  pub text: SharedString,
  pub line_number: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditorTextChange {
  pub range: Range<u64>,
  pub new_text: SharedString,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditorEditError {
  Unsupported,
  Rejected(SharedString),
}

pub trait EditorSnapshot {
  fn revision(&self) -> u64;

  fn byte_len(&self) -> u64;

  fn line_count(&self) -> u64;

  fn line_number_text(&self, row: u64) -> SharedString {
    (row.saturating_add(1)).to_string().into()
  }

  fn max_line_number_text(&self) -> SharedString {
    self.line_number_text(self.line_count().saturating_sub(1))
  }

  fn line(&self, row: u64) -> Option<EditorLine>;

  fn lines(&self, rows: Range<u64>) -> Arc<[EditorLine]> {
    rows
      .filter_map(|row| self.line(row))
      .collect::<Vec<_>>()
      .into()
  }

  fn line_range(&self, row: u64) -> Option<Range<u64>> {
    self.line(row).map(|line| line.byte_range)
  }

  fn text_for_range(&self, range: Range<u64>) -> Option<SharedString>;

  fn rope(&self) -> Rope {
    Rope::from(
      self
        .text_for_range(0..self.byte_len())
        .unwrap_or_default()
        .as_ref(),
    )
  }

  fn position_to_offset(&self, position: &Position) -> u64;

  fn offset_to_position(&self, offset: u64) -> Position;
}

pub trait EditorHighlighter {
  fn sync(&mut self, snapshot: &dyn EditorSnapshot, change: Option<&EditorTextChange>);

  fn highlight_range(
    &self, snapshot: &dyn EditorSnapshot, range: Range<u64>, theme: &HighlightTheme,
  ) -> Vec<(Range<u64>, HighlightStyle)>;
}

pub trait EditorHighlighterProvider {
  fn create_highlighter(&self) -> Option<Box<dyn EditorHighlighter>> {
    None
  }
}

pub trait EditorActionSink {
  fn on_user_action(&mut self, _action: &EditorUserAction) {}
}

pub trait EditorContextMenuProvider {
  fn extend_context_menu(
    &self, menu: PopupMenu, _state: &Entity<InputState>, _window: &mut Window,
  ) -> PopupMenu {
    menu
  }
}

pub trait EditorBackend:
  EditorActionSink + EditorContextMenuProvider + EditorHighlighterProvider {
  fn revision(&self) -> u64;

  fn capabilities(&self) -> EditorBackendCapabilities {
    EditorBackendCapabilities::default()
  }

  fn snapshot(&self) -> Arc<dyn EditorSnapshot>;

  fn apply_edit(
    &mut self, _request: EditorBackendEditRequest,
  ) -> Result<EditorBackendEditResult, EditorEditError> {
    Err(EditorEditError::Unsupported)
  }
}

#[derive(Clone)]
pub struct RopeEditorSnapshot {
  revision: u64,
  text: Rope,
}

impl RopeEditorSnapshot {
  pub fn new(revision: u64, text: Rope) -> Self {
    Self { revision, text }
  }

  pub fn text(&self) -> &Rope {
    &self.text
  }
}

impl EditorSnapshot for RopeEditorSnapshot {
  fn revision(&self) -> u64 {
    self.revision
  }

  fn byte_len(&self) -> u64 {
    self.text.len() as u64
  }

  fn line_count(&self) -> u64 {
    self.text.lines_len() as u64
  }

  fn line(&self, row: u64) -> Option<EditorLine> {
    let row = row as usize;
    if row >= self.text.lines_len() {
      return None;
    }

    let start = self.text.line_start_offset(row) as u64;
    let end = self.text.line_end_offset(row) as u64;
    Some(EditorLine {
      row: row as u64,
      byte_range: start..end,
      text: self
        .text
        .slice(start as usize..end as usize)
        .to_string()
        .into(),
      line_number: self.line_number_text(row as u64),
    })
  }

  fn text_for_range(&self, range: Range<u64>) -> Option<SharedString> {
    let start = (range.start as usize).min(self.text.len());
    let end = (range.end as usize).min(self.text.len());
    Some(self.text.slice(start..end).to_string().into())
  }

  fn rope(&self) -> Rope {
    self.text.clone()
  }

  fn position_to_offset(&self, position: &Position) -> u64 {
    self.text.position_to_offset(position) as u64
  }

  fn offset_to_position(&self, offset: u64) -> Position {
    self
      .text
      .offset_to_position((offset as usize).min(self.text.len()))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn rope_snapshot_exposes_lines_and_offsets() {
    let snapshot = RopeEditorSnapshot::new(7, Rope::from("alpha\nbeta\n"));

    assert_eq!(snapshot.revision(), 7);
    assert_eq!(snapshot.line_count(), 3);
    assert_eq!(snapshot.byte_len(), "alpha\nbeta\n".len() as u64);
    assert_eq!(snapshot.line(0).unwrap().text, SharedString::from("alpha"));
    assert_eq!(snapshot.line(1).unwrap().text, SharedString::from("beta"));
    assert_eq!(snapshot.position_to_offset(&Position::new(1, 2)), 8);
    assert_eq!(snapshot.offset_to_position(8), Position::new(1, 2));
  }
}
