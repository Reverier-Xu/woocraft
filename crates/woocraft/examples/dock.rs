use std::sync::Arc;

use gpui::{
  App, AppContext, Application, Bounds, Context, Edges, Entity, FocusHandle, Focusable,
  InteractiveElement as _, IntoElement, Menu, MenuItem, ParentElement, Render, SharedString,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, actions, div, px,
};
use woocraft::{
  ActiveTheme, AppMenuBar, DockArea, DockItem, DockPlacement, IconName, Panel, PanelEvent,
  PopupMenuItem, StyledExt as _, TitleBar, v_flex, window_border,
};

actions!(
  dock_example_menu,
  [
    ToggleLeftDock,
    ToggleBottomDock,
    ToggleRightDock,
    ExpandAllDocks,
    CollapseAllDocks,
    ResetDockSizes,
    AboutWoocraft,
    AboutVersion,
    AboutLicense,
    AboutChangelogHighlights,
    AboutChangelogRoadmap,
    AboutCreditsCore,
    AboutCreditsCommunity,
  ]
);

struct ExamplePanel {
  id: SharedString,
  title: SharedString,
  focus_handle: FocusHandle,
}

impl ExamplePanel {
  fn new(
    id: impl Into<SharedString>, title: impl Into<SharedString>, cx: &mut Context<Self>,
  ) -> Self {
    Self {
      id: id.into(),
      title: title.into(),
      focus_handle: cx.focus_handle(),
    }
  }
}

impl Panel for ExamplePanel {
  fn panel_name(&self) -> &'static str {
    "ExamplePanel"
  }

  fn panel_id(&self, _cx: &App) -> SharedString {
    self.id.clone()
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(self.title.clone())
  }

  fn title(&self, _cx: &App) -> SharedString {
    self.title.clone()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Grid
  }
}

impl gpui::EventEmitter<PanelEvent> for ExamplePanel {}

impl Focusable for ExamplePanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ExamplePanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .p_4()
      .gap_3()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(div().text_lg().font_semibold().child(self.title.clone()))
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child("This panel is rendered by the migrated dock system."),
      )
      .child(
        div()
          .flex_1()
          .rounded_md()
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().card),
      )
  }
}

struct DockExample {
  dock_area: Entity<DockArea>,
  app_menu_bar: Entity<AppMenuBar>,
}

impl DockExample {
  fn panel(
    id: impl Into<SharedString>, title: impl Into<SharedString>, _window: &mut Window,
    cx: &mut App,
  ) -> Entity<ExamplePanel> {
    let id: SharedString = id.into();
    let title: SharedString = title.into();
    cx.new(move |cx| ExamplePanel::new(id.clone(), title.clone(), cx))
  }

  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let dock_area = cx.new(|cx| DockArea::new("dock-example", Some(1), window, cx));
    let app_menu_bar = AppMenuBar::new(cx);
    let weak = dock_area.downgrade();

    let editor = Self::panel("editor:/workspace/main.rs", "Editor", window, cx);
    let preview = Self::panel("preview:/workspace/main.rs", "Preview", window, cx);
    let inspector = Self::panel("inspector:/workspace/main.rs", "Inspector", window, cx);
    let explorer = Self::panel("explorer:/workspace", "Explorer", window, cx);
    let outline = Self::panel("outline:/workspace/main.rs", "Outline", window, cx);
    let terminal = Self::panel("terminal:default", "Terminal", window, cx);
    let problems = Self::panel("problems:default", "Problems", window, cx);
    let references = Self::panel("references:/workspace/main.rs", "References", window, cx);

    let center = DockItem::h_split(
      vec![
        DockItem::tab(editor, &weak, window, cx).size(px(540.)),
        DockItem::v_split(
          vec![
            DockItem::tab(preview, &weak, window, cx),
            DockItem::tab(inspector, &weak, window, cx).size(px(220.)),
          ],
          &weak,
          window,
          cx,
        )
        .size(px(360.)),
      ],
      &weak,
      window,
      cx,
    );

    let left = DockItem::tabs(
      vec![Arc::new(explorer.clone()), Arc::new(outline.clone())],
      &weak,
      window,
      cx,
    );

    let bottom = DockItem::tabs(
      vec![Arc::new(terminal.clone()), Arc::new(problems.clone())],
      &weak,
      window,
      cx,
    );

    let right = DockItem::tabs(vec![Arc::new(references.clone())], &weak, window, cx);

    dock_area.update(cx, |dock, cx| {
      dock.set_center(center, window, cx);
      dock.set_left_dock(left, Some(px(260.)), true, window, cx);
      dock.set_bottom_dock(bottom, Some(px(220.)), true, window, cx);
      dock.set_right_dock(right, Some(px(280.)), true, window, cx);
      dock.set_dock_collapsible(
        Edges {
          left: true,
          bottom: true,
          right: true,
          ..Default::default()
        },
        window,
        cx,
      );
    });

    cx.new(|_| Self {
      dock_area,
      app_menu_bar,
    })
  }

  fn toggle_dock(&mut self, placement: DockPlacement, window: &mut Window, cx: &mut Context<Self>) {
    self.dock_area.update(cx, |dock, cx| {
      dock.toggle_dock(placement, window, cx);
    });
  }

  fn on_toggle_left_dock(
    &mut self, _: &ToggleLeftDock, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.toggle_dock(DockPlacement::Left, window, cx);
  }

  fn on_toggle_bottom_dock(
    &mut self, _: &ToggleBottomDock, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.toggle_dock(DockPlacement::Bottom, window, cx);
  }

  fn on_toggle_right_dock(
    &mut self, _: &ToggleRightDock, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.toggle_dock(DockPlacement::Right, window, cx);
  }

  fn on_expand_all_docks(
    &mut self, _: &ExpandAllDocks, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.dock_area.update(cx, |dock, cx| {
      let left = dock.left_dock().cloned();
      let bottom = dock.bottom_dock().cloned();
      let right = dock.right_dock().cloned();

      for dock in [left, bottom, right].into_iter().flatten() {
        dock.update(cx, |dock, cx| {
          dock.set_collapsed(false, window, cx);
        });
      }
    });
  }

  fn on_collapse_all_docks(
    &mut self, _: &CollapseAllDocks, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.dock_area.update(cx, |dock, cx| {
      let left = dock.left_dock().cloned();
      let bottom = dock.bottom_dock().cloned();
      let right = dock.right_dock().cloned();

      for dock in [left, bottom, right].into_iter().flatten() {
        dock.update(cx, |dock, cx| {
          dock.set_collapsed(true, window, cx);
        });
      }
    });
  }

  fn on_reset_dock_sizes(
    &mut self, _: &ResetDockSizes, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.dock_area.update(cx, |dock, cx| {
      if let Some(left) = dock.left_dock().cloned() {
        left.update(cx, |dock, cx| {
          dock.set_size(px(260.), window, cx);
          dock.set_collapsed(false, window, cx);
        });
      }
      if let Some(bottom) = dock.bottom_dock().cloned() {
        bottom.update(cx, |dock, cx| {
          dock.set_size(px(220.), window, cx);
          dock.set_collapsed(false, window, cx);
        });
      }
      if let Some(right) = dock.right_dock().cloned() {
        right.update(cx, |dock, cx| {
          dock.set_size(px(280.), window, cx);
          dock.set_collapsed(false, window, cx);
        });
      }
    });
  }

  fn on_about_woocraft(&mut self, _: &AboutWoocraft, _: &mut Window, _: &mut Context<Self>) {
    println!("Woocraft Dock Example: modular dock + title bar integration demo.");
  }

  fn on_about_version(&mut self, _: &AboutVersion, _: &mut Window, _: &mut Context<Self>) {
    println!("Version: woocraft crate demo for dock/menu/titlebar integration.");
  }

  fn on_about_license(&mut self, _: &AboutLicense, _: &mut Window, _: &mut Context<Self>) {
    println!("License: see repository LICENSE file for details.");
  }

  fn on_about_changelog_highlights(
    &mut self, _: &AboutChangelogHighlights, _: &mut Window, _: &mut Context<Self>,
  ) {
    println!("Changelog: v0.1 highlights include migrated dock, controls and title bar features.");
  }

  fn on_about_changelog_roadmap(
    &mut self, _: &AboutChangelogRoadmap, _: &mut Window, _: &mut Context<Self>,
  ) {
    println!("Roadmap: continue improving dock actions, panel workflows and menu integration.");
  }

  fn on_about_credits_core(&mut self, _: &AboutCreditsCore, _: &mut Window, _: &mut Context<Self>) {
    println!("Credits: Core Team");
  }

  fn on_about_credits_community(
    &mut self, _: &AboutCreditsCommunity, _: &mut Window, _: &mut Context<Self>,
  ) {
    println!("Credits: Community Contributors");
  }
}

impl Render for DockExample {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .on_action(cx.listener(Self::on_toggle_left_dock))
        .on_action(cx.listener(Self::on_toggle_bottom_dock))
        .on_action(cx.listener(Self::on_toggle_right_dock))
        .on_action(cx.listener(Self::on_expand_all_docks))
        .on_action(cx.listener(Self::on_collapse_all_docks))
        .on_action(cx.listener(Self::on_reset_dock_sizes))
        .on_action(cx.listener(Self::on_about_woocraft))
        .on_action(cx.listener(Self::on_about_version))
        .on_action(cx.listener(Self::on_about_license))
        .on_action(cx.listener(Self::on_about_changelog_highlights))
        .on_action(cx.listener(Self::on_about_changelog_roadmap))
        .on_action(cx.listener(Self::on_about_credits_core))
        .on_action(cx.listener(Self::on_about_credits_community))
        .child(
          TitleBar::new()
            .title("Woocraft Dock Example")
            .app_menu_bar(self.app_menu_bar.clone())
            .title_menu(|menu, _, _| {
              menu
                .item(PopupMenuItem::new("New Tab"))
                .item(PopupMenuItem::new("Split Pane"))
                .separator()
                .item(PopupMenuItem::new("Preferences"))
            })
            .theme_button(true)
            .language_button(true),
        )
        .child(self.dock_area.clone()),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    cx.activate(true);
    cx.set_menus(vec![
      Menu {
        name: "Layout".into(),
        items: vec![
          MenuItem::action("Toggle Left Dock", ToggleLeftDock),
          MenuItem::action("Toggle Bottom Dock", ToggleBottomDock),
          MenuItem::action("Toggle Right Dock", ToggleRightDock),
          MenuItem::separator(),
          MenuItem::action("Expand All Docks", ExpandAllDocks),
          MenuItem::action("Collapse All Docks", CollapseAllDocks),
          MenuItem::submenu(Menu {
            name: "Resize Presets".into(),
            items: vec![
              MenuItem::action("Reset to Default Sizes", ResetDockSizes),
              MenuItem::submenu(Menu {
                name: "Read Modes".into(),
                items: vec![
                  MenuItem::action("Focus Mode (Collapse All Docks)", CollapseAllDocks),
                  MenuItem::action("Balanced Mode (Expand All Docks)", ExpandAllDocks),
                ],
              }),
            ],
          }),
        ],
      },
      Menu {
        name: "About".into(),
        items: vec![
          MenuItem::action("About Woocraft Dock Example", AboutWoocraft),
          MenuItem::action("Version", AboutVersion),
          MenuItem::action("License", AboutLicense),
          MenuItem::separator(),
          MenuItem::submenu(Menu {
            name: "Changelog".into(),
            items: vec![
              MenuItem::action("v0.1 Highlights", AboutChangelogHighlights),
              MenuItem::action("Roadmap", AboutChangelogRoadmap),
            ],
          }),
          MenuItem::submenu(Menu {
            name: "Credits".into(),
            items: vec![
              MenuItem::action("Core Team", AboutCreditsCore),
              MenuItem::action("Community Contributors", AboutCreditsCommunity),
            ],
          }),
        ],
      },
    ]);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1200.), px(820.)), cx);
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
        |window, cx| DockExample::view(window, cx),
      )
      .expect("open dock example window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Dock Example");
      })
      .expect("update dock example window failed");
  });
}
