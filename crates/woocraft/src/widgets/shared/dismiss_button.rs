use gpui::{App, ClickEvent, IntoElement, RenderOnce, Window};

use crate::{Button, ButtonVariants, IconName, Sizable, Size};

pub type DismissHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

#[derive(IntoElement)]
pub struct DismissButton {
  size: Size,
  on_click: Option<DismissHandler>,
}

impl Default for DismissButton {
  fn default() -> Self {
    Self::new()
  }
}

impl DismissButton {
  pub fn new() -> Self {
    Self {
      size: Size::Small,
      on_click: None,
    }
  }

  pub fn size(mut self, size: Size) -> Self {
    self.size = size;
    self
  }

  pub fn on_click(
    mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_click = Some(Box::new(handler));
    self
  }
}

impl Sizable for DismissButton {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl RenderOnce for DismissButton {
  fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
    let mut btn = Button::new("dismiss")
      .icon(IconName::Dismiss)
      .flat()
      .with_size(self.size);

    if let Some(handler) = self.on_click {
      btn = btn.on_click(handler);
    }

    btn
  }
}
