use std::sync::Arc;

use gpuim::{
  App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
  IntoElement, Menu, MenuItem, ParentElement, Render, SharedString, Size as GpuiSize, Styled,
  Window, WindowBounds, WindowOptions, actions, div, px,
};
use woocraft::{
  ActiveTheme, AppMenuBar, DockArea, DockPlacement, IconName, Panel, PanelEvent, PopupMenuItem,
  StyledExt as _, TitleBar, v_flex, window_border,
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

impl gpuim::EventEmitter<PanelEvent> for ExamplePanel {}

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
    id: impl Into<SharedString>, title: impl Into<SharedString>, _window: &mut Window, cx: &mut App,
  ) -> Entity<ExamplePanel> {
    let id: SharedString = id.into();
    let title: SharedString = title.into();
    cx.new(move |cx| ExamplePanel::new(id.clone(), title.clone(), cx))
  }

  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let dock_area = cx.new(|cx| DockArea::new("dock-example", Some(1), window, cx));
    let app_menu_bar = AppMenuBar::new(cx);

    let editor = Self::panel("editor:/workspace/main.rs", "Editor", window, cx);
    let preview = Self::panel("preview:/workspace/main.rs", "Preview", window, cx);
    let inspector = Self::panel("inspector:/workspace/main.rs", "Inspector", window, cx);
    let explorer = Self::panel("explorer:/workspace", "Explorer", window, cx);
    let outline = Self::panel("outline:/workspace/main.rs", "Outline", window, cx);
    let terminal = Self::panel("terminal:default", "Terminal", window, cx);
    let problems = Self::panel("problems:default", "Problems", window, cx);
    let references = Self::panel("references:/workspace/main.rs", "References", window, cx);

    dock_area.update(cx, |dock, cx| {
      // Add panels to center area
      dock.add_to_center(Arc::new(editor.clone()), window, cx);
      dock.add_to_center(Arc::new(preview.clone()), window, cx);
      dock.add_to_center(Arc::new(inspector.clone()), window, cx);

      // Add panels to side docks
      dock.add_to_left_dock(Arc::new(explorer.clone()), window, cx);
      dock.add_to_left_dock(Arc::new(outline.clone()), window, cx);
      dock.add_to_bottom_dock(Arc::new(terminal.clone()), window, cx);
      dock.add_to_bottom_dock(Arc::new(problems.clone()), window, cx);
      dock.add_to_right_dock(Arc::new(references.clone()), window, cx);

      // Configure dock sizes
      dock.set_dock_size(DockPlacement::Left, px(260.), window, cx);
      dock.set_dock_size(DockPlacement::Bottom, px(220.), window, cx);
      dock.set_dock_size(DockPlacement::Right, px(280.), window, cx);
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
      dock.set_dock_collapsed(DockPlacement::Left, false, window, cx);
      dock.set_dock_collapsed(DockPlacement::Bottom, false, window, cx);
      dock.set_dock_collapsed(DockPlacement::Right, false, window, cx);
    });
  }

  fn on_collapse_all_docks(
    &mut self, _: &CollapseAllDocks, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.dock_area.update(cx, |dock, cx| {
      dock.set_dock_collapsed(DockPlacement::Left, true, window, cx);
      dock.set_dock_collapsed(DockPlacement::Bottom, true, window, cx);
      dock.set_dock_collapsed(DockPlacement::Right, true, window, cx);
    });
  }

  fn on_reset_dock_sizes(
    &mut self, _: &ResetDockSizes, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.dock_area.update(cx, |dock, cx| {
      dock.set_dock_size(DockPlacement::Left, px(260.), window, cx);
      dock.set_dock_collapsed(DockPlacement::Left, false, window, cx);
      dock.set_dock_size(DockPlacement::Bottom, px(220.), window, cx);
      dock.set_dock_collapsed(DockPlacement::Bottom, false, window, cx);
      dock.set_dock_size(DockPlacement::Right, px(280.), window, cx);
      dock.set_dock_collapsed(DockPlacement::Right, false, window, cx);
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
  let app = gpuim_platform::application().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    cx.activate(true);
    cx.set_menus(vec![
      Menu {
        disabled: false,
        name: "Layout".into(),
        items: vec![
          MenuItem::action("Toggle Left Dock", ToggleLeftDock),
          MenuItem::action("Toggle Bottom Dock", ToggleBottomDock),
          MenuItem::action("Toggle Right Dock", ToggleRightDock),
          MenuItem::separator(),
          MenuItem::action("Expand All Docks", ExpandAllDocks),
          MenuItem::action("Collapse All Docks", CollapseAllDocks),
          MenuItem::submenu(Menu {
            disabled: false,
            name: "Resize Presets".into(),
            items: vec![
              MenuItem::action("Reset to Default Sizes", ResetDockSizes),
              MenuItem::submenu(Menu {
                disabled: false,
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
        disabled: false,
        name: "About".into(),
        items: vec![
          MenuItem::action("About Woocraft Dock Example", AboutWoocraft),
          MenuItem::action("Version", AboutVersion),
          MenuItem::action("License", AboutLicense),
          MenuItem::separator(),
          MenuItem::submenu(Menu {
            disabled: false,
            name: "Changelog".into(),
            items: vec![
              MenuItem::action("v0.1 Highlights", AboutChangelogHighlights),
              MenuItem::action("Roadmap", AboutChangelogRoadmap),
            ],
          }),
          MenuItem::submenu(Menu {
            disabled: false,
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
          window_background: gpuim::WindowBackgroundAppearance::Transparent,
          #[cfg(target_os = "linux")]
          window_decorations: Some(gpuim::WindowDecorations::Client),
          ..Default::default()
        },
        DockExample::view,
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
