//! The alacritty-backed terminal emulator and PTY plumbing.
//!
//! This module owns all `alacritty_terminal` specifics: opening the PTY,
//! running the PTY I/O event loop, converting backend events into
//! [`TerminalEvent`]s, and taking content snapshots. Everything above this
//! module works with the crate's own gpui-free types only.

use std::{
  borrow::Cow,
  sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicBool, Ordering},
  },
};

use alacritty_terminal::{
  event::{Event as AlacEvent, EventListener},
  event_loop::{EventLoop, EventLoopSender, Msg},
  grid::{Dimensions as _, Row},
  index::{Column, Line, Point as AlacPoint, Side as AlacSide},
  selection::{Selection as AlacSelection, SelectionType as AlacSelectionType},
  sync::FairMutex,
  term::{
    Config as AlacConfig, SEMANTIC_ESCAPE_CHARS, Term,
    cell::{Cell as AlacCell, Flags as AlacFlags},
  },
  tty,
};
use anyhow::{Context as _, Result};
use tracing::debug;
use vte::ansi::{
  ClearMode, CursorShape as AlacCursorShape, CursorStyle as AlacCursorStyle, Handler as _,
  NamedPrivateMode, PrivateMode, Rgb,
};

use crate::{
  event::{ChildStatus, ClipboardFormatter, ColorFormatter, TerminalEvent},
  options::SpawnOptions,
  types::{
    Cell, CellFlags, Content, Cursor, CursorShape, IndexedCell, Modes, Point, ScrollKind,
    SelectionRange, TerminalBounds,
  },
};

pub(crate) type TermLock = FairMutex<Term<BackendListener>>;

/// The maximum number of scrollback lines retained by the emulator.
const MAX_SCROLL_HISTORY: usize = 1_000_000;

// ---------------------------------------------------------------------------
// PTY handle
// ---------------------------------------------------------------------------

/// A handle for writing to the PTY and controlling the I/O event loop.
#[derive(Clone)]
pub(crate) struct PtySender {
  sender: Arc<Mutex<EventLoopSender>>,
}

impl PtySender {
  pub(crate) fn new(sender: EventLoopSender) -> Self {
    Self {
      sender: Arc::new(Mutex::new(sender)),
    }
  }

  /// Writes raw bytes into the PTY.
  pub(crate) fn notify(&self, bytes: impl Into<Cow<'static, [u8]>>) {
    let bytes = bytes.into();
    // The event loop hangs if zero bytes are sent through.
    if bytes.is_empty() {
      return;
    }
    if let Err(error) = self.lock().send(Msg::Input(bytes)) {
      debug!("failed to write to pty: {error}");
    }
  }

  /// Resizes the PTY to match the given bounds.
  pub(crate) fn resize(&self, bounds: TerminalBounds) {
    let window_size = bounds.to_window_size();
    if let Err(error) = self.lock().send(Msg::Resize(window_size)) {
      debug!("failed to resize pty: {error}");
    }
  }

  /// Shuts the event loop down, which drops the PTY and terminates the child.
  pub(crate) fn shutdown(&self) {
    if let Err(error) = self.lock().send(Msg::Shutdown) {
      debug!("failed to shut down pty event loop: {error}");
    }
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, EventLoopSender> {
    self.sender.lock().expect("pty sender mutex poisoned")
  }
}

// ---------------------------------------------------------------------------
// Event listener
// ---------------------------------------------------------------------------

/// The `EventListener` installed into the emulator; it converts backend events
/// into [`TerminalEvent`]s and answers PTY queries that must keep their
/// ordering relative to other PTY writes (text-area size, application writes).
#[derive(Clone)]
pub(crate) struct BackendListener {
  events_tx: async_channel::Sender<TerminalEvent>,
  bounds: Arc<Mutex<TerminalBounds>>,
  /// Filled in right after the emulator is wrapped into its lock, before the
  /// event loop thread is spawned, so no event can observe an empty slot.
  term: Arc<OnceLock<Arc<TermLock>>>,
  /// Filled in right after the event loop is created, before its thread is
  /// spawned, so no event can observe an empty slot.
  pty_sender: Arc<OnceLock<PtySender>>,
  exited: Arc<AtomicBool>,
  exit_status: Arc<Mutex<Option<ChildStatus>>>,
}

impl BackendListener {
  pub(crate) fn new(
    events_tx: async_channel::Sender<TerminalEvent>, bounds: Arc<Mutex<TerminalBounds>>,
    exited: Arc<AtomicBool>, exit_status: Arc<Mutex<Option<ChildStatus>>>,
  ) -> Self {
    Self {
      events_tx,
      bounds,
      term: Arc::new(OnceLock::new()),
      pty_sender: Arc::new(OnceLock::new()),
      exited,
      exit_status,
    }
  }

  pub(crate) fn set_term(&self, term: Arc<TermLock>) {
    let _ = self.term.set(term);
  }

  pub(crate) fn set_pty_sender(&self, sender: PtySender) {
    let _ = self.pty_sender.set(sender);
  }

  fn send(&self, event: TerminalEvent) {
    // Unbounded channel: this never blocks and only fails once the host has
    // dropped every receiver, which means nobody is listening anymore.
    let _ = self.events_tx.send_blocking(event);
  }

  fn mark_exit(&self) {
    self.exited.store(true, Ordering::SeqCst);
  }

  fn write_to_pty(&self, bytes: Vec<u8>) {
    if let Some(sender) = self.pty_sender.get() {
      sender.notify(bytes);
    }
  }
}

impl EventListener for BackendListener {
  fn send_event(&self, event: AlacEvent) {
    match event {
      AlacEvent::MouseCursorDirty => {}
      AlacEvent::Title(title) => self.send(TerminalEvent::Title(title)),
      AlacEvent::ResetTitle => self.send(TerminalEvent::ResetTitle),
      AlacEvent::ClipboardStore(_, data) => self.send(TerminalEvent::ClipboardStore(data)),
      AlacEvent::ClipboardLoad(_, format) => {
        let formatter: ClipboardFormatter = Arc::new(move |text: &str| format(text).into_bytes());
        self.send(TerminalEvent::ClipboardLoad(formatter));
      }
      AlacEvent::ColorRequest(index, format) => {
        let formatter: ColorFormatter = Arc::new(move |rgb: Rgb| format(rgb).into_bytes());
        self.send(TerminalEvent::ColorRequest { index, formatter });
      }
      AlacEvent::PtyWrite(output) => self.write_to_pty(output.into_bytes()),
      AlacEvent::TextAreaSizeRequest(format) => {
        let window_size = self.bounds.lock().unwrap().to_window_size();
        self.write_to_pty(format(window_size).into_bytes());
      }
      AlacEvent::CursorBlinkingChange => {
        let blinking = self
          .term
          .get()
          .is_some_and(|term| term.lock().cursor_style().blinking);
        self.send(TerminalEvent::CursorBlinkingChanged(blinking));
      }
      AlacEvent::Wakeup => self.send(TerminalEvent::Wakeup),
      AlacEvent::Bell => self.send(TerminalEvent::Bell),
      AlacEvent::ChildExit(status) => {
        let status = ChildStatus::from(&status);
        *self.exit_status.lock().unwrap() = Some(status);
        self.mark_exit();
        self.send(TerminalEvent::ChildExit(status));
      }
      AlacEvent::Exit => {
        self.mark_exit();
        self.send(TerminalEvent::Exit);
      }
    }
  }
}

// ---------------------------------------------------------------------------
// Session backend construction
// ---------------------------------------------------------------------------

pub(crate) fn term_config(options: &SpawnOptions) -> AlacConfig {
  AlacConfig {
    scrolling_history: options.history().min(MAX_SCROLL_HISTORY),
    default_cursor_style: AlacCursorStyle {
      shape: alacritty_cursor_shape(options.cursor_shape.into()),
      blinking: false,
    },
    semantic_escape_chars: format!("{SEMANTIC_ESCAPE_CHARS}\u{2500}"),
    ..AlacConfig::default()
  }
}

pub(crate) fn normalize_terminal_bounds(bounds: TerminalBounds) -> TerminalBounds {
  TerminalBounds::new(
    bounds.line_height(),
    bounds.cell_width(),
    bounds.num_columns(),
    bounds.num_lines(),
  )
}

pub(crate) fn new_term(
  config: AlacConfig, bounds: TerminalBounds, listener: BackendListener,
  disable_alternate_scroll: bool,
) -> Arc<TermLock> {
  let mut term = Term::new(config, &bounds, listener);
  if disable_alternate_scroll {
    term.unset_private_mode(PrivateMode::Named(NamedPrivateMode::AlternateScroll));
  }
  Arc::new(FairMutex::new(term))
}

/// Opens the cross-platform PTY and starts its I/O event loop.
///
/// Returns the PTY write handle plus the pid of the direct child (when the
/// platform exposes it).
///
/// On unix the current signal mask is captured and applied in the child before
/// exec so that a blocked `SIGCHLD` in the host does not leak into it.
pub(crate) fn open_pty(
  options: &SpawnOptions, bounds: TerminalBounds, listener: &BackendListener, term: Arc<TermLock>,
) -> Result<(PtySender, Option<u32>)> {
  let (shell, args) = options
    .shell
    .clone()
    .unwrap_or_else(crate::options::default_shell);
  #[cfg(not(windows))]
  let child_signal_mask = tty::SignalMask::current().ok();

  let tty_options = tty::Options {
    shell: Some(tty::Shell::new(shell, args)),
    working_directory: options.working_directory.clone(),
    drain_on_exit: true,
    env: options.env.iter().cloned().collect(),
    #[cfg(not(windows))]
    child_signal_mask,
    #[cfg(windows)]
    escape_args: true,
  };

  let pty = tty::new(&tty_options, bounds.to_window_size(), 0).context("failed to open pty")?;
  let child_pid = child_pid_of_pty(&pty);

  let event_loop = EventLoop::new(term, listener.clone(), pty, true, false)
    .context("failed to create pty event loop")?;
  let pty_sender = PtySender::new(event_loop.channel());
  listener.set_pty_sender(pty_sender.clone());
  // The event loop thread keeps the PTY alive until shutdown; dropping the
  // join handle detaches it so the session never has to be joined.
  let _io_thread = event_loop.spawn();
  Ok((pty_sender, child_pid))
}

/// Returns the pid of the direct child attached to the PTY.
pub(crate) fn child_pid_of_pty(pty: &tty::Pty) -> Option<u32> {
  #[cfg(unix)]
  {
    Some(pty.child().id())
  }
  #[cfg(windows)]
  {
    pty.child_watcher().pid().map(u32::from)
  }
}

fn alacritty_cursor_shape(shape: crate::types::CursorShape) -> AlacCursorShape {
  match shape {
    CursorShape::Block => AlacCursorShape::Block,
    CursorShape::Underline => AlacCursorShape::Underline,
    CursorShape::Bar => AlacCursorShape::Beam,
    CursorShape::HollowBlock => AlacCursorShape::HollowBlock,
    CursorShape::Hidden => AlacCursorShape::Hidden,
  }
}

fn alacritty_cursor_shape_of(shape: AlacCursorShape) -> CursorShape {
  match shape {
    AlacCursorShape::Block => CursorShape::Block,
    AlacCursorShape::Underline => CursorShape::Underline,
    AlacCursorShape::Beam => CursorShape::Bar,
    AlacCursorShape::HollowBlock => CursorShape::HollowBlock,
    AlacCursorShape::Hidden => CursorShape::Hidden,
  }
}

// ---------------------------------------------------------------------------
// Emulator operations (all short-lived lock critical sections)
// ---------------------------------------------------------------------------

pub(crate) fn resize(term: &mut Term<BackendListener>, bounds: TerminalBounds) {
  term.resize(bounds);
}

pub(crate) fn scroll_display(term: &mut Term<BackendListener>, scroll: ScrollKind) {
  term.scroll_display(scroll.to_alacritty());
}

pub(crate) fn set_selection(
  term: &mut Term<BackendListener>, start: Point, end: Point, is_block: bool,
) {
  let selection_type = if is_block {
    AlacSelectionType::Block
  } else {
    AlacSelectionType::Lines
  };
  let mut selection = AlacSelection::new(selection_type, start.to_alacritty(), AlacSide::Left);
  selection.update(end.to_alacritty(), AlacSide::Right);
  term.selection = Some(selection);
}

pub(crate) fn clear_saved_screen(term: &mut Term<BackendListener>) {
  // Erase the saved lines (scrollback) first, then clear the visible screen
  // while keeping the current line, which is moved to the top. This mirrors
  // the behavior of the `clear` command combined with a viewport reset.
  term.clear_screen(ClearMode::Saved);

  let cursor = term.grid().cursor.point;
  term.grid_mut().reset_region(..cursor.line);

  let line = term.grid()[cursor.line][..Column(term.grid().columns())]
    .iter()
    .cloned()
    .enumerate()
    .collect::<Vec<(usize, AlacCell)>>();

  for (index, cell) in line {
    term.grid_mut()[Line(0)][Column(index)] = cell;
  }

  term.grid_mut().cursor.point = AlacPoint::new(Line(0), term.grid().cursor.point.column);
  let new_cursor = term.grid().cursor.point;

  if (new_cursor.line.0 as usize) < term.screen_lines() - 1 {
    term.grid_mut().reset_region((new_cursor.line + 1)..);
  }

  term.grid_mut().truncate();
}

/// Renders the full emulator state (visible grid + scrollback) into a string.
pub(crate) fn content_text(term: &Term<BackendListener>) -> String {
  let start = AlacPoint::new(term.topmost_line(), Column(0));
  let end = AlacPoint::new(term.bottommost_line(), term.last_column());
  term.bounds_to_string(start, end)
}

/// The last `line_count` non-empty logical lines, joining soft-wrapped rows.
pub(crate) fn last_non_empty_lines(term: &Term<BackendListener>, line_count: usize) -> Vec<String> {
  let grid = term.grid();
  let mut lines = Vec::new();
  let mut current_line = grid.bottommost_line().0;
  let topmost_line = grid.topmost_line().0;

  while current_line >= topmost_line && lines.len() < line_count {
    let logical_line_start = logical_line_start_for_row(grid, current_line, topmost_line);
    let mut line = String::new();
    for row in logical_line_start..=current_line {
      line.push_str(&row_to_string(&grid[Line(row)]));
    }
    let trimmed = line.trim_end().to_string();
    if !trimmed.is_empty() {
      lines.push(trimmed);
    }
    current_line = logical_line_start - 1;
  }

  lines.reverse();
  lines
}

fn logical_line_start_for_row(
  grid: &alacritty_terminal::grid::Grid<AlacCell>, current: i32, topmost: i32,
) -> i32 {
  let mut line_start = current;
  while line_start > topmost {
    let previous_line = Line(line_start - 1);
    let last_cell = &grid[previous_line][Column(grid.columns() - 1)];
    if !last_cell.flags.contains(AlacFlags::WRAPLINE) {
      break;
    }
    line_start -= 1;
  }
  line_start
}

fn row_to_string(row: &Row<AlacCell>) -> String {
  row[..Column(row.len())].iter().map(|cell| cell.c).collect()
}

/// Takes a consistent snapshot of the emulator state.
pub(crate) fn make_content(term: &Term<BackendListener>, bounds: &TerminalBounds) -> Content {
  let content = term.renderable_content();

  let mut cells = Vec::with_capacity(content.display_iter.size_hint().0);
  cells.extend(content.display_iter.map(|indexed| IndexedCell {
    point: Point::from_alacritty(indexed.point),
    cell: cell_from_alacritty(indexed.cell),
  }));

  let selection_text = if content.selection.is_some() {
    term.selection_to_string()
  } else {
    None
  };

  let grid = term.grid();
  Content {
    cells,
    mode: Modes::from_term_mode(content.mode),
    cursor: Cursor {
      shape: alacritty_cursor_shape_of(content.cursor.shape),
      point: Point::from_alacritty(content.cursor.point),
    },
    cursor_char: grid[content.cursor.point].c,
    total_lines: grid.total_lines(),
    display_offset: grid.display_offset(),
    columns: grid.columns(),
    screen_lines: grid.screen_lines(),
    selection: content.selection.map(|range| SelectionRange {
      start: Point::from_alacritty(range.start),
      end: Point::from_alacritty(range.end),
      is_block: range.is_block,
    }),
    selection_text,
    scrolled_to_top: content.display_offset == term.history_size(),
    scrolled_to_bottom: content.display_offset == 0,
    terminal_bounds: *bounds,
  }
}

fn cell_from_alacritty(cell: &AlacCell) -> Cell {
  Cell {
    c: cell.c,
    fg: cell.fg,
    bg: cell.bg,
    flags: CellFlags::from_alacritty(cell.flags),
    hyperlink: cell.hyperlink().map(|link| Arc::from(link.uri())),
    zerowidth: cell.zerowidth().map(<[char]>::to_vec).unwrap_or_default(),
  }
}

/// Parses raw bytes directly into the emulator state, bypassing the PTY.
///
/// Used for display-only sessions and tests; line feeds are converted to
/// carriage returns so output does not stair-step.
pub(crate) fn feed_display(
  term: &mut Term<BackendListener>, bytes: &[u8],
  processor: &mut vte::ansi::Processor<vte::ansi::StdSyncHandler>,
) {
  let mut previous_byte_was_cr = false;
  let converted = convert_lf_to_crlf(bytes, &mut previous_byte_was_cr);
  processor.advance(term, &converted);
}

fn convert_lf_to_crlf<'a>(bytes: &'a [u8], previous_byte_was_cr: &mut bool) -> Cow<'a, [u8]> {
  if !bytes.contains(&b'\n') {
    return Cow::Borrowed(bytes);
  }
  let mut converted = Vec::with_capacity(bytes.len() + 8);
  let mut previous_byte_was_cr = *previous_byte_was_cr;
  for &byte in bytes {
    match byte {
      b'\r' => {
        previous_byte_was_cr = true;
      }
      b'\n' => {
        if !previous_byte_was_cr {
          converted.push(b'\r');
        }
        previous_byte_was_cr = false;
      }
      _ => {
        previous_byte_was_cr = false;
      }
    }
    converted.push(byte);
  }
  Cow::Owned(converted)
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::{CellColor, NamedColor};

  #[test]
  fn lf_to_crlf_conversion() {
    let mut previous = false;
    let out = convert_lf_to_crlf(b"a\nb\r\nc", &mut previous);
    assert_eq!(&*out, b"a\r\nb\r\nc");

    let mut previous = false;
    let out = convert_lf_to_crlf(b"no newline", &mut previous);
    assert!(matches!(out, Cow::Borrowed(_)));
  }

  #[test]
  fn bounds_window_size() {
    let bounds = TerminalBounds::new(24.0, 8.0, 80, 24);
    let size = bounds.to_window_size();
    assert_eq!(size.num_cols, 80);
    assert_eq!(size.num_lines, 24);
    assert_eq!(size.cell_width, 8);
    assert_eq!(size.cell_height, 24);
  }

  #[test]
  fn cell_conversion() {
    let mut alac_cell = AlacCell {
      c: 'x',
      ..AlacCell::default()
    };
    alac_cell.flags.insert(AlacFlags::BOLD);
    alac_cell.flags.insert(AlacFlags::WIDE_CHAR);
    let cell = cell_from_alacritty(&alac_cell);
    assert_eq!(cell.c, 'x');
    assert!(cell.flags.contains(CellFlags::BOLD));
    assert!(cell.flags.contains(CellFlags::WIDE_CHAR));
    assert_eq!(cell.fg, CellColor::Named(NamedColor::Foreground));
  }
}
