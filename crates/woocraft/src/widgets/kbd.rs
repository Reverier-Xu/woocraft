use gpui::{
  Action, AsKeystroke, FocusHandle, IntoElement, KeyContext, Keystroke, ParentElement as _,
  RenderOnce, StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _, relative,
};

use crate::{ActiveTheme, StyledExt};

#[derive(IntoElement, Clone, Debug)]
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
  pub fn new(stroke: Keystroke) -> Self {
    Self {
      style: StyleRefinement::default(),
      stroke,
      appearance: true,
      outline: false,
    }
  }

  pub fn appearance(mut self, appearance: bool) -> Self {
    self.appearance = appearance;
    self
  }

  pub fn outline(mut self) -> Self {
    self.outline = true;
    self
  }

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

  pub fn binding_for_action_in(
    action: &dyn Action, focus_handle: &FocusHandle, window: &Window,
  ) -> Option<Self> {
    let binding = window.highest_precedence_binding_for_action_in(action, focus_handle)?;
    binding
      .keystrokes()
      .first()
      .map(|key| Self::new(key.as_keystroke().clone()))
  }

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
      #[cfg(target_os = "macos")]
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
