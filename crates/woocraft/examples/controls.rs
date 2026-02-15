use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Badge, Breadcrumb, BreadcrumbItem, Button, ButtonVariants, Checkbox, Divider,
  Kbd, Label, Link, Notification, NotificationCenter, NotificationPlacement, NotificationState,
  NotificationType, Popover, Progress, Selectable, Slider, SliderState, Spinner, StyledExt,
  Switch, Tag, Theme, ThemeMode, TitleBar, Tooltip, h_flex, init, v_flex, window_border,
};

struct ControlsWindow {
  checked: bool,
  switched: bool,
  slider_state: Entity<SliderState>,
  notification_state: Entity<NotificationState>,
  link_clicks: usize,
  breadcrumb_last: &'static str,
  popover_open: bool,
}

impl ControlsWindow {
  fn view(cx: &mut App) -> Entity<Self> {

    let slider_state = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(5.0)
        .default_value(20.0)
    });
    let notification_state = cx.new(|_| NotificationState::new().max_items(6));

    cx.new(|_| Self {
      checked: false,
      switched: true,
      slider_state,
      notification_state,
      link_clicks: 0,
      breadcrumb_last: "Home",
      popover_open: false,
    })
  }
}

impl Render for ControlsWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let slider_value = self.slider_state.read(cx).value().end();
    let is_dark = cx.theme().mode.is_dark();

    window_border().child(
      v_flex()
        .size_full()
        .child(TitleBar::new().title("Woocraft Controls Example"))
        .child(
          v_flex()
            .relative()
            .flex_1()
            .p_6()
            .gap_4()
            .child(
              div()
                .text_xl()
                .font_semibold()
                .child("Woocraft Controls Preview"),
            )
            .child(div().text_sm().child("Theme"))
            .child(
              h_flex()
                .gap_3()
                .child(
                  Button::new("btn-theme-light")
                    .label("Light")
                    .selected(!is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                )
                .child(
                  Button::new("btn-theme-dark")
                    .label("Dark")
                    .selected(is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                ),
            )
            .child(
              v_flex()
                .gap_2()
                .child(div().text_sm().child("Breadcrumb"))
                .child(
                  Breadcrumb::new()
                    .child(BreadcrumbItem::new("Home").on_click(cx.listener(
                      |this, _, _, cx| {
                        this.breadcrumb_last = "Home";
                        cx.notify();
                      },
                    )))
                    .child(BreadcrumbItem::new("Library").on_click(cx.listener(
                      |this, _, _, cx| {
                        this.breadcrumb_last = "Library";
                        cx.notify();
                      },
                    )))
                    .child(BreadcrumbItem::new("Components").on_click(cx.listener(
                      |this, _, _, cx| {
                        this.breadcrumb_last = "Components";
                        cx.notify();
                      },
                    )))
                    .child(BreadcrumbItem::new("Breadcrumb").disabled(true)),
                )
                .child(Label::new(format!("breadcrumb_last = {}", self.breadcrumb_last))),
            )
            .child(
              v_flex()
                .gap_2()
                .child(div().text_sm().child("Popover"))
                .child(
                  h_flex().items_center().gap_3().child(
                    Popover::new("demo-popover")
                      .on_open_change(cx.listener(|this, open, _, cx| {
                        this.popover_open = *open;
                        cx.notify();
                      }))
                      .trigger(
                        Button::new("popover-trigger")
                          .label("Open Popover")
                          .default(),
                      )
                      .content(|_, _, _| {
                        v_flex()
                          .gap_2()
                          .child(div().font_semibold().child("Popover Content"))
                          .child(
                            div()
                              .text_sm()
                              .child("Migrated from deprecated/ui and styled by new theme tokens."),
                          )
                      }),
                  )
                  .child(Label::new(format!("popover_open = {}", self.popover_open))),
                ),
            )
            .child(
              v_flex()
                .gap_2()
                .child(div().text_sm().child("Tooltip"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      Button::new("tooltip-trigger")
                        .label("Hover me")
                        .tooltip(|window, cx| {
                          Tooltip::new("Tooltip from woocraft::Tooltip")
                            .key_binding(Some(
                              Kbd::new(gpui::Keystroke::parse("ctrl-k").expect("valid keystroke")),
                            ))
                            .build(window, cx)
                        }),
                    )
                    .child(
                      Button::new("tooltip-action-trigger")
                        .label("Hover for action")
                        .tooltip(|window, cx| {
                          Tooltip::new("Shortcut resolved from action binding")
                            .action(
                              &woocraft::actions::Cancel,
                              Some(woocraft::actions::POPOVER_CONTEXT),
                            )
                            .build(window, cx)
                        }),
                    )
                    .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                      "Move cursor over the button to preview tooltip",
                    )),
                ),
            )
            .child(
              v_flex()
                .gap_2()
                .child(div().text_sm().child("Kbd"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_2()
                    .child(div().text_sm().child("Shortcut:"))
                    .child(Kbd::new(gpui::Keystroke::parse("ctrl-k").expect("valid keystroke")))
                    .child(
                      Kbd::new(gpui::Keystroke::parse("shift-enter").expect("valid keystroke"))
                        .outline(),
                    ),
                ),
            )
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("组件与值已绑定：交互后下方 Label 会实时反映状态"),
            )
            .child(
              v_flex()
                .gap_3()
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      Checkbox::new("demo-checkbox")
                        .checked(self.checked)
                        .label("Enable")
                        .on_click(cx.listener(|this, checked, _, cx| {
                          this.checked = *checked;
                          cx.notify();
                        })),
                    )
                    .child(Label::new(format!("checked = {}", self.checked))),
                )
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(Slider::new("demo-slider", &self.slider_state))
                    .child(Label::new(format!("slider = {:.1}", slider_value))),
                )
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      Button::new("notify-info")
                        .label("Info")
                        .on_click(cx.listener(|this, _, window, cx| {
                          this.notification_state.update(cx, |state, cx| {
                            state.push(
                              Notification::new()
                                .with_type(NotificationType::Info)
                                .key("save-draft")
                                .title("Info")
                                .message("Saved draft successfully.")
                                .action("Undo", |_, _| {}),
                              window,
                              cx,
                            );
                          });
                        })),
                    )
                    .child(
                      Button::new("notify-success")
                        .success()
                        .label("Success")
                        .on_click(cx.listener(|this, _, window, cx| {
                          this.notification_state.update(cx, |state, cx| {
                            state.push(
                              Notification::success("Build completed").title("Success"),
                              window,
                              cx,
                            );
                          });
                        })),
                    )
                    .child(
                      Button::new("notify-warning")
                        .label("Warning")
                        .on_click(cx.listener(|this, _, window, cx| {
                          this.notification_state.update(cx, |state, cx| {
                            state.push(
                              Notification::warning("Low disk space").title("Warning"),
                              window,
                              cx,
                            );
                          });
                        })),
                    )
                    .child(
                      Button::new("notify-error")
                        .danger()
                        .label("Error")
                        .on_click(cx.listener(|this, _, window, cx| {
                          this.notification_state.update(cx, |state, cx| {
                            state.push(
                              Notification::error("Upload failed")
                                .title("Error")
                                .autohide(false),
                              window,
                              cx,
                            );
                          });
                        })),
                    )
                    .child(
                      Link::new("demo-link")
                        .on_click(cx.listener(|this, _, _, cx| {
                          this.link_clicks += 1;
                          cx.notify();
                        }))
                        .child("Click me"),
                    )
                    .child(Label::new(format!("link_clicks = {}", self.link_clicks))),
                )
                .child(Divider::horizontal_dashed().label("basic migrated"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_4()
                    .child(
                      Badge::new()
                        .count(12)
                        .child(div().px_2().py_1().child("Inbox")),
                    )
                    .child(
                      Badge::new()
                        .dot()
                        .child(div().px_2().py_1().child("Status")),
                    )
                    .child(
                      Badge::new()
                        .icon(woocraft::IconName::Checkmark)
                        .child(div().px_2().py_1().child("Done")),
                    )
                    .child(Spinner::new())
                    .child(Label::new("Loading...")),
                )
                .child(
                  h_flex()
                    .items_center()
                    .gap_4()
                    .child(
                      Switch::new("demo-switch")
                        .checked(self.switched)
                        .label("Airplane mode")
                        .on_click(cx.listener(|this, checked, _, cx| {
                          this.switched = *checked;
                          cx.notify();
                        })),
                    )
                    .child(Tag::primary().child("Primary"))
                    .child(Tag::success().outline().child("Success"))
                    .child(
                      div().w(px(180.)).child(
                        Progress::new()
                          .label("Loading")
                          .color(cx.theme().primary)
                          .track_color(cx.theme().muted)
                          .text_color(cx.theme().muted_foreground)
                          .value(slider_value),
                      ),
                    ),
                ),
            )
            .child(
              div()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_3()
                .child(Label::new("Summary").secondary(format!(
                  "checked={}, switched={}, slider={:.1}, link_clicks={}, breadcrumb_last={}, popover_open={}",
                  self.checked,
                  self.switched,
                  slider_value,
                  self.link_clicks,
                  self.breadcrumb_last,
                  self.popover_open
                ))),
            )
            .child(
              NotificationCenter::new(&self.notification_state)
                .placement(NotificationPlacement::BottomRight),
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
        |_window, cx| ControlsWindow::view(cx),
      )
      .expect("open controls demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Controls Example");
      })
      .expect("update controls demo window failed");
  });
}
