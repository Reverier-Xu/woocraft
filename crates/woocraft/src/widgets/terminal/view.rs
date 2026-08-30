//! The terminal view entity: hosts a [`TerminalSession`] and drives it with
//! GPUI-native tasks.

use std::time::Duration;

use gpui::{
  App, ClipboardItem, Context, EventEmitter, FocusHandle, Focusable, InteractiveElement as _,
  IntoElement, KeyDownEvent, ParentElement as _, Pixels, Render, SharedString, Styled as _, Window,
  div, px,
};
use woocraft_terminal::{
  ChildStatus, Modes, Point as GridPoint, ScrollKind, SelectionKind, TerminalEvent, TerminalSession,
};

use super::{
  colors::TerminalPalette,
  element::TerminalElement,
  input::to_esc_str,
  mouse::{
    alt_scroll, grid_point, grid_point_and_side, mouse_button_report, mouse_moved_report,
    scroll_reports,
  },
};
use crate::ActiveTheme as _;

/// The key context used for terminal keybindings.
pub const CONTEXT: &str = "TerminalView";

/// Interval between cursor blink toggles while blinking is active.
const CURSOR_BLINK_INTERVAL: Duration = Duration::from_millis(500);

/// Window during which additional session events are coalesced before
/// repainting, mirroring Zed's event pump.
const EVENT_BATCH_INTERVAL: Duration = Duration::from_millis(4);

/// Minimum pointer movement before a click becomes a selection drag.
const SELECTION_DRAG_THRESHOLD: f64 = 2.0;

/// The phase of an in-progress mouse selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum SelectionPhase {
  #[default]
  Ended,
  Selecting,
}

/// Events emitted by the terminal view to its host.
#[derive(Debug, Clone, PartialEq)]
pub enum TerminalViewEvent {
  /// The application changed (or reset) its title.
  TitleChanged(Option<String>),
  /// The terminal bell was rung.
  Bell,
  /// The application stored text into the clipboard (OSC 52).
  ClipboardStored(String),
  /// The child process exited. The status is `None` when the session was
  /// shut down before reporting one.
  Exit(Option<ChildStatus>),
}

/// A GPUI view rendering a terminal session.
pub struct TerminalView {
  session: TerminalSession,
  focus: FocusHandle,
  /// The latest content snapshot, refreshed during element prepaint. Shared
  /// via `Arc` so the element can build this frame's layout from the same
  /// copy stored on the view without a second full-grid clone.
  pub(crate) content: std::sync::Arc<woocraft_terminal::Content>,
  title: Option<String>,
  pub(crate) marked_text: Option<String>,
  /// Whether the application requested a blinking cursor.
  blinking_terminal_enabled: bool,
  cursor_visible: bool,
  blink_generation: u64,
  pub(crate) font_family: Option<SharedString>,
  pub(crate) font_size: Option<Pixels>,
  /// Keeps the focus subscriptions alive for as long as the view lives.
  /// Keeps the focus subscriptions alive for as long as the view lives.
  #[allow(dead_code)]
  focus_subscriptions: Vec<gpui::Subscription>,
  // Selection interaction state. The anchor is fixed when the mouse goes
  // down; dragging moves the head, exactly like Zed's Selection model.
  selection_phase: SelectionPhase,
  selection_anchor: Option<(GridPoint, SelectionKind)>,
  /// Last selection head seen, so drag moves that didn't change the
  /// selection neither re-select nor invalidate the frame.
  selection_head: Option<GridPoint>,
  mouse_down_position: Option<gpui::Point<Pixels>>,
  last_mouse: Option<(GridPoint, bool)>,
  scroll_px: Pixels,
}

impl EventEmitter<TerminalViewEvent> for TerminalView {}
impl Focusable for TerminalView {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus.clone()
  }
}

impl TerminalView {
  /// Creates a terminal view for the given session and starts its event pump.
  pub fn new(session: TerminalSession, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let focus = cx.focus_handle();
    let focus_in = cx.on_focus_in(&focus, window, Self::focus_in);
    let focus_out = cx.on_focus_out(&focus, window, Self::focus_out);

    let events = session.events();
    cx.spawn(async move |this, cx| {
      // Process the first event immediately for low latency, then coalesce
      // everything that arrives during the batch window before notifying.
      loop {
        let Ok(event) = events.recv().await else {
          return;
        };
        let mut exited = false;
        let Ok(()) = this.update(cx, |view, cx| exited = view.process_event(event, cx)) else {
          return;
        };
        if exited {
          return;
        }

        cx.background_executor().timer(EVENT_BATCH_INTERVAL).await;

        let mut batch = Vec::new();
        loop {
          match events.try_recv() {
            Ok(event) => batch.push(event),
            Err(async_channel::TryRecvError::Empty) => break,
            Err(async_channel::TryRecvError::Closed) => return,
          }
        }
        let mut exited = false;
        let Ok(()) = this.update(cx, |view, cx| {
          for event in batch {
            exited |= view.process_event(event, cx);
          }
        }) else {
          return;
        };
        if exited {
          return;
        }
      }
    })
    .detach();

    Self {
      session,
      focus,
      content: std::sync::Arc::new(woocraft_terminal::Content::empty()),
      title: None,
      marked_text: None,
      blinking_terminal_enabled: false,
      cursor_visible: true,
      blink_generation: 0,
      font_family: None,
      font_size: None,
      selection_phase: SelectionPhase::Ended,
      selection_anchor: None,
      selection_head: None,
      mouse_down_position: None,
      last_mouse: None,
      scroll_px: px(0.),
      focus_subscriptions: vec![focus_in, focus_out],
    }
  }

  /// The wrapped session.
  pub fn session(&self) -> &TerminalSession {
    &self.session
  }

  /// The application title, if it set one.
  pub fn title(&self) -> Option<&str> {
    self.title.as_deref()
  }

  /// Overrides the monospace font family used for rendering.
  pub fn set_font_family(
    &mut self, family: Option<impl Into<SharedString>>, cx: &mut Context<Self>,
  ) {
    self.font_family = family.map(|family| family.into());
    cx.notify();
  }

  /// Overrides the terminal font size.
  pub fn set_font_size(&mut self, size: Option<Pixels>, cx: &mut Context<Self>) {
    self.font_size = size;
    cx.notify();
  }

  /// Copies the current selection; returns the copied text.
  pub fn copy(&mut self, cx: &mut Context<Self>) -> Option<String> {
    let text = self.session.copy_selection()?;
    cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
    Some(text)
  }

  /// Pastes text into the terminal, honoring bracketed-paste mode.
  pub fn paste(&mut self, text: &str, cx: &mut Context<Self>) {
    self.session.paste(text);
    cx.notify();
  }

  /// Clears the screen and scrollback.
  pub fn clear(&mut self, cx: &mut Context<Self>) {
    self.session.clear();
    cx.notify();
  }

  /// Scrolls the terminal display.
  pub fn scroll(&mut self, scroll: ScrollKind, cx: &mut Context<Self>) {
    self.session.scroll(scroll);
    cx.notify();
  }

  /// Selects the entire terminal content, including scrollback.
  pub fn select_all(&mut self, cx: &mut Context<Self>) {
    let content = &self.content;
    let topmost =
      content.display_offset as i32 - (content.total_lines - content.screen_lines) as i32;
    let end = GridPoint::new(
      content.screen_lines as i32 - 1 - content.display_offset as i32,
      content.columns.saturating_sub(1),
    );
    self
      .session
      .select(GridPoint::new(topmost, 0), end, SelectionKind::Characters);
    cx.notify();
  }

  /// Whether the terminal is currently in the alternate screen.
  pub fn is_alt_screen(&self) -> bool {
    self.content.mode.contains(Modes::ALT_SCREEN)
  }

  /// Whether a mouse selection drag is in progress.
  pub(crate) fn selection_started(&self) -> bool {
    self.selection_phase == SelectionPhase::Selecting
  }

  /// Whether mouse events are currently reported to the application.
  pub(crate) fn mouse_mode_enabled(&self) -> bool {
    self.content.mode.intersects(Modes::MOUSE_MODE)
  }

  /// The IME composition text, if any.
  pub(crate) fn marked_text(&self) -> Option<&str> {
    self.marked_text.as_deref()
  }

  fn process_event(&mut self, event: TerminalEvent, cx: &mut Context<Self>) -> bool {
    match event {
      TerminalEvent::Wakeup => cx.notify(),
      TerminalEvent::Title(title) => {
        self.title = Some(title);
        cx.emit(TerminalViewEvent::TitleChanged(self.title.clone()));
        cx.notify();
      }
      TerminalEvent::ResetTitle => {
        self.title = None;
        cx.emit(TerminalViewEvent::TitleChanged(None));
        cx.notify();
      }
      TerminalEvent::Bell => {
        cx.emit(TerminalViewEvent::Bell);
        cx.notify();
      }
      TerminalEvent::ClipboardStore(data) => {
        cx.write_to_clipboard(ClipboardItem::new_string(data.clone()));
        cx.emit(TerminalViewEvent::ClipboardStored(data));
      }
      TerminalEvent::ClipboardLoad(formatter) => {
        let text = cx
          .read_from_clipboard()
          .and_then(|item| item.text())
          .unwrap_or_default();
        // The terminal only supports pasting strings, not images.
        self.session.write_pty(formatter(&text));
      }
      TerminalEvent::ColorRequest { index, formatter } => {
        // Answer synchronously to preserve the ordering of PTY responses.
        let palette = TerminalPalette::from_theme(cx.theme());
        self
          .session
          .write_pty(formatter(palette.vte_rgb_at_index(index)));
      }
      TerminalEvent::CursorBlinkingChanged => {
        // The new state is read here, on the host thread: the listener emits
        // this event while the PTY event loop may hold the emulator lock, so
        // it must not query emulator state itself.
        self.blinking_terminal_enabled = self.session.cursor_blinking();
        self.start_blink(cx);
        cx.notify();
      }
      TerminalEvent::ChildExit(status) => {
        cx.emit(TerminalViewEvent::Exit(Some(status)));
        cx.notify();
      }
      TerminalEvent::Exit => {
        // ChildExit normally reports the status first; emit a bare exit only
        // when it did not.
        if self.session.child_exit_status().is_none() {
          cx.emit(TerminalViewEvent::Exit(None));
        }
        cx.notify();
        return true;
      }
    }
    false
  }

  /// Restarts the blink loop; the cursor stays visible while paused.
  fn start_blink(&mut self, cx: &mut Context<Self>) {
    self.cursor_visible = true;
    self.blink_generation += 1;
    let generation = self.blink_generation;
    cx.spawn(async move |this, cx| {
      loop {
        cx.background_executor().timer(CURSOR_BLINK_INTERVAL).await;
        let Ok(stop) = this.update(cx, |view, cx| {
          if view.blink_generation != generation || !view.blinking_terminal_enabled {
            return true;
          }
          view.cursor_visible = !view.cursor_visible;
          cx.notify();
          false
        }) else {
          return;
        };
        if stop {
          return;
        }
      }
    })
    .detach();
  }

  /// Keeps the cursor visible and restarts blinking (e.g. after a keystroke).
  pub(crate) fn pause_blink(&mut self, cx: &mut Context<Self>) {
    if self.blinking_terminal_enabled {
      self.start_blink(cx);
    }
  }

  fn focus_in(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    if self.content.mode.contains(Modes::FOCUS_IN_OUT) {
      self.session.input(b"\x1b[I");
    }
    self.start_blink(cx);
    cx.notify();
  }

  fn focus_out(
    &mut self, _event: gpui::FocusOutEvent, _window: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.content.mode.contains(Modes::FOCUS_IN_OUT) {
      self.session.input(b"\x1b[O");
    }
    self.cursor_visible = true;
    self.blink_generation += 1;
    cx.notify();
  }

  /// Attempts to process a keystroke in the terminal. Returns whether it was
  /// consumed; unconsumed keystrokes propagate (printable characters are
  /// delivered through the input handler instead).
  pub(crate) fn process_keystroke(
    &mut self, keystroke: &gpui::Keystroke, cx: &mut Context<Self>,
  ) -> bool {
    let Some(esc) = to_esc_str(keystroke, self.content.mode, false) else {
      return false;
    };
    let esc = esc.into_owned();
    self.session.input(esc.into_bytes());
    self.pause_blink(cx);
    cx.notify();
    true
  }

  fn key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
    // Once the child process has exited there is nothing left to receive
    // input, so let keystrokes propagate to the host UI (e.g. a Ctrl+Q quit
    // binding) instead of swallowing them as C0 control codes.
    if self.session.child_exit_status().is_some() {
      return;
    }
    if self.process_keystroke(&event.keystroke, cx) {
      cx.stop_propagation();
    }
  }

  fn on_action_clear(&mut self, _: &super::Clear, _: &mut Window, cx: &mut Context<Self>) {
    self.clear(cx);
  }

  fn on_action_copy(&mut self, _: &super::TerminalCopy, _: &mut Window, cx: &mut Context<Self>) {
    self.copy(cx);
  }

  fn on_action_paste(
    &mut self, _: &super::TerminalPaste, _window: &mut Window, cx: &mut Context<Self>,
  ) {
    if let Some(item) = cx.read_from_clipboard()
      && let Some(text) = item.text()
    {
      self.paste(&text, cx);
    }
  }

  fn on_action_select_all(
    &mut self, _: &super::TerminalSelectAll, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.select_all(cx);
  }

  fn on_action_scroll_line_up(
    &mut self, _: &super::ScrollLineUp, _: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.is_alt_screen() {
      cx.propagate();
      return;
    }
    self.scroll(ScrollKind::Delta(1), cx);
  }

  fn on_action_scroll_line_down(
    &mut self, _: &super::ScrollLineDown, _: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.is_alt_screen() {
      cx.propagate();
      return;
    }
    self.scroll(ScrollKind::Delta(-1), cx);
  }

  fn on_action_scroll_page_up(
    &mut self, _: &super::ScrollPageUp, _: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.is_alt_screen() {
      cx.propagate();
      return;
    }
    self.scroll(ScrollKind::PageUp, cx);
  }

  fn on_action_scroll_page_down(
    &mut self, _: &super::ScrollPageDown, _: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.is_alt_screen() {
      cx.propagate();
      return;
    }
    self.scroll(ScrollKind::PageDown, cx);
  }

  fn on_action_scroll_to_top(
    &mut self, _: &super::ScrollToTop, _: &mut Window, cx: &mut Context<Self>,
  ) {
    if self.is_alt_screen() {
      cx.propagate();
      return;
    }
    self.scroll(ScrollKind::Top, cx);
  }

  fn on_action_scroll_to_bottom(
    &mut self, _: &super::ScrollToBottom, _: &mut Window, cx: &mut Context<Self>,
  ) {
    self.scroll(ScrollKind::Bottom, cx);
  }

  /// Handles scroll wheel input: mouse reporting, alternate-screen scrolling,
  /// or display scrolling.
  pub(crate) fn scroll_wheel(
    &mut self, position: gpui::Point<Pixels>, delta: gpui::ScrollDelta, shift: bool,
    touch_phase: gpui::TouchPhase, cx: &mut Context<Self>,
  ) {
    let line_height = self.content.terminal_bounds.line_height();
    let mouse_mode = self.content.mode.intersects(Modes::MOUSE_MODE) && !shift;
    let scroll_lines = self.determine_scroll_lines(delta, line_height, touch_phase);
    let Some(scroll_lines) = scroll_lines.filter(|lines| *lines != 0) else {
      return;
    };

    if mouse_mode {
      let point = grid_point(
        position,
        self.content.terminal_bounds,
        self.content.display_offset,
      );
      if let Some(reports) = scroll_reports(point, scroll_lines, delta, self.content.mode) {
        for report in reports {
          self.session.write_pty(report);
        }
        cx.notify();
      }
    } else if self
      .content
      .mode
      .contains(Modes::ALT_SCREEN | Modes::ALTERNATE_SCROLL)
      && !shift
    {
      self.session.input(alt_scroll(scroll_lines));
      cx.notify();
    } else {
      self.session.scroll(ScrollKind::Delta(scroll_lines));
      cx.notify();
    }
  }

  /// Accumulates pixel deltas and converts them to whole scroll lines.
  fn determine_scroll_lines(
    &mut self, delta: gpui::ScrollDelta, line_height: f32, touch_phase: gpui::TouchPhase,
  ) -> Option<i32> {
    match touch_phase {
      // Reset scroll state when a gesture starts.
      gpui::TouchPhase::Started => {
        self.scroll_px = px(0.);
        None
      }
      gpui::TouchPhase::Moved => {
        let old_offset = (self.scroll_px / px(line_height)) as i32;
        self.scroll_px += delta.pixel_delta(px(line_height)).y;
        let new_offset = (self.scroll_px / px(line_height)) as i32;
        // Reset at the edges so direction changes respond quickly.
        self.scroll_px %= px(self.content.terminal_bounds.height());
        Some(new_offset - old_offset)
      }
      gpui::TouchPhase::Ended | gpui::TouchPhase::Cancelled => None,
    }
  }

  /// Handles a left mouse down: mouse reporting, or starting a selection.
  pub(crate) fn mouse_down(
    &mut self, position: gpui::Point<Pixels>, button: gpui::MouseButton,
    modifiers: gpui::Modifiers, click_count: usize, cx: &mut Context<Self>,
  ) {
    let point = grid_point(
      position,
      self.content.terminal_bounds,
      self.content.display_offset,
    );

    if self.mouse_mode(modifiers.shift) {
      if let Some(bytes) = mouse_button_report(point, button, modifiers, true, self.content.mode) {
        self.session.write_pty(bytes);
      }
      return;
    }

    match button {
      gpui::MouseButton::Left => {
        self.mouse_down_position = Some(position);
        if click_count == 0 {
          return; // This is a release.
        }
        if click_count == 1 && modifiers.shift {
          if self.content.selection.is_some() {
            // Extend the existing selection to this point.
            self.update_selection(point, cx);
          } else {
            // Shift is the escape hatch for selecting text while an app has
            // mouse tracking enabled.
            self.set_selection(point, SelectionKind::Characters, cx);
          }
          cx.notify();
          return;
        }
        let kind = match click_count {
          1 => Some(SelectionKind::Characters),
          2 => Some(SelectionKind::Words),
          3 => Some(SelectionKind::Lines),
          _ => None,
        };
        if let Some(kind) = kind {
          self.set_selection(point, kind, cx);
        }
      }
      // Middle-click pastes the primary selection on Linux.
      #[cfg(any(target_os = "linux", target_os = "freebsd"))]
      gpui::MouseButton::Middle => {
        if let Some(item) = cx.read_from_primary()
          && let Some(text) = item.text()
        {
          self.session.paste(&text);
          cx.notify();
        }
      }
      _ => {}
    }
  }

  /// Anchors a new selection at `point` with the given semantics.
  fn set_selection(&mut self, point: GridPoint, kind: SelectionKind, cx: &mut Context<Self>) {
    self.session.select(point, point, kind);
    self.selection_anchor = Some((point, kind));
    self.selection_head = Some(point);
    self.selection_phase = SelectionPhase::Selecting;
    cx.notify();
  }

  /// Handles mouse dragging: updates the selection and auto-scrolls near the
  /// viewport edges.
  pub(crate) fn mouse_drag(
    &mut self, position: gpui::Point<Pixels>, region: gpui::Bounds<Pixels>, shift: bool,
    cx: &mut Context<Self>,
  ) {
    if self.mouse_mode(shift) {
      return;
    }

    // Ignore tiny pointer movements so the window-focusing click does not
    // begin a selection.
    if self.selection_phase != SelectionPhase::Selecting
      && let Some(mouse_down_position) = self.mouse_down_position
      && (position - mouse_down_position).magnitude() <= SELECTION_DRAG_THRESHOLD
    {
      return;
    }

    self.selection_phase = SelectionPhase::Selecting;
    let point = grid_point(
      position,
      self.content.terminal_bounds,
      self.content.display_offset,
    );
    let mut changed = self.update_selection(point, cx);

    // Auto-scroll when dragging beyond the viewport (never on the alt screen).
    if !self.is_alt_screen() {
      let top = region.origin.y;
      let bottom = region.bottom_left().y;
      let scroll_lines = if position.y < top {
        let scroll_delta = (top - position.y).pow(1.1);
        Some((scroll_delta / px(self.content.terminal_bounds.line_height())).ceil() as i32)
      } else if position.y > bottom {
        let scroll_delta = (position.y - bottom).pow(1.1);
        Some(-(scroll_delta / px(self.content.terminal_bounds.line_height())).floor() as i32)
      } else {
        None
      }
      .map(|lines| lines.clamp(-3, 3));
      if let Some(lines) = scroll_lines.filter(|lines| *lines != 0) {
        self.session.scroll(ScrollKind::Delta(lines));
        changed = true;
      }
    }
    // Sub-cell pointer jitter must not repaint the whole terminal.
    if changed {
      cx.notify();
    }
  }

  /// Handles mouse moves: motion reports while a mouse mode is active.
  pub(crate) fn mouse_move(
    &mut self, position: gpui::Point<Pixels>, pressed_button: Option<gpui::MouseButton>,
    modifiers: gpui::Modifiers, cx: &mut Context<Self>,
  ) {
    if !self.mouse_mode(modifiers.shift) {
      return;
    }
    let (point, after_midpoint) = grid_point_and_side(
      position,
      self.content.terminal_bounds,
      self.content.display_offset,
    );
    if self.mouse_changed(point, after_midpoint)
      && let Some(bytes) = mouse_moved_report(point, pressed_button, modifiers, self.content.mode)
    {
      self.session.write_pty(bytes);
      // Invalidate only when a report was actually sent; sub-cell pointer
      // jitters must not repaint the whole terminal.
      cx.notify();
    }
  }

  /// Handles mouse ups: release reports and selection finalization.
  pub(crate) fn mouse_up(
    &mut self, position: gpui::Point<Pixels>, button: gpui::MouseButton, modifiers: gpui::Modifiers,
  ) {
    let point = grid_point(
      position,
      self.content.terminal_bounds,
      self.content.display_offset,
    );
    if self.mouse_mode(modifiers.shift)
      && let Some(bytes) = mouse_button_report(point, button, modifiers, false, self.content.mode)
    {
      self.session.write_pty(bytes);
    }
    self.reset_gesture_state();
  }

  /// Handles a left mouse-up released outside the terminal (over the dock,
  /// during a window resize, ...). `on_mouse_up` only fires while hovered, so
  /// without this the selection would stay active forever and hijack every
  /// subsequent drag in the app. No report is sent: the point is outside the
  /// grid, and mouse-tracking applications handle stray gestures themselves.
  pub(crate) fn mouse_up_outside(
    &mut self, _position: gpui::Point<Pixels>, _button: gpui::MouseButton,
    _modifiers: gpui::Modifiers,
  ) {
    self.reset_gesture_state();
  }

  /// Clears all in-progress gesture bookkeeping.
  fn reset_gesture_state(&mut self) {
    self.selection_phase = SelectionPhase::Ended;
    self.selection_anchor = None;
    self.selection_head = None;
    self.last_mouse = None;
    self.mouse_down_position = None;
  }

  /// Whether mouse events should be forwarded to the application instead of
  /// used for local selection. Shift bypasses mouse mode.
  fn mouse_mode(&self, shift: bool) -> bool {
    self.content.mode.intersects(Modes::MOUSE_MODE) && !shift
  }

  fn mouse_changed(&mut self, point: GridPoint, after_midpoint: bool) -> bool {
    match self.last_mouse {
      Some((last_point, last_side)) if last_point == point && last_side == after_midpoint => false,
      _ => {
        self.last_mouse = Some((point, after_midpoint));
        true
      }
    }
  }

  /// Moves the selection head to `point`, keeping the anchor fixed. Returns
  /// whether the selection actually changed, so callers can skip no-op
  /// invalidations.
  fn update_selection(&mut self, point: GridPoint, cx: &mut Context<Self>) -> bool {
    if let Some((anchor, kind)) = self.selection_anchor
      && self.selection_head != Some(point)
    {
      self.session.select(anchor, point, kind);
      self.selection_head = Some(point);
      cx.notify();
      return true;
    }
    false
  }
}

impl Render for TerminalView {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let focused = self.focus.is_focused(window);
    div()
      .id("terminal-view")
      .key_context(CONTEXT)
      .track_focus(&self.focus)
      .on_key_down(cx.listener(Self::key_down))
      .on_action(cx.listener(Self::on_action_clear))
      .on_action(cx.listener(Self::on_action_copy))
      .on_action(cx.listener(Self::on_action_paste))
      .on_action(cx.listener(Self::on_action_select_all))
      .on_action(cx.listener(Self::on_action_scroll_line_up))
      .on_action(cx.listener(Self::on_action_scroll_line_down))
      .on_action(cx.listener(Self::on_action_scroll_page_up))
      .on_action(cx.listener(Self::on_action_scroll_page_down))
      .on_action(cx.listener(Self::on_action_scroll_to_top))
      .on_action(cx.listener(Self::on_action_scroll_to_bottom))
      .size_full()
      .child(TerminalElement::new(
        cx.entity(),
        self.focus.clone(),
        focused,
      ))
  }
}
