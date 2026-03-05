use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Dialog, DialogMode, Selectable, StyledExt, Theme,
  ThemeMode, TitleBar, h_flex, init, v_flex, window_border,
};

#[derive(Default)]
struct DialogWindow {
  light_open: bool,
  modal_open: bool,
  close_count: usize,
}

impl DialogWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self::default())
  }
}

impl Render for DialogWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let this = cx.entity().clone();
    let is_dark = cx.theme().mode.is_dark();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Woocraft Dialog Example"))
        .child(
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
                .child("Dialog (Light / Modal)"),
            )
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(
                  "Light: click backdrop or press Escape to close. Modal: requires explicit close.",
                ),
            )
            .child(
              h_flex()
                .gap_3()
                .child(
                  Button::new("theme-light")
                    .label("Light")
                    .selected(!is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                )
                .child(
                  Button::new("theme-dark")
                    .label("Dark")
                    .selected(is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                ),
            )
            .child(
              h_flex()
                .gap_3()
                .child(
                  Button::new("open-light-dialog")
                    .label("Open Light Dialog")
                    .on_click(cx.listener(|this, _, _, cx| {
                      this.light_open = true;
                      cx.notify();
                    })),
                )
                .child(
                  Button::new("open-modal-dialog")
                    .primary()
                    .label("Open Modal Dialog")
                    .on_click(cx.listener(|this, _, _, cx| {
                      this.modal_open = true;
                      cx.notify();
                    })),
                ),
            )
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                  "State: light_open={}, modal_open={}, closed_count={}",
                  self.light_open, self.modal_open, self.close_count
                )),
            )
            .child(
              Dialog::new("light-dialog")
                .mode(DialogMode::Light)
                .title("Light Dialog")
                .width(px(520.))
                .open(self.light_open)
                .on_close({
                  let this = this.clone();
                  move |_, cx| {
                    this.update(cx, |this, cx| {
                      this.light_open = false;
                      this.close_count += 1;
                      cx.notify();
                    });
                  }
                })
                .content({
                  let this = this.clone();
                  move |_, _, _| {
                    v_flex()
                      .gap_3()
                      .child(
                        div()
                          .text_sm()
                          .child("Click outside, press Escape, or use buttons below to close."),
                      )
                      .child(
                        h_flex()
                          .justify_end()
                          .gap_2()
                          .child(Button::new("light-cancel").label("Cancel").on_click({
                            let this = this.clone();
                            move |_, _, cx| {
                              this.update(cx, |this, cx| {
                                this.light_open = false;
                                cx.notify();
                              });
                            }
                          }))
                          .child(
                            Button::new("light-confirm")
                              .primary()
                              .label("Confirm")
                              .on_click({
                                let this = this.clone();
                                move |_, _, cx| {
                                  this.update(cx, |this, cx| {
                                    this.light_open = false;
                                    this.close_count += 1;
                                    cx.notify();
                                  });
                                }
                              }),
                          ),
                      )
                  }
                }),
            )
            .child(
              Dialog::new("modal-dialog")
                .mode(DialogMode::Modal)
                .title("Modal Dialog")
                .width(px(560.))
                .open(self.modal_open)
                .closable_by_escape(false)
                .on_close({
                  let this = this.clone();
                  move |_, cx| {
                    this.update(cx, |this, cx| {
                      this.modal_open = false;
                      this.close_count += 1;
                      cx.notify();
                    });
                  }
                })
                .content({
                  let this = this.clone();
                  move |_, _, _| {
                    v_flex()
                      .gap_3()
                      .child(div().text_sm().child(
                        "Backdrop click and Escape are disabled. Use explicit actions to exit.",
                      ))
                      .child(
                        h_flex().justify_end().gap_2().child(
                          Button::new("modal-close")
                            .danger()
                            .label("Close Modal")
                            .on_click({
                              let this = this.clone();
                              move |_, _, cx| {
                                this.update(cx, |this, cx| {
                                  this.modal_open = false;
                                  this.close_count += 1;
                                  cx.notify();
                                });
                              }
                            }),
                        ),
                      )
                  }
                }),
            ),
        ),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(980.), px(640.)), cx);
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
        |_window, cx| DialogWindow::view(cx),
      )
      .expect("open dialog demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Dialog Example");
      })
      .expect("update dialog demo window failed");
  });
}
