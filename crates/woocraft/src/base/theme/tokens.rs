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
}
