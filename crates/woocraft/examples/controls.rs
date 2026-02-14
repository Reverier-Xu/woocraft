use gpui::{
    App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
    Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
    ActiveTheme, Badge, Button, ButtonVariants, Checkbox, Divider, Label, Link, Notification, NotificationCenter, NotificationPlacement, NotificationState, NotificationType, Progress, Select, SelectItem, SelectState, Selectable, Slider, SliderState, Spinner, StyledExt, Switch, Tag, Theme, ThemeMode, h_flex, init, v_flex
};

const SELECT_OPTIONS: [&str; 4] = ["Alpha", "Beta", "Gamma", "Delta"];

struct ControlsWindow {
    checked: bool,
    switched: bool,
    select_state: Entity<SelectState>,
    slider_state: Entity<SliderState>,
    notification_state: Entity<NotificationState>,
    link_clicks: usize,
}

impl ControlsWindow {
    fn view(cx: &mut App) -> Entity<Self> {

        let select_items = SELECT_OPTIONS
            .iter()
            .map(|item| SelectItem::from(*item))
            .collect::<Vec<_>>();
        let select_state = cx.new(|_| SelectState::new(select_items, "请选择"));
        select_state.update(cx, |state, _| state.set_selected_index(Some(0)));

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
            select_state,
            slider_state,
            notification_state,
            link_clicks: 0,
        })
    }

    fn selected_text(&self, cx: &App) -> String {
        self.select_state
            .read(cx)
            .selected_item()
            .map(|item| item.label.to_string())
            .unwrap_or_else(|| "<none>".to_string())
    }
}

impl Render for ControlsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let slider_value = self.slider_state.read(cx).value().end();
        let selected_text = self.selected_text(cx);
        let is_dark = cx.theme().mode.is_dark();

        v_flex()
            .size_full()
            .relative()
            .p_6()
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_xl().font_semibold().child("Woocraft Controls Preview"))
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
            .child(div().text_sm().text_color(cx.theme().muted_foreground).child(
                "组件与值已绑定：交互后下方 Label 会实时反映状态",
            ))
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
                            .child(
                                Slider::new("demo-slider", &self.slider_state),
                            )
                            .child(Label::new(format!("slider = {:.1}", slider_value))),
                    )
                    .child(
                        h_flex()
                            .items_center()
                            .gap_3()
                            .child(
                                Select::new("demo-select", &self.select_state),
                            )
                            .child(Label::new(format!("select = {}", selected_text))),
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
                                                Notification::success("Build completed")
                                                    .title("Success"),
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
                                                Notification::warning("Low disk space")
                                                    .title("Warning"),
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
                                Link::new("demo-link").on_click(cx.listener(|this, _, _, cx| {
                                    this.link_clicks += 1;
                                    cx.notify();
                                }))
                                .child("Click me"),
                            )
                            .child(Label::new(format!("link_clicks = {}", self.link_clicks))),
                    )
                    .child(
                        Divider::horizontal_dashed().label("basic migrated"),
                    )
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
                            .child(div().w(px(180.)).child(
                                Progress::new()
                                    .label("Loading")
                                    .color(cx.theme().primary)
                                    .track_color(cx.theme().muted)
                                    .text_color(cx.theme().muted_foreground)
                                    .value(slider_value),
                            )),
                    ),
            )
            .child(div().border_t_1().border_color(cx.theme().border).pt_3().child(
                Label::new("Summary").secondary(format!(
                    "checked={}, switched={}, slider={:.1}, select={}, link_clicks={}",
                    self.checked,
                    self.switched,
                    slider_value,
                    selected_text,
                    self.link_clicks
                )),
            ))
            .child(
                NotificationCenter::new(&self.notification_state)
                    .placement(NotificationPlacement::BottomRight),
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
