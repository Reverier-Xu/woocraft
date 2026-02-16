use std::{
  ops::Range,
  time::{Duration, Instant},
};

use gpui::{
  Action, Animation, AnimationExt as _, AnyElement, App, Bounds, ClipboardItem, Context, Corners,
  ElementInputHandler, Entity, EntityInputHandler, EventEmitter, FocusHandle, Focusable, Hsla,
  InteractiveElement as _, IntoElement, KeyBinding, KeyDownEvent, MouseButton, MouseDownEvent,
  MouseMoveEvent, MouseUpEvent, ParentElement as _, Pixels, Render, RenderOnce, SharedString,
  StyleRefinement, Styled, UTF16Selection, Window, actions, div, linear, point,
  prelude::FluentBuilder as _, px, relative,
};
use regex::Regex;
use serde::Deserialize;

use crate::{
  ActiveTheme as _, Button, ButtonVariants as _, Disableable, ElementExt, IconName, Selectable,
  Sizable, Size, StyleSized, StyledExt, WidgetGroup, WidgetGroupChild, h_flex,
};

const CONTEXT: &str = "Input";
const CARET_THICKNESS: Pixels = px(2.);
const CARET_BLINK_DURATION: Duration = Duration::from_millis(1200);
const CARET_STEADY_DURATION: Duration = Duration::from_millis(700);

fn caret_opacity(delta: f32) -> f32 {
  let wave = (1.0 + (delta * std::f32::consts::TAU).cos()) * 0.5;
  0.1 + wave * 0.9
}

fn shared_caret(color: Hsla, animate: bool, animation_id: &'static str) -> AnyElement {
  let caret = div().h_full().w(CARET_THICKNESS).bg(color);

  if animate {
    caret
      .with_animation(
        animation_id,
        Animation::new(CARET_BLINK_DURATION)
          .repeat()
          .with_easing(linear),
        |this, delta| this.opacity(caret_opacity(delta)),
      )
      .into_any_element()
  } else {
    caret.into_any_element()
  }
}

type InputValidate = Box<dyn Fn(&str, &mut Context<InputState>) -> bool + 'static>;

#[derive(Action, Clone, PartialEq, Eq, Deserialize)]
#[action(namespace = input, no_json)]
pub struct Enter {
  pub secondary: bool,
}

actions!(
  input,
  [
    Backspace,
    Delete,
    DeleteToPreviousWordStart,
    DeleteToNextWordEnd,
    MoveLeft,
    MoveRight,
    MoveHome,
    MoveEnd,
    SelectAll,
    SelectLeft,
    SelectRight,
    Copy,
    Cut,
    Paste,
    Undo,
    Redo,
    Escape,
    MoveToPreviousWord,
    MoveToNextWord,
  ]
);

pub fn init(cx: &mut App) {
  cx.bind_keys([
    KeyBinding::new("backspace", Backspace, Some(CONTEXT)),
    KeyBinding::new("delete", Delete, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-backspace", DeleteToPreviousWordStart, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-delete", DeleteToNextWordEnd, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-delete", DeleteToNextWordEnd, Some(CONTEXT)),
    KeyBinding::new("left", MoveLeft, Some(CONTEXT)),
    KeyBinding::new("right", MoveRight, Some(CONTEXT)),
    KeyBinding::new("home", MoveHome, Some(CONTEXT)),
    KeyBinding::new("end", MoveEnd, Some(CONTEXT)),
    KeyBinding::new("shift-left", SelectLeft, Some(CONTEXT)),
    KeyBinding::new("shift-right", SelectRight, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-left", MoveToPreviousWord, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("alt-right", MoveToNextWord, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-left", MoveToPreviousWord, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-right", MoveToNextWord, Some(CONTEXT)),
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
    KeyBinding::new("cmd-z", Undo, Some(CONTEXT)),
    #[cfg(target_os = "macos")]
    KeyBinding::new("cmd-shift-z", Redo, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-z", Undo, Some(CONTEXT)),
    #[cfg(not(target_os = "macos"))]
    KeyBinding::new("ctrl-y", Redo, Some(CONTEXT)),
    KeyBinding::new("enter", Enter { secondary: false }, Some(CONTEXT)),
    KeyBinding::new("secondary-enter", Enter { secondary: true }, Some(CONTEXT)),
    KeyBinding::new("escape", Escape, Some(CONTEXT)),
  ]);
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub struct Selection {
  pub start: usize,
  pub end: usize,
}

impl Selection {
  pub fn new(start: usize, end: usize) -> Self {
    Self { start, end }
  }

  pub fn is_empty(&self) -> bool {
    self.start == self.end
  }

  pub fn normalized(&self) -> Range<usize> {
    self.start.min(self.end)..self.start.max(self.end)
  }
}

impl From<Range<usize>> for Selection {
  fn from(value: Range<usize>) -> Self {
    Self::new(value.start, value.end)
  }
}

impl From<Selection> for Range<usize> {
  fn from(value: Selection) -> Self {
    value.start..value.end
  }
}

#[derive(Clone)]
pub enum InputEvent {
  Change,
  PressEnter { secondary: bool },
  Focus,
  Blur,
}

#[derive(Clone)]
struct Snapshot {
  text: String,
  selection: Selection,
}

pub struct InputState {
  focus_handle: FocusHandle,
  text: String,
  placeholder: SharedString,
  selected_range: Selection,
  ime_marked_range: Option<Selection>,
  selecting: bool,
  input_bounds: Bounds<Pixels>,
  horizontal_scroll: Pixels,
  caret_steady_until: Option<Instant>,
  disabled: bool,
  loading: bool,
  masked: bool,
  clean_on_escape: bool,
  size: Size,
  pattern: Option<Regex>,
  validate: Option<InputValidate>,
  undo_stack: Vec<Snapshot>,
  redo_stack: Vec<Snapshot>,
}

impl EventEmitter<InputEvent> for InputState {}

impl InputState {
  pub fn new(cx: &mut Context<Self>) -> Self {
    

    Self {
      focus_handle: cx.focus_handle().tab_stop(true),
      text: String::new(),
      placeholder: SharedString::default(),
      selected_range: Selection::default(),
      ime_marked_range: None,
      selecting: false,
      input_bounds: Bounds::default(),
      horizontal_scroll: px(0.),
      caret_steady_until: None,
      disabled: false,
      loading: false,
      masked: false,
      clean_on_escape: false,
      size: Size::default(),
      pattern: None,
      validate: None,
      undo_stack: Vec::new(),
      redo_stack: Vec::new(),
    }
  }

  fn hold_caret_visible(&mut self) {
    self.caret_steady_until = Some(Instant::now() + CARET_STEADY_DURATION);
  }

  fn should_animate_caret(&mut self) -> bool {
    match self.caret_steady_until {
      Some(until) if Instant::now() < until => false,
      Some(_) => {
        self.caret_steady_until = None;
        true
      }
      None => true,
    }
  }

  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = placeholder.into();
    self
  }

  pub fn set_placeholder(
    &mut self, placeholder: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.placeholder = placeholder.into();
    cx.notify();
  }

  pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
    self.text = value.into().to_string();
    self.selected_range = Selection::new(self.text.len(), self.text.len());
    self
  }

  pub fn set_value(
    &mut self, value: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.text = value.into().to_string();
    self.selected_range = Selection::new(self.text.len(), self.text.len());
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  pub fn value(&self) -> SharedString {
    self.text.clone().into()
  }

  pub fn unmask_value(&self) -> SharedString {
    self.value()
  }

  pub fn text(&self) -> &str {
    &self.text
  }

  pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.masked = masked;
    cx.notify();
  }

  pub fn masked(mut self, masked: bool) -> Self {
    self.masked = masked;
    self
  }

  pub fn clean_on_escape(mut self) -> Self {
    self.clean_on_escape = true;
    self
  }

  pub fn pattern(mut self, pattern: Regex) -> Self {
    self.pattern = Some(pattern);
    self
  }

  pub fn set_pattern(&mut self, pattern: Regex, _: &mut Window, _: &mut Context<Self>) {
    self.pattern = Some(pattern);
  }

  pub fn validate(mut self, f: impl Fn(&str, &mut Context<Self>) -> bool + 'static) -> Self {
    self.validate = Some(Box::new(f));
    self
  }

  pub fn set_loading(&mut self, loading: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.loading = loading;
    cx.notify();
  }

  pub fn focus(&self, window: &mut Window, cx: &mut Context<Self>) {
    let _ = cx;
    self.focus_handle.focus(window);
  }

  pub fn clean(&mut self, _: &mut Window, cx: &mut Context<Self>) {
    self.push_undo_snapshot();
    self.text.clear();
    self.selected_range = Selection::default();
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  pub fn insert(
    &mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.replace_text_in_range_silent(None, text.into().as_ref(), window, cx);
  }

  pub fn replace(
    &mut self, text: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.replace_text_in_range_silent(None, text.into().as_ref(), window, cx);
  }

  fn push_undo_snapshot(&mut self) {
    self.undo_stack.push(Snapshot {
      text: self.text.clone(),
      selection: self.selected_range,
    });
    if self.undo_stack.len() > 256 {
      self.undo_stack.remove(0);
    }
    self.redo_stack.clear();
  }

  fn restore_snapshot(&mut self, snapshot: Snapshot) {
    self.text = snapshot.text;
    self.selected_range = snapshot.selection;
  }

  fn cursor(&self) -> usize {
    self.selected_range.end.min(self.text.len())
  }

  fn byte_to_char_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())].chars().count()
  }

  fn normalize_to_char_boundary(text: &str, mut offset: usize) -> usize {
    offset = offset.min(text.len());
    while offset > 0 && !text.is_char_boundary(offset) {
      offset -= 1;
    }
    offset
  }

  fn char_to_byte_offset(text: &str, char_offset: usize) -> usize {
    if char_offset == 0 {
      return 0;
    }
    text
      .char_indices()
      .nth(char_offset)
      .map_or(text.len(), |(idx, _)| idx)
  }

  fn display_text(&self) -> String {
    if self.masked {
      "•".repeat(self.text.chars().count())
    } else {
      self.text.clone()
    }
  }

  fn shape_line_for_display(&self, display: &str, window: &Window) -> gpui::ShapedLine {
    let text_style = window.text_style();
    let font_size = text_style.font_size.to_pixels(window.rem_size());
    let runs = [text_style.to_run(display.len())];
    window
      .text_system()
      .shape_line(display.to_string().into(), font_size, &runs, None)
  }

  fn text_byte_to_display_index(&self, display: &str, text_byte_offset: usize) -> usize {
    let safe_text_byte = Self::normalize_to_char_boundary(&self.text, text_byte_offset);
    let char_offset = Self::byte_to_char_offset(&self.text, safe_text_byte);
    Self::char_to_byte_offset(display, char_offset)
  }

  fn display_index_to_text_byte(&self, display: &str, display_index: usize) -> usize {
    let safe_display_index = Self::normalize_to_char_boundary(display, display_index);
    let char_offset = Self::byte_to_char_offset(display, safe_display_index);
    Self::char_to_byte_offset(&self.text, char_offset)
  }

  fn cursor_x(&self, window: &Window) -> Pixels {
    let display = self.display_text();
    let shaped = self.shape_line_for_display(&display, window);
    let display_index = self.text_byte_to_display_index(&display, self.cursor());
    shaped.x_for_index(display_index)
  }

  fn byte_offset_for_mouse_position(
    &self, position: gpui::Point<Pixels>, window: &Window,
  ) -> usize {
    let display = self.display_text();
    if display.is_empty() {
      return 0;
    }

    let shaped = self.shape_line_for_display(&display, window);
    let bounds = self.input_bounds;

    let mut local_x = position.x - bounds.origin.x + self.horizontal_scroll;
    if local_x < px(0.) {
      local_x = px(0.);
    } else if local_x > shaped.width {
      local_x = shaped.width;
    }

    let display_index = shaped.closest_index_for_x(local_x);
    self.display_index_to_text_byte(&display, display_index)
  }

  fn ensure_cursor_visible(&mut self, window: &Window) {
    let viewport_width = self.input_bounds.size.width;
    if viewport_width <= px(0.) {
      return;
    }

    let display = self.display_text();
    let shaped = self.shape_line_for_display(&display, window);
    let cursor_display_index = self.text_byte_to_display_index(&display, self.cursor());
    let cursor_x = shaped.x_for_index(cursor_display_index);
    let horizontal_padding = px(4.);
    let left_edge = self.horizontal_scroll + horizontal_padding;
    let right_edge = self.horizontal_scroll + viewport_width - horizontal_padding;

    if cursor_x < left_edge {
      self.horizontal_scroll = (cursor_x - horizontal_padding).max(px(0.));
    } else if cursor_x > right_edge {
      self.horizontal_scroll = (cursor_x - viewport_width + horizontal_padding).max(px(0.));
    }

    let max_scroll = (shaped.width - viewport_width).max(px(0.));
    if self.horizontal_scroll > max_scroll {
      self.horizontal_scroll = max_scroll;
    }
  }

  fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    if self.disabled {
      return;
    }

    self.focus(window, cx);
    self.hold_caret_visible();
    self.selecting = true;

    let offset = self.byte_offset_for_mouse_position(event.position, window);
    if event.modifiers.shift {
      self.selected_range.end = offset;
    } else {
      self.selected_range = Selection::new(offset, offset);
    }

    self.ensure_cursor_visible(window);
    cx.notify();
  }

  fn on_mouse_move(&mut self, event: &MouseMoveEvent, window: &mut Window, cx: &mut Context<Self>) {
    if self.disabled || !self.selecting || event.pressed_button != Some(MouseButton::Left) {
      return;
    }

    let offset = self.byte_offset_for_mouse_position(event.position, window);
    self.selected_range.end = offset;
    self.hold_caret_visible();
    self.ensure_cursor_visible(window);
    cx.notify();
  }

  fn on_mouse_up(&mut self, _: &MouseUpEvent, _: &mut Window, cx: &mut Context<Self>) {
    if self.selecting {
      self.selecting = false;
      cx.notify();
    }
  }

  fn selected_range_normalized(&self) -> Range<usize> {
    self.selected_range.normalized()
  }

  fn byte_to_utf16_offset(text: &str, byte_offset: usize) -> usize {
    text[..byte_offset.min(text.len())]
      .chars()
      .map(char::len_utf16)
      .sum()
  }

  fn utf16_to_byte_offset(text: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    for (idx, ch) in text.char_indices() {
      if utf16_count >= utf16_offset {
        return idx;
      }
      utf16_count += ch.len_utf16();
      if utf16_count > utf16_offset {
        return idx;
      }
    }
    text.len()
  }

  fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
    Self::byte_to_utf16_offset(&self.text, range.start)
      ..Self::byte_to_utf16_offset(&self.text, range.end)
  }

  fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
    Self::utf16_to_byte_offset(&self.text, range_utf16.start)
      ..Self::utf16_to_byte_offset(&self.text, range_utf16.end)
  }

  fn previous_char_boundary(&self, mut offset: usize) -> usize {
    offset = offset.min(self.text.len());
    while offset > 0 && !self.text.is_char_boundary(offset) {
      offset -= 1;
    }
    if offset == 0 {
      return 0;
    }
    let mut prev = offset - 1;
    while prev > 0 && !self.text.is_char_boundary(prev) {
      prev -= 1;
    }
    prev
  }

  fn next_char_boundary(&self, mut offset: usize) -> usize {
    offset = offset.min(self.text.len());
    if offset >= self.text.len() {
      return self.text.len();
    }
    let mut next = offset + 1;
    while next < self.text.len() && !self.text.is_char_boundary(next) {
      next += 1;
    }
    next.min(self.text.len())
  }

  fn previous_word_start(&self, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = self.text.char_indices().collect();
    if chars.is_empty() {
      return 0;
    }
    let mut idx = chars
      .iter()
      .position(|(byte, _)| *byte >= offset)
      .unwrap_or(chars.len());
    idx = idx.saturating_sub(1);
    while idx > 0 && chars[idx].1.is_whitespace() {
      idx -= 1;
    }
    while idx > 0 && !chars[idx - 1].1.is_whitespace() {
      idx -= 1;
    }
    chars[idx].0
  }

  fn next_word_end(&self, offset: usize) -> usize {
    let chars: Vec<(usize, char)> = self.text.char_indices().collect();
    if chars.is_empty() {
      return 0;
    }
    let mut idx = chars
      .iter()
      .position(|(byte, _)| *byte >= offset)
      .unwrap_or(chars.len());
    while idx < chars.len() && chars[idx].1.is_whitespace() {
      idx += 1;
    }
    while idx < chars.len() && !chars[idx].1.is_whitespace() {
      idx += 1;
    }
    if idx >= chars.len() {
      self.text.len()
    } else {
      chars[idx].0
    }
  }

  fn is_valid_input(&self, new_text: &str, cx: &mut Context<Self>) -> bool {
    if let Some(pattern) = &self.pattern
      && !pattern.is_match(new_text) {
        return false;
      }

    if let Some(validate) = &self.validate {
      return validate(new_text, cx);
    }

    true
  }

  pub(crate) fn replace_text_in_range_silent(
    &mut self, range_utf16: Option<Range<usize>>, new_text: &str, _window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    if self.disabled {
      return;
    }

    let range = range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16))
      .or_else(|| self.ime_marked_range.as_ref().map(Selection::normalized))
      .unwrap_or_else(|| self.selected_range_normalized());

    let mut next_text = self.text.clone();
    if range.start <= range.end && range.end <= next_text.len() {
      next_text.replace_range(range.clone(), new_text);
    }

    if !self.is_valid_input(&next_text, cx) {
      return;
    }

    self.push_undo_snapshot();
    self.text = next_text;
    let cursor = range.start + new_text.len();
    self.selected_range = Selection::new(cursor, cursor);
    self.ime_marked_range = None;
    self.hold_caret_visible();
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  fn move_left(&mut self, _: &MoveLeft, _: &mut Window, cx: &mut Context<Self>) {
    self.ime_marked_range = None;
    let cursor = if self.selected_range.is_empty() {
      self.previous_char_boundary(self.cursor())
    } else {
      self.selected_range_normalized().start
    };
    self.selected_range = Selection::new(cursor, cursor);
    self.hold_caret_visible();
    cx.notify();
  }

  fn move_right(&mut self, _: &MoveRight, _: &mut Window, cx: &mut Context<Self>) {
    self.ime_marked_range = None;
    let cursor = if self.selected_range.is_empty() {
      self.next_char_boundary(self.cursor())
    } else {
      self.selected_range_normalized().end
    };
    self.selected_range = Selection::new(cursor, cursor);
    self.hold_caret_visible();
    cx.notify();
  }

  fn move_home(&mut self, _: &MoveHome, _: &mut Window, cx: &mut Context<Self>) {
    self.ime_marked_range = None;
    self.selected_range = Selection::new(0, 0);
    self.hold_caret_visible();
    cx.notify();
  }

  fn move_end(&mut self, _: &MoveEnd, _: &mut Window, cx: &mut Context<Self>) {
    self.ime_marked_range = None;
    let end = self.text.len();
    self.selected_range = Selection::new(end, end);
    self.hold_caret_visible();
    cx.notify();
  }

  fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
    let end = self.previous_char_boundary(self.selected_range.end);
    self.selected_range.end = end;
    cx.notify();
  }

  fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
    let end = self.next_char_boundary(self.selected_range.end);
    self.selected_range.end = end;
    cx.notify();
  }

  fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
    self.selected_range = Selection::new(0, self.text.len());
    cx.notify();
  }

  fn delete_previous_word(
    &mut self, _: &DeleteToPreviousWordStart, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range_silent(None, "", window, cx);
      return;
    }
    let cursor = self.cursor();
    let start = self.previous_word_start(cursor);
    self.replace_text_in_range_silent(Some(self.range_to_utf16(&(start..cursor))), "", window, cx);
  }

  fn delete_next_word(
    &mut self, _: &DeleteToNextWordEnd, window: &mut Window, cx: &mut Context<Self>,
  ) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range_silent(None, "", window, cx);
      return;
    }
    let cursor = self.cursor();
    let end = self.next_word_end(cursor);
    self.replace_text_in_range_silent(Some(self.range_to_utf16(&(cursor..end))), "", window, cx);
  }

  fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range_silent(None, "", window, cx);
      return;
    }
    let cursor = self.cursor();
    let start = self.previous_char_boundary(cursor);
    self.replace_text_in_range_silent(Some(self.range_to_utf16(&(start..cursor))), "", window, cx);
  }

  fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
    if !self.selected_range.is_empty() {
      self.replace_text_in_range_silent(None, "", window, cx);
      return;
    }
    let cursor = self.cursor();
    let end = self.next_char_boundary(cursor);
    self.replace_text_in_range_silent(Some(self.range_to_utf16(&(cursor..end))), "", window, cx);
  }

  fn move_to_previous_word(
    &mut self, _: &MoveToPreviousWord, _: &mut Window, cx: &mut Context<Self>,
  ) {
    let offset = self.previous_word_start(self.cursor());
    self.selected_range = Selection::new(offset, offset);
    self.hold_caret_visible();
    cx.notify();
  }

  fn move_to_next_word(&mut self, _: &MoveToNextWord, _: &mut Window, cx: &mut Context<Self>) {
    let offset = self.next_word_end(self.cursor());
    self.selected_range = Selection::new(offset, offset);
    self.hold_caret_visible();
    cx.notify();
  }

  fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
    let range = self.selected_range_normalized();
    if range.is_empty() {
      return;
    }
    cx.write_to_clipboard(ClipboardItem::new_string(self.text[range].to_string()));
  }

  fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
    let range = self.selected_range_normalized();
    if range.is_empty() {
      return;
    }
    cx.write_to_clipboard(ClipboardItem::new_string(
      self.text[range.clone()].to_string(),
    ));
    self.replace_text_in_range_silent(Some(self.range_to_utf16(&range)), "", window, cx);
  }

  fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
    if let Some(clipboard) = cx.read_from_clipboard() {
      let text = clipboard.text().unwrap_or_default().replace('\n', "");
      self.replace_text_in_range_silent(None, &text, window, cx);
    }
  }

  fn undo(&mut self, _: &Undo, _: &mut Window, cx: &mut Context<Self>) {
    if let Some(snapshot) = self.undo_stack.pop() {
      self.redo_stack.push(Snapshot {
        text: self.text.clone(),
        selection: self.selected_range,
      });
      self.restore_snapshot(snapshot);
      cx.emit(InputEvent::Change);
      cx.notify();
    }
  }

  fn redo(&mut self, _: &Redo, _: &mut Window, cx: &mut Context<Self>) {
    if let Some(snapshot) = self.redo_stack.pop() {
      self.undo_stack.push(Snapshot {
        text: self.text.clone(),
        selection: self.selected_range,
      });
      self.restore_snapshot(snapshot);
      cx.emit(InputEvent::Change);
      cx.notify();
    }
  }

  fn enter(&mut self, action: &Enter, _: &mut Window, cx: &mut Context<Self>) {
    cx.emit(InputEvent::PressEnter {
      secondary: action.secondary,
    });
  }

  fn escape(&mut self, _: &Escape, window: &mut Window, cx: &mut Context<Self>) {
    if self.clean_on_escape {
      self.clean(window, cx);
      return;
    }
    cx.propagate();
  }

  fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    let _ = event;
    let _ = window;
    if self.disabled {
      return;
    }

    self.hold_caret_visible();
    cx.notify();
  }
}

pub struct OtpState {
  focus_handle: FocusHandle,
  value: SharedString,
  caret_steady_until: Option<Instant>,
  masked: bool,
  length: usize,
  disabled: bool,
}

impl OtpState {
  pub fn new(length: usize, cx: &mut Context<Self>) -> Self {
    let focus_handle = cx.focus_handle();

    Self {
      focus_handle,
      value: SharedString::default(),
      caret_steady_until: None,
      masked: false,
      length: length.max(1),
      disabled: false,
    }
  }

  pub fn default_value(mut self, value: impl Into<SharedString>) -> Self {
    self.value = value.into();
    self
  }

  pub fn set_value(
    &mut self, value: impl Into<SharedString>, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.value = value.into();
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  pub fn value(&self) -> &SharedString {
    &self.value
  }

  pub fn masked(mut self, masked: bool) -> Self {
    self.masked = masked;
    self
  }

  pub fn set_masked(&mut self, masked: bool, _: &mut Window, cx: &mut Context<Self>) {
    self.masked = masked;
    cx.notify();
  }

  pub fn focus(&self, window: &mut Window, _: &mut Context<Self>) {
    self.focus_handle.focus(window);
  }

  fn hold_caret_visible(&mut self) {
    self.caret_steady_until = Some(Instant::now() + CARET_STEADY_DURATION);
  }

  fn should_animate_caret(&self) -> bool {
    self
      .caret_steady_until
      .is_none_or(|until| Instant::now() >= until)
  }

  fn on_input_mouse_down(
    &mut self, _: &gpui::MouseDownEvent, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.hold_caret_visible();
    self.focus(window, cx);
  }

  fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
    if self.disabled {
      return;
    }

    let mut chars: Vec<char> = self.value.chars().collect();
    let key = event.keystroke.key.as_str();

    match key {
      "backspace" => {
        if !chars.is_empty() {
          chars.pop();
          self.value = chars.iter().collect::<String>().into();
          self.hold_caret_visible();
          cx.emit(InputEvent::Change);
          cx.notify();
          window.prevent_default();
          cx.stop_propagation();
        }
      }
      _ => {
        if let Some(ch) = key.chars().next()
          && ch.is_ascii_digit() && chars.len() < self.length {
            chars.push(ch);
            self.value = chars.iter().collect::<String>().into();
            self.hold_caret_visible();
            cx.emit(InputEvent::Change);
            cx.notify();
            window.prevent_default();
            cx.stop_propagation();
          }
      }
    }
  }
}

impl EventEmitter<InputEvent> for OtpState {}

impl Focusable for OtpState {
  fn focus_handle(&self, _: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for OtpState {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
  }
}

#[derive(IntoElement)]
pub struct OtpInput {
  state: Entity<OtpState>,
  number_of_groups: usize,
  size: Size,
  disabled: bool,
}

impl OtpInput {
  pub fn new(state: &Entity<OtpState>) -> Self {
    Self {
      state: state.clone(),
      number_of_groups: 2,
      size: Size::default(),
      disabled: false,
    }
  }

  pub fn groups(mut self, n: usize) -> Self {
    self.number_of_groups = n.max(1);
    self
  }
}

impl Disableable for OtpInput {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Sizable for OtpInput {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl RenderOnce for OtpInput {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state = self.state.read(cx);
    let is_focused = state.focus_handle.is_focused(window);
    let animate_caret = state.should_animate_caret();
    let caret_color = cx.theme().primary;
    let value_chars = state.value.chars().collect::<Vec<_>>();
    let cursor_ix = value_chars.len().min(state.length.saturating_sub(1));
    let group_count = self.number_of_groups.max(1).min(state.length);
    let base_group_size = state.length / group_count;
    let extra = state.length % group_count;

    let mut groups: Vec<Vec<AnyElement>> = (0..group_count).map(|_| Vec::new()).collect();
    let mut group_ix = 0usize;
    let mut in_group_ix = 0usize;
    let mut current_group_cap = base_group_size + usize::from(extra > 0);

    for ix in 0..state.length {
      if in_group_ix >= current_group_cap && group_ix + 1 < group_count {
        group_ix += 1;
        in_group_ix = 0;
        current_group_cap = base_group_size + usize::from(group_ix < extra);
      }

      let ch = value_chars.get(ix).copied();
      let focused_cell = is_focused && ix == cursor_ix;

      groups[group_ix].push(
        h_flex()
          .border_1()
          .border_color(if focused_cell {
            cx.theme().ring
          } else {
            cx.theme().input
          })
          .bg(if self.disabled {
            cx.theme().muted
          } else {
            cx.theme().background
          })
          .rounded(cx.theme().radius)
          .items_center()
          .justify_center()
          .text_color(if self.disabled {
            cx.theme().muted_foreground
          } else {
            cx.theme().foreground
          })
          .input_h(self.size)
          .w(self.size.component_height())
          .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&self.state, OtpState::on_input_mouse_down),
          )
          .child(match ch {
            Some(c) => {
              if state.masked {
                "•".to_string().into_any_element()
              } else {
                c.to_string().into_any_element()
              }
            }
            None => {
              if focused_cell {
                div()
                  .h_4()
                  .child(shared_caret(caret_color, animate_caret, "otp-caret-blink"))
                  .into_any_element()
              } else {
                div().into_any_element()
              }
            }
          })
          .into_any_element(),
      );

      in_group_ix += 1;
    }

    h_flex()
      .id(("otp-input", self.state.entity_id()))
      .track_focus(&state.focus_handle)
      .when(!self.disabled, |this| {
        this.on_key_down(window.listener_for(&self.state, OtpState::on_key_down))
      })
      .items_center()
      .gap_4()
      .children(
        groups
          .into_iter()
          .map(|cells| h_flex().items_center().gap_1().children(cells)),
      )
  }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StepAction {
  Decrement,
  Increment,
}

pub enum NumberInputEvent {
  Step(StepAction),
}

impl EventEmitter<NumberInputEvent> for InputState {}

#[derive(IntoElement)]
pub struct NumberInput {
  state: Entity<InputState>,
  placeholder: SharedString,
  size: Size,
  appearance: bool,
  disabled: bool,
  style: StyleRefinement,
  step: f64,
  min: Option<f64>,
  max: Option<f64>,
}

impl NumberInput {
  pub fn new(state: &Entity<InputState>) -> Self {
    Self {
      state: state.clone(),
      placeholder: SharedString::default(),
      size: Size::default(),
      appearance: true,
      disabled: false,
      style: StyleRefinement::default(),
      step: 1.0,
      min: None,
      max: None,
    }
  }

  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = placeholder.into();
    self
  }

  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  pub fn step(mut self, step: f64) -> Self {
    self.step = step.max(0.000_001);
    self
  }

  pub fn min(mut self, min: f64) -> Self {
    self.min = Some(min);
    self
  }

  pub fn max(mut self, max: f64) -> Self {
    self.max = Some(max);
    self
  }

  fn format_number(v: f64) -> String {
    let rounded = (v * 1_000_000.0).round() / 1_000_000.0;
    let mut s = rounded.to_string();
    if s.contains('.') {
      while s.ends_with('0') {
        s.pop();
      }
      if s.ends_with('.') {
        s.pop();
      }
    }
    s
  }

  fn apply_step(
    state: &Entity<InputState>, action: StepAction, step: f64, min: Option<f64>, max: Option<f64>,
    window: &mut Window, cx: &mut App,
  ) {
    state.update(cx, |state, cx| {
      if state.disabled {
        return;
      }
      let current = state.value().as_ref().parse::<f64>().unwrap_or(0.0);
      let mut next = match action {
        StepAction::Increment => current + step,
        StepAction::Decrement => current - step,
      };
      if let Some(min) = min {
        next = next.max(min);
      }
      if let Some(max) = max {
        next = next.min(max);
      }
      state.set_value(Self::format_number(next), window, cx);
      state.focus(window, cx);
      cx.emit(NumberInputEvent::Step(action));
    });
  }
}

impl Disableable for NumberInput {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl Sizable for NumberInput {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Styled for NumberInput {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl RenderOnce for NumberInput {
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    self.state.update(cx, |state, _cx| {
      if !self.placeholder.is_empty() {
        state.placeholder = self.placeholder.clone();
      }
    });

    WidgetGroup::new(("number-input", self.state.entity_id()))
      .with_size(self.size)
      .disabled(self.disabled)
      .refine_style(&self.style)
      .children([
        WidgetGroupChild::from(
          Button::new(("minus", self.state.entity_id()))
            .with_size(self.size)
            .default()
            .icon(IconName::Subtract)
            .tab_stop(false)
            .disabled(self.disabled)
            .on_click({
              let state = self.state.clone();
              let step = self.step;
              let min = self.min;
              let max = self.max;
              move |_, window, cx| {
                Self::apply_step(&state, StepAction::Decrement, step, min, max, window, cx);
              }
            }),
        ),
        WidgetGroupChild::from(
          Input::new(&self.state)
            .appearance(self.appearance)
            .with_size(self.size)
            .disabled(self.disabled),
        ),
        WidgetGroupChild::from(
          Button::new(("plus", self.state.entity_id()))
            .with_size(self.size)
            .default()
            .icon(IconName::Add)
            .tab_stop(false)
            .disabled(self.disabled)
            .on_click({
              let state = self.state.clone();
              let step = self.step;
              let min = self.min;
              let max = self.max;
              move |_, window, cx| {
                Self::apply_step(&state, StepAction::Increment, step, min, max, window, cx);
              }
            }),
        ),
      ])
  }
}

impl EntityInputHandler for InputState {
  fn text_for_range(
    &mut self, range_utf16: Range<usize>, adjusted_range: &mut Option<Range<usize>>,
    _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<String> {
    let range = self.range_from_utf16(&range_utf16);
    adjusted_range.replace(self.range_to_utf16(&range));
    Some(self.text[range].to_string())
  }

  fn selected_text_range(
    &mut self, _ignore_disabled_input: bool, _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<UTF16Selection> {
    Some(UTF16Selection {
      range: self.range_to_utf16(&self.selected_range_normalized()),
      reversed: self.selected_range.start > self.selected_range.end,
    })
  }

  fn marked_text_range(
    &self, _window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<Range<usize>> {
    self
      .ime_marked_range
      .as_ref()
      .map(|range| self.range_to_utf16(&range.normalized()))
  }

  fn unmark_text(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if self.ime_marked_range.take().is_some() {
      cx.notify();
    }
  }

  fn replace_text_in_range(
    &mut self, range_utf16: Option<Range<usize>>, new_text: &str, window: &mut Window,
    cx: &mut Context<Self>,
  ) {
    self.replace_text_in_range_silent(range_utf16, new_text, window, cx);
  }

  fn replace_and_mark_text_in_range(
    &mut self, range_utf16: Option<Range<usize>>, new_text: &str,
    new_selected_range_utf16: Option<Range<usize>>, _window: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.disabled {
      return;
    }

    let range = range_utf16
      .as_ref()
      .map(|range_utf16| self.range_from_utf16(range_utf16))
      .or_else(|| self.ime_marked_range.as_ref().map(Selection::normalized))
      .unwrap_or_else(|| self.selected_range_normalized());

    let mut next_text = self.text.clone();
    if range.start <= range.end && range.end <= next_text.len() {
      next_text.replace_range(range.clone(), new_text);
    }

    if !self.is_valid_input(&next_text, cx) {
      return;
    }

    self.text = next_text;
    if new_text.is_empty() {
      self.ime_marked_range = None;
      self.selected_range = Selection::new(range.start, range.start);
    } else {
      let marked_range = range.start..(range.start + new_text.len()).min(self.text.len());
      self.ime_marked_range = Some(marked_range.clone().into());
      self.selected_range = new_selected_range_utf16
        .as_ref()
        .map(|selection_utf16| self.range_from_utf16(selection_utf16))
        .map(|selection| {
          let start = (range.start + selection.start).min(self.text.len());
          let end = (range.start + selection.end).min(self.text.len());
          Selection::new(start, end)
        })
        .unwrap_or_else(|| Selection::new(marked_range.end, marked_range.end));
    }

    self.hold_caret_visible();
    cx.emit(InputEvent::Change);
    cx.notify();
  }

  fn bounds_for_range(
    &mut self, range_utf16: Range<usize>, bounds: gpui::Bounds<gpui::Pixels>, window: &mut Window,
    _cx: &mut Context<Self>,
  ) -> Option<gpui::Bounds<gpui::Pixels>> {
    if bounds.size.width <= px(0.) {
      return Some(bounds);
    }

    let display = self.display_text();
    let shaped = self.shape_line_for_display(&display, window);
    let range = self.range_from_utf16(&range_utf16);
    let start_ix = self.text_byte_to_display_index(&display, range.start);
    let end_ix = self.text_byte_to_display_index(&display, range.end.max(range.start));

    let start_x = (shaped.x_for_index(start_ix) - self.horizontal_scroll).max(px(0.));
    let end_x = (shaped.x_for_index(end_ix) - self.horizontal_scroll)
      .max(start_x + px(1.))
      .min(bounds.size.width);

    let line_height = window.line_height().max(px(1.));
    let top = bounds.origin.y + ((bounds.size.height - line_height) / 2.);
    let left = bounds.origin.x + start_x;
    let right = bounds.origin.x + end_x;

    Some(Bounds::from_corners(
      point(left, top),
      point(right, top + line_height),
    ))
  }

  fn character_index_for_point(
    &mut self, point: gpui::Point<gpui::Pixels>, window: &mut Window, _cx: &mut Context<Self>,
  ) -> Option<usize> {
    let byte = self.byte_offset_for_mouse_position(point, window);
    Some(Self::byte_to_utf16_offset(&self.text, byte))
  }
}

impl Focusable for InputState {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for InputState {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    self.ensure_cursor_visible(window);

    let cursor = self.cursor();
    let is_focused = self.focus_handle.is_focused(window) && !self.disabled;
    let selection = self.selected_range_normalized();
    let has_selection = !selection.is_empty();
    let display = if self.masked {
      "•".repeat(self.text.chars().count())
    } else {
      self.text.clone()
    };

    let text_color = window.text_style().color;
    let caret_color = cx.theme().primary;
    let selection_color = cx.theme().primary.opacity(0.25);

    let cursor_char = Self::byte_to_char_offset(&self.text, cursor);
    let selection_start_char = Self::byte_to_char_offset(&self.text, selection.start);
    let selection_end_char = Self::byte_to_char_offset(&self.text, selection.end);

    let left_byte = Self::char_to_byte_offset(&display, selection_start_char);
    let selected_byte = Self::char_to_byte_offset(&display, selection_end_char);
    let cursor_byte = Self::char_to_byte_offset(&display, cursor_char);

    let left_text = &display[..left_byte.min(display.len())];
    let selected_text = &display[left_byte.min(display.len())..selected_byte.min(display.len())];
    let right_text = &display[selected_byte.min(display.len())..];

    let caret_left = (self.cursor_x(window) - self.horizontal_scroll).max(px(0.));
    let show_caret = is_focused && !has_selection;
    let animate_caret = show_caret && self.should_animate_caret();

    h_flex()
      .id("input-state")
      .relative()
      .flex_1()
      .h_full()
      .overflow_x_hidden()
      .items_center()
      .text_color(text_color)
      .child(
        h_flex()
          .id("input-content")
          .items_center()
          .ml(-self.horizontal_scroll)
          .child(if has_selection {
            left_text.to_string()
          } else {
            display[..cursor_byte.min(display.len())].to_string()
          })
          .when(has_selection, |this| {
            this
              .child(div().bg(selection_color).child(selected_text.to_string()))
              .child(right_text.to_string())
          })
          .when(!has_selection, |this| {
            this.child(display[cursor_byte.min(display.len())..].to_string())
          }),
      )
      .when(show_caret, |this| {
        this.child(
          div()
            .absolute()
            .left(caret_left)
            .top_0p5()
            .bottom_0p5()
            .child(shared_caret(
              caret_color,
              animate_caret,
              "input-caret-blink",
            )),
        )
      })
  }
}

#[derive(IntoElement)]
pub struct Input {
  state: Entity<InputState>,
  style: StyleRefinement,
  size: Size,
  border_corners: Corners<bool>,
  appearance: bool,
  cleanable: bool,
  mask_toggle: bool,
  disabled: bool,
  bordered: bool,
  focus_bordered: bool,
  tab_index: isize,
  selected: bool,
}

impl Input {
  pub fn new(state: &Entity<InputState>) -> Self {
    Self {
      state: state.clone(),
      style: StyleRefinement::default(),
      size: Size::default(),
      border_corners: Corners::all(true),
      appearance: true,
      cleanable: false,
      mask_toggle: false,
      disabled: false,
      bordered: true,
      focus_bordered: true,
      tab_index: 0,
      selected: false,
    }
  }

  pub fn border_corners(mut self, corners: impl Into<Corners<bool>>) -> Self {
    self.border_corners = corners.into();
    self
  }

  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  pub fn bordered(mut self, bordered: bool) -> Self {
    self.bordered = bordered;
    self
  }

  pub fn focus_bordered(mut self, bordered: bool) -> Self {
    self.focus_bordered = bordered;
    self
  }

  pub fn cleanable(mut self, cleanable: bool) -> Self {
    self.cleanable = cleanable;
    self
  }

  pub fn mask_toggle(mut self) -> Self {
    self.mask_toggle = true;
    self
  }

  pub fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }

  pub fn tab_index(mut self, index: isize) -> Self {
    self.tab_index = index;
    self
  }

  pub(crate) fn is_active(&self, window: &Window, cx: &App) -> bool {
    let state = self.state.read(cx);
    self.selected || (state.focus_handle.is_focused(window) && !state.disabled)
  }

  fn render_toggle_mask_button(state: Entity<InputState>) -> impl IntoElement {
    Button::new("toggle-mask")
      .icon(IconName::Eye)
      .small()
      .flat()
      .tab_stop(false)
      .on_click({
        let state = state.clone();
        move |_, window, cx| {
          state.update(cx, |state, cx| {
            let next = !state.masked;
            state.set_masked(next, window, cx);
          })
        }
      })
  }
}

impl Sizable for Input {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Selectable for Input {
  fn selected(mut self, selected: bool) -> Self {
    self.selected = selected;
    self
  }

  fn is_selected(&self) -> bool {
    self.selected
  }
}

impl Styled for Input {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

#[inline]
fn clear_button(cx: &App) -> Button {
  Button::new("clean")
    .icon(IconName::DismissCircle)
    .flat()
    .small()
    .tab_stop(false)
    .text_color(cx.theme().muted_foreground)
}

impl RenderOnce for Input {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let state_view = self.state.read(cx);
    let focused = state_view.focus_handle.is_focused(window) && !state_view.disabled;
    let gap_x = match self.size {
      Size::Small => px(4.),
      Size::Large => px(8.),
      _ => px(6.),
    };

    let bg = if state_view.disabled {
      cx.theme().muted
    } else {
      cx.theme().background
    };

    let show_clear_button =
      self.cleanable && !state_view.disabled && !state_view.loading && !state_view.text.is_empty();
    let has_suffix = state_view.loading || self.mask_toggle || show_clear_button;
    let placeholder = state_view.placeholder.clone();
    let show_placeholder = state_view.text.is_empty() && !focused;
    let state_focus_handle = state_view.focus_handle.clone();
    let _ = state_view;

    self.state.update(cx, |state, _| {
      state.disabled = self.disabled;
      state.size = self.size;
    });

    div()
      .id(("input", self.state.entity_id()))
      .flex()
      .items_center()
      .line_height(relative(1.25))
      .key_context(CONTEXT)
      .track_focus(&state_focus_handle)
      .tab_index(self.tab_index)
      .on_action(window.listener_for(&self.state, InputState::backspace))
      .on_action(window.listener_for(&self.state, InputState::delete))
      .on_action(window.listener_for(&self.state, InputState::delete_previous_word))
      .on_action(window.listener_for(&self.state, InputState::delete_next_word))
      .on_action(window.listener_for(&self.state, InputState::move_left))
      .on_action(window.listener_for(&self.state, InputState::move_right))
      .on_action(window.listener_for(&self.state, InputState::move_home))
      .on_action(window.listener_for(&self.state, InputState::move_end))
      .on_action(window.listener_for(&self.state, InputState::select_left))
      .on_action(window.listener_for(&self.state, InputState::select_right))
      .on_action(window.listener_for(&self.state, InputState::select_all))
      .on_action(window.listener_for(&self.state, InputState::move_to_previous_word))
      .on_action(window.listener_for(&self.state, InputState::move_to_next_word))
      .on_action(window.listener_for(&self.state, InputState::copy))
      .on_action(window.listener_for(&self.state, InputState::cut))
      .on_action(window.listener_for(&self.state, InputState::paste))
      .on_action(window.listener_for(&self.state, InputState::undo))
      .on_action(window.listener_for(&self.state, InputState::redo))
      .on_action(window.listener_for(&self.state, InputState::enter))
      .on_action(window.listener_for(&self.state, InputState::escape))
      .on_key_down(window.listener_for(&self.state, InputState::on_key_down))
      .on_mouse_down(
        MouseButton::Left,
        window.listener_for(&self.state, InputState::on_mouse_down),
      )
      .on_mouse_move(window.listener_for(&self.state, InputState::on_mouse_move))
      .on_mouse_up(
        MouseButton::Left,
        window.listener_for(&self.state, InputState::on_mouse_up),
      )
      .size_full()
      .input_size(self.size)
      .when(has_suffix, |this| this.pr_0())
      .cursor_text()
      .gap(gap_x)
      .when(self.appearance, |this| {
        this
          .bg(bg)
          .corner_radius(Corners {
            top_left: if self.border_corners.top_left {
              cx.theme().radius
            } else {
              px(0.)
            },
            top_right: if self.border_corners.top_right {
              cx.theme().radius
            } else {
              px(0.)
            },
            bottom_left: if self.border_corners.bottom_left {
              cx.theme().radius
            } else {
              px(0.)
            },
            bottom_right: if self.border_corners.bottom_right {
              cx.theme().radius
            } else {
              px(0.)
            },
          })
          .when(self.bordered, |this| {
            this
              .border_color(cx.theme().input)
              .border_1()
              .when(focused && self.focus_bordered, |this| {
                this.border_color(cx.theme().ring)
              })
          })
      })
      .refine_style(&self.style)
      .child(
        h_flex()
          .id("input-main")
          .relative()
          .flex_1()
          .h_full()
          .min_w_0()
          .items_center()
          .overflow_x_hidden()
          .on_prepaint({
            let state = self.state.clone();
            move |bounds, window, cx| {
              let focus_handle = state.read(cx).focus_handle.clone();
              window.handle_input(
                &focus_handle,
                ElementInputHandler::new(bounds, state.clone()),
                cx,
              );

              window.on_mouse_event({
                let state = state.clone();
                move |event: &MouseMoveEvent, _, window, cx| {
                  if event.pressed_button != Some(MouseButton::Left) {
                    return;
                  }

                  let (should_track_drag, outside_input_bounds) = {
                    let input_state = state.read(cx);
                    (
                      input_state.selecting && input_state.focus_handle.is_focused(window),
                      !input_state.input_bounds.contains(&event.position),
                    )
                  };

                  if should_track_drag && outside_input_bounds {
                    state.update(cx, |state, cx| {
                      state.on_mouse_move(event, window, cx);
                    });
                  }
                }
              });

              window.on_mouse_event({
                let state = state.clone();
                move |event: &MouseUpEvent, _, window, cx| {
                  if state.read(cx).selecting {
                    state.update(cx, |state, cx| {
                      state.on_mouse_up(event, window, cx);
                    });
                  }
                }
              });

              state.update(cx, |state, cx| {
                if state.input_bounds != bounds {
                  state.input_bounds = bounds;
                  state.ensure_cursor_visible(window);
                  cx.notify();
                }
              });
            }
          })
          .child(self.state.clone())
          .when(show_placeholder, |this| {
            this.child(
              div()
                .absolute()
                .left_0()
                .right_0()
                .top_0()
                .bottom_0()
                .flex()
                .items_center()
                .text_color(cx.theme().muted_foreground)
                .child(placeholder),
            )
          }),
      )
      .when(has_suffix, |this| {
        this.pr(self.size.input_px() / 2.).child(
          h_flex()
            .id("suffix")
            .gap(gap_x)
            .when(self.appearance, |this| this.bg(bg))
            .items_center()
            .when_some(self.state.read(cx).loading.then_some(()), |this, _| {
              this.child(crate::Spinner::new().color(cx.theme().muted_foreground))
            })
            .when(self.mask_toggle, |this| {
              this.child(Self::render_toggle_mask_button(self.state.clone()))
            })
            .when(show_clear_button, |this| {
              this.child(clear_button(cx).on_click({
                let state = self.state.clone();
                move |_, window, cx| {
                  state.update(cx, |state, cx| {
                    state.clean(window, cx);
                    state.focus(window, cx);
                  })
                }
              }))
            }),
        )
      })
  }
}
