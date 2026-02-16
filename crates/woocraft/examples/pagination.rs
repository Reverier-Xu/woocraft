use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Label, Pagination, Selectable, Sizable as _,
  StyledExt, Theme, ThemeMode, v_flex, window_border,
};

#[derive(Default)]
struct PaginationWindow {
  page: usize,
  total_pages: usize,
}

impl PaginationWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self {
      page: 1,
      total_pages: 18,
    })
  }
}

impl Render for PaginationWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();

    window_border().child(
      v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(
          div()
            .text_xl()
            .font_semibold()
            .child("Woocraft Pagination Example"),
        )
        .child(
          Label::new("Current Page").secondary(format!(
            "{}/{}",
            self.page, self.total_pages
          )),
        )
        .child(
          div().child(
            Pagination::new("pagination-default")
              .current_page(self.page)
              .total_pages(self.total_pages)
              .visible_pages(7)
              .on_click(cx.listener(|this, page, _, cx| {
                this.page = *page;
                cx.notify();
              })),
          ),
        )
        .child(div().text_sm().child("Compact"))
        .child(
          div().child(
            Pagination::new("pagination-compact")
              .compact()
              .current_page(self.page)
              .total_pages(self.total_pages)
              .on_click(cx.listener(|this, page, _, cx| {
                this.page = *page;
                cx.notify();
              })),
          ),
        )
        .child(div().text_sm().child("State"))
        .child(
          div().h_flex().gap_3().child(
            Button::new("page-prev")
              .label("Prev")
              .flat()
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                this.page = this.page.saturating_sub(1).max(1);
                cx.notify();
              })),
          )
          .child(
            Button::new("page-next")
              .label("Next")
              .flat()
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                this.page = (this.page + 1).min(this.total_pages);
                cx.notify();
              })),
          )
          .child(
            Button::new("theme-light")
              .label("Light")
              .selected(!is_dark)
              .small()
              .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
          )
          .child(
            Button::new("theme-dark")
              .label("Dark")
              .selected(is_dark)
              .small()
              .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
          ),
        ),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(920.), px(560.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| PaginationWindow::view(cx),
      )
      .expect("open pagination demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Pagination Example");
      })
      .expect("update pagination demo window failed");
  });
}
