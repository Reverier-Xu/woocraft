use std::{cell::RefCell, rc::Rc};

use gpui::{
  App, AppContext, Bounds, Context, Entity, IntoElement, ParentElement, Render, Size as GpuiSize,
  Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, Markdown, ParsedMarkdown, ScrollableElement as _, Selectable, Theme,
  ThemeMode, TitleBar, h_flex, init, v_flex, window_border,
};

const SAMPLE: &str = r##"# Woocraft Markdown

Drag to select text (copy with `cmd/ctrl-c` or the right-click menu).

A themed markdown document rendered with **native gpui elements**, no web view:
**bold**, *italic*, ~~strikethrough~~, `inline code`, a [link](https://design.woooo.tech/) and
Unicode text 中文，日本語，emoji 🎨.

## Inline code and links

Links are clickable: the example below records the last clicked URL.
Visit [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui), [Zed](https://zed.dev)
or a [relative link](./docs/markdown.md).

## Lists

- Bullet one
- Bullet two with `code`
  - Nested item
  - Deeply nested
    1. Ordered inside unordered
    2. Another step
- Bullet three

1. First
2. Second
3. Third

### Task lists

- [x] Render CommonMark
- [x] Syntax highlight fenced code
- [ ] Text selection and search
- [ ] Remote images

## Code blocks

```rust
use woocraft::{Markdown, ParsedMarkdown};

fn main() {
  let parsed = ParsedMarkdown::parse("# Hello");
  for block in parsed.blocks() {
    println!("{block:?}");
  }
}
```

```json
{
  "component": "markdown",
  "theme": "light",
  "features": ["tables", "tasklists", "footnotes"]
}
```

```text
Plain text fences get the same chrome without syntax colors.
```

## Tables

| Feature | Status | Notes |
| :-- | :--: | --: |
| Headings | Done | H1 to H6 |
| Tables | Done | Alignment honored |
| Footnotes | Done | Inline references |

## Block quotes

> A regular quote with **emphasis** and a [link](https://example.com).

> [!NOTE]
> GitHub flavored block quote kinds render with their own accent color.

> [!WARNING]
> Caution block: check the accent color against the theme.

---

## Footnotes

Footnote references stay in the text[^1] and definitions render indented.

[^1]: This is a footnote definition parsed from the document.

## Images

Asset-backed images render for real. The example embeds
`examples/assets/external/woocraft-mark.svg` in its asset source:

![Woocraft mark](external/woocraft-mark.svg)

Inline images stay in the text flow: ![mark](external/woocraft-mark.svg) like this.

Remote URLs are never fetched by the component, so they fall back to a
placeholder the application can replace through `image_handler`:

![Remote diagram](https://example.com/diagram.png)

## Raw HTML

Inline HTML tags are dropped (except line breaks).<br>This line follows a `<br>` hard break.

<div align="center">Block-level HTML is rendered as plain text.</div>
"##;

struct MarkdownWindow {
  parsed: Rc<ParsedMarkdown>,
  /// Link clicked in the document; shared with the click handler so the demo
  /// can display it without a view round trip through the parse result.
  last_link: Rc<RefCell<Option<String>>>,
}

impl MarkdownWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self {
      parsed: Rc::new(ParsedMarkdown::parse(SAMPLE)),
      last_link: Rc::new(RefCell::new(None)),
    })
  }
}

impl Render for MarkdownWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let is_dark = cx.theme().mode.is_dark();
    let last_link = self.last_link.clone();
    let last_clicked = self
      .last_link
      .borrow()
      .clone()
      .unwrap_or_else(|| "none yet".to_string());

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(
          TitleBar::new()
            .title("Woocraft Markdown Example")
            .zoom_button(true),
        )
        .child(
          h_flex()
            .w_full()
            .gap_3()
            .px_6()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
              Button::new("md-theme-light")
                .label("Light")
                .selected(!is_dark)
                .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
            )
            .child(
              Button::new("md-theme-dark")
                .label("Dark")
                .selected(is_dark)
                .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
            )
            .child(
              div()
                .text_sm()
                .text_color(cx.theme().muted_foreground)
                .child(format!("Last clicked link: {last_clicked}")),
            ),
        )
        .child(
          v_flex()
            .p_6()
            .gap_4()
            .overflow_y_scrollbar()
            .flex_1()
            .min_h_0()
            .child(
              Markdown::from_shared(self.parsed.clone())
                .on_link_click(move |url, _window, cx| {
                  *last_link.borrow_mut() = Some(url.to_string());
                  cx.refresh_windows();
                })
                // Applications can append their own items to the built-in
                // copy / copy-link / select-all menu.
                .context_menu(|menu, selected, _window, _cx| {
                  let label = match selected {
                    Some(text) if !text.is_empty() => {
                      format!("Selection: {} chars", text.chars().count())
                    }
                    _ => "No selection".to_string(),
                  };
                  menu.separator().item(woocraft::PopupMenuItem::new(label))
                }),
            ),
        ),
    )
  }
}

/// Serves the example's own SVG so the document can show a real image.
///
/// A markdown image URL is resolved through the application's asset source,
/// the same path icons and avatars use; this is the smallest source that can
/// demonstrate it without pulling in extra dependencies.
struct ExampleAssets;

impl gpui::AssetSource for ExampleAssets {
  fn load(&self, path: &str) -> gpui::Result<Option<std::borrow::Cow<'static, [u8]>>> {
    const MARK: &[u8] = include_bytes!("assets/external/woocraft-mark.svg");
    Ok((path == "external/woocraft-mark.svg").then_some(std::borrow::Cow::Borrowed(MARK)))
  }

  fn list(&self, _path: &str) -> gpui::Result<Vec<gpui::SharedString>> {
    Ok(vec![gpui::SharedString::from("external/woocraft-mark.svg")])
  }
}

fn main() {
  let assets = woocraft::CombinedSource::new()
    .with(woocraft::Assets)
    .with(ExampleAssets);

  gpui_platform::application()
    .with_assets(assets)
    .run(|cx: &mut App| {
      init(cx);
      cx.activate(true);

      let bounds = Bounds::centered(None, GpuiSize::new(px(900.), px(760.)), cx);
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
          |_window, cx| MarkdownWindow::view(cx),
        )
        .expect("open markdown demo window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Markdown Example");
        })
        .expect("update markdown demo window failed");
    });
}
