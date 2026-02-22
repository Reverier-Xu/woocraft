use std::ops::Range;

use gpui::{Entity, MouseButton, Window};

use super::state::InputState;
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

pub trait EditorDataBackend {
  /// Monotonic revision used by editor for cache synchronization.
  fn revision(&self) -> u64;

  /// Total logical line count.
  fn line_count(&self) -> u64;

  /// Return display line number text for a row.
  ///
  /// The returned value is not required to start from 0/1.
  fn line_number_text(&self, row: u64) -> Option<String> {
    Some((row.saturating_add(1)).to_string())
  }

  /// Return an upper-bound sample text used to measure line-number gutter
  /// width.
  fn max_line_number_text(&self) -> Option<String> {
    let line_count = self.line_count().max(1);
    self.line_number_text(line_count.saturating_sub(1))
  }

  /// Return utf-8 byte range for the target row.
  fn row_range(&self, row: u64) -> Option<Range<u64>>;

  /// Return text for utf-8 byte range.
  fn text_for_range(&self, range: Range<u64>) -> Option<String>;

  /// Return a full snapshot for layout/highlight cache.
  fn snapshot(&self) -> String;

  /// Apply an edit from editor input.
  fn apply_edit(&mut self, request: EditorBackendEditRequest) -> EditorBackendEditResult;

  /// Receive user operations from editor.
  fn on_user_action(&mut self, _action: &EditorUserAction) {}

  /// Allow backend to extend the right-click menu.
  fn extend_context_menu(
    &self, menu: PopupMenu, _state: &Entity<InputState>, _window: &mut Window,
  ) -> PopupMenu {
    menu
  }
}
