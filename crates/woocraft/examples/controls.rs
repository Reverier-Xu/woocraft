use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  ScrollStrategy, Size as GpuiSize, Styled, Task, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Badge, Breadcrumb, BreadcrumbItem, Button, ButtonVariants, Checkbox, Divider, Icon,
  IconLabel, IndexPath, Input, InputState, Kbd, Label, Link, List, ListDelegate, ListItem,
  ListState, Notification, NotificationCenter, NotificationPlacement,
  NotificationState, NotificationType, NumberInput, OtpInput, OtpState, Popover, Progress,
  Pagination, ProgressCircle, ScrollableElement, Selectable, Slider, SliderState, Spinner,
  StyledExt, Switch, Sizable, Tag, Theme, ThemeMode, TitleBar, Tooltip, WidgetGroup, h_flex,
  init, v_flex, window_border,
};

#[derive(Clone)]
struct DemoListEntry {
  title: &'static str,
  subtitle: &'static str,
  tag: &'static str,
}

struct DemoListDelegate {
  section_titles: Vec<&'static str>,
  entries: Vec<Vec<DemoListEntry>>,
  filtered_indices: Vec<Vec<usize>>,
  current_page: usize,
  page_size: usize,
  selected: Option<IndexPath>,
  right_clicked: Option<IndexPath>,
  confirmed: Option<IndexPath>,
  loading: bool,
}

impl DemoListDelegate {
  fn new() -> Self {
    let section_titles = vec!["Framework", "Utility", "Design"];
    let entries = vec![
      vec![
        DemoListEntry {
          title: "Renderer",
          subtitle: "High-performance viewport renderer",
          tag: "Core",
        },
        DemoListEntry {
          title: "Scheduler",
          subtitle: "Task orchestration and frame budget",
          tag: "Core",
        },
        DemoListEntry {
          title: "Input Hub",
          subtitle: "Unified keyboard and pointer pipeline",
          tag: "Input",
        },
        DemoListEntry {
          title: "Theme Engine",
          subtitle: "Runtime token and palette switching",
          tag: "Style",
        },
        DemoListEntry {
          title: "Widget Registry",
          subtitle: "Composable widget lifecycle management",
          tag: "UI",
        },
      ],
      vec![
        DemoListEntry {
          title: "Logger",
          subtitle: "Structured event and diagnostics",
          tag: "Observability",
        },
        DemoListEntry {
          title: "Inspector",
          subtitle: "Runtime tree and style inspection",
          tag: "Debug",
        },
        DemoListEntry {
          title: "Clipboard",
          subtitle: "Cross-platform clipboard bridge",
          tag: "Platform",
        },
        DemoListEntry {
          title: "Virtual Scroller",
          subtitle: "Variable-size list virtualization",
          tag: "List",
        },
        DemoListEntry {
          title: "Command Palette",
          subtitle: "Search and action execution",
          tag: "UX",
        },
      ],
      vec![
        DemoListEntry {
          title: "Typography",
          subtitle: "CJK-aware font fallback configuration",
          tag: "Text",
        },
        DemoListEntry {
          title: "Icon Pack",
          subtitle: "Semantic icon naming and rendering",
          tag: "Assets",
        },
        DemoListEntry {
          title: "Spacing",
          subtitle: "Consistent layout rhythm tokens",
          tag: "Layout",
        },
        DemoListEntry {
          title: "Motion",
          subtitle: "Animation timing and transitions",
          tag: "Interaction",
        },
        DemoListEntry {
          title: "Accessibility",
          subtitle: "Focus, contrast and keyboard support",
          tag: "A11y",
        },
      ],
    ];

    let filtered_indices = entries
      .iter()
      .map(|section| (0..section.len()).collect::<Vec<_>>())
      .collect::<Vec<_>>();
    Self {
      section_titles,
      entries,
      filtered_indices,
      current_page: 1,
      page_size: 3,
      selected: None,
      right_clicked: None,
      confirmed: None,
      loading: false,
    }
  }

  fn is_loading(&self) -> bool {
    self.loading
  }

  fn set_loading(&mut self, loading: bool) {
    self.loading = loading;
  }

  fn total_pages(&self) -> usize {
    let max_len = self.filtered_indices.iter().map(Vec::len).max().unwrap_or(0);
    let page_size = self.page_size.max(1);
    ((max_len + page_size - 1) / page_size).max(1)
  }

  fn current_page(&self) -> usize {
    self.current_page
  }

  fn set_current_page(&mut self, page: usize) {
    self.current_page = page.clamp(1, self.total_pages());
  }

  fn visible_bounds(&self, section: usize) -> (usize, usize, usize) {
    let total = self.filtered_indices.get(section).map_or(0, Vec::len);
    let start = (self.current_page.saturating_sub(1)) * self.page_size;
    let start = start.min(total);
    let end = (start + self.page_size).min(total);
    (start, end, total)
  }

  fn selected_summary(&self) -> String {
    self
      .selected
      .map(|ix| format!("{}:{}", ix.section, ix.row))
      .unwrap_or_else(|| "None".to_string())
  }

  fn right_clicked_summary(&self) -> String {
    self
      .right_clicked
      .map(|ix| format!("{}:{}", ix.section, ix.row))
      .unwrap_or_else(|| "None".to_string())
  }

  fn confirmed_summary(&self) -> String {
    self
      .confirmed
      .map(|ix| format!("{}:{}", ix.section, ix.row))
      .unwrap_or_else(|| "None".to_string())
  }
}

impl ListDelegate for DemoListDelegate {
  type Item = ListItem;

  fn perform_search(
    &mut self, query: &str, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) -> Task<()> {
    let query = query.trim().to_lowercase();
    self.filtered_indices = self
      .entries
      .iter()
      .map(|section| {
        section
          .iter()
          .enumerate()
          .filter_map(|(ix, item)| {
            if query.is_empty()
              || item.title.to_lowercase().contains(&query)
              || item.subtitle.to_lowercase().contains(&query)
              || item.tag.to_lowercase().contains(&query)
            {
              Some(ix)
            } else {
              None
            }
          })
          .collect::<Vec<_>>()
      })
      .collect::<Vec<_>>();
    self.current_page = 1;

    Task::ready(())
  }

  fn sections_count(&self, _cx: &App) -> usize {
    self.entries.len()
  }

  fn items_count(&self, section: usize, _cx: &App) -> usize {
    let (start, end, _) = self.visible_bounds(section);
    end.saturating_sub(start)
  }

  fn render_item(
    &mut self, ix: IndexPath, _window: &mut Window, cx: &mut Context<ListState<Self>>,
  ) -> Option<Self::Item> {
    let (start, _, _) = self.visible_bounds(ix.section);
    let filtered_ix = *self.filtered_indices.get(ix.section)?.get(start + ix.row)?;
    let item = self.entries.get(ix.section)?.get(filtered_ix)?;
    let title = item.title;
    let subtitle = item.subtitle;
    let tag = item.tag;
    let confirmed = self.confirmed.map(|v| v.eq_row(ix)).unwrap_or(false);

    Some(
      ListItem::new(("controls-list-item", ix.section * 1000 + ix.row))
        .check_icon(Icon::new(woocraft::IconName::CheckmarkCircle))
        .confirmed(confirmed)
        .child(
          v_flex()
            .gap_0p5()
            .child(div().font_medium().child(title))
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(subtitle),
            ),
        )
        .suffix(move |_, _| Tag::new().outline().small().child(tag)),
    )
  }

  fn render_section_header(
    &mut self, section: usize, _window: &mut Window, cx: &mut Context<ListState<Self>>,
  ) -> Option<impl IntoElement> {
    let total = self.filtered_indices.get(section).map_or(0, Vec::len);
    Some(
      h_flex()
        .px_2()
        .py_1()
        .justify_between()
        .bg(cx.theme().muted.opacity(0.3))
        .child(div().font_semibold().child(self.section_titles[section]))
        .child(div().text_color(cx.theme().muted_foreground).child(format!("{total} items"))),
    )
  }

  fn render_section_footer(
    &mut self, section: usize, _window: &mut Window, cx: &mut Context<ListState<Self>>,
  ) -> Option<impl IntoElement> {
    let (start, end, total) = self.visible_bounds(section);
    Some(
      h_flex()
        .w_full()
        .px_2()
        .py_1()
        .justify_between()
        .text_color(cx.theme().muted_foreground)
        .child(format!(
          "Page {} · Showing {}-{} / {}",
          self.current_page,
          start + usize::from(total > 0),
          end,
          total
        )),
    )
  }

  fn loading(&self, _cx: &App) -> bool {
    self.loading
  }

  fn set_selected_index(
    &mut self, ix: Option<IndexPath>, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) {
    self.selected = ix;
  }

  fn set_right_clicked_index(
    &mut self, ix: Option<IndexPath>, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) {
    self.right_clicked = ix;
  }

  fn confirm(
    &mut self, _secondary: bool, _window: &mut Window, _cx: &mut Context<ListState<Self>>,
  ) {
    self.confirmed = self.selected;
  }

  fn has_more(&self, _cx: &App) -> bool {
    false
  }

  fn load_more_threshold(&self) -> usize {
    0
  }

  fn load_more(&mut self, _window: &mut Window, _cx: &mut Context<ListState<Self>>) {
    // no-op, this demo uses explicit pagination
  }
}

struct ControlsWindow {
  checked: bool,
  switched: bool,
  slider_state: Entity<SliderState>,
  notification_state: Entity<NotificationState>,
  link_clicks: usize,
  breadcrumb_last: &'static str,
  popover_open: bool,
  button_group_selected: Vec<usize>,
  toggle_checked: bool,
  toggle_group_checks: Vec<bool>,
  input_state1: Entity<InputState>,
  input_state2: Entity<InputState>,
  number_input_state: Entity<InputState>,
  otp_state: Entity<OtpState>,
  list_state: Entity<ListState<DemoListDelegate>>,
  list_searchable: bool,
  list_selectable: bool,
}

impl ControlsWindow {
  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let slider_state = cx.new(|_| {
      SliderState::new()
        .min(0.0)
        .max(100.0)
        .step(5.0)
        .default_value(20.0)
    });
    let notification_state = cx.new(|_| NotificationState::new().max_items(6));
    let input_state1 = cx.new(|cx| InputState::new(cx).placeholder("Search package..."));
    let input_state2 = cx.new(|cx| InputState::new(cx).placeholder("Search package..."));
    let number_input_state = cx.new(|cx| InputState::new(cx).default_value("10"));
    let otp_state = cx.new(|cx| OtpState::new(6, cx));
    let list_state = cx.new(|cx| {
      ListState::new(DemoListDelegate::new(), window, cx)
        .searchable(true)
        .selectable(true)
    });

    cx.new(|_| Self {
      checked: false,
      switched: true,
      slider_state,
      notification_state,
      link_clicks: 0,
      breadcrumb_last: "Home",
      popover_open: false,
      button_group_selected: vec![0],
      toggle_checked: false,
      toggle_group_checks: vec![false, true, false],
      input_state1,
      input_state2,
      number_input_state,
      otp_state,
      list_state,
      list_searchable: true,
      list_selectable: true,
    })
  }
}

impl Render for ControlsWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let slider_value = self.slider_state.read(cx).value().end();
    let is_dark = cx.theme().mode.is_dark();
    let (list_selected, list_right_clicked, list_confirmed, list_loading, list_page, list_total_pages) = {
      let list_state = self.list_state.read(cx);
      let delegate = list_state.delegate();
      (
        delegate.selected_summary(),
        delegate.right_clicked_summary(),
        delegate.confirmed_summary(),
        delegate.is_loading(),
        delegate.current_page(),
        delegate.total_pages(),
      )
    };

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Woocraft Controls Example"))
        .child(
            v_flex()
            .p_6()
            .gap_4()
            .overflow_y_scrollbar()
            .flex_1()
            .min_h_0()
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
              v_flex()
                .gap_2()
                .child(div().text_sm().child("Scroll System"))
                .child(
                  div()
                    .overflow_y_scrollbar()
                    .h(px(140.))
                    .w(px(360.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius_container)
                    .children((1..=24).map(|i| {
                      div()
                        .px_3()
                        .py_2()
                        .border_b_1()
                        .border_color(cx.theme().border.opacity(0.5))
                        .child(format!("Scrollable row #{i}"))
                        .into_any_element()
                    })),
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
                .child(div().text_sm().child("Inputs (single / number / otp)"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      div().w(px(280.)).child(
                        Input::new(&self.input_state1)
                          .cleanable(true),
                      ),
                    ),
                )
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      div().w(px(220.)).child(
                        NumberInput::new(&self.number_input_state)
                          .step(0.5)
                          .min(0.0)
                          .max(99.0),
                      ),
                    ),
                )
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(OtpInput::new(&self.otp_state).groups(3)),
                )
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      Checkbox::new("demo-checkbox")
                        .checked(self.checked)
                        .tab_stop(true)
                        .tab_index(1)
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
                    .child(Tag::info().child("Info"))
                    .child(Tag::warning().rounded_full().child("Rounded Full"))
                    .child(
                      div().w(px(180.)).child(
                        Progress::new()
                          .label("Loading")
                          .color(cx.theme().primary)
                          .track_color(cx.theme().muted)
                          .text_color(cx.theme().muted_foreground)
                          .value(slider_value),
                      ),
                    )
                    .child(
                      ProgressCircle::new()
                        .value(slider_value)
                        .color(cx.theme().primary)
                        .track_color(cx.theme().muted)
                        .text_color(cx.theme().muted_foreground),
                    ),
                ),
            )
            .child(Divider::horizontal_dashed().label("newly added capabilities"))
            .child(
              v_flex()
                .gap_3()
                .child(div().text_sm().child("Button Variants + Features"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      Button::new("btn-info")
                        .info()
                        .label("Info")
                        .dropdown_caret(true),
                    )
                    .child(Button::new("btn-link").link().label("Link"))
                )
                .child(div().text_sm().child("WidgetGroup (buttons)"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      WidgetGroup::new("demo-widget-group")
                        .multiple(true)
                        .default()
                        .child(Button::new("bg-0").label("One").selected(self.button_group_selected.contains(&0)))
                        .child(Button::new("bg-1").label("Two").selected(self.button_group_selected.contains(&1)))
                        .child(Button::new("bg-2").label("Three").selected(self.button_group_selected.contains(&2)))
                        .on_click(cx.listener(|this, selected: &Vec<usize>, _, cx| {
                          this.button_group_selected = selected.clone();
                          cx.notify();
                        })),
                    )
                    .child(Label::new(format!(
                      "selected = {:?}",
                      self.button_group_selected
                    ))),
                )
                .child(div().text_sm().child("WidgetGroup (mixed components)"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                      div().w(px(560.)).child(
                        WidgetGroup::new("demo-widget-group-mixed")
                          .child(
                              Icon::new(woocraft::IconName::Search)
                          )
                          .child(Input::new(&self.input_state2).cleanable(true))
                          .child(
                            Button::new("wg-run")
                              .icon(woocraft::IconName::Play))
                          .child(
                            IconLabel::new("wg-status")
                              .icon(woocraft::IconName::Checkmark)
                              .label("Ready"),
                          ),
                      ),
                    ),
                )
                .child(div().text_sm().child("List (comprehensive)"))
                .child(
                  h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                      Button::new("list-toggle-search")
                        .label(if self.list_searchable {
                          "Search: On"
                        } else {
                          "Search: Off"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                          let next = !this.list_searchable;
                          this.list_searchable = next;
                          this.list_state.update(cx, |state, cx| {
                            state.set_searchable(next, cx);
                          });
                        })),
                    )
                    .child(
                      Button::new("list-toggle-select")
                        .label(if self.list_selectable {
                          "Selectable: On"
                        } else {
                          "Selectable: Off"
                        })
                        .on_click(cx.listener(|this, _, _, cx| {
                          let next = !this.list_selectable;
                          this.list_selectable = next;
                          this.list_state.update(cx, |state, cx| {
                            state.set_selectable(next, cx);
                          });
                        })),
                    )
                    .child(
                      Button::new("list-scroll-first")
                        .label("Scroll To First")
                        .on_click(cx.listener(|this, _, window, cx| {
                          let list_state = this.list_state.clone();
                          cx
                            .spawn_in(window, async move |_, cx| {
                              _ = list_state.update_in(cx, |state, window, cx| {
                                state.scroll_to_item(
                                  IndexPath::default(),
                                  ScrollStrategy::Top,
                                  window,
                                  cx,
                                );
                              });
                            })
                            .detach();
                        })),
                    )
                    .child(
                      Button::new("list-toggle-loading")
                        .label(if list_loading {
                          "Loading: On"
                        } else {
                          "Loading: Off"
                        })
                        .on_click(cx.listener(|this, _, window, cx| {
                          let list_state = this.list_state.clone();
                          cx
                            .spawn_in(window, async move |_, cx| {
                              _ = list_state.update_in(cx, |state, _, cx| {
                                let next = !state.delegate().is_loading();
                                state.delegate_mut().set_loading(next);
                                cx.notify();
                              });
                            })
                            .detach();
                        })),
                    )
                    .child(
                      Button::new("list-reset-page")
                        .label("Back To Page 1")
                        .on_click(cx.listener(|this, _, window, cx| {
                          let list_state = this.list_state.clone();
                          cx
                            .spawn_in(window, async move |_, cx| {
                              _ = list_state.update_in(cx, |state, window, cx| {
                                state.delegate_mut().set_current_page(1);
                                state.scroll_to_item(IndexPath::default(), ScrollStrategy::Top, window, cx);
                                cx.notify();
                              });
                            })
                            .detach();
                        })),
                    ),
                )
                .child(
                  div()
                    .h(px(320.))
                    .w(px(640.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(cx.theme().radius_container)
                    .child(
                      List::new(&self.list_state)
                        .search_placeholder("Search title, subtitle, or tag")
                        .scrollbar_visible(true),
                    ),
                )
                .child(
                  Pagination::new("controls-list-pagination")
                    .current_page(list_page)
                    .total_pages(list_total_pages)
                    .visible_pages(7)
                    .on_click(cx.listener(|this, page, _, cx| {
                      this.list_state.update(cx, |state, cx| {
                        state.delegate_mut().set_current_page(*page);
                        cx.notify();
                      });
                    })),
                )
                .child(
                  Label::new("List state").secondary(format!(
                    "selected={}, right_clicked={}, confirmed={}, loading={}, searchable={}, selectable={}, page={}/{}",
                    list_selected,
                    list_right_clicked,
                    list_confirmed,
                    list_loading,
                    self.list_searchable,
                    self.list_selectable,
                    list_page,
                    list_total_pages
                  )),
                )
            )
            .child(
              div()
                .border_t_1()
                .border_color(cx.theme().border)
                .pt_3()
                .child(Label::new("Summary").secondary(format!(
                  "checked={}, switched={}, slider={:.1}, link_clicks={}, breadcrumb_last={}, popover_open={}, button_group={:?}, toggle={}, toggle_group={:?}",
                  self.checked,
                  self.switched,
                  slider_value,
                  self.link_clicks,
                  self.breadcrumb_last,
                  self.popover_open,
                  self.button_group_selected,
                  self.toggle_checked,
                  self.toggle_group_checks
                ))),
            )
            ,
        ),
    ).child(
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
          titlebar: Some(TitleBar::title_bar_options()),
          #[cfg(target_os = "linux")]
          window_background: gpui::WindowBackgroundAppearance::Transparent,
          #[cfg(target_os = "linux")]
          window_decorations: Some(gpui::WindowDecorations::Client),
          ..Default::default()
        },
        ControlsWindow::view,
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
