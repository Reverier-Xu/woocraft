//! Minimal GPUI component primitives for Woocraft.

pub const SPACING_STEP_PX: u8 = 4;
pub const CONTAINER_HEIGHT_CLASS: &str = "h-10";
pub const CONTROL_HEIGHT_CLASS: &str = "h-8";
pub const WINDOW_RADIUS_CLASS: &str = "rounded-xl";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
  pub spacing_step_px: u8,
  pub container_height_class: &'static str,
  pub control_height_class: &'static str,
  pub window_radius_class: &'static str,
}

impl Default for Theme {
  fn default() -> Self {
    Self {
      spacing_step_px: SPACING_STEP_PX,
      container_height_class: CONTAINER_HEIGHT_CLASS,
      control_height_class: CONTROL_HEIGHT_CLASS,
      window_radius_class: WINDOW_RADIUS_CLASS,
    }
  }
}

pub trait GpuiStylable {
  fn class_name(&self, theme: &Theme) -> String;
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Button {
  pub style_overrides: String,
}

impl GpuiStylable for Button {
  fn class_name(&self, theme: &Theme) -> String {
    class_with_override(
      &format!(
        "{} inline-flex items-center px-3",
        theme.control_height_class
      ),
      &self.style_overrides,
    )
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Input {
  pub style_overrides: String,
}

impl GpuiStylable for Input {
  fn class_name(&self, theme: &Theme) -> String {
    class_with_override(
      &format!("{} w-full px-3", theme.control_height_class),
      &self.style_overrides,
    )
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Select {
  pub style_overrides: String,
}

impl GpuiStylable for Select {
  fn class_name(&self, theme: &Theme) -> String {
    class_with_override(
      &format!("{} w-full px-3", theme.control_height_class),
      &self.style_overrides,
    )
  }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WindowChrome {
  pub style_overrides: String,
}

impl GpuiStylable for WindowChrome {
  fn class_name(&self, theme: &Theme) -> String {
    class_with_override(
      &format!("{} overflow-hidden", theme.window_radius_class),
      &self.style_overrides,
    )
  }
}

pub fn class_with_override(base: &str, style_overrides: &str) -> String {
  if style_overrides.trim().is_empty() {
    return base.to_owned();
  }

  format!("{base} {style_overrides}")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn theme_defaults_match_single_size_design_spec() {
    let theme = Theme::default();
    assert_eq!(theme.spacing_step_px, 4);
    assert_eq!(theme.container_height_class, "h-10");
    assert_eq!(theme.control_height_class, "h-8");
    assert_eq!(theme.window_radius_class, "rounded-xl");
  }

  #[test]
  fn controls_share_single_height_class() {
    let theme = Theme::default();
    assert!(Button::default().class_name(&theme).contains("h-8"));
    assert!(Input::default().class_name(&theme).contains("h-8"));
    assert!(Select::default().class_name(&theme).contains("h-8"));
  }

  #[test]
  fn style_overrides_are_appended_for_gpui_customization() {
    let theme = Theme::default();
    let button = Button {
      style_overrides: "bg-brand text-white".to_owned(),
    };

    assert_eq!(
      button.class_name(&theme),
      "h-8 inline-flex items-center px-3 bg-brand text-white"
    );
  }

  #[test]
  fn window_uses_rounded_corners() {
    let theme = Theme::default();
    let classes = WindowChrome::default().class_name(&theme);

    assert!(classes.contains("rounded-xl"));
  }
}
