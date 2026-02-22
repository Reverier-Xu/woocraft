use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, IconName, Selectable, Sizable, StyledExt, Theme, ThemeMode, TitleBar,
  TreeItem, TreeState, h_flex, init, tree, v_flex, window_border,
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
}

impl TreeWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    let tree_state = cx.new(|cx| TreeState::new(cx).items(demo_tree(true)));
    cx.new(|_| Self { tree_state })
  }
}

impl Render for TreeWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();

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
                .child("Click row or use keyboard (Up/Down/Left/Right/Enter) to navigate."),
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
                .child(tree(&self.tree_state)),
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
