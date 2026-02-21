use gpui::{linear, Animation};

use crate::base::theme::duration;

pub fn spinner_animation() -> Animation {
  Animation::new(duration::SPINNER)
    .repeat()
    .with_easing(linear)
}
