//! theme playground — light/dark switching and the full oklch color system.
//!
//! renders every derived [`ThemeColors`] entry as a labeled swatch, the
//! semantic oklch hue chips, a full hue ruler, and a readability preview, so
//! adjustments to [`ThemeTokens`] can be judged in one screen.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example palette
//! ```

use gpui::{
  App, AppContext, Bounds, ClickEvent, Context, Hsla, InteractiveElement, IntoElement,
  ParentElement, Point, Render, SharedString, StatefulInteractiveElement, Styled, Window,
  WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{ActiveTheme, Assets, Theme, ThemeColors, ThemeMode, application, init, logging};

fn main() {
  let _ = logging::init();

  application().with_assets(Assets).run(|cx: &mut App| {
    if let Err(err) = init(cx) {
      eprintln!("woocraft init failed: {err}");
      return;
    }

    let bounds = Bounds {
      origin: Point {
        x: px(120.),
        y: px(120.),
      },
      size: size(px(1040.), px(880.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_window, cx| cx.new(|_| PaletteDemo))
        .expect("failed to open the palette window");
    })
    .detach();
  });
}

struct PaletteDemo;

impl Render for PaletteDemo {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();

    div()
      .id("palette-root")
      .size_full()
      .overflow_y_scroll()
      .font_family(theme.font_family.clone())
      .bg(theme.background)
      .text_color(theme.foreground)
      .flex()
      .flex_col()
      .gap_6()
      .p_6()
      .child(header(theme))
      .child(text_preview(theme))
      .child(swatches("surface", surface_entries(theme), theme))
      .child(swatches("actions", action_entries(theme), theme))
      .child(swatches("status", status_entries(theme), theme))
      .child(swatches("lines & overlays", line_entries(theme), theme))
      .child(swatches("tabs", tab_entries(theme), theme))
      .child(swatches("editor", editor_entries(theme), theme))
      .child(semantic_hues(theme))
      .child(hue_ruler(theme))
  }
}

/// mode switcher row: explicit light/dark buttons plus system sync.
fn header(theme: &Theme) -> impl IntoElement {
  let dark = theme.mode.is_dark();

  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_3()
    .child(div().text_lg().child("woocraft palette"))
    .child(
      div()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(
          format!("{:?}", theme.mode).to_lowercase(),
        )),
    )
    .child(div().flex_1())
    .child(mode_button(theme, ThemeMode::Light, !dark))
    .child(mode_button(theme, ThemeMode::Dark, dark))
    .child(
      div()
        .id("sync-system")
        .cursor_pointer()
        .rounded_md()
        .border_1()
        .border_color(theme.border)
        .px_3()
        .py_1()
        .text_sm()
        .text_color(theme.foreground)
        .on_click(|_: &ClickEvent, _, cx| Theme::sync_system_appearance(cx))
        .child("sync system"),
    )
}

fn mode_button(theme: &Theme, mode: ThemeMode, active: bool) -> impl IntoElement {
  let label = format!("{:?}", mode).to_lowercase();

  div()
    .id(SharedString::from(format!("mode-{label}")))
    .cursor_pointer()
    .rounded_md()
    .border_1()
    .border_color(if active { theme.primary } else { theme.border })
    .when(active, |this| {
      this.bg(theme.primary).text_color(theme.primary_foreground)
    })
    .px_3()
    .py_1()
    .text_sm()
    .on_click(move |_: &ClickEvent, _, cx| Theme::set_mode(mode, cx))
    .child(SharedString::from(label))
}

/// readability preview over card/popover surfaces with semantic accents.
fn text_preview(theme: &Theme) -> impl IntoElement {
  let colors = &theme.colors;

  div()
    .rounded_lg()
    .border_1()
    .border_color(colors.border)
    .bg(colors.card)
    .p_4()
    .flex()
    .flex_col()
    .gap_2()
    .child(
      div()
        .text_lg()
        .text_color(colors.card_foreground)
        .child("the quick brown fox jumps over the lazy dog"),
    )
    .child(
      div()
        .text_sm()
        .text_color(colors.muted_foreground)
        .child("muted foreground: 箱根の山々を望む 0123456789 —`\"'"),
    )
    .child(
      div()
        .text_sm()
        .child("semantic accents: ")
        .child(accent_word("primary", colors.primary))
        .child(accent_word(" success", colors.success))
        .child(accent_word(" warning", colors.warning))
        .child(accent_word(" danger", colors.danger))
        .child(accent_word(" info", colors.ring)),
    )
}

fn accent_word(word: &'static str, color: Hsla) -> impl IntoElement {
  div().text_color(color).child(word)
}

fn swatches(
  title: &'static str, entries: Vec<(&'static str, Hsla)>, theme: &Theme,
) -> impl IntoElement {
  let cells = entries
    .into_iter()
    .map(|(name, color)| swatch(name, color, theme))
    .collect::<Vec<_>>();

  section(
    title,
    theme,
    div().flex().flex_wrap().gap_2().children(cells),
  )
}

fn swatch(name: impl Into<SharedString>, color: Hsla, theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .w(rems(9.5))
    .gap_1()
    .child(
      div()
        .h(px(32.))
        .rounded_sm()
        .border_1()
        .border_color(theme.border)
        .bg(color),
    )
    .child(
      div()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(name.into()),
    )
}

fn section(title: &'static str, theme: &Theme, content: impl IntoElement) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(
      div()
        .text_xs()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(title.to_uppercase())),
    )
    .child(content)
}

/// the seven semantic oklch hues with their token values.
fn semantic_hues(theme: &Theme) -> impl IntoElement {
  let tokens = &theme.tokens;
  let hues = [
    ("primary", tokens.primary),
    ("info", tokens.info),
    ("success", tokens.success),
    ("warning", tokens.warning),
    ("error", tokens.error),
  ];

  let cells = hues
    .into_iter()
    .map(|(name, hue)| {
      let color = tokens.syntax_color(hue);
      div()
        .flex()
        .flex_col()
        .gap_1()
        .w(rems(9.5))
        .child(
          div()
            .h(px(32.))
            .rounded_sm()
            .border_1()
            .border_color(theme.border)
            .bg(color),
        )
        .child(
          div()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(format!("{name} · {hue}°"))),
        )
    })
    .collect::<Vec<_>>();

  let summary = SharedString::from(format!(
    "lightness {} · chroma {} · bg l {}/{} c {}/{}",
    tokens.lightness,
    tokens.chroma,
    tokens.light_bg_lightness,
    tokens.dark_bg_lightness,
    tokens.light_bg_chroma,
    tokens.dark_bg_chroma,
  ));

  section(
    "oklch semantic hues",
    theme,
    div()
      .flex()
      .flex_col()
      .gap_2()
      .child(div().flex().flex_wrap().gap_2().children(cells))
      .child(
        div()
          .text_xs()
          .text_color(theme.muted_foreground)
          .child(summary),
      ),
  )
}

/// the full hue circle at the shared lightness/chroma.
fn hue_ruler(theme: &Theme) -> impl IntoElement {
  let tokens = &theme.tokens;
  let steps = (0..360).step_by(15).map(|hue| {
    div()
      .flex_1()
      .h(px(28.))
      .bg(tokens.syntax_color(hue as f32))
      .child("")
  });

  section(
    "hue ruler · 15° steps at shared lightness/chroma",
    theme,
    div()
      .flex()
      .w_full()
      .rounded_sm()
      .overflow_hidden()
      .border_1()
      .border_color(theme.border)
      .children(steps),
  )
}

fn surface_entries(t: &ThemeColors) -> Vec<(&'static str, Hsla)> {
  vec![
    ("background", t.background),
    ("foreground", t.foreground),
    ("card", t.card),
    ("card_foreground", t.card_foreground),
    ("popover", t.popover),
    ("popover_foreground", t.popover_foreground),
    ("title_bar", t.title_bar),
    ("tiles", t.tiles),
  ]
}

fn action_entries(t: &ThemeColors) -> Vec<(&'static str, Hsla)> {
  vec![
    ("primary", t.primary),
    ("primary_foreground", t.primary_foreground),
    ("secondary", t.secondary),
    ("secondary_hover", t.secondary_hover),
    ("secondary_foreground", t.secondary_foreground),
    ("muted", t.muted),
    ("muted_foreground", t.muted_foreground),
    ("accent", t.accent),
    ("accent_foreground", t.accent_foreground),
    ("ring", t.ring),
  ]
}

fn status_entries(t: &ThemeColors) -> Vec<(&'static str, Hsla)> {
  vec![
    ("success", t.success),
    ("warning", t.warning),
    ("danger", t.danger),
    ("red", t.red),
    ("green", t.green),
    ("blue", t.blue),
    ("yellow", t.yellow),
    ("cyan", t.cyan),
  ]
}

fn line_entries(t: &ThemeColors) -> Vec<(&'static str, Hsla)> {
  vec![
    ("border", t.border),
    ("input", t.input),
    ("selection", t.selection),
    ("caret", t.caret),
    ("scrollbar", t.scrollbar),
    ("scrollbar_thumb", t.scrollbar_thumb),
    ("scrollbar_thumb_hover", t.scrollbar_thumb_hover),
    ("drag_border", t.drag_border),
    ("drop_target", t.drop_target),
  ]
}

fn tab_entries(t: &ThemeColors) -> Vec<(&'static str, Hsla)> {
  vec![
    ("tab_bar", t.tab_bar),
    ("tab_bar_segmented", t.tab_bar_segmented),
    ("tab_foreground", t.tab_foreground),
    ("tab_active", t.tab_active),
    ("tab_active_foreground", t.tab_active_foreground),
  ]
}

fn editor_entries(t: &ThemeColors) -> Vec<(&'static str, Hsla)> {
  vec![
    ("editor_background", t.editor_background),
    ("editor_foreground", t.editor_foreground),
    ("editor_active_line", t.editor_active_line),
    ("editor_line_number", t.editor_line_number),
    ("editor_active_line_number", t.editor_active_line_number),
    ("editor_invisible", t.editor_invisible),
  ]
}
