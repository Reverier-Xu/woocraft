//! Dock + terminal integration demo — a small "workbench" shell.
//!
//! Combines the dock system with [`woocraft::TerminalView`] panels:
//!
//! - the bottom dock hosts multiple terminal panels, each backed by its own PTY
//!   session (spawned with the platform default shell),
//! - terminal tabs pick up the application title (OSC 0/2) as their tab name,
//! - the application menu drives sessions externally: spawn a new terminal
//!   panel at runtime, or type a command into a terminal programmatically;
//! - closing a terminal tab kills its PTY session (via `Panel::on_removed`).
//!
//! Run with: `cargo run -p woocraft --example terminal_dock`

use std::sync::Arc;

use gpui::{
  App, AppContext as _, Context, Entity, FocusHandle, Focusable, InteractiveElement as _,
  IntoElement, ParentElement as _, Render, SharedString, Styled as _, Window, WindowBounds,
  WindowOptions, actions, div, px,
};
use woocraft::{
  ActiveTheme, AppMenuBar, DockArea, DockPlacement, IconName, Panel, PanelEvent, StyledExt as _,
  TerminalView, TerminalViewEvent, TitleBar, init, v_flex, window_border,
};
use woocraft_terminal::{SpawnOptions, TerminalBounds, TerminalSession};

actions!(
  terminal_dock_example,
  [NewTerminal, RunBuildInTerminal, ClearActiveTerminal]
);

/// A dock panel hosting one terminal session.
struct TerminalPanel {
  label: SharedString,
  terminal: Entity<TerminalView>,
  /// Application-provided title (OSC 0/2), mirrored from view events.
  app_title: Option<String>,
  focus_handle: FocusHandle,
}

impl TerminalPanel {
  fn new(label: impl Into<SharedString>, window: &mut Window, cx: &mut Context<Self>) -> Self {
    let session = TerminalSession::spawn(
      SpawnOptions::default_shell_options(),
      TerminalBounds::default(),
    )
    .expect("failed to spawn terminal session");

    let terminal = cx.new(|cx| TerminalView::new(session, window, cx));
    let focus_handle = terminal.focus_handle(cx);
    cx.subscribe(&terminal, Self::on_terminal_event).detach();

    Self {
      label: label.into(),
      terminal,
      app_title: None,
      focus_handle,
    }
  }

  fn on_terminal_event(
    &mut self, _: Entity<TerminalView>, event: &TerminalViewEvent, cx: &mut Context<Self>,
  ) {
    if let TerminalViewEvent::TitleChanged(title) = event {
      self.app_title = title.clone();
      // Tab names are re-read after this notification.
      cx.notify();
    }
  }

  fn tab_title(&self) -> SharedString {
    match &self.app_title {
      Some(title) if !title.is_empty() => SharedString::from(title.clone()),
      _ => self.label.clone(),
    }
  }
}

impl Panel for TerminalPanel {
  fn panel_name(&self) -> &'static str {
    "TerminalPanel"
  }

  fn panel_id(&self, _cx: &App) -> SharedString {
    SharedString::from(format!("terminal:{}", self.terminal.entity_id().as_u64()))
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(self.tab_title())
  }

  fn title(&self, _cx: &App) -> SharedString {
    self.tab_title()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Prompt
  }

  fn on_removed(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    // Closing the tab ends the session: kill the PTY so the shell exits.
    self.terminal.read(cx).session().kill();
  }
}

impl gpui::EventEmitter<PanelEvent> for TerminalPanel {}

impl Focusable for TerminalPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for TerminalPanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    div()
      .id("terminal-panel")
      .size_full()
      .bg(cx.theme().background)
      .child(self.terminal.clone())
  }
}

/// A plain placeholder panel (explorer / editor placeholders).
struct PlaceholderPanel {
  id: SharedString,
  title: SharedString,
  focus_handle: FocusHandle,
}

impl PlaceholderPanel {
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

impl Panel for PlaceholderPanel {
  fn panel_name(&self) -> &'static str {
    "PlaceholderPanel"
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
    IconName::Code
  }
}

impl gpui::EventEmitter<PanelEvent> for PlaceholderPanel {}

impl Focusable for PlaceholderPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for PlaceholderPanel {
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
          .child("Placeholder content — terminals live in the bottom dock."),
      )
  }
}

/// The workbench root view.
struct TerminalDockDemo {
  dock_area: Entity<DockArea>,
  app_menu_bar: Entity<AppMenuBar>,
  bottom_dock_terminals: Vec<Entity<TerminalPanel>>,
  next_terminal_index: usize,
}

impl TerminalDockDemo {
  fn add_terminal_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
    let index = self.next_terminal_index;
    self.next_terminal_index += 1;

    let panel = cx.new(|cx| TerminalPanel::new(format!("Terminal {index}"), window, cx));

    // Keep panel entities alive for the whole demo; the dock holds its own
    // strong reference while the tab exists.
    self.bottom_dock_terminals.push(panel.clone());

    self.dock_area.update(cx, |dock, cx| {
      dock.add_to_bottom_dock(Arc::new(panel.clone()), window, cx);
      dock.set_dock_collapsed(DockPlacement::Bottom, false, window, cx);
    });
  }

  /// External control demo: type a command into a terminal programmatically.
  fn send_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
    if self.bottom_dock_terminals.is_empty() {
      self.add_terminal_panel(window, cx);
    }

    let panel = self.bottom_dock_terminals[0].clone();
    panel.update(cx, |panel, cx| {
      // The PTY echoes the command, so the user sees what was typed.
      panel.terminal.read(cx).session().input_str(command);
    });
  }

  fn on_new_terminal(&mut self, _: &NewTerminal, window: &mut Window, cx: &mut Context<Self>) {
    self.add_terminal_panel(window, cx);
  }

  fn on_run_build(&mut self, _: &RunBuildInTerminal, window: &mut Window, cx: &mut Context<Self>) {
    self.send_command("echo [external] running build in terminal...\r", window, cx);
  }

  fn on_clear_terminal(
    &mut self, _: &ClearActiveTerminal, _window: &mut Window, cx: &mut Context<Self>,
  ) {
    if let Some(panel) = self.bottom_dock_terminals.first() {
      let panel = panel.clone();
      panel.update(cx, |panel, cx| {
        panel.terminal.read(cx).session().clear();
      });
    }
  }

  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    cx.new(|cx| {
      let dock_area = cx.new(|cx| DockArea::new("terminal-dock-example", Some(1), window, cx));
      let app_menu_bar = AppMenuBar::new(cx);

      let explorer = cx.new(|cx| PlaceholderPanel::new("explorer:/workspace", "Explorer", cx));
      let editor = cx.new(|cx| PlaceholderPanel::new("editor:/workspace/main.rs", "Editor", cx));

      dock_area.update(cx, |dock, cx| {
        dock.add_to_left_dock(Arc::new(explorer.clone()), window, cx);
        dock.add_to_center(Arc::new(editor.clone()), window, cx);
        dock.set_dock_size(DockPlacement::Left, px(240.), window, cx);
        dock.set_dock_size(DockPlacement::Bottom, px(260.), window, cx);
      });

      let mut demo = Self {
        dock_area,
        app_menu_bar,
        bottom_dock_terminals: Vec::new(),
        next_terminal_index: 0,
      };

      // Seed the bottom dock with one terminal panel.
      demo.add_terminal_panel(window, cx);

      demo
    })
  }
}

impl Render for TerminalDockDemo {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .on_action(cx.listener(Self::on_new_terminal))
        .on_action(cx.listener(Self::on_run_build))
        .on_action(cx.listener(Self::on_clear_terminal))
        .child(
          TitleBar::new()
            .title("Woocraft Terminal Dock Example")
            .app_menu_bar(self.app_menu_bar.clone()),
        )
        .child(self.dock_area.clone()),
    )
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      init(cx);
      cx.activate(true);

      let bounds = gpui::Bounds::centered(None, gpui::Size::new(px(1180.), px(760.)), cx);
      cx.open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          titlebar: Some(TitleBar::title_bar_options()),
          #[cfg(target_os = "linux")]
          window_background: gpui::WindowBackgroundAppearance::Transparent,
          #[cfg(target_os = "linux")]
          window_decorations: Some(gpui::WindowDecorations::Client),
          ..Default::default()
        },
        TerminalDockDemo::view,
      )
      .expect("open terminal dock example window failed");
    });
}
