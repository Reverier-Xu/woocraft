use std::sync::Arc;

use gpui::{
  App, AppContext, Application, Bounds, Context, Edges, Entity, FocusHandle, Focusable,
  IntoElement, ParentElement, Render, SharedString, Size as GpuiSize, Styled, Window, WindowBounds,
  WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, CodeEditor, DockArea, DockItem, DockPlacement,
  EditorState, IconName, Panel, PanelEvent, Theme, ThemeMode, TitleBar, h_flex, v_flex,
  window_border,
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
    soft_wrap: bool, show_whitespaces: bool, window: &mut Window, cx: &mut App,
  ) -> Entity<EditorPanel> {
    let title: SharedString = title.into();
    let language: SharedString = language.into();
    let language_for_editor = language.clone();

    let editor_state = cx.new(move |cx| {
      EditorState::new(window, cx)
        .code_editor(language_for_editor.clone())
        .line_number(true)
        .soft_wrap(soft_wrap)
        .show_whitespaces(show_whitespaces)
        .default_value(source)
    });

    cx.new(move |cx| EditorPanel::new(title.clone(), language.clone(), editor_state.clone(), cx))
  }

  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let dock_area = cx.new(|cx| DockArea::new("editor-dock-example", Some(1), window, cx));
    let weak = dock_area.downgrade();

    let rust = Self::editor_panel("main.rs", "rust", RUST_SAMPLE, false, true, window, cx);
    let ts = Self::editor_panel(
      "greet.ts",
      "typescript",
      TYPESCRIPT_SAMPLE,
      false,
      true,
      window,
      cx,
    );
    let python = Self::editor_panel(
      "billing.py",
      "python",
      PYTHON_SAMPLE,
      false,
      true,
      window,
      cx,
    );
    let toml = Self::editor_panel("Cargo.toml", "toml", TOML_SAMPLE, false, true, window, cx);
    let json = Self::editor_panel(
      "settings.json",
      "json",
      JSON_SAMPLE,
      false,
      true,
      window,
      cx,
    );
    let markdown = Self::editor_panel(
      "README.md",
      "markdown",
      MARKDOWN_SAMPLE,
      true,
      false,
      window,
      cx,
    );
    let yaml = Self::editor_panel(
      "docker-compose.yml",
      "yaml",
      YAML_SAMPLE,
      false,
      true,
      window,
      cx,
    );
    let bash = Self::editor_panel("bootstrap.sh", "bash", BASH_SAMPLE, false, true, window, cx);

    let center = DockItem::h_split(
      vec![
        DockItem::tab(rust, &weak, window, cx).size(px(620.)),
        DockItem::v_split(
          vec![
            DockItem::tab(ts, &weak, window, cx),
            DockItem::tab(markdown, &weak, window, cx).size(px(250.)),
          ],
          &weak,
          window,
          cx,
        )
        .size(px(420.)),
      ],
      &weak,
      window,
      cx,
    );

    let left = DockItem::tabs(
      vec![Arc::new(python.clone()), Arc::new(toml.clone())],
      &weak,
      window,
      cx,
    );

    let bottom = DockItem::tabs(
      vec![Arc::new(json.clone()), Arc::new(bash.clone())],
      &weak,
      window,
      cx,
    );

    let right = DockItem::tabs(vec![Arc::new(yaml.clone())], &weak, window, cx);

    dock_area.update(cx, |dock, cx| {
      dock.set_center(center, window, cx);
      dock.set_left_dock(left, Some(px(320.)), true, window, cx);
      dock.set_bottom_dock(bottom, Some(px(250.)), true, window, cx);
      dock.set_right_dock(right, Some(px(340.)), true, window, cx);
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
        .child(TitleBar::new().title("Woocraft Editor Example"))
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
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
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
        |window, cx| EditorDockExample::view(window, cx),
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
