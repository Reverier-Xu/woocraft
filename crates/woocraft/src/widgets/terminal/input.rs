//! Keyboard input mapping: GPUI keystrokes to terminal escape sequences.
//!
//! The mappings follow the xterm control sequences that Alacritty implements
//! (application cursor keys, PC-style function key modifier codes, caret
//! notation for ctrl combos, and alt-as-meta handling).

use std::borrow::Cow;

use gpui::Keystroke;
use woocraft_terminal::Modes;

/// The modifier combinations that map to distinct escape sequences.
#[derive(Debug, PartialEq, Eq)]
enum TerminalModifiers {
  None,
  Alt,
  Ctrl,
  Shift,
  CtrlShift,
  Other,
}

impl TerminalModifiers {
  fn new(keystroke: &Keystroke) -> Self {
    match (
      keystroke.modifiers.alt,
      keystroke.modifiers.control,
      keystroke.modifiers.shift,
    ) {
      (false, false, false) => Self::None,
      (true, false, false) => Self::Alt,
      (false, true, false) => Self::Ctrl,
      (false, false, true) => Self::Shift,
      (false, true, true) => Self::CtrlShift,
      _ => Self::Other,
    }
  }

  fn any(&self) -> bool {
    !matches!(self, Self::None)
  }
}

/// Maps a keystroke to the escape sequence that should be written to the PTY.
///
/// Returns `None` when the keystroke has no terminal-specific meaning; plain
/// printable characters are delivered through the input handler (IME) instead.
pub fn to_esc_str(
  keystroke: &Keystroke, mode: Modes, option_as_meta: bool,
) -> Option<Cow<'static, str>> {
  let modifiers = TerminalModifiers::new(keystroke);

  // Manual bindings, including modifier-specific ones.
  let manual_esc_str: Option<&'static str> = match (keystroke.key.as_ref(), &modifiers) {
    // Basic special keys.
    ("tab", TerminalModifiers::None) => Some("\x09"),
    ("escape", TerminalModifiers::None) => Some("\x1b"),
    ("enter", TerminalModifiers::None) => Some("\x0d"),
    ("enter", TerminalModifiers::Shift) => Some("\x0a"),
    ("enter", TerminalModifiers::Alt) => Some("\x1b\x0d"),
    ("backspace", TerminalModifiers::None) => Some("\x7f"),
    // Interesting escape codes.
    ("tab", TerminalModifiers::Shift) => Some("\x1b[Z"),
    ("backspace", TerminalModifiers::Ctrl) => Some("\x08"),
    ("backspace", TerminalModifiers::Alt) => Some("\x1b\x7f"),
    ("backspace", TerminalModifiers::Shift) => Some("\x7f"),
    ("space", TerminalModifiers::Ctrl) => Some("\x00"),
    ("home", TerminalModifiers::None) if mode.contains(Modes::APP_CURSOR) => Some("\x1bOH"),
    ("home", TerminalModifiers::None) => Some("\x1b[H"),
    ("end", TerminalModifiers::None) if mode.contains(Modes::APP_CURSOR) => Some("\x1bOF"),
    ("end", TerminalModifiers::None) => Some("\x1b[F"),
    ("up", TerminalModifiers::None) if mode.contains(Modes::APP_CURSOR) => Some("\x1bOA"),
    ("up", TerminalModifiers::None) => Some("\x1b[A"),
    ("down", TerminalModifiers::None) if mode.contains(Modes::APP_CURSOR) => Some("\x1bOB"),
    ("down", TerminalModifiers::None) => Some("\x1b[B"),
    ("right", TerminalModifiers::None) if mode.contains(Modes::APP_CURSOR) => Some("\x1bOC"),
    ("right", TerminalModifiers::None) => Some("\x1b[C"),
    ("left", TerminalModifiers::None) if mode.contains(Modes::APP_CURSOR) => Some("\x1bOD"),
    ("left", TerminalModifiers::None) => Some("\x1b[D"),
    ("back", TerminalModifiers::None) => Some("\x7f"),
    ("insert", TerminalModifiers::None) => Some("\x1b[2~"),
    ("delete", TerminalModifiers::None) => Some("\x1b[3~"),
    ("pageup", TerminalModifiers::None) => Some("\x1b[5~"),
    ("pagedown", TerminalModifiers::None) => Some("\x1b[6~"),
    ("f1", TerminalModifiers::None) => Some("\x1bOP"),
    ("f2", TerminalModifiers::None) => Some("\x1bOQ"),
    ("f3", TerminalModifiers::None) => Some("\x1bOR"),
    ("f4", TerminalModifiers::None) => Some("\x1bOS"),
    ("f5", TerminalModifiers::None) => Some("\x1b[15~"),
    ("f6", TerminalModifiers::None) => Some("\x1b[17~"),
    ("f7", TerminalModifiers::None) => Some("\x1b[18~"),
    ("f8", TerminalModifiers::None) => Some("\x1b[19~"),
    ("f9", TerminalModifiers::None) => Some("\x1b[20~"),
    ("f10", TerminalModifiers::None) => Some("\x1b[21~"),
    ("f11", TerminalModifiers::None) => Some("\x1b[23~"),
    ("f12", TerminalModifiers::None) => Some("\x1b[24~"),
    ("f13", TerminalModifiers::None) => Some("\x1b[25~"),
    ("f14", TerminalModifiers::None) => Some("\x1b[26~"),
    ("f15", TerminalModifiers::None) => Some("\x1b[28~"),
    ("f16", TerminalModifiers::None) => Some("\x1b[29~"),
    ("f17", TerminalModifiers::None) => Some("\x1b[31~"),
    ("f18", TerminalModifiers::None) => Some("\x1b[32~"),
    ("f19", TerminalModifiers::None) => Some("\x1b[33~"),
    ("f20", TerminalModifiers::None) => Some("\x1b[34~"),
    // Caret notation for ctrl-combos (1 = ^A .. 26 = ^Z).
    //
    // Note: the guard skips non-letters so the punctuation arms below still
    // apply.
    (key, TerminalModifiers::Ctrl)
      if key.len() == 1
        && key
          .chars()
          .next()
          .is_some_and(|key| key.is_ascii_lowercase()) =>
    {
      let key = key.chars().next()?;
      Some(C0_OFFSETS[(key as u8 - b'a') as usize])
    }
    ("A", TerminalModifiers::CtrlShift) => Some("\x01"),
    ("B", TerminalModifiers::CtrlShift) => Some("\x02"),
    ("C", TerminalModifiers::CtrlShift) => Some("\x03"),
    ("D", TerminalModifiers::CtrlShift) => Some("\x04"),
    ("E", TerminalModifiers::CtrlShift) => Some("\x05"),
    ("F", TerminalModifiers::CtrlShift) => Some("\x06"),
    ("G", TerminalModifiers::CtrlShift) => Some("\x07"),
    ("H", TerminalModifiers::CtrlShift) => Some("\x08"),
    ("I", TerminalModifiers::CtrlShift) => Some("\x09"),
    ("J", TerminalModifiers::CtrlShift) => Some("\x0a"),
    ("K", TerminalModifiers::CtrlShift) => Some("\x0b"),
    ("L", TerminalModifiers::CtrlShift) => Some("\x0c"),
    ("M", TerminalModifiers::CtrlShift) => Some("\x0d"),
    ("N", TerminalModifiers::CtrlShift) => Some("\x0e"),
    ("O", TerminalModifiers::CtrlShift) => Some("\x0f"),
    ("P", TerminalModifiers::CtrlShift) => Some("\x10"),
    ("Q", TerminalModifiers::CtrlShift) => Some("\x11"),
    ("R", TerminalModifiers::CtrlShift) => Some("\x12"),
    ("S", TerminalModifiers::CtrlShift) => Some("\x13"),
    ("T", TerminalModifiers::CtrlShift) => Some("\x14"),
    ("U", TerminalModifiers::CtrlShift) => Some("\x15"),
    ("V", TerminalModifiers::CtrlShift) => Some("\x16"),
    ("W", TerminalModifiers::CtrlShift) => Some("\x17"),
    ("X", TerminalModifiers::CtrlShift) => Some("\x18"),
    ("Y", TerminalModifiers::CtrlShift) => Some("\x19"),
    ("Z", TerminalModifiers::CtrlShift) => Some("\x1a"),
    ("@", TerminalModifiers::Ctrl) => Some("\x00"),
    ("[", TerminalModifiers::Ctrl) => Some("\x1b"),
    ("\\", TerminalModifiers::Ctrl) => Some("\x1c"),
    ("]", TerminalModifiers::Ctrl) => Some("\x1d"),
    ("^", TerminalModifiers::Ctrl) => Some("\x1e"),
    ("_", TerminalModifiers::Ctrl) => Some("\x1f"),
    ("?", TerminalModifiers::Ctrl) => Some("\x7f"),
    _ => None,
  };
  if let Some(esc_str) = manual_esc_str {
    return Some(Cow::Borrowed(esc_str));
  }

  // Generated bindings applying the modifier code to known keys.
  if modifiers.any() {
    let code = modifier_code(keystroke);
    let modified_esc_str = match keystroke.key.as_ref() {
      "up" => Some(format!("\x1b[1;{code}A")),
      "down" => Some(format!("\x1b[1;{code}B")),
      "right" => Some(format!("\x1b[1;{code}C")),
      "left" => Some(format!("\x1b[1;{code}D")),
      "f1" => Some(format!("\x1b[1;{code}P")),
      "f2" => Some(format!("\x1b[1;{code}Q")),
      "f3" => Some(format!("\x1b[1;{code}R")),
      "f4" => Some(format!("\x1b[1;{code}S")),
      "f5" => Some(format!("\x1b[15;{code}~")),
      "f6" => Some(format!("\x1b[17;{code}~")),
      "f7" => Some(format!("\x1b[18;{code}~")),
      "f8" => Some(format!("\x1b[19;{code}~")),
      "f9" => Some(format!("\x1b[20;{code}~")),
      "f10" => Some(format!("\x1b[21;{code}~")),
      "f11" => Some(format!("\x1b[23;{code}~")),
      "f12" => Some(format!("\x1b[24;{code}~")),
      "f13" => Some(format!("\x1b[25;{code}~")),
      "f14" => Some(format!("\x1b[26;{code}~")),
      "f15" => Some(format!("\x1b[28;{code}~")),
      "f16" => Some(format!("\x1b[29;{code}~")),
      "f17" => Some(format!("\x1b[31;{code}~")),
      "f18" => Some(format!("\x1b[32;{code}~")),
      "f19" => Some(format!("\x1b[33;{code}~")),
      "f20" => Some(format!("\x1b[34;{code}~")),
      "insert" => Some(format!("\x1b[2;{code}~")),
      "pageup" => Some(format!("\x1b[5;{code}~")),
      "pagedown" => Some(format!("\x1b[6;{code}~")),
      "end" => Some(format!("\x1b[1;{code}F")),
      "home" => Some(format!("\x1b[1;{code}H")),
      _ => None,
    };
    if let Some(esc_str) = modified_esc_str {
      return Some(Cow::Owned(esc_str));
    }
  }

  // Alt acts as meta for single ASCII characters.
  if (!cfg!(target_os = "macos") || option_as_meta)
    && keystroke.modifiers.alt
    && keystroke.key.is_ascii()
    && keystroke.key.len() == 1
  {
    let key = keystroke.key.chars().next()?;

    if modifiers == TerminalModifiers::Alt {
      return Some(Cow::Owned(format!("\x1b{key}")));
    } else if keystroke.modifiers.shift {
      return Some(Cow::Owned(format!("\x1b{}", key.to_ascii_uppercase())));
    } else if keystroke.modifiers.control && key.is_ascii_lowercase() {
      let code = (key as u8 - b'a' + 1) as char;
      return Some(Cow::Owned(format!("\x1b{code}")));
    }
  }

  None
}

/// The caret-notation byte for each ctrl-combo, `ctrl-a` (0x01) through
/// `ctrl-z` (0x1a).
const C0_OFFSETS: [&str; 26] = [
  "\x01", "\x02", "\x03", "\x04", "\x05", "\x06", "\x07", "\x08", "\x09", "\x0a", "\x0b", "\x0c",
  "\x0d", "\x0e", "\x0f", "\x10", "\x11", "\x12", "\x13", "\x14", "\x15", "\x16", "\x17", "\x18",
  "\x19", "\x1a",
];

/// The xterm "PC-style function key" modifier parameter.
///
/// ```text
///   Code     Modifiers
/// ---------+---------------------------
///    2     | Shift
///    3     | Alt
///    4     | Shift + Alt
///    5     | Control
///    6     | Shift + Control
///    7     | Alt + Control
///    8     | Shift + Alt + Control
/// ---------+---------------------------
/// ```
fn modifier_code(keystroke: &Keystroke) -> u32 {
  let mut code = 0;
  if keystroke.modifiers.shift {
    code |= 1;
  }
  if keystroke.modifiers.alt {
    code |= 1 << 1;
  }
  if keystroke.modifiers.control {
    code |= 1 << 2;
  }
  code + 1
}

#[cfg(test)]
mod tests {
  use gpui::Keystroke;

  use super::*;

  fn esc(keystroke: &str) -> Option<String> {
    to_esc_str(&Keystroke::parse(keystroke).unwrap(), Modes::NONE, false)
      .map(|cow| cow.into_owned())
  }

  #[test]
  fn plain_special_keys() {
    assert_eq!(esc("tab").as_deref(), Some("\x09"));
    assert_eq!(esc("escape").as_deref(), Some("\x1b"));
    assert_eq!(esc("enter").as_deref(), Some("\x0d"));
    assert_eq!(esc("shift-enter").as_deref(), Some("\x0a"));
    assert_eq!(esc("backspace").as_deref(), Some("\x7f"));
    assert_eq!(esc("delete").as_deref(), Some("\x1b[3~"));
    assert_eq!(esc("pageup").as_deref(), Some("\x1b[5~"));
  }

  #[test]
  fn application_cursor_mode() {
    assert_eq!(esc("up").as_deref(), Some("\x1b[A"));
    let mode = Modes::APP_CURSOR;
    assert_eq!(
      to_esc_str(&Keystroke::parse("up").unwrap(), mode, false).as_deref(),
      Some("\x1bOA")
    );
    assert_eq!(
      to_esc_str(&Keystroke::parse("home").unwrap(), mode, false).as_deref(),
      Some("\x1bOH")
    );
    assert_eq!(
      to_esc_str(&Keystroke::parse("end").unwrap(), mode, false).as_deref(),
      Some("\x1bOF")
    );
  }

  #[test]
  fn ctrl_caret_codes() {
    assert_eq!(esc("ctrl-a").as_deref(), Some("\x01"));
    assert_eq!(esc("ctrl-z").as_deref(), Some("\x1a"));
    assert_eq!(esc("ctrl-@").as_deref(), Some("\x00"));
    assert_eq!(esc("ctrl-[").as_deref(), Some("\x1b"));
    assert_eq!(esc("ctrl-?").as_deref(), Some("\x7f"));
    // Shifted letter combos normalize identically regardless of spelling.
    for (lower, upper) in ('a'..='z').zip('A'..='Z') {
      assert_eq!(
        esc(&format!("ctrl-shift-{lower}")),
        esc(&format!("ctrl-{upper}")),
        "letter {lower}/{upper}"
      );
    }
  }

  #[test]
  fn modifier_codes() {
    assert_eq!(esc("shift-up").as_deref(), Some("\x1b[1;2A"));
    assert_eq!(esc("ctrl-up").as_deref(), Some("\x1b[1;5A"));
    assert_eq!(esc("shift-ctrl-alt-up").as_deref(), Some("\x1b[1;8A"));
    assert_eq!(esc("shift-pagedown").as_deref(), Some("\x1b[6;2~"));
    assert_eq!(esc("shift-end").as_deref(), Some("\x1b[1;2F"));
    assert_eq!(esc("alt-f5").as_deref(), Some("\x1b[15;3~"));
  }

  #[test]
  fn alt_is_meta_for_ascii() {
    assert_eq!(esc("alt-x").as_deref(), Some("\x1bx"));
    assert_eq!(esc("alt-shift-x").as_deref(), Some("\x1bX"));
    assert_eq!(esc("alt-ctrl-x").as_deref(), Some("\x1b\x18"));
    // Non-ASCII keys are untouched.
    assert_eq!(esc("alt-键盘"), None);
  }

  #[test]
  fn printable_and_unknown_keys_pass_through() {
    assert_eq!(esc("a"), None);
    assert_eq!(esc("b"), None);
    assert_eq!(esc("space"), None);
    // Multi-char keys never map to anything.
    assert_eq!(esc("🙃"), None);
  }
}
