//! Options for spawning terminal sessions.

use std::path::PathBuf;

use crate::types::CursorShape;

/// Default scrollback history length.
pub const DEFAULT_SCROLLING_HISTORY: usize = 10_000;

/// Upper bound for the scrollback history length.
pub const MAX_SCROLLING_HISTORY: usize = 1_000_000;

/// The default cursor shapes a session can be configured with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShapeKind {
  #[default]
  Block,
  Underline,
  Bar,
  Hollow,
}

impl From<CursorShapeKind> for CursorShape {
  fn from(kind: CursorShapeKind) -> Self {
    match kind {
      CursorShapeKind::Block => CursorShape::Block,
      CursorShapeKind::Underline => CursorShape::Underline,
      CursorShapeKind::Bar => CursorShape::Bar,
      CursorShapeKind::Hollow => CursorShape::HollowBlock,
    }
  }
}

/// Options describing how to spawn a terminal session.
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
  /// Initial cursor shape.
  pub cursor_shape: CursorShapeKind,
  /// Whether scrolling in the alternate screen maps to arrow-key sequences.
  pub alternate_scroll: bool,
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
      cursor_shape: CursorShapeKind::default(),
      alternate_scroll: false,
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
    assert_eq!(options.cursor_shape, CursorShapeKind::Block);
    assert!(!options.alternate_scroll);
  }

  #[test]
  fn default_shell_is_sane() {
    let (program, args) = default_shell();
    assert!(!program.is_empty());
    assert!(args.is_empty());
  }
}
