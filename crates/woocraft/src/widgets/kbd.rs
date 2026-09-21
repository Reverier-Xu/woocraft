//! Visual representation of keyboard keys and keyboard shortcuts.
//!
//! Kbd displays a keystroke or key combination in a styled box, useful for
//! documenting keyboard shortcuts in help text, tooltips, or instructions.
//! Modifier keys (Ctrl, Shift, Alt, Cmd) and special keys (Enter, Escape,
//! etc.) are formatted with platform-specific symbols (⌘ on macOS, Ctrl
//! elsewhere).

use gpui::{
  Action, AsKeystroke, FocusHandle, IntoElement, KeyContext, Keystroke, ParentElement as _,
  RenderOnce, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative,
};

use crate::{ActiveTheme, base::StyledExt};

#[derive(IntoElement, Clone, Debug)]
/// Visual keyboard key or shortcut display.
///
/// Default appearance applies theme styling; `appearance(false)` removes it.
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

  /// Looks up the highest-precedence keybinding for an action and creates a
  /// Kbd from it.
  ///
  /// Returns `None` if the action has no keybinding. Optionally filters by
  /// key context (e.g., "vim", "editor").
  pub fn binding_for_action(
    action: &dyn Action, context: Option<&str>, window: &Window,
  ) -> Option<Self> {
    let key_context = context.and_then(|context| KeyContext::parse(context).ok());
    let binding = match key_context {
      Some(context) => window.highest_precedence_binding_for_action_in_context(action, context),
      None => window.highest_precedence_binding_for_action(action),
    }?;

    binding
      .keystrokes()
      .first()
      .map(|key| Self::new(key.as_keystroke().clone()))
  }

  /// Looks up the highest-precedence keybinding for an action in a specific
  /// focus context.
  ///
  /// Returns `None` if the action has no keybinding in that focus context.
  pub fn binding_for_action_in(
    action: &dyn Action, focus_handle: &FocusHandle, window: &Window,
  ) -> Option<Self> {
    let binding = window.highest_precedence_binding_for_action_in(action, focus_handle)?;
    binding
      .keystrokes()
      .first()
      .map(|key| Self::new(key.as_keystroke().clone()))
  }

  /// Formats a keystroke for human-readable display.
  ///
  /// Combines modifiers (Ctrl, Shift, Alt, Cmd) with the key, using
  /// platform-specific symbols (⌘ on macOS, Ctrl on others) and special
  /// key names (Space, Enter, Esc, Backspace, etc.).
  pub fn format(key: &Keystroke) -> String {
    #[cfg(target_os = "macos")]
    const DIVIDER: &str = "";
    #[cfg(not(target_os = "macos"))]
    const DIVIDER: &str = "+";

    let mut parts = vec![];

    if key.modifiers.control {
      #[cfg(target_os = "macos")]
      parts.push("⌃");

      #[cfg(not(target_os = "macos"))]
      parts.push("Ctrl");
    }

    if key.modifiers.alt {
      #[cfg(target_os = "macos")]
      parts.push("⌥");

      #[cfg(not(target_os = "macos"))]
      parts.push("Alt");
    }

    if key.modifiers.shift {
      #[cfg(target_os = "macos")]
      parts.push("⇧");

      #[cfg(not(target_os = "macos"))]
      parts.push("Shift");
    }

    if key.modifiers.platform {
      #[cfg(target_os = "macos")]
      parts.push("⌘");

      #[cfg(not(target_os = "macos"))]
      parts.push("Win");
    }

    let mut keys = String::new();
    let key_str = key.key.as_str();
    match key_str {
      #[cfg(target_os = "macos")]
      "ctrl" => keys.push('⌃'),
      #[cfg(not(target_os = "macos"))]
      "ctrl" => keys.push_str("Ctrl"),
      #[cfg(target_os = "macos")]
      "alt" => keys.push('⌥'),
      #[cfg(not(target_os = "macos"))]
      "alt" => keys.push_str("Alt"),
      #[cfg(target_os = "macos")]
      "shift" => keys.push('⇧'),
      #[cfg(not(target_os = "macos"))]
      "shift" => keys.push_str("Shift"),
      #[cfg(target_os = "macos")]
      "cmd" => keys.push('⌘'),
      #[cfg(not(target_os = "macos"))]
      "cmd" => keys.push_str("Win"),
      "space" => keys.push_str("Space"),
      #[cfg(target_os = "macos")]
      "backspace" => keys.push('⌫'),
      #[cfg(not(target_os = "macos"))]
      "backspace" => keys.push_str("Backspace"),
      #[cfg(target_os = "macos")]
      "delete" => keys.push('⌫'),
      #[cfg(not(target_os = "macos"))]
      "delete" => keys.push_str("Delete"),
      #[cfg(target_os = "macos")]
      "escape" => keys.push('⎋'),
      #[cfg(not(target_os = "macos"))]
      "escape" => keys.push_str("Esc"),
      #[cfg(target_os = "macos")]
      "enter" => keys.push('⏎'),
      #[cfg(not(target_os = "macos"))]
      "enter" => keys.push_str("Enter"),
      "pagedown" => keys.push_str("Page Down"),
      "pageup" => keys.push_str("Page Up"),
      #[cfg(target_os = "macos")]
      "left" => keys.push('←'),
      #[cfg(not(target_os = "macos"))]
      "left" => keys.push_str("Left"),
      #[cfg(target_os = "macos")]
      "right" => keys.push('→'),
      #[cfg(not(target_os = "macos"))]
      "right" => keys.push_str("Right"),
      #[cfg(target_os = "macos")]
      "up" => keys.push('↑'),
      #[cfg(not(target_os = "macos"))]
      "up" => keys.push_str("Up"),
      #[cfg(target_os = "macos")]
      "down" => keys.push('↓'),
      #[cfg(not(target_os = "macos"))]
      "down" => keys.push_str("Down"),
      _ => {
        if key_str.len() == 1 {
          keys.push_str(&key_str.to_uppercase());
        } else {
          let mut chars = key_str.chars();
          if let Some(first_char) = chars.next() {
            keys.push_str(&format!(
              "{}{}",
              first_char.to_uppercase(),
              chars.collect::<String>()
            ));
          } else {
            keys.push_str(key_str);
          }
        }
      }
    }

    parts.push(&keys);
    parts.join(DIVIDER)
  }
}

impl Styled for Kbd {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

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
          .border(cx.theme().border_width)
          .border_color(cx.theme().border)
          .bg(cx.theme().background)
      })
      .py_0p5()
      .px_1()
      .min_w_5()
      .text_center()
      .rounded(cx.theme().radius)
      .line_height(relative(1.))
      .whitespace_normal()
      .flex_shrink_0()
      .refine_style(&self.style)
      .child(Self::format(&self.stroke))
      .into_any_element()
  }
}

#[cfg(test)]
mod tests {
  use gpui::Keystroke;

  use super::Kbd;

  #[test]
  fn modifiers_format_with_platform_divider() {
    let stroke = Keystroke::parse("ctrl-alt-delete").unwrap();
    #[cfg(target_os = "macos")]
    let expected = "⌃⌥⌫".to_string();
    #[cfg(not(target_os = "macos"))]
    let expected = "Ctrl+Alt+Delete";

    assert_eq!(Kbd::format(&stroke), expected);
  }

  #[test]
  fn single_characters_uppercase() {
    let stroke = Keystroke::parse("a").unwrap();
    assert_eq!(Kbd::format(&stroke), "A");
  }

  #[test]
  fn plain_words_keep_casing_via_first_capital() {
    let stroke = Keystroke::parse("pageup").unwrap();
    #[cfg(target_os = "macos")]
    let expected = "Page Up";
    #[cfg(not(target_os = "macos"))]
    let expected = "Page Up";

    assert_eq!(Kbd::format(&stroke), expected);
  }
}
