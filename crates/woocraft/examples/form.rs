use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, Checkbox, Disableable, Input, InputState, Label,
  NumberInput, ScrollableElement, Selectable, Sizable as _, StyledExt, Switch, Theme, ThemeMode,
  TitleBar, field, h_flex, h_form, v_flex, v_form, window_border,
};

struct FormWindow {
  name_state: Entity<InputState>,
  email_state: Entity<InputState>,
  company_state: Entity<InputState>,
  budget_state: Entity<InputState>,
  api_host_state: Entity<InputState>,
  timeout_state: Entity<InputState>,
  receive_updates: bool,
  accept_terms: bool,
  production_mode: bool,
  submit_count: usize,
  last_submit: String,
}

impl FormWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    let name_state = cx.new(|cx| InputState::new(cx).placeholder("Acme Workspace"));
    let email_state = cx.new(|cx| InputState::new(cx).placeholder("team@acme.dev"));
    let company_state = cx.new(|cx| InputState::new(cx).placeholder("Acme Inc."));
    let budget_state = cx.new(|cx| InputState::new(cx).default_value("1200"));
    let api_host_state = cx.new(|cx| InputState::new(cx).default_value("https://api.acme.dev"));
    let timeout_state = cx.new(|cx| InputState::new(cx).default_value("30"));

    cx.new(|_| Self {
      name_state,
      email_state,
      company_state,
      budget_state,
      api_host_state,
      timeout_state,
      receive_updates: true,
      accept_terms: false,
      production_mode: false,
      submit_count: 0,
      last_submit: "No submission yet".to_string(),
    })
  }
}

impl Render for FormWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Woocraft Form Example"))
        .child(
          v_flex()
            .p_6()
            .gap_5()
            .overflow_y_scrollbar()
            .child(
              div()
                .text_xl()
                .font_semibold()
                .child("Woocraft Form Infrastructure Preview"),
            )
            .child(
              h_flex()
                .gap_3()
                .child(
                  Button::new("theme-light")
                    .label("Light")
                    .small()
                    .selected(!is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                )
                .child(
                  Button::new("theme-dark")
                    .label("Dark")
                    .small()
                    .selected(is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                ),
            )
            .child(div().text_sm().font_semibold().child("Vertical Form"))
            .child(
              div()
                .p_4()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius_container)
                .bg(cx.theme().card)
                .child(
                  v_form()
                    .label_width(px(150.))
                    .child(
                      field()
                        .label("Display Name")
                        .required(true)
                        .description("Shown to collaborators and in notifications.")
                        .child(Input::new(&self.name_state).cleanable(true)),
                    )
                    .child(
                      field()
                        .label("Email")
                        .required(true)
                        .description("Used for account and delivery notifications.")
                        .child(Input::new(&self.email_state)),
                    )
                    .child(
                      field()
                        .label("Company")
                        .child(Input::new(&self.company_state)),
                    )
                    .child(
                      field()
                        .label("Monthly Budget")
                        .description("USD")
                        .child(NumberInput::new(&self.budget_state).step(100.0).min(0.0)),
                    )
                    .child(
                      field().label("Product Updates").child(
                        Switch::new("form-updates-switch")
                          .checked(self.receive_updates)
                          .on_click(cx.listener(|this, checked, _, cx| {
                            this.receive_updates = *checked;
                            cx.notify();
                          })),
                      ),
                    )
                    .child(
                      field()
                        .label("Terms")
                        .required(true)
                        .description("You must accept terms before submitting.")
                        .child(
                          Checkbox::new("form-terms-checkbox")
                            .checked(self.accept_terms)
                            .label("I agree to the Terms of Service")
                            .on_click(cx.listener(|this, checked, _, cx| {
                              this.accept_terms = *checked;
                              cx.notify();
                            })),
                        ),
                    )
                    .child(
                      field().label_indent(false).child(
                        h_flex()
                          .gap_3()
                          .child(
                            Button::new("form-submit")
                              .primary()
                              .label("Submit")
                              .disabled(!self.accept_terms)
                              .on_click(cx.listener(|this, _, _, cx| {
                                let name = this.name_state.read(cx).value().to_string();
                                let email = this.email_state.read(cx).value().to_string();
                                let budget = this.budget_state.read(cx).value().to_string();
                                this.submit_count += 1;
                                this.last_submit = format!(
                                  "#{}, name={}, email={}, budget=${}, updates={}, production={}",
                                  this.submit_count,
                                  name,
                                  email,
                                  budget,
                                  this.receive_updates,
                                  this.production_mode
                                );
                                cx.notify();
                              })),
                          )
                          .child(Button::new("form-reset").default().label("Reset").on_click(
                            cx.listener(|this, _, window, cx| {
                              let name_state = this.name_state.clone();
                              let email_state = this.email_state.clone();
                              let company_state = this.company_state.clone();
                              let budget_state = this.budget_state.clone();
                              let api_host_state = this.api_host_state.clone();
                              let timeout_state = this.timeout_state.clone();

                              name_state.update(cx, |state, cx| state.set_value("", window, cx));
                              email_state.update(cx, |state, cx| state.set_value("", window, cx));
                              company_state.update(cx, |state, cx| state.set_value("", window, cx));
                              budget_state.update(cx, |state, cx| state.set_value("0", window, cx));
                              api_host_state.update(cx, |state, cx| {
                                state.set_value("https://api.acme.dev", window, cx)
                              });
                              timeout_state
                                .update(cx, |state, cx| state.set_value("30", window, cx));

                              this.receive_updates = true;
                              this.accept_terms = false;
                              this.production_mode = false;
                              this.last_submit = "Form reset".to_string();
                              cx.notify();
                            }),
                          ))
                          .child(
                            Label::new("Submit Count").secondary(self.submit_count.to_string()),
                          ),
                      ),
                    ),
                ),
            )
            .child(
              div()
                .text_sm()
                .font_semibold()
                .child("Horizontal Grid Form"),
            )
            .child(
              div()
                .p_4()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(cx.theme().radius_container)
                .bg(cx.theme().card)
                .child(
                  h_form()
                    .columns(2)
                    .label_width(px(120.))
                    .child(
                      field()
                        .label("API Host")
                        .col_span(2)
                        .child(Input::new(&self.api_host_state)),
                    )
                    .child(
                      field()
                        .label("Timeout")
                        .description("seconds")
                        .child(NumberInput::new(&self.timeout_state).step(5.0).min(1.0)),
                    )
                    .child(
                      field().label("Environment").child(
                        Switch::new("form-production-switch")
                          .label("Production")
                          .checked(self.production_mode)
                          .on_click(cx.listener(|this, checked, _, cx| {
                            this.production_mode = *checked;
                            cx.notify();
                          })),
                      ),
                    )
                    .child(
                      field().label_indent(false).col_span(2).child(
                        Button::new("test-connection")
                          .info()
                          .label("Test Connection"),
                      ),
                    ),
                ),
            )
            .child(Label::new("Last Submit").secondary(self.last_submit.clone())),
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

      let bounds = Bounds::centered(None, GpuiSize::new(px(1080.), px(760.)), cx);
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
          FormWindow::view,
        )
        .expect("open form demo window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Form Example");
        })
        .expect("update form demo window failed");
    });
}
