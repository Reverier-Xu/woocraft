//! Terminal sessions: the handle through which hosts control a PTY terminal.

use std::{
  borrow::Cow,
  sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
  },
};

use anyhow::Result;
use vte::ansi::Processor;

use crate::{
  backend::{self, BackendListener, PtySender, TermLock},
  event::{ChildStatus, TerminalEvent},
  options::SpawnOptions,
  types::{CellColor, Point, ScrollKind, SelectionKind, TerminalBounds},
};

/// A running terminal session.
///
/// The session is a cheap, cloneable handle: all state lives behind shared
/// locks so it can be controlled from any thread. Dropping the last clone
/// shuts the session down (closing the PTY, which terminates the child).
#[derive(Clone)]
pub struct TerminalSession {
  inner: Arc<SessionInner>,
}

struct SessionInner {
  kind: SessionKind,
  term: Arc<TermLock>,
  bounds: Mutex<TerminalBounds>,
  events_tx: async_channel::Sender<TerminalEvent>,
  /// Keeps the event channel open even if the host drops its receiver.
  events_rx: async_channel::Receiver<TerminalEvent>,
  exited: Arc<AtomicBool>,
  exit_status: Arc<Mutex<Option<ChildStatus>>>,
  display_processor: Mutex<Processor<vte::ansi::StdSyncHandler>>,
}

enum SessionKind {
  /// A session backed by a live PTY and child process.
  Pty {
    pty_tx: PtySender,
    child_pid: Option<u32>,
  },
  /// A session with no child process; output is fed in via
  /// [`TerminalSession::feed_display`].
  DisplayOnly,
}

impl TerminalSession {
  /// Spawns a new PTY session.
  pub fn spawn(options: SpawnOptions, bounds: TerminalBounds) -> Result<Self> {
    let bounds = backend::normalize_terminal_bounds(bounds);
    let (events_tx, events_rx) = async_channel::unbounded();
    let exited = Arc::new(AtomicBool::new(false));
    let exit_status = Arc::new(Mutex::new(None));
    let bounds_shared = Arc::new(Mutex::new(bounds));

    let listener = BackendListener::new(
      events_tx.clone(),
      bounds_shared.clone(),
      exited.clone(),
      exit_status.clone(),
    );
    let term = backend::new_term(
      backend::term_config(&options),
      bounds,
      listener.clone(),
      !options.alternate_scroll,
    );

    let (pty_tx, child_pid) = backend::open_pty(&options, bounds, &listener, term.clone())?;

    Ok(Self {
      inner: Arc::new(SessionInner {
        kind: SessionKind::Pty { pty_tx, child_pid },
        term,
        bounds: Mutex::new(bounds),
        events_tx,
        events_rx,
        exited,
        exit_status,
        display_processor: Mutex::new(Processor::new()),
      }),
    })
  }

  /// Spawns a display-only session with no child process.
  ///
  /// Terminal state changes are driven by [`TerminalSession::feed_display`],
  /// which is mostly useful for rendering tests and previews.
  pub fn spawn_display_only(options: SpawnOptions, bounds: TerminalBounds) -> Self {
    let bounds = backend::normalize_terminal_bounds(bounds);
    let (events_tx, events_rx) = async_channel::unbounded();
    let exited = Arc::new(AtomicBool::new(false));
    let exit_status = Arc::new(Mutex::new(None));
    let bounds_shared = Arc::new(Mutex::new(bounds));

    let listener = BackendListener::new(
      events_tx.clone(),
      bounds_shared.clone(),
      exited.clone(),
      exit_status.clone(),
    );
    let term = backend::new_term(
      backend::term_config(&options),
      bounds,
      listener.clone(),
      !options.alternate_scroll,
    );

    Self {
      inner: Arc::new(SessionInner {
        kind: SessionKind::DisplayOnly,
        term,
        bounds: Mutex::new(bounds),
        events_tx,
        events_rx,
        exited,
        exit_status,
        display_processor: Mutex::new(Processor::new()),
      }),
    }
  }

  /// Returns a receiver for the session's event stream.
  pub fn events(&self) -> async_channel::Receiver<TerminalEvent> {
    self.inner.events_rx.clone()
  }

  /// Writes raw bytes into the PTY, as if typed by the user.
  pub fn input(&self, bytes: impl Into<Cow<'static, [u8]>>) {
    self.write_pty(bytes);
  }

  /// Writes a string into the PTY, as if typed by the user.
  pub fn input_str(&self, text: &str) {
    self.write_pty(Cow::Owned(text.as_bytes().to_vec()));
  }

  /// Writes raw bytes into the PTY without marking them as user keyboard
  /// input. This is the escape hatch hosts use to answer
  /// [`TerminalEvent::ClipboardLoad`] and [`TerminalEvent::ColorRequest`].
  pub fn write_pty(&self, bytes: impl Into<Cow<'static, [u8]>>) {
    if let SessionKind::Pty { pty_tx, .. } = &self.inner.kind {
      pty_tx.notify(bytes);
    }
  }

  /// Pastes `text` into the terminal, honoring the application's
  /// bracketed-paste mode and normalizing line endings.
  pub fn paste(&self, text: &str) {
    let paste_text = if self
      .snapshot()
      .mode
      .contains(crate::types::Modes::BRACKETED_PASTE)
    {
      format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', ""))
    } else {
      text.replace("\r\n", "\r").replace('\n', "\r")
    };
    self.input(Cow::Owned(paste_text.into_bytes()));
  }

  /// Resizes the terminal viewport, updating the emulator and the PTY.
  pub fn resize(&self, bounds: TerminalBounds) {
    let bounds = backend::normalize_terminal_bounds(bounds);
    *self.inner.bounds.lock().unwrap() = bounds;
    if let SessionKind::Pty { pty_tx, .. } = &self.inner.kind {
      pty_tx.resize(bounds);
    }
    backend::resize(&mut self.inner.term.lock(), bounds);
    self.wake();
  }

  /// Scrolls the display.
  pub fn scroll(&self, scroll: ScrollKind) {
    backend::scroll_display(&mut self.inner.term.lock(), scroll);
    self.wake();
  }

  /// The current viewport bounds.
  pub fn bounds(&self) -> TerminalBounds {
    *self.inner.bounds.lock().unwrap()
  }

  /// Whether the emulator cursor is configured to blink.
  pub fn cursor_blinking(&self) -> bool {
    self.inner.term.lock().cursor_style().blinking
  }

  /// Takes a point-in-time snapshot of the terminal state.
  pub fn snapshot(&self) -> crate::types::Content {
    let bounds = *self.inner.bounds.lock().unwrap();
    backend::make_content(&self.inner.term.lock(), &bounds)
  }

  /// Renders the full terminal content (visible grid + scrollback) as text.
  pub fn text(&self) -> String {
    backend::content_text(&self.inner.term.lock())
  }

  /// The last `n` non-empty logical lines of the terminal output.
  pub fn last_n_non_empty_lines(&self, n: usize) -> Vec<String> {
    backend::last_non_empty_lines(&self.inner.term.lock(), n)
  }

  /// Selects a range of cells with the given [`SelectionKind`] semantics.
  pub fn select(&self, start: Point, end: Point, kind: SelectionKind) {
    backend::set_selection(&mut self.inner.term.lock(), start, end, kind);
    self.wake();
  }

  /// Clears the current selection, if any.
  pub fn clear_selection(&self) {
    backend::clear_selection(&mut self.inner.term.lock());
    self.wake();
  }

  /// The text covered by the current selection, if any.
  pub fn copy_selection(&self) -> Option<String> {
    self.inner.term.lock().selection_to_string()
  }

  /// Clears the screen and the scrollback history.
  pub fn clear(&self) {
    backend::clear_saved_screen(&mut self.inner.term.lock());
    self.wake();
  }

  /// The pid of the direct child process, when the platform exposes it.
  pub fn pid(&self) -> Option<u32> {
    match &self.inner.kind {
      SessionKind::Pty { child_pid, .. } => *child_pid,
      SessionKind::DisplayOnly => None,
    }
  }

  /// Whether the child process is still running. Display-only sessions
  /// have no child and always report `false`.
  pub fn is_alive(&self) -> bool {
    matches!(self.inner.kind, SessionKind::Pty { .. }) && !self.inner.exited.load(Ordering::SeqCst)
  }

  /// The exit status of the child process, once it has exited.
  pub fn child_exit_status(&self) -> Option<ChildStatus> {
    *self.inner.exit_status.lock().unwrap()
  }

  /// The foreground/background colors currently set by the application.
  ///
  /// Hosts use this as the fallback when rendering cells with default colors.
  pub fn default_colors(&self) -> (CellColor, CellColor) {
    let term = self.inner.term.lock();
    let colors = term.colors();
    let fg = colors[crate::types::NamedColor::Foreground]
      .map(CellColor::Spec)
      .unwrap_or(CellColor::Named(crate::types::NamedColor::Foreground));
    let bg = colors[crate::types::NamedColor::Background]
      .map(CellColor::Spec)
      .unwrap_or(CellColor::Named(crate::types::NamedColor::Background));
    (fg, bg)
  }

  /// Feeds raw output bytes directly into the emulator state, bypassing the
  /// PTY. Only meaningful for display-only sessions.
  pub fn feed_display(&self, bytes: &[u8]) {
    {
      let mut term = self.inner.term.lock();
      let mut processor = self.inner.display_processor.lock().unwrap();
      backend::feed_display(&mut term, bytes, &mut processor);
    }
    self.wake();
  }

  /// Shuts the session down: stops the PTY event loop, which closes the PTY
  /// and thereby terminates the child process.
  ///
  /// Idempotent. The first call reports the session as terminated (exit code
  /// unavailable) and emits [`TerminalEvent::ChildExit`] plus
  /// [`TerminalEvent::Exit`], since the PTY event loop will not report them
  /// after a shutdown request.
  pub fn kill(&self) {
    if let SessionKind::Pty { pty_tx, .. } = &self.inner.kind {
      pty_tx.shutdown();
      if !self.inner.exited.swap(true, Ordering::SeqCst) {
        #[cfg(unix)]
        let status = ChildStatus {
          code: None,
          signal: None,
        };
        #[cfg(windows)]
        let status = ChildStatus { code: None };
        *self.inner.exit_status.lock().unwrap() = Some(status);
        let _ = self
          .inner
          .events_tx
          .send_blocking(TerminalEvent::ChildExit(status));
        let _ = self.inner.events_tx.send_blocking(TerminalEvent::Exit);
      }
    }
  }

  fn wake(&self) {
    let _ = self.inner.events_tx.send_blocking(TerminalEvent::Wakeup);
  }
}

impl Drop for SessionInner {
  fn drop(&mut self) {
    if let SessionKind::Pty { pty_tx, .. } = &self.kind {
      pty_tx.shutdown();
    }
  }
}

impl std::fmt::Debug for TerminalSession {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.debug_struct("TerminalSession")
      .field("pid", &self.pid())
      .field("alive", &self.is_alive())
      .finish()
  }
}
