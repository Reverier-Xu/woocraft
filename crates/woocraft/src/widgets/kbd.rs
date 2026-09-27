//! Visual representation of keyboard keys and keyboard shortcuts.
//!
//! Kbd displays a keystroke or key combination in a styled box, useful for
//! documenting keyboard shortcuts in help text, tooltips, or instructions.
//! Automatically formats modifier keys (Ctrl, Shift, Alt, Cmd) and special keys
//! (Enter, Escape, etc.) with platform-specific symbols (e.g., ⌘ on macOS, Ctrl
//! on Linux/Windows).

use gpui::{
  Action, AsKeystroke, FocusHandle, IntoElement, KeyBinding, KeyContext, Keystroke,
  ParentElement as _, RenderOnce, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _, relative,
};

use crate::{ActiveTheme, StyledExt};

/// Platform-specific display symbols for special keys and modifier names,
/// keyed by the lowercase gpui key name. The modifier pass in `format` looks
/// up the same table ("ctrl"/"alt"/"shift", plus "cmd" for the platform
/// modifier) so modifiers and standalone modifier keys always render
/// identically.
#[cfg(target_os = "macos")]
const SPECIAL_KEY_SYMBOLS: &[(&str, &str)] = &[
  ("ctrl", "⌃"),
  ("alt", "⌥"),
  ("shift", "⇧"),
  ("cmd", "⌘"),
  ("space", "Space"),
  ("backspace", "⌫"),
  ("delete", "⌫"),
  ("escape", "⎋"),
  ("enter", "⏎"),
  ("pagedown", "Page Down"),
  ("pageup", "Page Up"),
  ("left", "←"),
  ("right", "→"),
  ("up", "↑"),
  ("down", "↓"),
];

#[cfg(not(target_os = "macos"))]
const SPECIAL_KEY_SYMBOLS: &[(&str, &str)] = &[
  ("ctrl", "Ctrl"),
  ("alt", "Alt"),
  ("shift", "Shift"),
  ("cmd", "Win"),
  ("backspace", "Backspace"),
  ("delete", "Delete"),
  ("escape", "Esc"),
  ("enter", "Enter"),
  ("pagedown", "Page Down"),
  ("pageup", "Page Up"),
  ("left", "Left"),
  ("right", "Right"),
  ("up", "Up"),
  ("down", "Down"),
];

/// Looks up the display symbol for a special key or modifier name; returns
/// `None` for regular keys, which are capitalized for display instead.
fn special_key_symbol(key: &str) -> Option<&'static str> {
  SPECIAL_KEY_SYMBOLS
    .iter()
    .find(|(name, _)| *name == key)
    .map(|(_, symbol)| *symbol)
}

/// Capitalizes a regular key name for display ("home" -> "Home"); single
/// characters are fully uppercased.
fn capitalize_first(key: &str) -> String {
  if key.len() == 1 {
    return key.to_uppercase();
  }

  let mut chars = key.chars();
  match chars.next() {
    Some(first) => format!("{}{}", first.to_uppercase(), chars.collect::<String>()),
    None => key.to_string(),
  }
}

#[derive(IntoElement, Clone, Debug)]
/// Visual keyboard key or shortcut display.
///
/// Renders a keystroke in a styled box with platform-appropriate formatting.
/// Default appearance applies theme styling; `appearance(false)` removes
/// styling.
pub struct Kbd {
  style: StyleRefinement,
  stroke: Keystroke,
  appearance: bool,
  outline: bool,
}

impl From<Keystroke> for Kbd {
  fn from(stroke: Keystroke) -> Self {
    Self {
      style: StyleRefinement::default(),
      stroke,
      appearance: true,
      outline: false,
    }
  }
}

impl Kbd {
  /// Creates a new keyboard key display for the given keystroke.
  ///
  /// Default uses theme styling and platform-appropriate key symbols.
  pub fn new(stroke: Keystroke) -> Self {
    Self {
      style: StyleRefinement::default(),
      stroke,
      appearance: true,
      outline: false,
    }
  }

  /// Toggles the themed appearance (rounded box with background).
  ///
  /// When `false`, renders plain undecorated text with no styling.
  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  /// Switches to outline appearance (transparent background, bordered).
  pub fn outline(mut self) -> Self {
    self.outline = true;
    self
  }

  /// Looks up the highest-precedence keybinding for an action and creates a Kbd
  /// from it.
  ///
  /// Returns `None` if the action has no keybinding.
  /// Optionally filters by key context (e.g., "vim", "editor").
  pub fn binding_for_action(
    action: &dyn Action, context: Option<&str>, window: &Window,
  ) -> Option<Self> {
    let key_context = context.and_then(|context| KeyContext::parse(context).ok());
    let binding = match key_context {
      Some(context) => window.highest_precedence_binding_for_action_in_context(action, context),
      None => window.highest_precedence_binding_for_action(action),
    }?;

    Self::from_binding(&binding)
  }

  /// Looks up the highest-precedence keybinding for an action in a specific
  /// focus context.
  ///
  /// Returns `None` if the action has no keybinding in that focus context.
  pub fn binding_for_action_in(
    action: &dyn Action, focus_handle: &FocusHandle, window: &Window,
  ) -> Option<Self> {
    Self::from_binding(&window.highest_precedence_binding_for_action_in(action, focus_handle)?)
  }

  /// Extracts the first keystroke of a binding and wraps it in a Kbd.
  fn from_binding(binding: &KeyBinding) -> Option<Self> {
    binding
      .keystrokes()
      .first()
      .map(|key| Self::new(key.as_keystroke().clone()))
  }

  /// Formats a keystroke for human-readable display.
  ///
  /// Combines modifiers (Ctrl, Shift, Alt, Cmd) with the key, using
  /// platform-specific symbols (⌘ on macOS, Ctrl on others) and special
  /// key names (Space, Enter, Esc, Backspace, etc.) formatted appropriately.
  pub fn format(key: &Keystroke) -> String {
    #[cfg(target_os = "macos")]
    const DIVIDER: &str = "";
    #[cfg(not(target_os = "macos"))]
    const DIVIDER: &str = "+";

    let mut parts = Vec::new();
    // Modifier display order: Ctrl, Alt, Shift, then the platform key.
    for (enabled, name) in [
      (key.modifiers.control, "ctrl"),
      (key.modifiers.alt, "alt"),
      (key.modifiers.shift, "shift"),
      (key.modifiers.platform, "cmd"),
    ] {
      if enabled && let Some(symbol) = special_key_symbol(name) {
        parts.push(symbol);
      }
    }

    let key_str = key.key.as_str();
    let keys = special_key_symbol(key_str)
      .map(str::to_string)
      .unwrap_or_else(|| capitalize_first(key_str));

    parts.push(&keys);
    parts.join(DIVIDER)
  }
}

impl_styled!(Kbd);

impl RenderOnce for Kbd {
  fn render(self, _: &mut gpui::Window, cx: &mut gpui::App) -> impl gpui::IntoElement {
    if !self.appearance {
      return Self::format(&self.stroke).into_any_element();
    }

    div()
      .text_color(cx.theme().muted_foreground)
      .bg(cx.theme().muted)
      .when(self.outline, |this| {
        this
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().background)
      })
      .py_0p5()
      .px_1()
      .min_w_5()
      .text_center()
      .rounded_sm()
      .line_height(relative(1.))
      .text_xs()
      .whitespace_normal()
      .flex_shrink_0()
      .refine_style(&self.style)
      .child(Self::format(&self.stroke))
      .into_any_element()
  }
}
