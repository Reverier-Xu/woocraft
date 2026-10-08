use std::{
  sync::{Arc, Mutex},
  time::Duration,
};

use gpui::{
  App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable, IntoElement, ParentElement,
  Render, SharedString, Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, CodeEditor, DockArea, DockPlacement, EditorActionSink,
  EditorBackend, EditorBackendCapabilities, EditorContextMenuProvider, EditorHighlighterProvider,
  EditorSnapshot, EditorState, IconName, Panel, PanelEvent, Rope, RopeEditorSnapshot, RopeExt,
  Sizable, Theme, ThemeMode, TitleBar, h_flex, v_flex, window_border,
};

const RUST_SAMPLE: &str = r#"use std::collections::HashMap;

fn collect_scores(names: &[&str]) -> HashMap<String, usize> {
  let mut scores = HashMap::new();
  for (ix, name) in names.iter().enumerate() {
    scores.insert((*name).to_string(), ix * 10 + 42);
  }
  scores
}

fn main() {
  let users = ["alice", "bob", "charlie"];
  let scores = collect_scores(&users);

  for (name, score) in scores {
    println!("{name}: {score}");
  }
}
"#;

const TYPESCRIPT_SAMPLE: &str = r#"type User = {
  id: string;
  name: string;
  admin?: boolean;
};

export function makeGreeting(user: User): string {
  const role = user.admin ? "admin" : "member";
  return `[${role}] hello, ${user.name}`;
}

const users: User[] = [
  { id: "1", name: "Lin" },
  { id: "2", name: "River", admin: true },
];

console.log(users.map(makeGreeting).join("\n"));
"#;

const PYTHON_SAMPLE: &str = r#"from dataclasses import dataclass
from typing import Iterable

@dataclass
class Item:
    name: str
    price: float

def total(items: Iterable[Item]) -> float:
    return sum(item.price for item in items)

basket = [Item("apple", 2.5), Item("orange", 3.2)]
print(f"total={total(basket):.2f}")
"#;

const TOML_SAMPLE: &str = r#"[package]
name = "woocraft-example"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }

[profile.release]
lto = "thin"
opt-level = 3
"#;

const JSON_SAMPLE: &str = r#"{
  "name": "editor-demo",
  "active": true,
  "tags": ["dock", "editor", "highlight"],
  "threshold": 0.875,
  "nested": {
    "count": 3,
    "nullable": null
  }
}
"#;

const MARKDOWN_SAMPLE: &str = r#"# Woocraft Editor

This panel uses `markdown` highlighting with code fences:

```rust
pub fn clamp(v: i32, min: i32, max: i32) -> i32 {
  v.max(min).min(max)
}
```

```toml
[workspace]
members = ["crates/*"]
```
"#;

const YAML_SAMPLE: &str = r#"version: "3.9"
services:
  web:
    image: nginx:alpine
    ports:
      - "8080:80"
    environment:
      APP_ENV: development
      LOG_LEVEL: debug
"#;

const BASH_SAMPLE: &str = r#"#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "root=${ROOT_DIR}"

for file in src/*.rs; do
  echo "checking ${file}"
done
"#;

// ---------------------------------------------------------------------------
// Live log panel – a read-only editor streaming log batches through a custom
// backend, with follow-tail scrolling.
// ---------------------------------------------------------------------------

/// A read-only editor backend whose content lives in shared state, so a
/// producer outside the editor can append batches between frames.
#[derive(Clone)]
struct LiveLogBackend {
  state: Arc<Mutex<LiveLogState>>,
}

struct LiveLogState {
  text: Rope,
  revision: u64,
  seq: u64,
}

impl LiveLogBackend {
  fn new() -> Self {
    Self {
      state: Arc::new(Mutex::new(LiveLogState {
        text: Rope::from("woocraft log viewer ready\n"),
        revision: 1,
        seq: 0,
      })),
    }
  }

  /// Append a batch of realistic-looking log lines.
  fn push(&self, lines: usize) {
    const LEVELS: [&str; 5] = ["INFO", "WARN", "DEBUG", "ERROR", "TRACE"];
    let mut state = self.state.lock().unwrap();
    let mut batch = String::new();
    for _ in 0..lines {
      state.seq += 1;
      let level = LEVELS[(state.seq % LEVELS.len() as u64) as usize];
      let line = format!(
        "2026-02-08 12:34:{:02}.{:03} [{level:>5}] worker-{:02} — processed request #{:06} successfully in {} ms\n",
        (state.seq % 60) as u8,
        (state.seq % 1000) as u16,
        (state.seq % 8) + 1,
        state.seq * 37,
        3 + (state.seq % 40),
      );
      batch.push_str(&line);
      if state.seq.is_multiple_of(17) {
        batch.push_str(&format!(
          "2026-02-08 12:34:{:02}.{:03} [{level:>5}] worker-{:02} — detail: payload {{\"id\": {:06}, \"path\": \"/api/v1/items?query=aVeryLongFilterStringToForceHorizontalScrolling\", \"ok\": true}}\n",
          (state.seq % 60) as u8,
          (state.seq % 1000) as u16,
          (state.seq % 8) + 1,
          state.seq * 37,
        ));
      }
    }
    let mut text = std::mem::take(&mut state.text);
    text.replace(text.len()..text.len(), &batch);
    state.text = text;
    state.revision += 1;
  }

  fn line_count(&self) -> usize {
    self.state.lock().unwrap().text.lines_len()
  }
}

impl EditorActionSink for LiveLogBackend {}
impl EditorContextMenuProvider for LiveLogBackend {}
impl EditorHighlighterProvider for LiveLogBackend {}

impl EditorBackend for LiveLogBackend {
  fn revision(&self) -> u64 {
    self.state.lock().unwrap().revision
  }

  fn capabilities(&self) -> EditorBackendCapabilities {
    EditorBackendCapabilities::read_only()
  }

  fn snapshot(&self) -> Arc<dyn EditorSnapshot> {
    let state = self.state.lock().unwrap();
    Arc::new(RopeEditorSnapshot::new(state.revision, state.text.clone()))
  }
}

struct LiveLogPanel {
  backend: LiveLogBackend,
  editor_state: Entity<EditorState>,
  streaming: bool,
  focus_handle: FocusHandle,
}

impl LiveLogPanel {
  fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
    let backend = LiveLogBackend::new();
    backend.push(40);

    let editor_state = cx.new(|cx| {
      EditorState::new(window, cx)
        .code_editor("text")
        .read_only(true)
        .follow_output(true)
        .line_number(false)
        .backend(backend.clone())
    });

    // Stream a small batch every few hundred milliseconds. The editor picks
    // the changes up on its next render (revision bump), re-wraps only the
    // appended lines and keeps the viewport pinned to the tail while the
    // user has not scrolled away.
    let stream = backend.clone();
    cx.spawn_in(window, async move |this, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(350))
          .await;
        if this
          .update(cx, |panel, cx| {
            if panel.streaming {
              stream.push(2 + (panel.backend.line_count() % 5));
              cx.notify();
            }
          })
          .is_err()
        {
          // Panel dropped (dock closed) — stop streaming.
          break;
        }
      }
    })
    .detach();

    Self {
      backend,
      editor_state,
      streaming: true,
      focus_handle: cx.focus_handle(),
    }
  }
}

impl Panel for LiveLogPanel {
  fn panel_name(&self) -> &'static str {
    "LiveLogPanel"
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some("Live Log".into())
  }

  fn title(&self, _cx: &App) -> SharedString {
    "Live Log".into()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::DocumentText
  }

  fn inner_padding(&self, _cx: &App) -> bool {
    false
  }
}

impl gpui::EventEmitter<PanelEvent> for LiveLogPanel {}

impl Focusable for LiveLogPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for LiveLogPanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let editor = self.editor_state.clone();
    let streaming = self.streaming;
    let line_count = self.backend.line_count();

    v_flex()
      .size_full()
      .min_h_0()
      .bg(cx.theme().background)
      .child(
        h_flex()
          .h(px(32.))
          .px_3()
          .gap_2()
          .items_center()
          .justify_between()
          .border_b_1()
          .border_color(cx.theme().border)
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child(format!("Streaming log · {line_count} lines · read-only"))
          .child(
            h_flex()
              .gap_2()
              .child(
                Button::new("log-stream")
                  .label(if streaming { "Pause" } else { "Resume" })
                  .small()
                  .flat()
                  .on_click(cx.listener(|this, _, _, cx| {
                    this.streaming = !this.streaming;
                    cx.notify();
                  })),
              )
              .child(
                Button::new("log-tail")
                  .label("Jump to tail")
                  .small()
                  .flat()
                  .on_click(move |_, window, cx| {
                    editor.update(cx, |state, cx| {
                      state.scroll_to_bottom(window, cx);
                    });
                  }),
              ),
          ),
      )
      .child(
        div().flex_1().min_h_0().child(
          CodeEditor::new(&self.editor_state)
            .h_full()
            .w_full()
            .appearance(false)
            .bordered(false)
            .focus_bordered(false),
        ),
      )
  }
}

struct EditorPanel {
  title: SharedString,
  language: SharedString,
  editor_state: Entity<EditorState>,
  focus_handle: FocusHandle,
}

impl EditorPanel {
  fn new(
    title: impl Into<SharedString>, language: impl Into<SharedString>,
    editor_state: Entity<EditorState>, cx: &mut Context<Self>,
  ) -> Self {
    Self {
      title: title.into(),
      language: language.into(),
      editor_state,
      focus_handle: cx.focus_handle(),
    }
  }
}

impl Panel for EditorPanel {
  fn panel_name(&self) -> &'static str {
    "EditorPanel"
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

  fn inner_padding(&self, _cx: &App) -> bool {
    false
  }
}

impl gpui::EventEmitter<PanelEvent> for EditorPanel {}

impl Focusable for EditorPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for EditorPanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .min_h_0()
      .bg(cx.theme().background)
      .child(
        h_flex()
          .h(px(32.))
          .px_3()
          .items_center()
          .justify_between()
          .border_b_1()
          .border_color(cx.theme().border)
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child(format!("Language: {}", self.language))
          .child("Ctrl/Cmd+F Search"),
      )
      .child(
        div().flex_1().min_h_0().child(
          CodeEditor::new(&self.editor_state)
            .h_full()
            .w_full()
            .appearance(false)
            .bordered(false)
            .focus_bordered(false),
        ),
      )
  }
}

struct EditorDockExample {
  dock_area: Entity<DockArea>,
}

impl EditorDockExample {
  fn editor_panel(
    title: impl Into<SharedString>, language: impl Into<SharedString>, source: &'static str,
    show_whitespaces: bool, window: &mut Window, cx: &mut App,
  ) -> Entity<EditorPanel> {
    let title: SharedString = title.into();
    let language: SharedString = language.into();
    let language_for_editor = language.clone();

    let editor_state = cx.new(move |cx| {
      EditorState::new(window, cx)
        .code_editor(language_for_editor.clone())
        .line_number(true)
        .show_whitespaces(show_whitespaces)
        .default_value(source)
    });

    cx.new(move |cx| EditorPanel::new(title.clone(), language.clone(), editor_state.clone(), cx))
  }

  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let dock_area = cx.new(|cx| DockArea::new("editor-dock-example", Some(1), window, cx));

    let rust = Self::editor_panel("main.rs", "rust", RUST_SAMPLE, true, window, cx);
    let ts = Self::editor_panel(
      "greet.ts",
      "typescript",
      TYPESCRIPT_SAMPLE,
      true,
      window,
      cx,
    );
    let python = Self::editor_panel("billing.py", "python", PYTHON_SAMPLE, true, window, cx);
    let toml = Self::editor_panel("Cargo.toml", "toml", TOML_SAMPLE, true, window, cx);
    let json = Self::editor_panel("settings.json", "json", JSON_SAMPLE, true, window, cx);
    let markdown = Self::editor_panel("README.md", "markdown", MARKDOWN_SAMPLE, false, window, cx);
    let yaml = Self::editor_panel("docker-compose.yml", "yaml", YAML_SAMPLE, true, window, cx);
    let bash = Self::editor_panel("bootstrap.sh", "bash", BASH_SAMPLE, true, window, cx);
    let live_log = cx.new(|cx| LiveLogPanel::new(window, cx));

    dock_area.update(cx, |dock, cx| {
      // Add panels to center area
      dock.add_to_center(Arc::new(rust.clone()), window, cx);
      dock.add_to_center(Arc::new(ts.clone()), window, cx);
      dock.add_to_center(Arc::new(markdown.clone()), window, cx);

      // Add panels to side docks
      dock.add_to_left_dock(Arc::new(python.clone()), window, cx);
      dock.add_to_left_dock(Arc::new(toml.clone()), window, cx);
      dock.add_to_bottom_dock(Arc::new(live_log.clone()), window, cx);
      dock.add_to_bottom_dock(Arc::new(json.clone()), window, cx);
      dock.add_to_bottom_dock(Arc::new(bash.clone()), window, cx);
      dock.add_to_right_dock(Arc::new(yaml.clone()), window, cx);

      // Configure dock sizes
      dock.set_dock_size(DockPlacement::Left, px(320.), window, cx);
      dock.set_dock_size(DockPlacement::Bottom, px(250.), window, cx);
      dock.set_dock_size(DockPlacement::Right, px(340.), window, cx);
    });

    cx.new(|_| Self { dock_area })
  }
}

impl Render for EditorDockExample {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().clone();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(
          TitleBar::new()
            .title("Woocraft Editor Example")
            .zoom_button(true),
        )
        .child(
          h_flex()
            .h(px(48.))
            .px_4()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(
              Button::new("dock-theme")
                .label("Toggle Theme")
                .on_click(|_, _, cx| {
                  let next = if cx.theme().mode.is_dark() {
                    ThemeMode::Light
                  } else {
                    ThemeMode::Dark
                  };
                  Theme::set_mode(next, cx);
                }),
            )
            .child(Button::new("toggle-left").label("Left").flat().on_click({
              let view = view.clone();
              move |_, window, cx| {
                view.update(cx, |this, cx| {
                  this.dock_area.update(cx, |dock, cx| {
                    dock.toggle_dock(DockPlacement::Left, window, cx);
                  });
                });
              }
            }))
            .child(
              Button::new("toggle-bottom")
                .label("Bottom")
                .flat()
                .on_click({
                  let view = view.clone();
                  move |_, window, cx| {
                    view.update(cx, |this, cx| {
                      this.dock_area.update(cx, |dock, cx| {
                        dock.toggle_dock(DockPlacement::Bottom, window, cx);
                      });
                    });
                  }
                }),
            )
            .child(Button::new("toggle-right").label("Right").flat().on_click(
              move |_, window, cx| {
                view.update(cx, |this, cx| {
                  this.dock_area.update(cx, |dock, cx| {
                    dock.toggle_dock(DockPlacement::Right, window, cx);
                  });
                });
              },
            )),
        )
        .child(self.dock_area.clone()),
    )
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      woocraft::init(cx);
      cx.activate(true);

      let bounds = Bounds::centered(None, GpuiSize::new(px(1400.), px(900.)), cx);
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
          EditorDockExample::view,
        )
        .expect("open editor example window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Editor Dock Example");
        })
        .expect("update editor example window failed");
    });
}
