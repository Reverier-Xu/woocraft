use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Label, Pagination, Selectable, Sizable as _, StyledExt,
  Theme, ThemeMode, TitleBar, h_flex, v_flex, window_border,
};

#[derive(Default)]
struct PaginationWindow {
  page: usize,
  total_pages: usize,
}

impl PaginationWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
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
        .min_h_0()
        .child(TitleBar::new().title("Woocraft Pagination Example"))
        .child(
          v_flex()
            .p_6()
            .gap_4()
            .child(
              div()
                .text_xl()
                .font_semibold()
                .child("Woocraft Pagination Example"),
            )
            .child(
              Label::new("Current Page").secondary(format!("{}/{}", self.page, self.total_pages)),
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
              h_flex()
                .gap_3()
                .child(
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
        ),
    )
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      woocraft::init(cx);
      cx.activate(true);

      let bounds = Bounds::centered(None, GpuiSize::new(px(980.), px(680.)), cx);
      let window = cx
        .open_window(
          WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            #[cfg(target_os = "linux")]
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
          },
          PaginationWindow::view,
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
