use gpuim::{
  IntoElement, ParentElement as _, RenderOnce, Styled, div, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme, Size, TableThemeExt, h_flex, v_flex};

#[derive(IntoElement)]
pub(super) struct Loading {
  size: Size,
}

impl Loading {
  pub(super) fn new() -> Self {
    Self { size: Size::Medium }
  }

  pub(super) fn size(mut self, size: Size) -> Self {
    self.size = size;
    self
  }
}

#[derive(IntoElement)]
struct LoadingRow {
  header: bool,
  size: Size,
}

impl LoadingRow {
  fn header() -> Self {
    Self {
      header: true,
      size: Size::Medium,
    }
  }

  fn row() -> Self {
    Self {
      header: false,
      size: Size::Medium,
    }
  }

  fn size(mut self, size: Size) -> Self {
    self.size = size;
    self
  }
}

impl RenderOnce for LoadingRow {
  fn render(self, _: &mut gpuim::Window, cx: &mut gpuim::App) -> impl IntoElement {
    let paddings = self.size.table_cell_padding();
    let height = self.size.table_row_height() * 0.5;
    let placeholder_color = if self.header {
      cx.theme().foreground.opacity(0.08)
    } else {
      cx.theme().foreground.opacity(0.06)
    };
    let placeholder = |w| div().h(height).w(w).rounded(px(4.0)).bg(placeholder_color);

    h_flex()
      .gap_3()
      .h(self.size.table_row_height())
      .overflow_hidden()
      .pt(paddings.top)
      .pb(paddings.bottom)
      .pl(paddings.left)
      .pr(paddings.right)
      .items_center()
      .justify_between()
      .overflow_hidden()
      .when(self.header, |this| this.bg(cx.theme().table_head()))
      .child(
        h_flex()
          .gap_3()
          .flex_1()
          .child(placeholder(px(96.0)))
          .child(placeholder(px(192.0)))
          .child(placeholder(px(64.0))),
      )
      .child(placeholder(px(96.0)))
  }
}

impl RenderOnce for Loading {
  fn render(self, _window: &mut gpuim::Window, _cx: &mut gpuim::App) -> impl IntoElement {
    v_flex()
      .gap_0()
      .child(LoadingRow::header().size(self.size))
      .child(LoadingRow::row().size(self.size))
      .child(LoadingRow::row().size(self.size))
      .child(LoadingRow::row().size(self.size))
      .child(LoadingRow::row().size(self.size))
  }
}
