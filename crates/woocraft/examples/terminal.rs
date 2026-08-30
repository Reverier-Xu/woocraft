//! GPUI terminal demo — embeds a [`woocraft::TerminalView`] running the
//! platform default shell.
//!
//! Demonstrates:
//! - spawning a PTY-backed terminal session and hosting the view,
//! - reacting to view events (`TitleChanged`, `Bell`, `ClipboardStored`,
//!   `Exit`) to update the window chrome,
//! - the Woocraft theme colors driving the terminal palette.
//!
//! Run with: `cargo run -p woocraft --example terminal`

use std::time::Duration;

use gpui::{
  App, AppContext, Bounds, ClipboardItem, Context, Entity, FocusHandle, Focusable,
  InteractiveElement as _, IntoElement, KeyBinding, ParentElement as _, Render, SharedString,
  Size as GpuiSize, Styled as _, Window, WindowBounds, WindowOptions, actions, div,
  prelude::FluentBuilder as _, px,
};
use woocraft::{
  ActiveTheme, Assets, StyledExt as _, TerminalView, TerminalViewEvent, TitleBar, h_flex, init,
  v_flex,
};
use woocraft_terminal::{ChildStatus, SpawnOptions, TerminalBounds, TerminalSession};

actions!(terminal_example, [Quit]);

/// How long a transient header indicator stays visible.
const FLASH_DURATION: Duration = Duration::from_millis(800);

/// The demo shell window: a header, the terminal, and an exit banner.
struct TerminalDemo {
  terminal: Entity<TerminalView>,
  exit_status: Option<ChildStatus>,
  /// The last application-provided title (OSC 0/2), if any.
  title: Option<String>,
  /// The window title already applied, to avoid redundant updates.
  applied_window_title: Option<String>,
  /// Transient indicator text (bell / OSC 52 feedback).
  flash: Option<SharedString>,
}

impl Focusable for TerminalDemo {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.terminal.focus_handle(_cx)
  }
}

impl TerminalDemo {
  /// The view builder passed to `open_window`.
  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    cx.new(|cx: &mut Context<Self>| {
      let session = TerminalSession::spawn(
        SpawnOptions::default_shell_options(),
        TerminalBounds::default(),
      )
      .expect("failed to spawn terminal session");
      let terminal = cx.new(|cx| TerminalView::new(session, window, cx));
      cx.subscribe(&terminal, Self::on_terminal_event).detach();
      terminal.focus_handle(cx).focus(window, cx);

      Self {
        terminal,
        exit_status: None,
        title: None,
        applied_window_title: None,
        flash: None,
      }
    })
  }

  fn on_terminal_event(
    &mut self, _: Entity<TerminalView>, event: &TerminalViewEvent, cx: &mut Context<Self>,
  ) {
    match event {
      TerminalViewEvent::TitleChanged(title) => {
        self.title = title.clone();
      }
      TerminalViewEvent::Bell => self.flash_for("Bell", cx),
      TerminalViewEvent::ClipboardStored(text) => {
        // OSC 52: the terminal application wants to set the clipboard.
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        self.flash_for("Copied via OSC 52", cx);
      }
      TerminalViewEvent::Exit(_) => {
        self.exit_status = self.terminal.read(cx).session().child_exit_status();
      }
    }
    cx.notify();
  }

  /// Shows a transient header indicator, clearing it shortly after.
  fn flash_for(&mut self, text: &'static str, cx: &mut Context<Self>) {
    self.flash = Some(SharedString::from(text));
    cx.spawn(async move |this, cx| {
      cx.background_executor().timer(FLASH_DURATION).await;
      let Ok(()) = this.update(cx, |demo, cx| {
        demo.flash = None;
        cx.notify();
      }) else {
        return;
      };
    })
    .detach();
  }

  fn header_label(&self) -> String {
    match &self.title {
      Some(title) if !title.is_empty() => title.clone(),
      _ => "Woocraft Terminal".to_string(),
    }
  }
}

impl Render for TerminalDemo {
  fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();

    // Keep the OS window title in sync with the terminal title.
    let window_title = self.header_label();
    if self.applied_window_title.as_deref() != Some(window_title.as_str()) {
      window.set_window_title(&window_title);
      self.applied_window_title = Some(window_title.clone());
    }

    let flash = self.flash.clone();
    let exit_status = self.exit_status;

    v_flex()
      .size_full()
      .bg(theme.background)
      .text_color(theme.foreground)
      // Header: demo label + live terminal title + transient indicators.
      .child(
        h_flex()
          .flex_none()
          .items_center()
          .justify_between()
          .px_4()
          .py_2()
          .border_b_1()
          .border_color(theme.border)
          .child(div().text_sm().font_semibold().child("Terminal Example"))
          .child(
            h_flex()
              .items_center()
              .gap_3()
              .when_some(flash, |this, flash| {
                this.child(
                  div()
                    .text_xs()
                    .px_2()
                    .py_0p5()
                    .rounded(theme.radius)
                    .bg(theme.muted)
                    .text_color(theme.muted_foreground)
                    .child(flash),
                )
              })
              .child(
                div()
                  .text_xs()
                  .text_color(theme.muted_foreground)
                  .child(window_title),
              ),
          ),
      )
      // The terminal itself; the element measures the font and resizes the
      // PTY to the available grid size.
      .child(
        div()
          .id("terminal-slot")
          .flex_1()
          .min_h_0()
          .child(self.terminal.clone()),
      )
      // Exit banner shown once the child process is gone.
      .when_some(exit_status, |this, status| {
        let signal_suffix = {
          #[cfg(unix)]
          {
            status
              .signal
              .map(|signal| format!(" (terminated by signal {signal})"))
              .unwrap_or_default()
          }
          #[cfg(not(unix))]
          {
            let _ = &status;
            String::new()
          }
        };
        let message = format!(
          "Shell exited with code {}{signal_suffix} — press Ctrl+Q to close.",
          status.code()
        );
        this.child(
          h_flex()
            .flex_none()
            .items_center()
            .justify_center()
            .px_4()
            .py_2()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.warning.opacity(0.15))
            .child(div().text_sm().text_color(theme.warning).child(message)),
        )
      })
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(Assets)
    .run(|cx: &mut App| {
      init(cx);
      cx.activate(true);
      // Ctrl+Q quits the demo. The terminal view only consumes keystrokes
      // while the shell is alive, so this fires once the shell has exited.
      cx.bind_keys([KeyBinding::new("ctrl-q", Quit, None)]);
      cx.on_action(|_: &Quit, cx: &mut App| cx.quit());

      let bounds = Bounds::centered(None, GpuiSize::new(px(1000.), px(640.)), cx);
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
          TerminalDemo::view,
        )
        .expect("open terminal example window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Terminal Example");
        })
        .expect("update terminal example window failed");
    });
}
