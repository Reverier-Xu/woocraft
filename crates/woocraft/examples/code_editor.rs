use std::{collections::HashMap, sync::Arc};

use gpuim::{
  App, AppContext, Bounds, Context, Entity, FocusHandle, Focusable,
  InteractiveElement as _, IntoElement, KeyBinding, Menu, MenuItem, ParentElement, Render,
  SharedString, Size as GpuiSize, Styled, Subscription, Window, WindowBounds, WindowOptions,
  actions, div, px,
};
use woocraft::{
  ActiveTheme, AppMenuBar, CodeEditor, DockArea, DockPlacement, EditorEvent, EditorState, IconName,
  Panel, PanelEvent, PopupMenuItem, Size, StyleSized, TitleBar, TreeEvent, TreeItem, TreeState,
  h_flex, init, tree, v_flex, window_border,
};
#[cfg(debug_assertions)]
use woocraft::{ScrollableElement, StyledExt};

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

actions!(
  code_editor_actions,
  [
    NewFile,
    OpenFolder,
    Save,
    SaveAs,
    CloseFile,
    Quit,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    SelectAll,
    Find,
    ToggleLeftDock,
    ToggleBottomDock,
    ToggleRightDock,
    ToggleInspector,
    WordWrap,
    ShowAbout,
  ]
);

// ---------------------------------------------------------------------------
// Virtual file system – simple in-memory files for demo
// ---------------------------------------------------------------------------

fn demo_files() -> HashMap<SharedString, (&'static str, &'static str)> {
  // id -> (language, content)
  let mut m = HashMap::new();
  m.insert(
    SharedString::from("src/main.rs"),
    (
      "rust",
      r#"use std::collections::HashMap;

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
"#,
    ),
  );
  m.insert(
    SharedString::from("src/lib.rs"),
    (
      "rust",
      r#"pub mod utils;

/// A simple greeting function.
pub fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        assert_eq!(greet("world"), "Hello, world!");
    }
}
"#,
    ),
  );
  m.insert(
    SharedString::from("src/utils.rs"),
    (
      "rust",
      r#"/// Clamp a value between min and max.
pub fn clamp(v: i32, lo: i32, hi: i32) -> i32 {
    v.max(lo).min(hi)
}
"#,
    ),
  );
  m.insert(
    SharedString::from("Cargo.toml"),
    (
      "toml",
      r#"[package]
name = "my-project"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1.0"
serde = { version = "1.0", features = ["derive"] }

[profile.release]
lto = "thin"
opt-level = 3
"#,
    ),
  );
  m.insert(
    SharedString::from("README.md"),
    (
      "markdown",
      r#"# My Project

A demo project for the Woocraft code editor example.

## Getting Started

```bash
cargo run
```

## License

MIT
"#,
    ),
  );
  m.insert(
    SharedString::from("config/settings.json"),
    (
      "json",
      r#"{
  "editor.fontSize": 14,
  "editor.tabSize": 4,
  "editor.wordWrap": true,
  "theme": "dark",
  "files.exclude": ["target", ".git"]
}
"#,
    ),
  );
  m.insert(
    SharedString::from("scripts/build.sh"),
    (
      "bash",
      r#"#!/usr/bin/env bash
set -euo pipefail

echo "Building project..."
cargo build --release
echo "Done!"
"#,
    ),
  );
  m
}

fn demo_tree_items() -> Vec<TreeItem> {
  vec![
    TreeItem::new("src", "src")
      .icon(IconName::Folder)
      .expanded(true)
      .child(TreeItem::new("src/main.rs", "main.rs").icon(IconName::Code))
      .child(TreeItem::new("src/lib.rs", "lib.rs").icon(IconName::Code))
      .child(TreeItem::new("src/utils.rs", "utils.rs").icon(IconName::Code)),
    TreeItem::new("config", "config")
      .icon(IconName::Folder)
      .expanded(true)
      .child(TreeItem::new("config/settings.json", "settings.json").icon(IconName::Code)),
    TreeItem::new("scripts", "scripts")
      .icon(IconName::Folder)
      .expanded(true)
      .child(TreeItem::new("scripts/build.sh", "build.sh").icon(IconName::Code)),
    TreeItem::new("Cargo.toml", "Cargo.toml").icon(IconName::DocumentText),
    TreeItem::new("README.md", "README.md").icon(IconName::DocumentText),
  ]
}

#[cfg(debug_assertions)]
fn setup_inspector_renderer(cx: &mut App) {
  cx.register_inspector_element::<gpuim::DivInspectorState, _>(|_id, state, _window, cx| {
    v_flex()
      .gap_1()
      .p_2()
      .border_1()
      .border_color(cx.theme().border)
      .child(div().text_xs().font_semibold().child("Div"))
      .child(
        div()
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child(format!("bounds: {:?}", state.bounds)),
      )
      .child(
        div()
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child(format!("content_size: {:?}", state.content_size)),
      )
  });

  cx.set_inspector_renderer(Box::new(|inspector, window, cx| {
    let active_element = inspector
      .active_element_id()
      .map(|id| format!("{id:?}"))
      .unwrap_or_else(|| "未选中元素".to_string());
    let mode = if inspector.is_picking() {
      "拾取模式"
    } else {
      "已锁定"
    };
    let inspector_states = inspector.render_inspector_states(window, cx);

    let state_view = if inspector_states.is_empty() {
      div()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child("将鼠标移动到任意元素上即可查看 inspector 状态。")
        .into_any_element()
    } else {
      v_flex()
        .gap_2()
        .children(inspector_states)
        .into_any_element()
    };

    v_flex()
      .size_full()
      .min_w_0()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .border_l_1()
      .border_color(cx.theme().border)
      .p_2()
      .gap_2()
      .child(div().text_sm().font_semibold().child("GPUI Inspector"))
      .child(
        div()
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child(format!("模式：{}", mode)),
      )
      .child(
        div()
          .text_xs()
          .text_color(cx.theme().muted_foreground)
          .child("当前元素："),
      )
      .child(div().min_w_0().truncate().text_xs().child(active_element))
      .child(
        div()
          .flex_1()
          .min_h_0()
          .min_w_0()
          .overflow_y_scrollbar()
          .child(state_view),
      )
      .into_any_element()
  }));
}

// ---------------------------------------------------------------------------
// FileExplorerPanel – non-closable tree panel on the left dock
// ---------------------------------------------------------------------------

struct FileExplorerPanel {
  tree_state: Entity<TreeState>,
  focus_handle: FocusHandle,
  _subscriptions: Vec<Subscription>,
}

impl FileExplorerPanel {
  fn new(
    tree_state: Entity<TreeState>, app_state: Entity<CodeEditorApp>, _window: &mut Window,
    cx: &mut Context<Self>,
  ) -> Self {
    let subscriptions =
      vec![cx.subscribe(
        &tree_state,
        move |_this, _, event: &TreeEvent, cx| match event {
          TreeEvent::DoubleClicked(ix) => {
            app_state.update(cx, |state, cx| {
              state.open_file_by_tree_index(*ix, cx);
            });
          }
          TreeEvent::Select(ix) => {
            app_state.update(cx, |state, cx| {
              state.select_tree_item(*ix, cx);
            });
          }
          _ => {}
        },
      )];

    Self {
      tree_state,
      focus_handle: cx.focus_handle(),
      _subscriptions: subscriptions,
    }
  }
}

impl Panel for FileExplorerPanel {
  fn panel_name(&self) -> &'static str {
    "FileExplorerPanel"
  }

  fn panel_id(&self, _cx: &App) -> SharedString {
    "file-explorer".into()
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some("文件管理器".into())
  }

  fn title(&self, _cx: &App) -> SharedString {
    "文件管理器".into()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Folder
  }

  fn closable(&self, _cx: &App) -> bool {
    false
  }

  fn inner_padding(&self, _cx: &App) -> bool {
    false
  }
}

impl gpuim::EventEmitter<PanelEvent> for FileExplorerPanel {}

impl Focusable for FileExplorerPanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for FileExplorerPanel {
  fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
    let tree_state_for_menu = self.tree_state.clone();

    tree(&self.tree_state)
      .min_w_0()
      .overflow_hidden()
      .container_padding(Size::Medium)
      .context_menu(move |ix, entry, menu, _window, _cx| {
        let label = entry.item().label.clone();
        let is_folder = entry.is_folder();
        let item_id = entry.item().id.clone();
        let tree_entity = tree_state_for_menu.clone();

        if is_folder {
          menu
            .item(PopupMenuItem::label(format!("📁 {}", label)))
            .separator()
            .item(PopupMenuItem::new("新建文件").on_click(move |_, _, _| {
              println!("新建文件：在 {} 下", item_id);
            }))
            .item(PopupMenuItem::new("新建文件夹").on_click(move |_, _, _| {
              println!("新建文件夹：在 {} 下", label);
            }))
        } else {
          let item_id_open = item_id.clone();
          let item_id_rename = item_id.clone();
          let item_id_delete = item_id.clone();
          menu
            .item(PopupMenuItem::label(format!("📄 {}", label)))
            .separator()
            .item(PopupMenuItem::new("打开文件").on_click(move |_, _, _cx| {
              println!("打开文件：{}", item_id_open);
            }))
            .item(PopupMenuItem::new("选中").on_click(move |_, _, cx| {
              tree_entity.update(cx, |state, cx| {
                state.set_selected_index(Some(ix), cx);
              });
            }))
            .separator()
            .item(PopupMenuItem::new("重命名").on_click(move |_, _, _| {
              println!("重命名：{}", item_id_rename);
            }))
            .item(PopupMenuItem::new("删除").on_click(move |_, _, _| {
              println!("删除：{}", item_id_delete);
            }))
        }
      })
  }
}

// ---------------------------------------------------------------------------
// EditorPanel – a panel wrapping CodeEditor for a single file
// ---------------------------------------------------------------------------

struct EditorPanel {
  file_id: SharedString,
  title: SharedString,
  language: SharedString,
  editor_state: Entity<EditorState>,
  modified: bool,
  focus_handle: FocusHandle,
  app_state: Entity<CodeEditorApp>,
  _subscriptions: Vec<Subscription>,
}

impl EditorPanel {
  fn new(
    file_id: impl Into<SharedString>, title: impl Into<SharedString>,
    language: impl Into<SharedString>, editor_state: Entity<EditorState>,
    app_state: Entity<CodeEditorApp>, cx: &mut Context<Self>,
  ) -> Self {
    let file_id: SharedString = file_id.into();

    let subscriptions =
      vec![
        cx.subscribe(&editor_state, move |this, _, event: &EditorEvent, cx| {
          match event {
            EditorEvent::Change => {
              this.modified = true;
              cx.notify();
            }
            EditorEvent::Focus => {
              // Sync file explorer focus when editor gets focus
              this.app_state.update(cx, |state, cx| {
                state.sync_tree_to_file(&this.file_id, cx);
              });
            }
            _ => {}
          }
        }),
      ];

    Self {
      file_id,
      title: title.into(),
      language: language.into(),
      editor_state,
      modified: false,
      focus_handle: cx.focus_handle(),
      app_state,
      _subscriptions: subscriptions,
    }
  }

  fn save(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
    let content = self.editor_state.read(cx).value();
    // In a real app, write to disk here
    println!("💾 已保存文件：{} ({} bytes)", self.file_id, content.len());
    self.modified = false;
    cx.notify();
  }
}

impl Panel for EditorPanel {
  fn panel_name(&self) -> &'static str {
    "EditorPanel"
  }

  fn panel_id(&self, _cx: &App) -> SharedString {
    format!("editor:{}", self.file_id).into()
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    if self.modified {
      Some(format!("● {}", self.title).into())
    } else {
      Some(self.title.clone())
    }
  }

  fn title(&self, _cx: &App) -> SharedString {
    self.title.clone()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Code
  }

  fn inner_padding(&self, _cx: &App) -> bool {
    false
  }
}

impl gpuim::EventEmitter<PanelEvent> for EditorPanel {}

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
          .child(format!("{}  |  语言：{}", self.file_id, self.language))
          .child(if self.modified {
            "● 未保存"
          } else {
            "✓ 已保存"
          }),
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

// ---------------------------------------------------------------------------
// CodeEditorApp – main application state
// ---------------------------------------------------------------------------

struct CodeEditorApp {
  dock_area: Entity<DockArea>,
  app_menu_bar: Entity<AppMenuBar>,
  tree_state: Entity<TreeState>,
  /// Maps file_id -> (language, content) for the virtual FS
  files: HashMap<SharedString, (&'static str, &'static str)>,
  /// Currently opened editor panels, keyed by file_id
  open_editors: HashMap<SharedString, Entity<EditorPanel>>,
}

impl CodeEditorApp {
  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let dock_area = cx.new(|cx| DockArea::new("code-editor-dock", Some(1), window, cx));
    let app_menu_bar = AppMenuBar::new(cx);
    let files = demo_files();

    let tree_state = cx.new(|cx| TreeState::new(cx).items(demo_tree_items()));

    // Pre-create the entity so we can pass it to panels
    let app_entity: Entity<Self> = cx.new(|_| Self {
      dock_area: dock_area.clone(),
      app_menu_bar: app_menu_bar.clone(),
      tree_state: tree_state.clone(),
      files,
      open_editors: HashMap::new(),
    });

    // Create the file explorer panel
    let explorer = {
      let tree_state = tree_state.clone();
      let app_state = app_entity.clone();
      cx.new(|cx| FileExplorerPanel::new(tree_state, app_state, window, cx))
    };

    // Create a welcome editor for main.rs
    let main_rs_panel = Self::create_editor_panel(
      "src/main.rs",
      "main.rs",
      "rust",
      demo_files()
        .get(&SharedString::from("src/main.rs"))
        .unwrap()
        .1,
      app_entity.clone(),
      window,
      cx,
    );

    let lib_rs_panel = Self::create_editor_panel(
      "src/lib.rs",
      "lib.rs",
      "rust",
      demo_files()
        .get(&SharedString::from("src/lib.rs"))
        .unwrap()
        .1,
      app_entity.clone(),
      window,
      cx,
    );

    // Configure dock
    dock_area.update(cx, |dock, cx| {
      dock.add_to_left_dock(Arc::new(explorer.clone()), window, cx);
      dock.add_to_center(Arc::new(main_rs_panel.clone()), window, cx);
      dock.add_to_center(Arc::new(lib_rs_panel.clone()), window, cx);

      dock.set_dock_size(DockPlacement::Left, px(240.), window, cx);
    });

    // Register open editors
    app_entity.update(cx, |state, _cx| {
      state
        .open_editors
        .insert("src/main.rs".into(), main_rs_panel);
      state.open_editors.insert("src/lib.rs".into(), lib_rs_panel);
    });

    app_entity
  }

  fn create_editor_panel(
    file_id: &str, title: &str, language: &str, content: &'static str,
    app_state: Entity<CodeEditorApp>, window: &mut Window, cx: &mut App,
  ) -> Entity<EditorPanel> {
    let file_id = SharedString::from(file_id.to_string());
    let title = SharedString::from(title.to_string());
    let language = SharedString::from(language.to_string());
    let lang_for_editor = language.clone();

    let editor_state = cx.new(move |cx| {
      EditorState::new(window, cx)
        .code_editor(lang_for_editor.clone())
        .line_number(true)
        .show_whitespaces(true)
        .default_value(content)
    });

    let fid = file_id.clone();
    let t = title.clone();
    let l = language.clone();
    cx.new(move |cx| {
      EditorPanel::new(
        fid.clone(),
        t.clone(),
        l.clone(),
        editor_state,
        app_state.clone(),
        cx,
      )
    })
  }

  /// Open a file by tree index (double-click)
  fn open_file_by_tree_index(&mut self, ix: usize, cx: &mut App) {
    let entry = {
      let state = self.tree_state.read(cx);
      let entries = state.entries();
      if ix >= entries.len() {
        return;
      }
      let entry = &entries[ix];
      if entry.is_folder() {
        return; // Don't open folders
      }
      (entry.item().id.clone(), entry.item().label.clone())
    };

    let (file_id, _label) = entry;

    // If already open, activate it
    if self.open_editors.contains_key(&file_id) {
      self.dock_area.update(cx, |_dock, _cx| {
        println!(
          "\u{6fc0}\u{6d3b}\u{5df2}\u{6253}\u{5f00}\u{7684}\u{6587}\u{4ef6}: {}",
          file_id
        );
      });
      return;
    }

    // Look up content
    let (language, _content) = if let Some((lang, content)) = self.files.get(&file_id) {
      (*lang, *content)
    } else {
      ("text", "// 文件内容为空\n")
    };

    println!("📂 打开文件：{} ({})", file_id, language);
  }

  /// Select a tree item (single click - just track selection)
  fn select_tree_item(&mut self, _ix: usize, _cx: &mut App) {
    // Selection is already handled by TreeState, nothing extra needed
  }

  /// Sync tree selection to match the currently focused editor file
  fn sync_tree_to_file(&mut self, file_id: &SharedString, cx: &mut App) {
    let tree_state = self.tree_state.clone();
    let entries = tree_state.read(cx).entries().to_vec();
    for (ix, entry) in entries.iter().enumerate() {
      if &entry.item().id == file_id {
        tree_state.update(cx, |state, cx| {
          state.set_selected_index(Some(ix), cx);
        });
        return;
      }
    }
  }

  // --- Action handlers ---

  fn on_save(&mut self, _: &Save, window: &mut Window, cx: &mut Context<Self>) {
    // Save the currently focused editor, if any
    // We iterate open editors and save the modified ones
    for panel in self.open_editors.values() {
      panel.update(cx, |p, cx| {
        if p.modified {
          p.save(window, cx);
        }
      });
    }
  }

  fn on_toggle_left_dock(
    &mut self, _: &ToggleLeftDock, window: &mut Window, cx: &mut Context<Self>,
  ) {
    self.dock_area.update(cx, |dock, cx| {
      dock.toggle_dock(DockPlacement::Left, window, cx);
    });
  }

  fn on_new_file(&mut self, _: &NewFile, _window: &mut Window, _cx: &mut Context<Self>) {
    println!("📝 新建文件 (demo)");
  }

  fn on_toggle_inspector(
    &mut self, _: &ToggleInspector, window: &mut Window, cx: &mut Context<Self>,
  ) {
    #[cfg(debug_assertions)]
    {
      window.toggle_inspector(cx);
    }

    #[cfg(not(debug_assertions))]
    {
      let _ = window;
      let _ = cx;
      println!("Inspector is only available in debug builds.");
    }
  }

  fn on_quit(&mut self, _: &Quit, _window: &mut Window, cx: &mut Context<Self>) {
    cx.quit();
  }

  fn on_about(&mut self, _: &ShowAbout, _window: &mut Window, _cx: &mut Context<Self>) {
    println!("Woocraft Code Editor Example v0.1.0");
  }
}

impl Render for CodeEditorApp {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .on_action(cx.listener(Self::on_save))
        .on_action(cx.listener(Self::on_toggle_left_dock))
        .on_action(cx.listener(Self::on_toggle_inspector))
        .on_action(cx.listener(Self::on_new_file))
        .on_action(cx.listener(Self::on_quit))
        .on_action(cx.listener(Self::on_about))
        .child(
          TitleBar::new()
            .title("Woocraft Code Editor")
            .app_menu_bar(self.app_menu_bar.clone())
            .theme_button(true)
            .language_button(true),
        )
        .child(self.dock_area.clone()),
    )
  }
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

fn main() {
  let app = gpuim_platform::application().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    init(cx);
    cx.activate(true);
    #[cfg(debug_assertions)]
    setup_inspector_renderer(cx);

    cx.bind_keys([
      KeyBinding::new("cmd-shift-i", ToggleInspector, None),
      KeyBinding::new("ctrl-shift-i", ToggleInspector, None),
    ]);

    // Set up application menus (use i18n keys for AppMenuBar translation)
    cx.set_menus(vec![
      Menu {
        disabled: false,
        name: "tech.woooo.woocraft.menu.file".into(),
        items: vec![
          MenuItem::action("tech.woooo.woocraft.menu.new_file", NewFile),
          MenuItem::action("tech.woooo.woocraft.menu.open_folder", OpenFolder),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.save", Save),
          MenuItem::action("tech.woooo.woocraft.menu.save_as", SaveAs),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.close_file", CloseFile),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.quit", Quit),
        ],
      },
      Menu {
        disabled: false,
        name: "tech.woooo.woocraft.menu.edit".into(),
        items: vec![
          MenuItem::action("tech.woooo.woocraft.menu.undo", Undo),
          MenuItem::action("tech.woooo.woocraft.menu.redo", Redo),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.cut", Cut),
          MenuItem::action("tech.woooo.woocraft.menu.copy", Copy),
          MenuItem::action("tech.woooo.woocraft.menu.paste", Paste),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.select_all", SelectAll),
          MenuItem::action("tech.woooo.woocraft.menu.find", Find),
        ],
      },
      Menu {
        disabled: false,
        name: "tech.woooo.woocraft.menu.view".into(),
        items: vec![
          MenuItem::action("tech.woooo.woocraft.menu.toggle_sidebar", ToggleLeftDock),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.toggle_inspector", ToggleInspector),
          MenuItem::separator(),
          MenuItem::action("tech.woooo.woocraft.menu.word_wrap", WordWrap),
        ],
      },
      Menu {
        disabled: false,
        name: "tech.woooo.woocraft.menu.help".into(),
        items: vec![MenuItem::action(
          "tech.woooo.woocraft.menu.about",
          ShowAbout,
        )],
      },
    ]);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1400.), px(900.)), cx);
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
        CodeEditorApp::view,
      )
      .expect("open code editor window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Code Editor");
      })
      .expect("update code editor window failed");
  });
}
