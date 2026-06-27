use std::time::Duration;

use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Subscription, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, ContextMenuExt as _, IconName, ListItem, PopupMenuItem,
  Selectable, Sizable, StyledExt, Theme, ThemeMode, TitleBar, TreeEvent, TreeItem, TreeState,
  h_flex, init, tree, v_flex, window_border,
};

fn demo_tree_with_loading(
  loading_item_id: Option<&str>, extra_children: &[(String, String)],
) -> Vec<TreeItem> {
  let mut base_children: Vec<TreeItem> = vec![
    TreeItem::new("src/base/style.rs", "style.rs").icon(IconName::Code),
    TreeItem::new("src/base/theme.rs", "theme.rs")
      .icon(IconName::Code)
      .loading(loading_item_id == Some("src/base/theme.rs")),
    TreeItem::new("src/base/tree.rs", "tree.rs").icon(IconName::Code),
  ];
  // Append dynamically loaded files under src/base.
  for (id, label) in extra_children {
    base_children.push(TreeItem::new(id.clone(), label.clone()).icon(IconName::Code));
  }

  vec![
    TreeItem::new("src", "src")
      .expanded(true)
      .child(
        TreeItem::new("src/base", "base")
          .expanded(true)
          .loading(loading_item_id == Some("src/base"))
          .children(base_children),
      )
      .child(
        TreeItem::new("src/widgets", "widgets")
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
  /// Dynamically loaded extra files under src/base (simulates lazy loading).
  extra_children: Vec<(String, String)>,
  _subscriptions: Vec<Subscription>,
}

impl LoadingWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    let tree_state = cx.new(|cx| {
      TreeState::new(cx)
        .items(demo_tree_with_loading(None, &[]))
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
            if let TreeEvent::Select(ix) = event {
              println!("Tree select: {}", ix);
            }
            cx.notify();
          }),
        ];

      Self {
        tree_state,
        list_items,
        loading_item_id: None,
        extra_children: vec![],
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
        .child(TitleBar::new().title("Loading & Hot-Update Example"))
        .child(
          v_flex()
            .size_full()
            .min_h_0()
            .p_6()
            .gap_4()
            .child(
              div()
                .text_xl()
                .font_semibold()
                .child("Loading & Hot-Update Demo"),
            )
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(
                  "Demonstrates loading spinners and hot-update support. \
                   Right-click tree items to see context menus that survive data updates. \
                   Selection, expand/collapse state, and scroll position are all preserved.",
                ),
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
                .child(self.render_tree_section(cx))
                .child(self.render_list_section(cx)),
            ),
        ),
    )
  }
}

impl LoadingWindow {
  fn render_tree_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
    let tree_state_for_menu = self.tree_state.clone();

    v_flex()
      .flex_1()
      .min_h_0()
      .child(div().text_lg().font_medium().child("Tree Hot-Update"))
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child(
            "Uses update_items() to replace data while preserving expand state, \
             selection, and context menus. Right-click any item, then click a \
             button — the menu stays open.",
          ),
      )
      .child(
        h_flex()
          .gap_2()
          .flex_wrap()
          .mb_2()
          .child(
            Button::new("load-src-base")
              .label("Toggle base/ loading")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                this.loading_item_id = if this.loading_item_id.as_deref() == Some("src/base") {
                  None
                } else {
                  Some("src/base".to_string())
                };
                this.tree_state.update(cx, |state, cx| {
                  state.update_items(
                    demo_tree_with_loading(this.loading_item_id.as_deref(), &this.extra_children),
                    cx,
                  );
                });
              })),
          )
          .child(
            Button::new("load-theme-rs")
              .label("Toggle theme.rs loading")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                this.loading_item_id =
                  if this.loading_item_id.as_deref() == Some("src/base/theme.rs") {
                    None
                  } else {
                    Some("src/base/theme.rs".to_string())
                  };
                this.tree_state.update(cx, |state, cx| {
                  state.update_items(
                    demo_tree_with_loading(this.loading_item_id.as_deref(), &this.extra_children),
                    cx,
                  );
                });
              })),
          )
          .child(
            Button::new("load-input-rs")
              .label("Toggle input.rs loading")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                this.loading_item_id =
                  if this.loading_item_id.as_deref() == Some("src/widgets/input.rs") {
                    None
                  } else {
                    Some("src/widgets/input.rs".to_string())
                  };
                this.tree_state.update(cx, |state, cx| {
                  state.update_items(
                    demo_tree_with_loading(this.loading_item_id.as_deref(), &this.extra_children),
                    cx,
                  );
                });
              })),
          )
          .child(
            Button::new("async-load")
              .label("Async Load Files")
              .small()
              .primary()
              .on_click(cx.listener(|this, _, window, cx| {
                // Step 1: set loading state
                this.loading_item_id = Some("src/base".to_string());
                this.tree_state.update(cx, |state, cx| {
                  state.update_items(
                    demo_tree_with_loading(this.loading_item_id.as_deref(), &this.extra_children),
                    cx,
                  );
                });

                // Step 2: simulate async load — after 2s add new files
                let entity = cx.entity().clone();
                cx.spawn_in(window, async move |_, cx| {
                  smol::Timer::after(Duration::from_secs(2)).await;
                  _ = entity.update_in(cx, |this: &mut LoadingWindow, _, cx| {
                    this.loading_item_id = None;
                    this.extra_children = vec![
                      ("src/base/color.rs".into(), "color.rs".into()),
                      ("src/base/layout.rs".into(), "layout.rs".into()),
                    ];
                    this.tree_state.update(cx, |state, cx| {
                      state.update_items(demo_tree_with_loading(None, &this.extra_children), cx);
                    });
                  });
                })
                .detach();
              })),
          )
          .child(
            Button::new("reset-tree")
              .label("Reset")
              .small()
              .flat()
              .on_click(cx.listener(|this, _, _, cx| {
                this.loading_item_id = None;
                this.extra_children.clear();
                this.tree_state.update(cx, |state, cx| {
                  state.update_items(demo_tree_with_loading(None, &[]), cx);
                });
              })),
          ),
      )
      .child(
        div()
          .flex_grow(1.)
          .h_64()
          .border_1()
          .border_color(cx.theme().border)
          .rounded(cx.theme().radius_container)
          .bg(cx.theme().card)
          .p_2()
          .child(tree(&self.tree_state).bottom_gap(px(64.)).context_menu(
            move |ix, entry, menu, _window, _cx| {
              let label = entry.item().label.clone();
              let is_folder = entry.is_folder();
              let tree_entity = tree_state_for_menu.clone();

              menu
                .item(PopupMenuItem::label(format!("[{}] {}", ix, label)))
                .separator()
                .item(if is_folder {
                  PopupMenuItem::new("Toggle Expand").on_click(move |_, _, _cx| {
                    println!("toggle expand: ix={}", ix);
                  })
                } else {
                  PopupMenuItem::new("Open File").on_click(move |_, _, _cx| {
                    println!("Open file: {}", label);
                  })
                })
                .item(
                  PopupMenuItem::new("Select This Item").on_click(move |_, _, cx| {
                    tree_entity.update(cx, |state, cx| {
                      state.set_selected_index(Some(ix), cx);
                    });
                  }),
                )
            },
          )),
      )
  }

  fn render_list_section(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
             Right-click items to see context menus. A spinner appears when loading.",
          ),
      )
      .child(
        h_flex()
          .gap_2()
          .flex_wrap()
          .mb_2()
          .child(
            Button::new("load-item-2")
              .label("Load Item 2")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                toggle_list_loading(&mut this.list_items, "item-2");
                cx.notify();
              })),
          )
          .child(
            Button::new("load-item-4")
              .label("Load Item 4")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                toggle_list_loading(&mut this.list_items, "item-4");
                cx.notify();
              })),
          )
          .child(
            Button::new("load-all")
              .label("Load All")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                for item in &mut this.list_items {
                  item.loading = true;
                }
                cx.notify();
              })),
          )
          .child(
            Button::new("clear-loading")
              .label("Clear Loading")
              .small()
              .on_click(cx.listener(|this, _, _, cx| {
                for item in &mut this.list_items {
                  item.loading = false;
                }
                cx.notify();
              })),
          ),
      )
      .child(
        div()
          .flex_grow(1.)
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
                let label: &str = item.label;
                ListItem::new(item.id)
                  .loading(item.loading)
                  .child(item.label)
                  .context_menu(move |menu, _, _| {
                    menu
                      .item(PopupMenuItem::label(label))
                      .separator()
                      .item(PopupMenuItem::new("Action 1"))
                      .item(PopupMenuItem::new("Action 2"))
                  })
              })),
          ),
      )
  }
}

fn toggle_list_loading(items: &mut [LoadingListItem], target_id: &str) {
  for item in items.iter_mut() {
    if item.id == target_id {
      item.loading = !item.loading;
    }
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
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
          window.set_window_title("Loading & Hot-Update Example");
        })
        .expect("update loading demo window failed");
    });
}
