use gpuim::{IntoElement, ParentElement as _, RenderOnce, Styled};

use super::ListItem;
use crate::{Spinner, h_flex};

#[derive(IntoElement)]
pub struct Loading;

impl RenderOnce for Loading {
  fn render(self, _window: &mut gpuim::Window, _cx: &mut gpuim::App) -> impl IntoElement {
    ListItem::new("list-loading").disabled(true).child(
      h_flex()
        .size_full()
        .items_center()
        .justify_center()
        .gap_2()
        .child(Spinner::new()),
    )
  }
}
