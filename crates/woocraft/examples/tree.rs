use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Subscription, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, IconName, PopupMenuItem, Selectable, Sizable,
  StyledExt, Theme, ThemeMode, TitleBar, TreeEvent, TreeItem, TreeState, h_flex, init, tree,
  v_flex, window_border,
};

fn demo_tree(expanded: bool) -> Vec<TreeItem> {
  vec![
    TreeItem::new("src", "src")
      .icon(IconName::Folder)
      .expanded(expanded)
      .child(
        TreeItem::new("src/base", "base")
          .icon(IconName::Folder)
          .expanded(expanded)
          .children([
            TreeItem::new("src/base/style.rs", "style.rs").icon(IconName::Code),
            TreeItem::new("src/base/theme.rs", "theme.rs").icon(IconName::Code),
            TreeItem::new("src/base/tree.rs", "tree.rs").icon(IconName::Code),
          ]),
      )
      .child(
        TreeItem::new("src/widgets", "widgets")
          .icon(IconName::Folder)
          .expanded(expanded)
          .children([
            TreeItem::new("src/widgets/button.rs", "button.rs").icon(IconName::Code),
            TreeItem::new("src/widgets/input.rs", "input.rs").icon(IconName::Code),
            TreeItem::new("src/widgets/tree.rs", "tree.rs").icon(IconName::Code),
          ]),
      ),
    TreeItem::new("examples", "examples")
      .icon(IconName::Folder)
      .expanded(expanded)
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

struct TreeWindow {
  tree_state: Entity<TreeState>,
  last_event: String,
  multi_select: bool,
  drag_enabled: bool,
  _subscriptions: Vec<Subscription>,
}

impl TreeWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    let tree_state = cx.new(|cx| {
      TreeState::new(cx)
        .items(demo_tree(true))
        .multi_selectable(false)
        .draggable(false)
    });

    cx.new(|cx| {
      let subscriptions =
        vec![
          cx.subscribe(&tree_state, |this: &mut Self, _, event: &TreeEvent, cx| {
            this.last_event = match event {
              TreeEvent::Select(ix) => format!("Select({ix})"),
              TreeEvent::DoubleClicked(ix) => format!("DoubleClicked({ix})"),
              TreeEvent::RightClicked(ix) => format!("RightClicked({ix})"),
              TreeEvent::ClearSelection => "ClearSelection".to_string(),
              TreeEvent::ToggleSelect(ix) => format!("ToggleSelect({ix})"),
              TreeEvent::RangeSelect(ix) => format!("RangeSelect({ix})"),
              TreeEvent::MoveItem(from, to) => format!("MoveItem({from} -> {to})"),
            };
            cx.notify();
          }),
        ];

      Self {
        tree_state,
        last_event: "Ready".to_string(),
        multi_select: false,
        drag_enabled: false,
        _subscriptions: subscriptions,
      }
    })
  }
}

impl Render for TreeWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();
    let selected_info = {
      let state = self.tree_state.read(cx);
      if self.multi_select {
        let indices = state.model().selected_indices();
        if indices.is_empty() {
          "none".to_string()
        } else {
          format!("{} items", indices.len())
        }
      } else if let Some(item) = state.selected_item() {
        format!("{}", item.label)
      } else {
        "none".to_string()
      }
    };

    let tree_state_for_menu = self.tree_state.clone();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(TitleBar::new().title("Woocraft Tree Example"))
        .child(
          v_flex()
            .size_full()
            .min_h_0()
            .p_6()
            .gap_4()
            .child(div().text_xl().font_semibold().child("Tree Component"))
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child("Click row or use keyboard (Up/Down/Left/Right/Enter) to navigate. Right-click for context menu."),
            )
            .child(
              h_flex()
                .gap_3()
                .child(
                  Button::new("tree-theme-light")
                    .label("Light")
                    .small()
                    .selected(!is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                )
                .child(
                  Button::new("tree-theme-dark")
                    .label("Dark")
                    .small()
                    .selected(is_dark)
                    .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                )
                .child(
                  Button::new("tree-expand-all")
                    .label("Expand All")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                      this.tree_state.update(cx, |state, cx| {
                        state.set_items(demo_tree(true), cx);
                      });
                    })),
                )
                .child(
                  Button::new("tree-collapse-all")
                    .label("Collapse All")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                      this.tree_state.update(cx, |state, cx| {
                        state.set_items(demo_tree(false), cx);
                      });
                    })),
                )
                .child(
                  Button::new("tree-toggle-multi")
                    .label(if self.multi_select {
                      "Multi-Select: ON"
                    } else {
                      "Multi-Select: OFF"
                    })
                    .small()
                    .when(self.multi_select, |this| this.primary())
                    .on_click(cx.listener(|this, _, _, cx| {
                      this.multi_select = !this.multi_select;
                      let enabled = this.multi_select;
                      this.tree_state.update(cx, |state, cx| {
                        state.multi_selectable = enabled;
                        state.model_mut().set_multi_selectable(enabled);
                        state.clear_selection(cx);
                      });
                      cx.notify();
                    })),
                )
                .child(
                  Button::new("tree-toggle-drag")
                    .label(if self.drag_enabled {
                      "Drag: ON"
                    } else {
                      "Drag: OFF"
                    })
                    .small()
                    .when(self.drag_enabled, |this| this.primary())
                    .on_click(cx.listener(|this, _, _, cx| {
                      this.drag_enabled = !this.drag_enabled;
                      let enabled = this.drag_enabled;
                      this.tree_state.update(cx, |state, _| {
                        state.draggable = enabled;
                      });
                      cx.notify();
                    })),
                ),
            )
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                  "Selected: {} | Last Event: {}",
                  selected_info, self.last_event
                )),
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
                  tree(&self.tree_state).context_menu(move |ix, entry, menu, _window, _cx| {
                    let label = entry.item().label.clone();
                    let is_folder = entry.is_folder();
                    let item_id = entry.item().id.clone();
                    let tree_entity = tree_state_for_menu.clone();

                    menu
                      .item(PopupMenuItem::label(format!("{}", label)))
                      .separator()
                      .item(
                        PopupMenuItem::new(if is_folder {
                          "Toggle Expand"
                        } else {
                          "Open File"
                        })
                        .on_click(move |_, _, _cx| {
                          println!(
                            "tree example: action on {} ({})",
                            label,
                            if is_folder { "folder" } else { "file" }
                          );
                        }),
                      )
                      .item(
                        PopupMenuItem::new("Select This Item").on_click(
                          move |_, _, cx| {
                            tree_entity.update(cx, |state, cx| {
                              state.set_selected_index(Some(ix), cx);
                            });
                          },
                        ),
                      )
                      .item(PopupMenuItem::new("Print Info").on_click(
                        move |_, _, _| {
                          println!("tree example: item id={}", item_id);
                        },
                      ))
                  }),
                ),
            ),
        ),
    )
  }
}

fn main() {
  let app = gpui::Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(980.), px(700.)), cx);
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
        TreeWindow::view,
      )
      .expect("open tree demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Tree Example");
      })
      .expect("update tree demo window failed");
  });
}
