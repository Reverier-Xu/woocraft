//! Options for spawning terminal sessions.

use std::path::PathBuf;

use crate::types::CursorShape;

/// Default scrollback history length.
pub const DEFAULT_SCROLLING_HISTORY: usize = 10_000;

/// Upper bound for the scrollback history length.
pub const MAX_SCROLLING_HISTORY: usize = 1_000_000;

/// Options describing how to spawn a terminal session.
///
/// These control the *process and emulator* semantics. Presentation-level
/// behavior (cursor shape overrides, blink cadence, link annotation) lives in
/// the view layer's `TerminalViewOptions`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnOptions {
  /// The shell (or command) to run: `(program, args)`.
  ///
  /// `None` spawns the platform default shell. This can also be used to run
  /// a one-shot command, e.g. `("sh".into(), vec!["-c".into(), "ls".into()])`.
  pub shell: Option<(String, Vec<String>)>,
  /// Working directory for the spawned process.
  pub working_directory: Option<PathBuf>,
  /// Extra environment variables injected into the child process.
  pub env: Vec<(String, String)>,
  /// Scrollback history length in lines, clamped to [`MAX_SCROLLING_HISTORY`].
  pub scrolling_history: usize,
  /// The cursor shape used until the application changes it (e.g. via
  /// DECSCUSR) or hides the cursor.
  pub cursor_shape: CursorShape,
  /// Whether scrolling in the alternate screen maps to arrow-key sequences.
  pub alternate_scroll: bool,
  /// The set of characters that delimit a "word" for word-wise (double-click)
  /// selection and semantic selection.
  ///
  /// `None` uses the xterm default set plus the box-drawing character `─`.
  /// Applications never see this value; it only shapes local selection.
  pub word_separators: Option<String>,
}

impl SpawnOptions {
  /// Options for the platform default shell.
  pub fn default_shell_options() -> Self {
    Self {
      shell: None,
      ..Self::default()
    }
  }

  /// Options running a specific command (interactive shell if it stays alive).
  pub fn with_shell(shell: (String, Vec<String>)) -> Self {
    Self {
      shell: Some(shell),
      ..Self::default()
    }
  }

  /// Convenience constructor for one-shot commands, e.g. `("sh", ["-c", cmd])`.
  pub fn with_command(program: impl Into<String>, args: Vec<String>) -> Self {
    Self::with_shell((program.into(), args))
  }

  /// Sets the characters that delimit a word for double-click selection.
  pub fn with_word_separators(mut self, separators: impl Into<String>) -> Self {
    self.word_separators = Some(separators.into());
    self
  }

  pub(crate) fn history(&self) -> usize {
    self.scrolling_history.min(MAX_SCROLLING_HISTORY)
  }
}

impl Default for SpawnOptions {
  fn default() -> Self {
    Self {
      shell: None,
      working_directory: None,
      env: Vec::new(),
      scrolling_history: DEFAULT_SCROLLING_HISTORY,
      cursor_shape: CursorShape::Block,
      alternate_scroll: false,
      word_separators: None,
    }
  }
}

/// Returns the platform default shell as `(program, args)`.
///
/// - Unix: `$SHELL`, falling back to `/bin/sh`.
/// - Windows: PowerShell.
pub fn default_shell() -> (String, Vec<String>) {
  #[cfg(unix)]
  {
    (
      std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string()),
      Vec::new(),
    )
  }
  #[cfg(windows)]
  {
    ("powershell.exe".to_string(), Vec::new())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::types::CursorShape;

  #[test]
  fn history_is_clamped() {
    let mut options = SpawnOptions::default();
    assert_eq!(options.history(), DEFAULT_SCROLLING_HISTORY);

    options.scrolling_history = MAX_SCROLLING_HISTORY * 2;
    assert_eq!(options.history(), MAX_SCROLLING_HISTORY);
  }

  #[test]
  fn defaults() {
    let options = SpawnOptions::default_shell_options();
    assert_eq!(options.shell, None);
    assert_eq!(options.cursor_shape, CursorShape::Block);
    assert!(!options.alternate_scroll);
    assert_eq!(options.word_separators, None);
  }

  #[test]
  fn default_shell_is_sane() {
    let (program, args) = default_shell();
    assert!(!program.is_empty());
    assert!(args.is_empty());
  }

  #[test]
  fn word_separators_builder() {
    let options = SpawnOptions::default().with_word_separators(" ()[]");
    assert_eq!(options.word_separators.as_deref(), Some(" ()[]"));
  }
}
