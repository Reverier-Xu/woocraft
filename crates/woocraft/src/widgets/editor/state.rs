//! A text input field that allows the user to enter text.
//!
//! Based on the `Input` example from the `gpui` crate.
//! https://github.com/zed-industries/zed/blob/main/crates/gpui/examples/input.rs
use std::{ops::Range, rc::Rc, sync::Arc};

use anyhow::Result;
use gpui::{
  Action, App, AppContext, Bounds, ClipboardItem, Context, Entity, EntityInputHandler,
  EventEmitter, FocusHandle, Focusable, Half, InteractiveElement as _, IntoElement, KeyBinding,
  KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement as _,
  Pixels, Point, Render, ScrollWheelEvent, ShapedLine, SharedString, Styled as _, Subscription,
  Task, TextAlign, UTF16Selection, Window, actions, div, point, prelude::FluentBuilder as _, px,
};
use ropey::{Rope, RopeSlice};
use serde::Deserialize;
use sum_tree::Bias;
use unicode_segmentation::*;

use super::{
  EditorBackend, EditorBackendEditRequest, EditorBackendEditResult, EditorEditError,
  EditorHighlighter, EditorPointerButton, EditorSnapshot, EditorUserAction, Position,
  RopeBufferBackend,
  blink_cursor::BlinkCursor,
  change::Change,
  highlighter::DiagnosticSet,
  lsp::{HoverDefinition, InlineCompletion, Lsp},
  mask_pattern::MaskPattern,
  mode::InputMode,
  movement::MoveDirection,
  popovers::{ContextMenu, DiagnosticPopover, HoverPopover},
  search::{self, SearchPanel},
  text_wrapper::{LineLayout, TextWrapper},
  viewport,
  viewport_element::ViewportElement,
};
use crate::{
  ActiveTheme as _, PixelsExt, Selection, Size,
  actions::{SelectDown, SelectLeft, SelectRight, SelectUp},
  widgets::{editor::RopeExt as _, history::History},
};

type ValidateFn<T> = dyn Fn(&str, &mut Context<T>) -> bool + 'static;

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = editor, no_json)]
pub struct Enter {
  /// Is confirm with secondary.
  pub secondary: bool,
}

actions!(
  editor,
  [
    Backspace,
    Delete,
    DeleteToBeginningOfLine,
    DeleteToEndOfLine,
    DeleteToPreviousWordStart,
    DeleteToNextWordEnd,
    Indent,
    Outdent,
    IndentInline,
    OutdentInline,
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    MovePageUp,
    MovePageDown,
    SelectAll,
    SelectToStartOfLine,
    SelectToEndOfLine,
    SelectToStart,
    SelectToEnd,
    SelectToPreviousWordStart,
    SelectToNextWordEnd,
    ShowCharacterPalette,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    MoveToStartOfLine,
    MoveToEndOfLine,
    MoveToStart,
    MoveToEnd,
    MoveToPreviousWord,
    MoveToNextWord,
    Escape,
    ToggleCodeActions,
    Search,
    GoToDefinition,
  ]
);

#[derive(Clone)]
pub enum InputEvent {
  Change,
  PressEnter { secondary: bool },
  Focus,
  Blur,
}

pub(super) const CONTEXT: &str = "CodeEditor";

pub(crate) fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
    KeyBinding::new("shift-backspace", Backspace, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("ctrl-backspace", Backspace, Some(CONTEXT)),
    KeyBinding::new("delete", Delete, Some(CONTEXT)),
    KeyBinding::new("shift-delete", Delete, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-backspace", DeleteToBeginningOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-delete", DeleteToEndOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-delete", DeleteToNextWordEnd, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-delete", DeleteToNextWordEnd, Some(CONTEXT)),
    KeyBinding::new("enter", Enter { secondary: false }, Some(CONTEXT)),
    KeyBinding::new("shift-enter", Enter { secondary: false }, Some(CONTEXT)),
    KeyBinding::new("secondary-enter", Enter { secondary: true }, Some(CONTEXT)),
    KeyBinding::new("escape", Escape, Some(CONTEXT)),
    KeyBinding::new("up", MoveUp, Some(CONTEXT)),
    KeyBinding::new("down", MoveDown, Some(CONTEXT)),
    KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
    KeyBinding::new("right", MoveRight, Some(CONTEXT)),
    KeyBinding::new("pageup", MovePageUp, Some(CONTEXT)),
    KeyBinding::new("pagedown", MovePageDown, Some(CONTEXT)),
    KeyBinding::new("tab", IndentInline, Some(CONTEXT)),
    KeyBinding::new("shift-tab", OutdentInline, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-]", Indent, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-]", Indent, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-[", Outdent, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-[", Outdent, Some(CONTEXT)),
    KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
    KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
    KeyBinding::new("shift-up", SelectUp, Some(CONTEXT)),
    KeyBinding::new("shift-down", SelectDown, Some(CONTEXT)),
    KeyBinding::new("home", MoveHome, Some(CONTEXT)),
    KeyBinding::new("end", MoveEnd, Some(CONTEXT)),
    KeyBinding::new("shift-home", SelectToStartOfLine, Some(CONTEXT)),
    KeyBinding::new("shift-end", SelectToEndOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("ctrl-shift-a", SelectToStartOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("ctrl-shift-e", SelectToEndOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("shift-cmd-left", SelectToStartOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("shift-cmd-right", SelectToEndOfLine, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-shift-left", SelectToPreviousWordStart, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-shift-left", SelectToPreviousWordStart, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-shift-right", SelectToNextWordEnd, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-shift-right", SelectToNextWordEnd, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("ctrl-cmd-space", ShowCharacterPalette, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-a", SelectAll, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-a", SelectAll, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-c", Copy, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-c", Copy, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-x", Cut, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-x", Cut, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-v", Paste, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-v", Paste, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("ctrl-a", MoveHome, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-left", MoveHome, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("ctrl-e", MoveEnd, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-right", MoveEnd, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-up", MoveToStart, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-down", MoveToEnd, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-left", MoveToPreviousWord, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-right", MoveToNextWord, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-left", MoveToPreviousWord, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-right", MoveToNextWord, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-shift-up", SelectToStart, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-shift-down", SelectToEnd, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-y", Redo, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-.", ToggleCodeActions, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-.", ToggleCodeActions, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-f", Search, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-f", Search, Some(CONTEXT)),
  ]);

  search::init(cx);
}

/// Whitespace indicators for rendering spaces and tabs.
#[derive(Clone, Default)]
pub(crate) struct WhitespaceIndicators {
  /// Shaped line for space character indicator (•)
  pub(crate) space: ShapedLine,
  /// Shaped line for tab character indicator (→)
  pub(crate) tab: ShapedLine,
}

#[derive(Clone)]
pub(super) struct LastLayout {
  /// The visible range (no wrap) of lines in the viewport, the value is row
  /// (0-based) index.
  pub(super) visible_range: Range<usize>,
  /// The first visible line top position in scroll viewport.
  pub(super) visible_top: Pixels,
  /// The range of byte offset of the visible lines.
  pub(super) visible_range_offset: Range<usize>,
  /// The last layout lines (Only have visible lines).
  pub(super) lines: Rc<Vec<LineLayout>>,
  /// The line_height of text layout, this will change will InputElement
  /// painted.
  pub(super) line_height: Pixels,
  /// The wrap width of text layout, this will change will InputElement painted.
  pub(super) wrap_width: Option<Pixels>,
  /// The line number area width of text layout, if not line number, this will
  /// be 0px.
  pub(super) line_number_width: Pixels,
  /// The cursor position (top, left) in pixels.
  pub(super) cursor_bounds: Option<Bounds<Pixels>>,
  /// The text align of the text layout.
  pub(super) text_align: TextAlign,
  /// The content width of the text layout.
  pub(super) content_width: Pixels,
}

impl LastLayout {
  /// Get the line layout for the given row (0-based).
  ///
  /// 0 is the viewport first visible line.
  ///
  /// Returns None if the row is out of range.
  #[allow(dead_code)]
  pub(crate) fn line(&self, row: usize) -> Option<&LineLayout> {
    if row < self.visible_range.start || row >= self.visible_range.end {
      return None;
    }

    self.lines.get(row.saturating_sub(self.visible_range.start))
  }

  /// Get the alignment offset for the given line width.
  pub(super) fn alignment_offset(&self, line_width: Pixels) -> Pixels {
    match self.text_align {
      TextAlign::Left => px(0.),
      TextAlign::Center => (self.content_width - line_width).half().max(px(0.)),
      TextAlign::Right => (self.content_width - line_width).max(px(0.)),
    }
  }
}

/// InputState to keep editing state of the [`super::Input`].
pub struct InputState {
  pub(super) focus_handle: FocusHandle,
  pub(super) mode: InputMode,
  pub(super) text: Rope,
  pub(super) backend: Option<Box<dyn EditorBackend>>,
  pub(super) backend_revision: u64,
  pub(super) backend_snapshot: Option<Arc<dyn EditorSnapshot>>,
  pub(super) highlighter: Option<Box<dyn EditorHighlighter>>,
  pub(super) highlighter_revision: u64,
  pub(super) builtin_backend_language: Option<SharedString>,
  pub(super) text_wrapper: TextWrapper,
  pub(super) history: History<Change>,
  pub(super) blink_cursor: Entity<BlinkCursor>,
  pub(super) loading: bool,
  /// Range in UTF-8 length for the selected text.
  ///
  /// - "Hello 世界💝" = 16
  /// - "💝" = 4
  pub(super) selected_range: Selection,
  pub(super) search_panel: Option<Entity<SearchPanel>>,
  pub(super) searchable: bool,
  /// Range for save the selected word, use to keep word range when drag move.
  pub(super) selected_word_range: Option<Selection>,
  pub(super) selection_reversed: bool,
  /// The marked range is the temporary insert text on IME typing.
  pub(super) ime_marked_range: Option<Selection>,
  pub(super) last_layout: Option<LastLayout>,
  pub(super) last_cursor: Option<usize>,
  /// The input container bounds
  pub(super) input_bounds: Bounds<Pixels>,
  /// The text bounds
  pub(super) last_bounds: Option<Bounds<Pixels>>,
  pub(super) last_selected_range: Option<Selection>,
  pub(super) selecting: bool,
  pub(super) scrollbar_dragging: bool,
  pub(super) top_row: usize,
  pub(super) size: Size,
  pub(super) disabled: bool,
  pub(super) read_only: bool,
  pub(super) masked: bool,
  pub(super) clean_on_escape: bool,
  pub(super) show_whitespaces: bool,
  pub(super) pattern: Option<regex::Regex>,
  pub(super) validate: Option<Box<ValidateFn<InputState>>>,
  pub(super) text_align: TextAlign,

  /// The mask pattern for formatting the input text
  pub(crate) mask_pattern: MaskPattern,
  pub(super) placeholder: SharedString,

  /// Popover
  diagnostic_popover: Option<Entity<DiagnosticPopover>>,
  /// Completion/CodeAction context menu
  pub(super) context_menu: Option<ContextMenu>,
  /// Shared popup context menu (from `ContextMenuExt`) open state.
  pub(super) shared_context_menu_open: bool,
  /// A flag to indicate if we are currently inserting a completion item.
  pub(super) completion_inserting: bool,
  pub(super) hover_popover: Option<Entity<HoverPopover>>,
  /// The LSP definitions locations for "Go to Definition" feature.
  pub(super) hover_definition: HoverDefinition,

  pub lsp: Lsp,

  /// A flag to indicate if we have a pending update to the text.
  ///
  /// If true, will call some update (for example LSP, Syntax Highlight) before
  /// render.
  _pending_update: bool,
  /// A flag to indicate if we should ignore the next completion event.
  pub(super) silent_replace_text: bool,

  /// To remember the horizontal column (x-coordinate) of the cursor position
  /// for keep column for move up/down.
  ///
  /// The first element is the x-coordinate (Pixels), preferred to use this.
  /// The second element is the column (usize), fallback to use this.
  pub(super) preferred_column: Option<(Pixels, usize)>,
  _subscriptions: Vec<Subscription>,

  pub(super) _context_menu_task: Task<Result<()>>,
  pub(super) inline_completion: InlineCompletion,
}

impl EventEmitter<InputEvent> for InputState {}

impl InputState {
  /// Create an editor state with default plain-text multi-line mode.
  ///
  /// See also: [`Self::auto_grow`] and [`Self::code_editor`] to set other
  /// modes.
  pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle().tab_stop(true);
    let blink_cursor = cx.new(|_| BlinkCursor::new());
    let history = History::new().group_interval(std::time::Duration::from_secs(1));

    let _subscriptions = vec![
      // Observe the blink cursor to repaint the view when it changes.
      cx.observe(&blink_cursor, |_, _, cx| cx.notify()),
      // Blink the cursor when the window is active, pause when it's not.
      cx.observe_window_activation(window, |input, window, cx| {
        if window.is_window_active() {
          let focus_handle = input.focus_handle.clone();
          if focus_handle.is_focused(window) {
            input.blink_cursor.update(cx, |blink_cursor, cx| {
              blink_cursor.start(cx);
            });
          }
        }
      }),
      cx.on_focus(&focus_handle, window, Self::on_focus),
      cx.on_blur(&focus_handle, window, Self::on_blur),
    ];

    let text_style = window.text_style();

    Self {
      focus_handle: focus_handle.clone(),
      text: "".into(),
      backend: None,
      backend_revision: 0,
      backend_snapshot: None,
      highlighter: None,
      highlighter_revision: 0,
      builtin_backend_language: None,
      text_wrapper: TextWrapper::new(text_style.font(), window.rem_size(), None),
      blink_cursor,
      history,
      selected_range: Selection::default(),
      search_panel: None,
      searchable: false,
      selected_word_range: None,
      selection_reversed: false,
      ime_marked_range: None,
      input_bounds: Bounds::default(),
      selecting: false,
      disabled: false,
      read_only: false,
      masked: false,
      clean_on_escape: false,
      show_whitespaces: false,
      loading: false,
      pattern: None,
      validate: None,
      mode: InputMode::default(),
      last_layout: None,
      last_bounds: None,
      last_selected_range: None,
      last_cursor: None,
      scrollbar_dragging: false,
      top_row: 0,
      preferred_column: None,
      placeholder: SharedString::default(),
      mask_pattern: MaskPattern::default(),
      text_align: TextAlign::Left,
      lsp: Lsp::default(),
      diagnostic_popover: None,
      context_menu: None,
      shared_context_menu_open: false,
      completion_inserting: false,
      hover_popover: None,
      hover_definition: HoverDefinition::default(),
      silent_replace_text: false,
      size: Size::default(),
      _subscriptions,
      _context_menu_task: Task::ready(Ok(())),
      _pending_update: false,
      inline_completion: InlineCompletion::default(),
    }
  }

  /// Set Input to use [`InputMode::AutoGrow`] mode with min, max rows limit.
  pub fn auto_grow(mut self, min_rows: usize, max_rows: usize) -> Self {
    self.mode = InputMode::auto_grow(min_rows, max_rows);
    self
  }

  /// Set Input to use [`InputMode::CodeEditor`] mode.
  ///
  /// Default options:
  ///
  /// - line_number: true
  /// - tab_size: 2
  /// - hard_tabs: false
  /// - height: 100%
  /// - indent_guides: true
  ///
  /// Code Editor aim for help used to simple code editing or display, not a
  /// full-featured code editor.
  ///
  /// ## Features
  ///
  /// - Syntax Highlighting
  /// - Auto Indent
  /// - Line Number
  /// - Large Text support, up to 50K lines.
  pub fn code_editor(mut self, language: impl Into<SharedString>) -> Self {
    let language: SharedString = language.into();
    self.mode = InputMode::code_editor();
    self.builtin_backend_language = Some(language);
    self.searchable = true;
    self
  }

  /// Set an editor backend.
  pub fn backend(mut self, backend: impl EditorBackend + 'static) -> Self {
    self.backend = Some(Box::new(backend));
    self.backend_revision = 0;
    if let Some(backend) = self.backend.as_ref() {
      let snapshot = backend.snapshot();
      let snapshot_text = snapshot
        .text_for_range(0..snapshot.byte_len())
        .unwrap_or_default();
      self.text = Rope::from(snapshot_text.as_ref());
      self.backend_revision = backend.revision();
      self.backend_snapshot = Some(snapshot);
      self.highlighter = backend.create_highlighter();
      self.highlighter_revision = 0;
      self.text_wrapper.set_default_text(&self.text);
      self.refresh_backend_highlighter(true);
    }
    self
  }

  /// Replace the current editor backend.
  pub fn set_backend(
    &mut self, backend: impl EditorBackend + 'static, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.backend = Some(Box::new(backend));
    self.backend_revision = 0;
    self.backend_snapshot = None;
    self.highlighter = None;
    self.highlighter_revision = 0;
    self.sync_text_with_backend(true, window, cx);
    cx.notify();
  }

  /// Remove the current editor backend and switch back to direct rope state.
  pub fn clear_backend(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    self.backend = None;
    self.backend_revision = 0;
    self.backend_snapshot = None;
    self.highlighter = None;
    self.highlighter_revision = 0;
    cx.notify();
  }

  /// Whether current editor is driven by a backend.
  pub fn has_backend(&self) -> bool {
    self.backend.is_some()
  }

  pub(crate) fn current_backend_snapshot(&self) -> Option<Arc<dyn EditorSnapshot>> {
    self
      .backend_snapshot
      .clone()
      .or_else(|| self.backend.as_ref().map(|backend| backend.snapshot()))
  }

  /// Total line count as u64, for ultra-large data sources.
  pub fn line_count_u64(&self) -> u64 {
    self
      .current_backend_snapshot()
      .map(|snapshot| snapshot.line_count())
      .unwrap_or(self.text.lines_len() as u64)
  }

  pub(crate) fn display_row_count(&self) -> usize {
    self.text_wrapper.len().max(1)
  }

  pub(crate) fn display_row_to_line_row(&self, display_row: usize) -> (usize, usize) {
    self
      .text_wrapper
      .display_row_to_line_row(display_row)
      .unwrap_or((0, 0))
  }

  pub(crate) fn viewport_rows(&self, line_height: Pixels) -> usize {
    viewport::viewport_rows(self.input_bounds.size.height, line_height)
  }

  pub(crate) fn clamp_top_row(&mut self, line_height: Pixels) {
    self.top_row = viewport::clamp_top_row(
      self.top_row,
      self.display_row_count(),
      self.viewport_rows(line_height),
    );
  }

  pub(crate) fn set_top_row(&mut self, row: usize, line_height: Pixels) -> bool {
    let clamped = viewport::clamp_top_row(
      row,
      self.display_row_count(),
      self.viewport_rows(line_height),
    );
    if clamped == self.top_row {
      return false;
    }

    self.top_row = clamped;
    self.diagnostic_popover = None;
    true
  }

  pub(crate) fn scroll_rows(&mut self, delta_rows: i64, line_height: Pixels) -> bool {
    let target_row = if delta_rows < 0 {
      self.top_row.saturating_sub((-delta_rows) as usize)
    } else {
      self.top_row.saturating_add(delta_rows as usize)
    };
    self.set_top_row(target_row, line_height)
  }

  /// Display line number text for row.
  pub fn line_number_text_for_row(&self, row: u64) -> String {
    self
      .current_backend_snapshot()
      .map(|snapshot| snapshot.line_number_text(row).to_string())
      .unwrap_or_else(|| row.saturating_add(1).to_string())
  }

  /// Largest line-number text sample used for gutter width measuring.
  pub fn max_line_number_text(&self) -> String {
    self
      .current_backend_snapshot()
      .map(|snapshot| snapshot.max_line_number_text().to_string())
      .unwrap_or_else(|| self.line_count_u64().max(1).to_string())
  }

  /// Range for row in utf-8 bytes.
  pub fn row_range_u64(&self, row: u64) -> Option<Range<u64>> {
    if let Some(snapshot) = self.current_backend_snapshot() {
      return snapshot.line_range(row);
    }

    let row = (row as usize).min(self.text.lines_len().saturating_sub(1));
    let start = self.text.line_start_offset(row) as u64;
    let end = self.text.line_end_offset(row) as u64;
    Some(start..end)
  }

  /// Read text by utf-8 byte range.
  pub fn text_for_u64_range(&self, range: Range<u64>) -> Option<String> {
    if let Some(snapshot) = self.current_backend_snapshot() {
      return snapshot.text_for_range(range).map(|text| text.to_string());
    }

    let start = (range.start as usize).min(self.text.len());
    let end = (range.end as usize).min(self.text.len());
    Some(self.text.slice(start..end).to_string())
  }

  pub(crate) fn extend_context_menu_from_backend(
    &self, menu: crate::PopupMenu, state: &Entity<InputState>, window: &mut Window,
  ) -> crate::PopupMenu {
    if let Some(backend) = self.backend.as_ref() {
      return backend.extend_context_menu(menu, state, window);
    }

    menu
  }

  pub(crate) fn sync_wrap_metrics_for_view(
    &mut self, wrap_width: Pixels, window: &Window, cx: &mut App,
  ) {
    let style = window.text_style();
    let font = style.font();
    let font_size = style.font_size.to_pixels(window.rem_size());
    self.text_wrapper.sync(
      &self.text,
      font,
      font_size,
      Some(wrap_width.max(px(1.0))),
      window,
      cx,
    );
  }

  pub(super) fn emit_backend_action(&mut self, action: EditorUserAction) {
    if let Some(backend) = self.backend.as_mut() {
      backend.on_user_action(&action);
    }
  }

  fn install_builtin_backend_if_needed(&mut self) {
    if !self.mode.is_code_editor() || self.backend.is_some() {
      return;
    }

    let Some(language) = self.builtin_backend_language.clone() else {
      return;
    };

    self.backend = Some(Box::new(
      RopeBufferBackend::new(self.text.to_string()).with_language(language),
    ));
    self.backend_revision = 0;
    self.backend_snapshot = None;
    self.highlighter = None;
    self.highlighter_revision = 0;
  }

  fn refresh_backend_highlighter(&mut self, force: bool) {
    let Some(snapshot) = self.backend_snapshot.clone() else {
      self.highlighter = None;
      self.highlighter_revision = 0;
      return;
    };

    if self.highlighter.is_none() {
      self.highlighter = self
        .backend
        .as_ref()
        .and_then(|backend| backend.create_highlighter());
      self.highlighter_revision = 0;
    }

    if !force && self.highlighter_revision == snapshot.revision() {
      return;
    }

    if let Some(highlighter) = self.highlighter.as_mut() {
      highlighter.sync(snapshot.as_ref(), None);
      self.highlighter_revision = snapshot.revision();
    }
  }

  fn sync_text_with_backend(
    &mut self, force: bool, window: &mut Window, cx: &mut Context<Self>,
  ) -> bool {
    let Some(backend) = self.backend.as_ref() else {
      return false;
    };

    let revision = backend.revision();
    if !force && revision == self.backend_revision {
      return false;
    }

    let snapshot = backend.snapshot();
    let snapshot_text = snapshot
      .text_for_range(0..snapshot.byte_len())
      .unwrap_or_default();
    self.text = Rope::from(snapshot_text.as_ref());
    self.backend_revision = revision;
    self.backend_snapshot = Some(snapshot);

    if let Some(diagnostics) = self.mode.diagnostics_mut() {
      diagnostics.reset(&self.text);
    }

    if let Some(last_layout) = self.last_layout.as_ref() {
      self.sync_wrap_metrics_for_view(last_layout.content_width, window, cx);
    } else {
      self.text_wrapper.set_default_text(&self.text);
    }
    self.refresh_backend_highlighter(true);
    self.lsp.update(&self.text, window, cx);
    self.update_search(cx);
    if !self.mode.is_code_editor() {
      self.mode.update_auto_grow(&self.text_wrapper);
    }
    self.clamp_top_row(window.line_height());
    true
  }

  fn apply_custom_backend_edit(
    &mut self, range: &Range<usize>, new_text: &str, marked: bool, window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Option<EditorBackendEditResult> {
    let backend = self.backend.as_mut()?;

    let request = EditorBackendEditRequest {
      range: (range.start as u64)..(range.end as u64),
      new_text: new_text.to_string(),
      marked,
    };
    let response = match backend.apply_edit(request) {
      Ok(response) => response,
      Err(EditorEditError::Unsupported | EditorEditError::Rejected(_)) => EditorBackendEditResult {
        accepted: false,
        selection: None,
        cursor: None,
      },
    };
    if response.accepted {
      self.sync_text_with_backend(true, window, cx);
    }
    Some(response)
  }

  /// Set this input is searchable, default is false (Default true for Code
  /// Editor).
  pub fn searchable(mut self, searchable: bool) -> Self {
    self.searchable = searchable;
    self
  }

  /// Set placeholder
  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = placeholder.into();
    self
  }

  /// Set enable/disable line number, only for [`InputMode::CodeEditor`] mode.
  pub fn line_number(mut self, line_number: bool) -> Self {
    debug_assert!(self.mode.is_code_editor());
    if let InputMode::CodeEditor { line_number: l, .. } = &mut self.mode {
      *l = line_number;
    }
    self
  }

  /// Set line number, only for [`InputMode::CodeEditor`] mode.
  pub fn set_line_number(&mut self, line_number: bool, _: &mut Window, cx: &mut Context<Self>) {
    debug_assert!(self.mode.is_code_editor());
    if let InputMode::CodeEditor { line_number: l, .. } = &mut self.mode {
      *l = line_number;
    }
    cx.notify();
  }

  /// Set the number of rows for the editor.
  pub fn rows(mut self, rows: usize) -> Self {
    match &mut self.mode {
      InputMode::PlainText { rows: r, .. } | InputMode::CodeEditor { rows: r, .. } => *r = rows,
      InputMode::AutoGrow {
        max_rows: max_r,
        rows: r,
        ..
      } => {
        *r = rows;
        *max_r = rows;
      }
    }
    self
  }

  #[inline]
  pub fn diagnostics(&self) -> Option<&DiagnosticSet> {
    self.mode.diagnostics()
  }

  #[inline]
  pub fn diagnostics_mut(&mut self) -> Option<&mut DiagnosticSet> {
    self.mode.diagnostics_mut()
  }

  /// Set placeholder
  pub fn set_placeholder(
    &mut self, placeholder: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.placeholder = placeholder.into();
    cx.notify();
  }

  /// Find which line and sub-line the given offset belongs to, along with the
  /// position within that sub-line.
  ///
  /// Returns:
  ///
  /// - The index of the line (zero-based) containing the offset.
  /// - The index of the sub-line (zero-based) within the line containing the
  ///   offset.
  /// - The position of the offset.
  pub(super) fn line_and_position_for_offset(
    &self, offset: usize,
  ) -> (usize, usize, Option<Point<Pixels>>) {
    let Some(last_layout) = &self.last_layout else {
      return (0, 0, None);
    };
    let line_height = last_layout.line_height;

    let mut prev_lines_offset = last_layout.visible_range_offset.start;
    let mut y_offset = last_layout.visible_top;
    for (line_index, line) in last_layout.lines.iter().enumerate() {
      let local_offset = offset.saturating_sub(prev_lines_offset);
      if let Some(pos) = line.position_for_index(local_offset, last_layout) {
        let sub_line_index = (pos.y / line_height) as usize;
        let adjusted_pos = point(pos.x + last_layout.line_number_width, pos.y + y_offset);
        return (line_index, sub_line_index, Some(adjusted_pos));
      }

      y_offset += line.size(line_height).height;
      prev_lines_offset += line.len() + 1;
    }
    (0, 0, None)
  }

  /// Set the text of the input field.
  ///
  /// And the selection_range will be reset to 0..0.
  pub fn set_value(
    &mut self, value: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.history.ignore = true;
    self.replace_text(value, window, cx);
    self.history.ignore = false;

    // Ensure cursor to start when set text.
    self.selected_range.clear();

    if self.mode.is_code_editor() {
      self._pending_update = true;
      self.lsp.reset();
    }

    // Move scroll to top
    self.top_row = 0;

    cx.notify();
  }

  /// Insert text at the current cursor position.
  ///
  /// And the cursor will be moved to the end of inserted text.
  pub fn insert(
    &mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let was_disabled = self.disabled;
    self.disabled = false;
    let text: SharedString = text.into();
    let range_utf16 = self.range_to_utf16(&(self.cursor()..self.cursor()));
    self.replace_text_in_range_silent(Some(range_utf16), &text, window, cx);
    self.selected_range = (self.selected_range.end..self.selected_range.end).into();
    self.disabled = was_disabled;
  }

  /// Replace text at the current cursor position.
  ///
  /// And the cursor will be moved to the end of replaced text.
  pub fn replace(
    &mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let was_disabled = self.disabled;
    self.disabled = false;
    let text: SharedString = text.into();
    self.replace_text_in_range_silent(None, &text, window, cx);
    self.selected_range = (self.selected_range.end..self.selected_range.end).into();
    self.disabled = was_disabled;
  }

  fn replace_text(
    &mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let was_disabled = self.disabled;
    self.disabled = false;
    let text: SharedString = text.into();
    let range = 0..self.text.chars().map(|c| c.len_utf16()).sum();
    self.replace_text_in_range_silent(Some(range), &text, window, cx);
    self.disabled = was_disabled;
  }

  /// Set with disabled mode.
  ///
  /// See also: [`Self::set_disabled`], [`Self::is_disabled`].
  #[allow(unused)]
  pub(crate) fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  /// Set with read-only mode.
  ///
  /// When read-only, write actions are disabled but navigation, selection,
  /// copy and select-all still work. The cursor is hidden.
  pub fn read_only(mut self, read_only: bool) -> Self {
    self.read_only = read_only;
    self
  }

  /// Set the read-only state of the input field.
  pub fn set_read_only(&mut self, read_only: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.read_only = read_only;
    cx.notify();
  }

  /// Return whether the input field is read-only.
  pub fn is_read_only(&self) -> bool {
    self.read_only
  }

  /// Set with password masked state.
  pub fn masked(mut self, masked: bool) -> Self {
    self.masked = masked;
    self
  }

  /// Set the password masked state of the input field.
  pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.masked = masked;
    cx.notify();
  }

  /// Set true to clear the input by pressing Escape key.
  pub fn clean_on_escape(mut self) -> Self {
    self.clean_on_escape = true;
    self
  }

  /// Set whether to show whitespace characters.
  pub fn show_whitespaces(mut self, show: bool) -> Self {
    self.show_whitespaces = show;
    self
  }

  /// Update whether to show whitespace characters.
  pub fn set_show_whitespaces(&mut self, show: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.show_whitespaces = show;
    cx.notify();
  }

  /// Set the regular expression pattern of the input field.
  pub fn pattern(mut self, pattern: regex::Regex) -> Self {
    self.pattern = Some(pattern);
    self
  }

  /// Set the regular expression pattern of the input field with reference.
  pub fn set_pattern(
    &mut self, pattern: regex::Regex, _window: &mut Window, _cx: &mut Context<Self>,
  ) {
    self.pattern = Some(pattern);
  }

  /// Set the validation function of the input field.
  pub fn validate(mut self, f: impl Fn(&str, &mut Context<Self>) -> bool + 'static) -> Self {
    self.validate = Some(Box::new(f));
    self
  }

  /// Set true to show spinner at the input right.
  pub fn set_loading(&mut self, loading: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.loading = loading;
    cx.notify();
  }

  /// Set the default value of the input field.
  pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
    let text: SharedString = value.into();
    self.text = Rope::from(text.as_str());
    if let Some(diagnostics) = self.mode.diagnostics_mut() {
      diagnostics.reset(&self.text)
    }
    if !self.mode.is_code_editor() {
      self.text_wrapper.set_default_text(&self.text);
    }
    self._pending_update = true;
    self
  }

  /// Return the value of the input field.
  pub fn value(&self) -> SharedString {
    if let Some(snapshot) = self.current_backend_snapshot() {
      return snapshot
        .text_for_range(0..snapshot.byte_len())
        .unwrap_or_default();
    }

    SharedString::new(self.text.to_string())
  }

  /// Return the value without mask.
  pub fn unmask_value(&self) -> SharedString {
    self.mask_pattern.unmask(self.value().as_ref()).into()
  }

  /// Return the text [`Rope`] of the input field.
  pub fn text(&self) -> &Rope {
    &self.text
  }

  /// Return the (0-based) [`Position`] of the cursor.
  pub fn cursor_position(&self) -> Position {
    let offset = self.cursor();
    self.text.offset_to_position(offset)
  }

  /// Set (0-based) [`Position`] of the cursor.
  ///
  /// This will move the cursor to the specified line and column, and update the
  /// selection range.
  pub fn set_cursor_position(
    &mut self, position: impl Into<Position>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    let position: Position = position.into();
    let offset = self.text.position_to_offset(&position);

    self.move_to(offset, None, cx);
    self.update_preferred_column();
    self.focus(window, cx);
  }

  /// Focus the input field.
  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    self.focus_handle.focus(window);
    self.blink_cursor.update(cx, |cursor, cx| {
      cursor.start(cx);
    });
  }

  pub(super) fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
    self.select_to(self.previous_boundary(self.cursor()), cx);
  }

  pub(super) fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
    self.select_to(self.next_boundary(self.cursor()), cx);
  }

  pub(super) fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
    let offset = self.start_of_line().saturating_sub(1);
    self.select_to(self.previous_boundary(offset), cx);
  }

  pub(super) fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
    let offset = (self.end_of_line() + 1).min(self.text.len());
    self.select_to(self.next_boundary(offset), cx);
  }

  pub(super) fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
    self.selected_range = (0..self.text.len()).into();
    self.emit_backend_action(EditorUserAction::Select {
      range: (self.selected_range.start as u64)..(self.selected_range.end as u64),
      reversed: self.selection_reversed,
    });
    cx.notify();
  }

  pub(super) fn select_to_start(
    &mut self, _: &SelectToStart, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.select_to(0, cx);
  }

  pub(super) fn select_to_end(&mut self, _: &SelectToEnd, _: &mut Window, cx: &mut Context<Self>) {
    let end = self.text.len();
    self.select_to(end, cx);
  }

  pub(super) fn select_to_start_of_line(
    &mut self, _: &SelectToStartOfLine, _: &mut Window, cx: &mut Context<Self>,
  ) {
    let offset = self.start_of_line();
    self.select_to(offset, cx);
  }

  pub(super) fn select_to_end_of_line(
    &mut self, _: &SelectToEndOfLine, _: &mut Window, cx: &mut Context<Self>,
  ) {
    let offset = self.end_of_line();
    self.select_to(offset, cx);
  }

  pub(super) fn select_to_previous_word(
    &mut self, _: &SelectToPreviousWordStart, _: &mut Window, cx: &mut Context<Self>,
  ) {
    let offset = self.previous_start_of_word();
    self.select_to(offset, cx);
  }

  pub(super) fn select_to_next_word(
    &mut self, _: &SelectToNextWordEnd, _: &mut Window, cx: &mut Context<Self>,
  ) {
    let offset = self.next_end_of_word();
    self.select_to(offset, cx);
  }

  /// Return the start offset of the previous word.
  pub(super) fn previous_start_of_word(&mut self) -> usize {
    let offset = self.selected_range.start;
    let offset = self.offset_from_utf16(self.offset_to_utf16(offset));
    // FIXME: Avoid to_string
    let left_part = self.text.slice(0..offset).to_string();

    UnicodeSegmentation::split_word_bound_indices(left_part.as_str())
      .rfind(|(_, s)| !s.trim_start().is_empty())
      .map(|(i, _)| i)
      .unwrap_or(0)
  }

  /// Return the next end offset of the next word.
  pub(super) fn next_end_of_word(&mut self) -> usize {
    let offset = self.cursor();
    let offset = self.offset_from_utf16(self.offset_to_utf16(offset));
    let right_part = self.text.slice(offset..self.text.len()).to_string();

    UnicodeSegmentation::split_word_bound_indices(right_part.as_str())
      .find(|(_, s)| !s.trim_start().is_empty())
      .map(|(i, s)| offset + i + s.len())
      .unwrap_or(self.text.len())
  }

  /// Get start of line byte offset of cursor
  pub(super) fn start_of_line(&self) -> usize {
    let row = self.text.offset_to_point(self.cursor()).row;
    self.text.line_start_offset(row)
  }

  /// Get end of line byte offset of cursor
  pub(super) fn end_of_line(&self) -> usize {
    let row = self.text.offset_to_point(self.cursor()).row;
    self.text.line_end_offset(row)
  }

  /// Get start line of selection start or end (The min value).
  ///
  /// This is means is always get the first line of selection.
  pub(super) fn start_of_line_of_selection(
    &mut self, window: &mut Window, cx: &mut Context<Self>,
  ) -> usize {
    let mut offset = self.previous_boundary(self.selected_range.start.min(self.selected_range.end));
    if self.text.char_at(offset) == Some('\r') {
      offset += 1;
    }

    self
      .text_for_range(self.range_to_utf16(&(0..offset + 1)), &mut None, window, cx)
      .unwrap_or_default()
      .rfind('\n')
      .map(|i| i + 1)
      .unwrap_or(0)
  }

  /// Get indent string of next line.
  ///
  /// To get current and next line indent, to return more depth one.
  pub(super) fn indent_of_next_line(&mut self) -> String {
    let mut current_indent = String::new();
    let mut next_indent = String::new();
    let current_line_start_pos = self.start_of_line();
    let next_line_start_pos = self.end_of_line();
    for c in self.text.slice(current_line_start_pos..).chars() {
      if !c.is_whitespace() {
        break;
      }
      if c == '\n' || c == '\r' {
        break;
      }
      current_indent.push(c);
    }

    for c in self.text.slice(next_line_start_pos..).chars() {
      if !c.is_whitespace() {
        break;
      }
      if c == '\n' || c == '\r' {
        break;
      }
      next_indent.push(c);
    }

    if next_indent.len() > current_indent.len() {
      next_indent
    } else {
      current_indent
    }
  }

  pub(super) fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
    if self.selected_range.is_empty() {
      self.select_to(self.previous_boundary(self.cursor()), cx)
    }
    self.replace_text_in_range(None, "", window, cx);
    self.pause_blink_cursor(cx);
  }

  pub(super) fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
    if self.selected_range.is_empty() {
      self.select_to(self.next_boundary(self.cursor()), cx)
    }
    self.replace_text_in_range(None, "", window, cx);
    self.pause_blink_cursor(cx);
  }

  pub(super) fn delete_to_beginning_of_line(
    &mut self, _: &DeleteToBeginningOfLine, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range(None, "", window, cx);
      self.pause_blink_cursor(cx);
      return;
    }

    let mut offset = self.start_of_line();
    if offset == self.cursor() {
      offset = offset.saturating_sub(1);
    }
    self.replace_text_in_range_silent(
      Some(self.range_to_utf16(&(offset..self.cursor()))),
      "",
      window,
      cx,
    );
    self.pause_blink_cursor(cx);
  }

  pub(super) fn delete_to_end_of_line(
    &mut self, _: &DeleteToEndOfLine, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range(None, "", window, cx);
      self.pause_blink_cursor(cx);
      return;
    }

    let mut offset = self.end_of_line();
    if offset == self.cursor() {
      offset = (offset + 1).clamp(0, self.text.len());
    }
    self.replace_text_in_range_silent(
      Some(self.range_to_utf16(&(self.cursor()..offset))),
      "",
      window,
      cx,
    );
    self.pause_blink_cursor(cx);
  }

  pub(super) fn delete_previous_word(
    &mut self, _: &DeleteToPreviousWordStart, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range(None, "", window, cx);
      self.pause_blink_cursor(cx);
      return;
    }

    let offset = self.previous_start_of_word();
    self.replace_text_in_range_silent(
      Some(self.range_to_utf16(&(offset..self.cursor()))),
      "",
      window,
      cx,
    );
    self.pause_blink_cursor(cx);
  }

  pub(super) fn delete_next_word(
    &mut self, _: &DeleteToNextWordEnd, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range(None, "", window, cx);
      self.pause_blink_cursor(cx);
      return;
    }

    let offset = self.next_end_of_word();
    self.replace_text_in_range_silent(
      Some(self.range_to_utf16(&(self.cursor()..offset))),
      "",
      window,
      cx,
    );
    self.pause_blink_cursor(cx);
  }

  pub(super) fn enter(&mut self, action: &Enter, window: &mut Window, cx: &mut Context<Self>) {
    if self.handle_action_for_context_menu(Box::new(action.clone()), window, cx) {
      return;
    }

    // Clear inline completion on enter (user chose not to accept it)
    if self.has_inline_completion() {
      self.clear_inline_completion(cx);
    }

    // Get current line indent
    let indent = if self.mode.is_code_editor() {
      self.indent_of_next_line()
    } else {
      "".to_string()
    };

    // Add newline and indent
    let new_line_text = format!("\n{}", indent);
    self.replace_text_in_range_silent(None, &new_line_text, window, cx);
    self.pause_blink_cursor(cx);

    cx.emit(InputEvent::PressEnter {
      secondary: action.secondary,
    });
  }

  pub(super) fn clean(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    self.replace_text("", window, cx);
    self.selected_range = (0..0).into();
    self.scroll_to(0, None, cx);
  }

  pub(super) fn escape(&mut self, action: &Escape, window: &mut Window, cx: &mut Context<Self>) {
    if self.handle_action_for_context_menu(Box::new(action.clone()), window, cx) {
      return;
    }

    // Clear inline completion on escape
    if self.has_inline_completion() {
      self.clear_inline_completion(cx);
      return; // Consume the escape, don't propagate
    }

    if self.ime_marked_range.is_some() {
      self.unmark_text(window, cx);
    }

    if self.clean_on_escape {
      return self.clean(window, cx);
    }

    cx.propagate();
  }

  pub(super) fn on_mouse_down(
    &mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.disabled {
      return;
    }

    // Ignore click-through while the shared context menu is open.
    if self.shared_context_menu_open {
      if self.focus_handle.is_focused(window) {
        self.shared_context_menu_open = false;
      } else if event.button == MouseButton::Left {
        return;
      }
    }

    // Clear inline completion on any mouse interaction
    self.clear_inline_completion(cx);

    // If there have IME marked range and is empty (Means pressed Esc to abort IME
    // typing) Clear the marked range.
    if let Some(ime_marked_range) = &self.ime_marked_range
      && ime_marked_range.is_empty()
    {
      self.ime_marked_range = None;
    }

    // Keep editor focus before opening the shared right-click menu.
    self.focus(window, cx);
    self.selecting = true;
    let offset = self.index_for_mouse_position(event.position);
    self.emit_backend_action(EditorUserAction::MouseDown {
      offset: offset as u64,
      button: EditorPointerButton::from(event.button),
      click_count: event.click_count.min(u8::MAX as usize) as u8,
      shift: event.modifiers.shift,
    });

    if self.handle_click_hover_definition(event, offset, window, cx) {
      return;
    }

    // Triple click to select line
    if event.button == MouseButton::Left && event.click_count >= 3 {
      self.select_line(offset, window, cx);
      return;
    }

    // Double click to select word
    if event.button == MouseButton::Left && event.click_count == 2 {
      self.select_word(offset, window, cx);
      return;
    }

    // Right-click keeps cursor/selection synchronized before opening the
    // shared context menu component.
    if event.button == MouseButton::Right {
      self.emit_backend_action(EditorUserAction::ContextMenuRequested {
        offset: offset as u64,
      });
      cx.stop_propagation();
      let selected = self.selected_range.normalized();
      let clicked_in_selection =
        !selected.is_empty() && (selected.contains(&offset) || offset == selected.end);
      if !clicked_in_selection {
        self.move_to(offset, None, cx);
      }

      if self.mode.is_code_editor() {
        self.handle_hover_definition(offset, window, cx);
      }
      return;
    }

    if event.modifiers.shift {
      self.select_to(offset, cx);
    } else {
      self.move_to(offset, None, cx)
    }
  }

  pub(super) fn on_mouse_up(
    &mut self, event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>,
  ) {
    let offset = self.index_for_mouse_position(event.position);
    self.emit_backend_action(EditorUserAction::MouseUp {
      offset: offset as u64,
      button: EditorPointerButton::from(event.button),
    });

    if self.selected_range.is_empty() {
      self.selection_reversed = false;
    }
    self.scrollbar_dragging = false;
    self.selecting = false;
    self.selected_word_range = None;
  }

  pub(super) fn on_mouse_move(
    &mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>,
  ) {
    // Show diagnostic popover on mouse move
    let offset = self.index_for_mouse_position(event.position);
    self.emit_backend_action(EditorUserAction::MouseMove {
      offset: offset as u64,
    });
    self.handle_mouse_move(offset, event, window, cx);

    if self.mode.is_code_editor() {
      if let Some(diagnostic) = self
        .mode
        .diagnostics()
        .and_then(|set| set.for_offset(offset))
      {
        if let Some(diagnostic_popover) = self.diagnostic_popover.as_ref()
          && diagnostic_popover.read(cx).diagnostic.range == diagnostic.range
        {
          diagnostic_popover.update(cx, |this, cx| {
            this.show(cx);
          });

          return;
        }

        self.diagnostic_popover = Some(DiagnosticPopover::new(diagnostic, cx.entity(), cx));
        cx.notify();
      } else if let Some(diagnostic_popover) = self.diagnostic_popover.as_mut() {
        diagnostic_popover.update(cx, |this, cx| {
          this.check_to_hide(event.position, cx);
        })
      }
    }
  }

  pub(super) fn on_scroll_wheel(
    &mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>,
  ) {
    const SCROLL_LINES: i64 = 3;

    let line_height = self
      .last_layout
      .as_ref()
      .map(|layout| layout.line_height)
      .unwrap_or(window.line_height());
    let delta = event.delta.pixel_delta(line_height);
    self.emit_backend_action(EditorUserAction::Scroll {
      delta_x: delta.x.as_f64() as f32,
      delta_y: delta.y.as_f64() as f32,
    });

    let delta_px = delta.y.as_f32();
    if delta_px.abs() < 0.01 {
      return;
    }

    let line_height_px = line_height.as_f32();
    let ticks = (delta_px / line_height_px).round() as i64;
    let delta_rows = if ticks == 0 {
      if delta_px > 0.0 {
        -SCROLL_LINES
      } else {
        SCROLL_LINES
      }
    } else {
      -ticks * SCROLL_LINES
    };

    if self.scroll_rows(delta_rows, line_height) {
      cx.stop_propagation();
      cx.notify();
    }

    self.diagnostic_popover = None;
  }

  /// Scroll to make the given offset visible.
  ///
  /// If `direction` is Some, will keep edges at the same side.
  pub(crate) fn scroll_to(
    &mut self, offset: usize, direction: Option<MoveDirection>, cx: &mut Context<Self>,
  ) {
    let Some(last_layout) = self.last_layout.as_ref() else {
      return;
    };
    let display_point = self
      .text_wrapper
      .offset_to_display_point(offset.min(self.text.len()));
    let viewport_rows = self.viewport_rows(last_layout.line_height);
    let margin_rows = if direction.is_some() && self.mode.is_code_editor() {
      3
    } else {
      1
    };

    let mut new_top_row = self.top_row;
    if display_point.row < self.top_row.saturating_add(margin_rows) {
      new_top_row = display_point.row.saturating_sub(margin_rows);
    } else if display_point.row.saturating_add(margin_rows)
      >= self.top_row.saturating_add(viewport_rows)
    {
      let keep_rows = viewport_rows.saturating_sub(1);
      new_top_row = display_point
        .row
        .saturating_add(margin_rows)
        .saturating_sub(keep_rows);
    }

    if self.set_top_row(new_top_row, last_layout.line_height) {
      cx.notify();
    }
  }

  pub(super) fn show_character_palette(
    &mut self, _: &ShowCharacterPalette, window: &mut Window, _: &mut Context<Self>,
  ) {
    window.show_character_palette();
  }

  pub(super) fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
    if self.selected_range.is_empty() {
      return;
    }

    let selected_text = self.text.slice(self.selected_range).to_string();
    self.emit_backend_action(EditorUserAction::Copy {
      range: (self.selected_range.start as u64)..(self.selected_range.end as u64),
    });
    cx.write_to_clipboard(ClipboardItem::new_string(selected_text));
  }

  pub(super) fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
    if self.selected_range.is_empty() {
      return;
    }

    let selected_text = self.text.slice(self.selected_range).to_string();
    self.emit_backend_action(EditorUserAction::Cut {
      range: (self.selected_range.start as u64)..(self.selected_range.end as u64),
    });
    cx.write_to_clipboard(ClipboardItem::new_string(selected_text));

    self.replace_text_in_range_silent(None, "", window, cx);
  }

  pub(super) fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(clipboard) = cx.read_from_clipboard() {
      let new_text = clipboard.text().unwrap_or_default();

      self.emit_backend_action(EditorUserAction::Paste {
        range: (self.selected_range.start as u64)..(self.selected_range.end as u64),
        text: new_text.clone(),
      });
      self.replace_text_in_range_silent(None, &new_text, window, cx);
      self.scroll_to(self.cursor(), None, cx);
    }
  }

  fn push_history(&mut self, text: &Rope, range: &Range<usize>, new_text: &str) {
    if self.history.ignore {
      return;
    }

    let range = text.clip_offset(range.start, Bias::Left)..text.clip_offset(range.end, Bias::Right);
    let old_text = text.slice(range.clone()).to_string();
    let new_range = range.start..range.start + new_text.len();

    self
      .history
      .push(Change::new(range, &old_text, new_range, new_text));
  }

  pub(super) fn undo(&mut self, _: &Undo, window: &mut Window, cx: &mut Context<Self>) {
    self.emit_backend_action(EditorUserAction::UndoRequested);

    self.history.ignore = true;
    if let Some(changes) = self.history.undo() {
      for change in changes {
        let range_utf16 = self.range_to_utf16(&change.new_range.into());
        self.replace_text_in_range_silent(Some(range_utf16), &change.old_text, window, cx);
      }
    }
    self.history.ignore = false;
  }

  pub(super) fn redo(&mut self, _: &Redo, window: &mut Window, cx: &mut Context<Self>) {
    self.emit_backend_action(EditorUserAction::RedoRequested);

    self.history.ignore = true;
    if let Some(changes) = self.history.redo() {
      for change in changes {
        let range_utf16 = self.range_to_utf16(&change.old_range.into());
        self.replace_text_in_range_silent(Some(range_utf16), &change.new_text, window, cx);
      }
    }
    self.history.ignore = false;
  }

  /// Get byte offset of the cursor.
  ///
  /// The offset is the UTF-8 offset.
  pub fn cursor(&self) -> usize {
    if let Some(ime_marked_range) = &self.ime_marked_range {
      return ime_marked_range.end;
    }

    if self.selection_reversed {
      self.selected_range.start
    } else {
      self.selected_range.end
    }
  }

  pub(crate) fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
    // If the text is empty, always return 0
    if self.text.len() == 0 {
      return 0;
    }

    let (Some(bounds), Some(last_layout)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
    else {
      return 0;
    };

    let line_height = last_layout.line_height;
    let line_number_width = last_layout.line_number_width;

    // TIP: About the IBeam cursor
    //
    // If cursor style is IBeam, the mouse mouse position is in the middle of the
    // cursor (This is special in OS)

    // The position is relative to the bounds of the text input
    //
    // bounds.origin:
    //
    // - included the input padding.
    // - included the scroll offset.
    let inner_position =
      position - bounds.origin - point(line_number_width, last_layout.visible_top);

    let mut index = last_layout.visible_range_offset.start;
    let mut y_offset = last_layout.visible_top;
    for line_layout in last_layout.lines.iter() {
      let line_origin = point(px(0.), y_offset);
      let pos = inner_position - line_origin;
      if let Some(v) = line_layout.closest_index_for_position(pos, last_layout) {
        index += v;
        break;
      } else if pos.y < px(0.) {
        break;
      }

      y_offset += line_layout.size(line_height).height;
      index += line_layout.len() + 1;
    }

    let index = if index > self.text.len() {
      self.text.len()
    } else {
      index
    };

    if self.masked {
      // When is masked, the index is char index, need convert to byte index.
      self.text.char_index_to_offset(index)
    } else {
      index
    }
  }

  /// Select the text from the current cursor position to the given offset.
  ///
  /// The offset is the UTF-8 offset.
  ///
  /// Ensure the offset use self.next_boundary or self.previous_boundary to get
  /// the correct offset.
  pub(crate) fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
    self.clear_inline_completion(cx);

    let offset = offset.clamp(0, self.text.len());
    if self.selection_reversed {
      self.selected_range.start = offset
    } else {
      self.selected_range.end = offset
    };

    if self.selected_range.end < self.selected_range.start {
      self.selection_reversed = !self.selection_reversed;
      self.selected_range = (self.selected_range.end..self.selected_range.start).into();
    }

    // Ensure keep word selected range
    if let Some(word_range) = self.selected_word_range.as_ref() {
      if self.selected_range.start > word_range.start {
        self.selected_range.start = word_range.start;
      }
      if self.selected_range.end < word_range.end {
        self.selected_range.end = word_range.end;
      }
    }
    if self.selected_range.is_empty() {
      self.update_preferred_column();
    }
    self.emit_backend_action(EditorUserAction::Select {
      range: (self.selected_range.start as u64)..(self.selected_range.end as u64),
      reversed: self.selection_reversed,
    });
    cx.notify()
  }

  /// Unselects the currently selected text.
  pub fn unselect(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    let offset = self.cursor();
    self.selected_range = (offset..offset).into();
    self.emit_backend_action(EditorUserAction::Select {
      range: (offset as u64)..(offset as u64),
      reversed: false,
    });
    cx.notify()
  }

  #[inline]
  pub(super) fn offset_from_utf16(&self, offset: usize) -> usize {
    self.text.offset_utf16_to_offset(offset)
  }

  #[inline]
  pub(super) fn offset_to_utf16(&self, offset: usize) -> usize {
    self.text.offset_to_offset_utf16(offset)
  }

  #[inline]
  pub(super) fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
    self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
  }

  #[inline]
  pub(super) fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
    self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
  }

  pub(super) fn previous_boundary(&self, offset: usize) -> usize {
    let mut offset = self.text.clip_offset(offset.saturating_sub(1), Bias::Left);
    if let Some(ch) = self.text.char_at(offset)
      && ch == '\r'
    {
      offset -= 1;
    }

    offset
  }

  pub(super) fn next_boundary(&self, offset: usize) -> usize {
    let mut offset = self.text.clip_offset(offset + 1, Bias::Right);
    if let Some(ch) = self.text.char_at(offset)
      && ch == '\r'
    {
      offset += 1;
    }

    offset
  }

  /// Returns true when caret should be painted.
  pub(crate) fn show_cursor(&self, window: &Window, cx: &App) -> bool {
    (self.focus_handle.is_focused(window)
      || self.is_context_menu_open(cx)
      || self.shared_context_menu_open)
      && !self.disabled
      && !self.read_only
      && self.blink_cursor.read(cx).is_active()
      && window.is_window_active()
  }

  pub(crate) fn caret_opacity(&self, cx: &App) -> f32 {
    if self.shared_context_menu_open || self.is_context_menu_open(cx) {
      return 1.0;
    }

    self.blink_cursor.read(cx).opacity()
  }

  fn on_focus(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    self.shared_context_menu_open = false;
    self.blink_cursor.update(cx, |cursor, cx| {
      cursor.start(cx);
    });
    cx.emit(InputEvent::Focus);
  }

  fn on_blur(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if self.is_context_menu_open(cx) || self.shared_context_menu_open {
      return;
    }

    // NOTE: Do not cancel select, when blur.
    // Because maybe user want to copy the selected text by AppMenuBar (will take
    // focus handle).

    self.hover_popover = None;
    self.diagnostic_popover = None;
    self.context_menu = None;
    self.clear_inline_completion(cx);
    self.blink_cursor.update(cx, |cursor, cx| {
      cursor.stop(cx);
    });
    cx.emit(InputEvent::Blur);
    cx.notify();
  }

  pub(super) fn pause_blink_cursor(&mut self, cx: &mut Context<Self>) {
    self.blink_cursor.update(cx, |cursor, cx| {
      cursor.pause(cx);
    });
  }

  pub(super) fn on_key_down(&mut self, _: &KeyDownEvent, _: &mut Window, cx: &mut Context<Self>) {
    self.pause_blink_cursor(cx);
    self.emit_backend_action(EditorUserAction::MoveCursor {
      offset: self.cursor() as u64,
    });
  }

  pub(super) fn on_drag_move(
    &mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.text.len() == 0 {
      return;
    }

    if self.last_layout.is_none() {
      return;
    }

    if !self.focus_handle.is_focused(window) {
      return;
    }

    if !self.selecting {
      return;
    }

    let offset = self.index_for_mouse_position(event.position);
    self.select_to(offset, cx);
  }

  fn is_valid_input(&self, new_text: &str, cx: &mut Context<Self>) -> bool {
    if new_text.is_empty() {
      return true;
    }

    if let Some(validate) = &self.validate
      && !validate(new_text, cx)
    {
      return false;
    }

    if !self.mask_pattern.is_valid(new_text) {
      return false;
    }

    let Some(pattern) = &self.pattern else {
      return true;
    };

    pattern.is_match(new_text)
  }

  /// Set the mask pattern for formatting the input text.
  ///
  /// The pattern can contain:
  /// - 9: Any digit or dot
  /// - A: Any letter
  /// - *: Any character
  /// - Other characters will be treated as literal mask characters
  ///
  /// Example: "(999)999-999" for phone numbers
  pub fn mask_pattern(mut self, pattern: impl Into<MaskPattern>) -> Self {
    self.mask_pattern = pattern.into();
    if let Some(placeholder) = self.mask_pattern.placeholder() {
      self.placeholder = placeholder.into();
    }
    self
  }

  pub fn set_mask_pattern(
    &mut self, pattern: impl Into<MaskPattern>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.mask_pattern = pattern.into();
    if let Some(placeholder) = self.mask_pattern.placeholder() {
      self.placeholder = placeholder.into();
    }
    cx.notify();
  }

  #[allow(dead_code)]
  pub(super) fn set_input_bounds(&mut self, new_bounds: Bounds<Pixels>, cx: &mut Context<Self>) {
    if self.mode.is_code_editor() {
      self.input_bounds = new_bounds;
      return;
    }

    let wrap_width_changed = self.input_bounds.size.width != new_bounds.size.width;
    self.input_bounds = new_bounds;

    // Update text_wrapper wrap_width if changed.
    if let Some(_last_layout) = self.last_layout.as_ref()
      && wrap_width_changed
    {
      self.text_wrapper.set_default_text(&self.text);
      self.mode.update_auto_grow(&self.text_wrapper);
      cx.notify();
    }
  }

  pub(super) fn selected_text(&self) -> RopeSlice<'_> {
    let range_utf16 = self.range_to_utf16(&self.selected_range.into());
    let range = self.range_from_utf16(&range_utf16);
    self.text.slice(range)
  }

  #[allow(dead_code)]
  pub(crate) fn range_to_bounds(&self, range: &Range<usize>) -> Option<Bounds<Pixels>> {
    let last_layout = self.last_layout.as_ref()?;
    let last_bounds = self.last_bounds?;

    let (_, _, start_pos) = self.line_and_position_for_offset(range.start);
    let (_, _, end_pos) = self.line_and_position_for_offset(range.end);

    let start_pos = start_pos?;
    let end_pos = end_pos?;

    Some(Bounds::from_corners(
      last_bounds.origin + start_pos,
      last_bounds.origin + end_pos + point(px(0.), last_layout.line_height),
    ))
  }

  /// Replace text by [`lsp_types::Range`].
  ///
  /// See also: [`EntityInputHandler::replace_text_in_range`]
  #[allow(unused)]
  pub(crate) fn replace_text_in_lsp_range(
    &mut self, lsp_range: &lsp_types::Range, new_text: &str, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    let start = self.text.position_to_offset(&lsp_range.start);
    let end = self.text.position_to_offset(&lsp_range.end);
    self.replace_text_in_range_silent(
      Some(self.range_to_utf16(&(start..end))),
      new_text,
      window,
      cx,
    );
  }

  /// Replace text in range in silent.
  ///
  /// This will not trigger any UI interaction, such as auto-completion.
  pub(crate) fn replace_text_in_range_silent(
    &mut self, range_utf16: Option<Range<usize>>, new_text: &str, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.silent_replace_text = true;
    self.replace_text_in_range(range_utf16, new_text, window, cx);
    self.silent_replace_text = false;
  }
}

impl EntityInputHandler for InputState {
  fn text_for_range(
    &mut self, range_utf16: Range<usize>, adjusted_range: &mut Option<Range<usize>>,
    _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<String> {
    let range = self.range_from_utf16(&range_utf16);
    adjusted_range.replace(self.range_to_utf16(&range));
    if self.backend.is_some() {
      return self.text_for_u64_range((range.start as u64)..(range.end as u64));
    }

    Some(self.text.slice(range).to_string())
  }

  fn selected_text_range(
    &mut self, _ignore_disabled_input: bool, _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<UTF16Selection> {
    Some(UTF16Selection {
      range: self.range_to_utf16(&self.selected_range.into()),
      reversed: false,
    })
  }

  fn marked_text_range(
    &self, _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<Range<usize>> {
    self
      .ime_marked_range
      .map(|range| self.range_to_utf16(&range.into()))
  }

  fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    self.ime_marked_range = None;
  }

  /// Replace text in range.
  ///
  /// - If the new text is invalid, it will not be replaced.
  /// - If `range_utf16` is not provided, the current selected range will be
  ///   used.
  fn replace_text_in_range(
    &mut self, range_utf16: Option<Range<usize>>, new_text: &str, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.disabled || self.read_only {
      return;
    }

    self.pause_blink_cursor(cx);

    let range = range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16))
      .or(self.ime_marked_range.map(|range| {
        let range = self.range_to_utf16(&(range.start..range.end));
        self.range_from_utf16(&range)
      }))
      .unwrap_or(self.selected_range.into());

    self.emit_backend_action(EditorUserAction::Replace {
      range: (range.start as u64)..(range.end as u64),
      new_text: new_text.to_string(),
      marked: false,
      silent: self.silent_replace_text,
    });

    if let Some(response) = self.apply_custom_backend_edit(&range, new_text, false, window, cx) {
      if !response.accepted {
        return;
      }

      let text_len = self.text.len();
      if let Some(selection) = response.selection {
        let mut start = (selection.start as usize).min(text_len);
        let mut end = (selection.end as usize).min(text_len);
        self.selection_reversed = false;
        if end < start {
          std::mem::swap(&mut start, &mut end);
          self.selection_reversed = true;
        }
        self.selected_range = (start..end).into();
      } else {
        let new_offset = response
          .cursor
          .map(|offset| (offset as usize).min(text_len))
          .unwrap_or((range.start + new_text.len()).min(text_len));
        self.selection_reversed = false;
        self.selected_range = (new_offset..new_offset).into();
      }

      self.ime_marked_range.take();
      self.update_preferred_column();
      self.update_search(cx);
      if !self.mode.is_code_editor() {
        self.mode.update_auto_grow(&self.text_wrapper);
      }
      self.clamp_top_row(window.line_height());
      if !self.silent_replace_text {
        self.handle_completion_trigger(&range, new_text, window, cx);
      }
      cx.emit(InputEvent::Change);
      cx.notify();
      return;
    }

    let old_text = self.text.clone();
    self.text.replace(range.clone(), new_text);

    let mut new_offset = (range.start + new_text.len()).min(self.text.len());

    if !self.mode.is_code_editor() {
      let pending_text = self.text.to_string();
      // Check if the new text is valid
      if !self.is_valid_input(&pending_text, cx) {
        self.text = old_text;
        return;
      }

      if !self.mask_pattern.is_none() {
        let mask_text = self.mask_pattern.mask(&pending_text);
        self.text = Rope::from(mask_text.as_str());
        let new_text_len = (new_text.len() + mask_text.len()).saturating_sub(pending_text.len());
        new_offset = (range.start + new_text_len).min(mask_text.len());
      }
    }

    self.push_history(&old_text, &range, new_text);
    self.history.end_grouping();
    if let Some(diagnostics) = self.mode.diagnostics_mut() {
      diagnostics.reset(&self.text)
    }
    if !self.mode.is_code_editor() {
      self
        .text_wrapper
        .update(&self.text, &range, &Rope::from(new_text), window, cx);
    }
    self.lsp.update(&self.text, window, cx);
    self.selected_range = (new_offset..new_offset).into();
    self.ime_marked_range.take();
    self.update_preferred_column();
    self.update_search(cx);
    if !self.mode.is_code_editor() {
      self.mode.update_auto_grow(&self.text_wrapper);
    }
    self.clamp_top_row(window.line_height());
    if !self.silent_replace_text {
      self.handle_completion_trigger(&range, new_text, window, cx);
    }
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  /// Mark text is the IME temporary insert on typing.
  fn replace_and_mark_text_in_range(
    &mut self, range_utf16: Option<Range<usize>>, new_text: &str,
    new_selected_range_utf16: Option<Range<usize>>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.disabled {
      return;
    }

    self.lsp.reset();

    let range = range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16))
      .or(self.ime_marked_range.map(|range| {
        let range = self.range_to_utf16(&(range.start..range.end));
        self.range_from_utf16(&range)
      }))
      .unwrap_or(self.selected_range.into());

    self.emit_backend_action(EditorUserAction::Replace {
      range: (range.start as u64)..(range.end as u64),
      new_text: new_text.to_string(),
      marked: true,
      silent: self.silent_replace_text,
    });

    if let Some(response) = self.apply_custom_backend_edit(&range, new_text, true, window, cx) {
      if !response.accepted {
        return;
      }

      let text_len = self.text.len();
      if new_text.is_empty() {
        self.selected_range = (range.start.min(text_len)..range.start.min(text_len)).into();
        self.ime_marked_range = None;
      } else if let Some(selection) = response.selection {
        let mut start = (selection.start as usize).min(text_len);
        let mut end = (selection.end as usize).min(text_len);
        self.selection_reversed = false;
        if end < start {
          std::mem::swap(&mut start, &mut end);
          self.selection_reversed = true;
        }
        self.selected_range = (start..end).into();
        self.ime_marked_range = Some((start..end).into());
      } else {
        let fallback = response
          .cursor
          .map(|offset| (offset as usize).min(text_len))
          .unwrap_or((range.start + new_text.len()).min(text_len));
        let selected = new_selected_range_utf16
          .as_ref()
          .map(|range_utf16| self.range_from_utf16(range_utf16))
          .unwrap_or(fallback..fallback);
        self.selected_range = selected.into();
        self.ime_marked_range = Some((fallback..fallback).into());
      }

      if !self.mode.is_code_editor() {
        self.mode.update_auto_grow(&self.text_wrapper);
      }
      self.clamp_top_row(window.line_height());
      self.history.start_grouping();
      cx.notify();
      return;
    }

    let old_text = self.text.clone();
    self.text.replace(range.clone(), new_text);

    if !self.mode.is_code_editor() {
      let pending_text = self.text.to_string();
      if !self.is_valid_input(&pending_text, cx) {
        self.text = old_text;
        return;
      }
    }

    if let Some(diagnostics) = self.mode.diagnostics_mut() {
      diagnostics.reset(&self.text)
    }
    if !self.mode.is_code_editor() {
      self
        .text_wrapper
        .update(&self.text, &range, &Rope::from(new_text), window, cx);
    }
    self.lsp.update(&self.text, window, cx);
    if new_text.is_empty() {
      // Cancel selection, when cancel IME input.
      self.selected_range = (range.start..range.start).into();
      self.ime_marked_range = None;
    } else {
      self.ime_marked_range = Some((range.start..range.start + new_text.len()).into());
      self.selected_range = new_selected_range_utf16
        .as_ref()
        .map(|range_utf16| self.range_from_utf16(range_utf16))
        .map(|new_range| new_range.start + range.start..new_range.end + range.end)
        .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len())
        .into();
    }
    if !self.mode.is_code_editor() {
      self.mode.update_auto_grow(&self.text_wrapper);
    }
    self.clamp_top_row(window.line_height());
    self.history.start_grouping();
    self.push_history(&old_text, &range, new_text);
    cx.notify();
  }

  /// Used to position IME candidates.
  fn bounds_for_range(
    &mut self, range_utf16: Range<usize>, bounds: Bounds<Pixels>, _window: &mut Window,
    _cx: &mut Context<Self>,
  ) -> Option<Bounds<Pixels>> {
    let last_layout = self.last_layout.as_ref()?;
    let line_height = last_layout.line_height;
    let line_number_width = last_layout.line_number_width;
    let range = self.range_from_utf16(&range_utf16);

    let mut start_origin = None;
    let mut end_origin = None;
    let line_number_origin = point(line_number_width, px(0.));
    let mut y_offset = last_layout.visible_top;
    let mut index_offset = last_layout.visible_range_offset.start;

    for line in last_layout.lines.iter() {
      if start_origin.is_some() && end_origin.is_some() {
        break;
      }

      if start_origin.is_none()
        && let Some(p) =
          line.position_for_index(range.start.saturating_sub(index_offset), last_layout)
      {
        start_origin = Some(p + point(px(0.), y_offset));
      }

      if end_origin.is_none()
        && let Some(p) =
          line.position_for_index(range.end.saturating_sub(index_offset), last_layout)
      {
        end_origin = Some(p + point(px(0.), y_offset));
      }

      index_offset += line.len() + 1;
      y_offset += line.size(line_height).height;
    }

    let start_origin = start_origin.unwrap_or_default();
    let mut end_origin = end_origin.unwrap_or_default();
    // Ensure at same line.
    end_origin.y = start_origin.y;

    Some(Bounds::from_corners(
      bounds.origin + line_number_origin + start_origin,
      // + line_height for show IME panel under the cursor line.
      bounds.origin + line_number_origin + point(end_origin.x, end_origin.y + line_height),
    ))
  }

  fn character_index_for_point(
    &mut self, position: gpui::Point<Pixels>, _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<usize> {
    let last_layout = self.last_layout.as_ref()?;
    let bounds = self.last_bounds?;
    let inner_point =
      position - bounds.origin - point(last_layout.line_number_width, last_layout.visible_top);
    let mut offset = last_layout.visible_range_offset.start;
    let mut y_offset = last_layout.visible_top;

    for line in last_layout.lines.iter() {
      let local_point = inner_point - point(px(0.0), y_offset);
      if let Some(utf8_index) = line.index_for_position(local_point, last_layout) {
        return Some(self.offset_to_utf16(offset + utf8_index));
      }

      offset += line.len() + 1;
      y_offset += line.size(last_layout.line_height).height;
    }

    None
  }
}

impl InputState {
  fn drag_scrollbar_to(&mut self, position: Point<Pixels>, window: &Window) -> bool {
    let line_height = self
      .last_layout
      .as_ref()
      .map(|layout| layout.line_height)
      .unwrap_or(window.line_height());
    let viewport_rows = self.viewport_rows(line_height);
    let local_y = position.y - self.input_bounds.origin.y;
    let row = viewport::scrollbar_y_to_top_row(
      local_y,
      self.display_row_count(),
      viewport_rows,
      self.input_bounds.size.height,
    );

    self.set_top_row(row, line_height)
  }

  fn render_vertical_scrollbar(
    &self, window: &mut Window, cx: &mut Context<Self>,
  ) -> gpui::AnyElement {
    const THUMB_INSET: Pixels = px(2.0);

    let line_height = self
      .last_layout
      .as_ref()
      .map(|layout| layout.line_height)
      .unwrap_or(window.line_height());
    let total_rows = self.display_row_count();
    let viewport_rows = self.viewport_rows(line_height);
    let (thumb_y, thumb_h) = viewport::compute_scrollbar_thumb(
      self.top_row,
      viewport_rows,
      total_rows,
      self.input_bounds.size.height,
    );
    let entity = cx.entity().clone();
    let theme = cx.theme();

    let mut scrollbar = div()
      .w(viewport::VERTICAL_SCROLLBAR_WIDTH)
      .h_full()
      .flex_shrink_0()
      .bg(theme.background)
      .border_l_1()
      .border_color(theme.border)
      .relative()
      .on_mouse_down(
        MouseButton::Left,
        move |event: &MouseDownEvent, window, cx| {
          entity.update(cx, |this, cx| {
            this.scrollbar_dragging = true;
            if this.drag_scrollbar_to(event.position, window) {
              cx.notify();
            }
            cx.stop_propagation();
          });
        },
      )
      .on_mouse_move({
        let entity = cx.entity().clone();
        move |event: &MouseMoveEvent, window, cx| {
          if !event.dragging() {
            return;
          }

          entity.update(cx, |this, cx| {
            if !this.scrollbar_dragging {
              return;
            }

            if this.drag_scrollbar_to(event.position, window) {
              cx.notify();
            }
            cx.stop_propagation();
          });
        }
      });

    if total_rows > viewport_rows {
      scrollbar = scrollbar.child(
        div()
          .absolute()
          .left(THUMB_INSET)
          .right(THUMB_INSET)
          .top(thumb_y)
          .h(thumb_h)
          .bg(theme.primary.opacity(0.45))
          .rounded(px(999.0)),
      );
    }

    scrollbar.into_any_element()
  }
}

impl Focusable for InputState {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for InputState {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.install_builtin_backend_if_needed();
    self.sync_text_with_backend(false, window, cx);
    if let Some((content_width, line_height)) = self
      .last_layout
      .as_ref()
      .map(|layout| (layout.content_width, layout.line_height))
    {
      self.sync_wrap_metrics_for_view(content_width, window, cx);
      self.clamp_top_row(line_height);
    }
    self.refresh_backend_highlighter(false);

    if self._pending_update {
      self.refresh_backend_highlighter(true);
      self.lsp.update(&self.text, window, cx);
      self._pending_update = false;
    }

    window.on_mouse_event({
      let entity = cx.entity().clone();
      move |event: &MouseMoveEvent, _, window, cx| {
        entity.update(cx, |this, cx| {
          if !this.scrollbar_dragging {
            return;
          }

          if event.pressed_button != Some(MouseButton::Left) {
            this.scrollbar_dragging = false;
            return;
          }

          if this.drag_scrollbar_to(event.position, window) {
            cx.notify();
          }
          cx.stop_propagation();
        });
      }
    });
    window.on_mouse_event({
      let entity = cx.entity().clone();
      move |_event: &MouseUpEvent, _, _window, cx| {
        entity.update(cx, |this, _| {
          this.scrollbar_dragging = false;
        });
      }
    });

    div()
      .id("input-state")
      .flex_1()
      .h_full()
      .flex_grow()
      .min_w_0()
      .min_h_0()
      .overflow_hidden()
      .relative()
      .child(
        div()
          .flex()
          .size_full()
          .min_w_0()
          .min_h_0()
          .child(
            div().flex_1().min_w_0().min_h_0().child(
              ViewportElement::new(cx.entity().clone()).placeholder(self.placeholder.clone()),
            ),
          )
          .when(self.mode.is_code_editor(), |this| {
            this.child(self.render_vertical_scrollbar(window, cx))
          }),
      )
      .children(self.diagnostic_popover.clone())
      .children(self.context_menu.as_ref().map(|menu| menu.render()))
      .children(self.hover_popover.clone())
  }
}
