pub mod opacity {
  pub const DISABLED: f32 = 0.6;

  pub mod transparent {
    pub const HOVER: f32 = 0.05;
    pub const ACTIVE: f32 = 0.1;
  }

  pub mod solid {
    pub const HOVER: f32 = 0.9;
    pub const ACTIVE: f32 = 0.8;
  }

  pub const MUTED_FOREGROUND: f32 = 0.75;
  pub const BORDER: f32 = 0.1;
}

pub mod duration {
  use std::time::Duration;

  pub const SPINNER: Duration = Duration::from_millis(800);
  pub const CARET_BLINK_ON: Duration = Duration::from_millis(700);
  pub const CARET_BLINK_OFF: Duration = Duration::from_millis(1200);
  pub const SWITCH_TOGGLE: Duration = Duration::from_millis(150);
  pub const NOTIFICATION_DEFAULT: Duration = Duration::from_secs(5);
  pub const ANIMATION_FRAME: Duration = Duration::from_millis(33);

  /// Checkbox indicator fade/color transition.
  pub const CHECKBOX_TOGGLE: Duration = Duration::from_millis(150);
  /// Radio indicator fill/border color transition.
  pub const RADIO_TOGGLE: Duration = Duration::from_millis(150);
  /// Slider/switch thumb grow and brighten on hover.
  pub const THUMB_HOVER: Duration = Duration::from_millis(150);
  /// Dialog backdrop fade + panel rise on open.
  pub const DIALOG_ENTER: Duration = Duration::from_millis(180);
  /// Popover content fade + slide on open.
  pub const POPOVER_ENTER: Duration = Duration::from_millis(120);
  /// Tooltip fade-in when shown.
  pub const TOOLTIP_ENTER: Duration = Duration::from_millis(100);
  /// Notification item slide-in on push.
  pub const NOTIFICATION_ENTER: Duration = Duration::from_millis(200);
  /// Notification item fade-out and collapse on close.
  pub const NOTIFICATION_EXIT: Duration = Duration::from_millis(160);
}
