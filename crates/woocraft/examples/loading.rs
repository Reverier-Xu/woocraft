use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Subscription, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, IconName, ListItem, Selectable, Sizable, StyledExt, Theme, ThemeMode,
  TitleBar, TreeEvent, TreeItem, TreeState, h_flex, init, tree, v_flex, window_border,
};

fn demo_tree_with_loading(loading_item_id: Option<&str>) -> Vec<TreeItem> {
  vec![
    TreeItem::new("src", "src")
      .icon(IconName::Folder)
      .expanded(true)
      .child(
        TreeItem::new("src/base", "base")
          .icon(IconName::Folder)
          .expanded(true)
          .loading(loading_item_id == Some("src/base"))
          .children([
            TreeItem::new("src/base/style.rs", "style.rs").icon(IconName::Code),
            TreeItem::new("src/base/theme.rs", "theme.rs")
              .icon(IconName::Code)
              .loading(loading_item_id == Some("src/base/theme.rs")),
            TreeItem::new("src/base/tree.rs", "tree.rs").icon(IconName::Code),
          ]),
      )
      .child(
        TreeItem::new("src/widgets", "widgets")
          .icon(IconName::Folder)
          .expanded(true)
          .children([
            TreeItem::new("src/widgets/button.rs", "button.rs").icon(IconName::Code),
            TreeItem::new("src/widgets/input.rs", "input.rs")
              .icon(IconName::Code)
              .loading(loading_item_id == Some("src/widgets/input.rs")),
            TreeItem::new("src/widgets/tree.rs", "tree.rs").icon(IconName::Code),
          ]),
      ),
    TreeItem::new("examples", "examples")
      .icon(IconName::Folder)
      .expanded(true)
      .children([
        TreeItem::new("examples/controls.rs", "controls.rs").icon(IconName::Code),
        TreeItem::new("examples/tree.rs", "tree.rs").icon(IconName::Code),
      ]),
    TreeItem::new("Cargo.toml", "Cargo.toml").icon(IconName::DocumentText),
    TreeItem::new("README.md", "README.md")
      .icon(IconName::DocumentText)
      .disabled(true),
  ]
}

struct LoadingListItem {
  id: &'static str,
  label: &'static str,
  loading: bool,
}

struct LoadingWindow {
  tree_state: Entity<TreeState>,
  list_items: Vec<LoadingListItem>,
  loading_item_id: Option<String>,
  _subscriptions: Vec<Subscription>,
}

impl LoadingWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    let tree_state = cx.new(|cx| {
      TreeState::new(cx)
        .items(demo_tree_with_loading(None))
        .multi_selectable(false)
    });

    let list_items = vec![
      LoadingListItem {
        id: "item-1",
        label: "Item 1",
        loading: false,
      },
      LoadingListItem {
        id: "item-2",
        label: "Item 2",
        loading: false,
      },
      LoadingListItem {
        id: "item-3",
        label: "Item 3",
        loading: false,
      },
      LoadingListItem {
        id: "item-4",
        label: "Item 4",
        loading: false,
      },
      LoadingListItem {
        id: "item-5",
        label: "Item 5",
        loading: false,
      },
    ];

    cx.new(|cx| {
      let subscriptions =
        vec![
          cx.subscribe(&tree_state, |_this: &mut Self, _, event: &TreeEvent, cx| {
            match event {
              TreeEvent::Select(ix) => {
                println!("Tree select: {}", ix);
              }
              _ => {}
            }
            cx.notify();
          }),
        ];

      Self {
        tree_state,
        list_items,
        loading_item_id: None,
        _subscriptions: subscriptions,
      }
    })
  }
}

impl Render for LoadingWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Loading Spinner Example"))
        .child(
          v_flex()
            .size_full()
            .min_h_0()
            .p_6()
            .gap_4()
            .child(div().text_xl().font_semibold().child("Loading State Demo"))
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Demonstrates loading spinners for Tree and ListItem components."),
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
            .child(
              h_flex()
                .gap_6()
                .child(
                  v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(div().text_lg().font_medium().child("Tree Loading"))
                    .child(
                      div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                          "Click buttons below to toggle loading state on tree items. \
                       The chevron arrow is replaced by a spinner when loading.",
                        ),
                    )
                    .child(
                      h_flex()
                        .gap_2()
                        .mb_2()
                        .child(
                          Button::new("load-src-base")
                            .label("Load src/base")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                              this.loading_item_id =
                                if this.loading_item_id.as_deref() == Some("src/base") {
                                  None
                                } else {
                                  Some("src/base".to_string())
                                };
                              this.tree_state.update(cx, |state, cx| {
                                state.set_items(
                                  demo_tree_with_loading(this.loading_item_id.as_deref()),
                                  cx,
                                );
                              });
                            })),
                        )
                        .child(
                          Button::new("load-theme-rs")
                            .label("Load theme.rs")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                              this.loading_item_id =
                                if this.loading_item_id.as_deref() == Some("src/base/theme.rs") {
                                  None
                                } else {
                                  Some("src/base/theme.rs".to_string())
                                };
                              this.tree_state.update(cx, |state, cx| {
                                state.set_items(
                                  demo_tree_with_loading(this.loading_item_id.as_deref()),
                                  cx,
                                );
                              });
                            })),
                        )
                        .child(
                          Button::new("load-input-rs")
                            .label("Load input.rs")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                              this.loading_item_id = if this.loading_item_id.as_deref()
                                == Some("src/widgets/input.rs")
                              {
                                None
                              } else {
                                Some("src/widgets/input.rs".to_string())
                              };
                              this.tree_state.update(cx, |state, cx| {
                                state.set_items(
                                  demo_tree_with_loading(this.loading_item_id.as_deref()),
                                  cx,
                                );
                              });
                            })),
                        ),
                    )
                    .child(
                      div()
                        .flex_grow()
                        .h_64()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius_container)
                        .bg(cx.theme().card)
                        .p_2()
                        .child(tree(&self.tree_state)),
                    ),
                )
                .child(
                  v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(div().text_lg().font_medium().child("List Loading"))
                    .child(
                      div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(
                          "Click buttons below to toggle loading state on list items. \
                       A spinner appears on the right side when loading.",
                        ),
                    )
                    .child(
                      h_flex()
                        .gap_2()
                        .mb_2()
                        .child(
                          Button::new("load-item-2")
                            .label("Load Item 2")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                              this.list_items = vec![
                                LoadingListItem {
                                  id: "item-1",
                                  label: "Item 1",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-2",
                                  label: "Item 2",
                                  loading: true,
                                },
                                LoadingListItem {
                                  id: "item-3",
                                  label: "Item 3",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-4",
                                  label: "Item 4",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-5",
                                  label: "Item 5",
                                  loading: false,
                                },
                              ];
                              cx.notify();
                            })),
                        )
                        .child(
                          Button::new("load-item-4")
                            .label("Load Item 4")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                              this.list_items = vec![
                                LoadingListItem {
                                  id: "item-1",
                                  label: "Item 1",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-2",
                                  label: "Item 2",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-3",
                                  label: "Item 3",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-4",
                                  label: "Item 4",
                                  loading: true,
                                },
                                LoadingListItem {
                                  id: "item-5",
                                  label: "Item 5",
                                  loading: false,
                                },
                              ];
                              cx.notify();
                            })),
                        )
                        .child(Button::new("load-all").label("Load All").small().on_click(
                          cx.listener(|this, _, _, cx| {
                            this.list_items = vec![
                              LoadingListItem {
                                id: "item-1",
                                label: "Item 1",
                                loading: true,
                              },
                              LoadingListItem {
                                id: "item-2",
                                label: "Item 2",
                                loading: true,
                              },
                              LoadingListItem {
                                id: "item-3",
                                label: "Item 3",
                                loading: true,
                              },
                              LoadingListItem {
                                id: "item-4",
                                label: "Item 4",
                                loading: true,
                              },
                              LoadingListItem {
                                id: "item-5",
                                label: "Item 5",
                                loading: true,
                              },
                            ];
                            cx.notify();
                          }),
                        ))
                        .child(
                          Button::new("clear-loading")
                            .label("Clear Loading")
                            .small()
                            .on_click(cx.listener(|this, _, _, cx| {
                              this.list_items = vec![
                                LoadingListItem {
                                  id: "item-1",
                                  label: "Item 1",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-2",
                                  label: "Item 2",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-3",
                                  label: "Item 3",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-4",
                                  label: "Item 4",
                                  loading: false,
                                },
                                LoadingListItem {
                                  id: "item-5",
                                  label: "Item 5",
                                  loading: false,
                                },
                              ];
                              cx.notify();
                            })),
                        ),
                    )
                    .child(
                      div()
                        .flex_grow()
                        .min_h_0()
                        .border_1()
                        .border_color(cx.theme().border)
                        .rounded(cx.theme().radius_container)
                        .bg(cx.theme().card)
                        .p_2()
                        .child(
                          v_flex()
                            .gap_1()
                            .children(self.list_items.iter().map(|item| {
                              ListItem::new(item.id)
                                .loading(item.loading)
                                .child(item.label)
                            })),
                        ),
                    ),
                ),
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

    let bounds = Bounds::centered(None, GpuiSize::new(px(1200.), px(700.)), cx);
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
        LoadingWindow::view,
      )
      .expect("open loading demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Loading Spinner Example");
      })
      .expect("update loading demo window failed");
  });
}
