//! Theme palette inspector.
//!
//! Renders every color derived from the current `ThemeTokens` (light and
//! dark), the raw token values, and the syntax highlight hues. Use it to
//! eyeball palette tweaks before committing them to `ThemeTokens::default`.

use gpui::{
  App, AppContext, Bounds, Context, Entity, Hsla, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, black, div, px, white,
};
use palette::{FromColor, Oklch, Srgb};
use woocraft::{
  ActiveTheme, Button, ScrollableElement as _, Selectable, Sizable, StyledExt, SyntaxTokenHues,
  Theme, ThemeMode, TitleBar, h_flex, v_flex, window_border,
};

struct PaletteWindow;

impl PaletteWindow {
  fn view(_window: &mut Window, cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self)
  }
}

fn hex(color: Hsla) -> String {
  let rgb = color.to_rgb();
  let r = (rgb.r.clamp(0.0, 1.0) * 255.0).round() as u8;
  let g = (rgb.g.clamp(0.0, 1.0) * 255.0).round() as u8;
  let b = (rgb.b.clamp(0.0, 1.0) * 255.0).round() as u8;
  if (color.a - 1.0).abs() < 1e-3 {
    format!("#{r:02X}{g:02X}{b:02X}")
  } else {
    format!("#{r:02X}{g:02X}{b:02X} @ {:.0}%", color.a * 100.0)
  }
}

/// Picks black or white text for readability on `background`, using the same
/// OKLCH lightness scale the theme tokens are defined in.
fn readable_on(background: Hsla) -> Hsla {
  let rgb = background.to_rgb();
  let oklch = Oklch::from_color(Srgb::new(rgb.r, rgb.g, rgb.b));
  // OKLCH lightness is perceptually uniform; ~0.6 is where black text starts
  // winning against white on mid-tone backgrounds.
  if oklch.l > 0.6 { black() } else { white() }
}

/// One swatch card: filled block with the color name and hex value.
fn swatch(name: &'static str, color: Hsla, cx: &App) -> impl IntoElement {
  let label_color = readable_on(color.opacity(1.0));
  v_flex()
    .w(px(168.0))
    .gap_0()
    .rounded(cx.theme().radius)
    .border_1()
    .border_color(cx.theme().border)
    .overflow_hidden()
    .child(
      v_flex()
        .h(px(64.0))
        .w_full()
        .justify_end()
        .p_1()
        // Clip the fill to the card's rounded corners; `overflow_hidden`
        // alone lets the paint poke past the top corners.
        .rounded_t(cx.theme().radius)
        .bg(color)
        .child(
          div()
            .text_xs()
            .text_color(label_color)
            .child(name.to_string()),
        ),
    )
    .child(
      div()
        .px_2()
        .py_1()
        .text_xs()
        .text_color(cx.theme().muted_foreground)
        .child(hex(color)),
    )
}

fn section(title: &'static str, colors: Vec<(&'static str, Hsla)>, cx: &App) -> impl IntoElement {
  v_flex()
    .gap_2()
    .child(
      div()
        .text_sm()
        .font_semibold()
        .text_color(cx.theme().foreground)
        .child(title.to_string()),
    )
    .child(
      h_flex().flex_wrap().gap_2().children(
        colors
          .into_iter()
          .map(|(name, color)| swatch(name, color, cx)),
      ),
    )
}

fn syntax_sections(theme: &Theme) -> Vec<(&'static str, Vec<(&'static str, Hsla)>)> {
  let s: SyntaxTokenHues = theme.tokens.syntax;
  let c = |hue: f32| theme.color_for_hue(hue);
  vec![(
    "Syntax Hues",
    vec![
      ("attribute", c(s.attribute)),
      ("boolean", c(s.boolean)),
      ("comment", c(s.comment)),
      ("comment_doc", c(s.comment_doc)),
      ("constant", c(s.constant)),
      ("constructor", c(s.constructor)),
      ("embedded", c(s.embedded)),
      ("emphasis", c(s.emphasis)),
      ("emphasis_strong", c(s.emphasis_strong)),
      ("enum", c(s.enum_)),
      ("function", c(s.function)),
      ("hint", c(s.hint)),
      ("keyword", c(s.keyword)),
      ("label", c(s.label)),
      ("link_text", c(s.link_text)),
      ("link_uri", c(s.link_uri)),
      ("number", c(s.number)),
      ("operator", c(s.operator)),
      ("predictive", c(s.predictive)),
      ("preproc", c(s.preproc)),
      ("primary", c(s.primary)),
      ("property", c(s.property)),
      ("punctuation", c(s.punctuation)),
      ("punctuation_bracket", c(s.punctuation_bracket)),
      ("punctuation_delimiter", c(s.punctuation_delimiter)),
      ("punctuation_list_marker", c(s.punctuation_list_marker)),
      ("punctuation_special", c(s.punctuation_special)),
      ("string", c(s.string)),
      ("string_escape", c(s.string_escape)),
      ("string_regex", c(s.string_regex)),
      ("string_special", c(s.string_special)),
      ("string_special_symbol", c(s.string_special_symbol)),
      ("tag", c(s.tag)),
      ("tag_doctype", c(s.tag_doctype)),
      ("text_literal", c(s.text_literal)),
      ("title", c(s.title)),
      ("type", c(s.type_)),
      ("variable", c(s.variable)),
      ("variable_special", c(s.variable_special)),
      ("variant", c(s.variant)),
    ],
  )]
}

impl Render for PaletteWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let colors = theme.colors;
    let tokens = theme.tokens;
    let is_dark = theme.mode.is_dark();

    let sections = vec![
      (
        "Surface",
        vec![
          ("background", colors.background),
          ("foreground", colors.foreground),
          ("card", colors.card),
          ("card_foreground", colors.card_foreground),
          ("popover", colors.popover),
          ("popover_foreground", colors.popover_foreground),
          ("muted", colors.muted),
          ("muted_foreground", colors.muted_foreground),
          ("accent", colors.accent),
          ("accent_foreground", colors.accent_foreground),
        ],
      ),
      (
        "Interactive",
        vec![
          ("primary", colors.primary),
          ("primary_foreground", colors.primary_foreground),
          ("secondary", colors.secondary),
          ("secondary_hover", colors.secondary_hover),
          ("secondary_foreground", colors.secondary_foreground),
          ("border", colors.border),
          ("input", colors.input),
          ("ring", colors.ring),
          ("selection", colors.selection),
          ("caret", colors.caret),
        ],
      ),
      (
        "Semantic",
        vec![
          ("success", colors.success),
          ("warning", colors.warning),
          ("danger", colors.danger),
          ("red", colors.red),
          ("green", colors.green),
          ("blue", colors.blue),
          ("yellow", colors.yellow),
          ("cyan", colors.cyan),
        ],
      ),
      (
        "Editor",
        vec![
          ("editor_background", colors.editor_background),
          ("editor_foreground", colors.editor_foreground),
          ("editor_active_line", colors.editor_active_line),
          ("editor_line_number", colors.editor_line_number),
          (
            "editor_active_line_number",
            colors.editor_active_line_number,
          ),
          ("editor_invisible", colors.editor_invisible),
        ],
      ),
      (
        "Tabs & Bars",
        vec![
          ("tab_bar", colors.tab_bar),
          ("tab_bar_segmented", colors.tab_bar_segmented),
          ("tab_foreground", colors.tab_foreground),
          ("tab_active", colors.tab_active),
          ("tab_active_foreground", colors.tab_active_foreground),
          ("title_bar", colors.title_bar),
          ("tiles", colors.tiles),
          ("drag_border", colors.drag_border),
          ("drop_target", colors.drop_target),
        ],
      ),
      (
        "Scrollbar",
        vec![
          ("scrollbar", colors.scrollbar),
          ("scrollbar_thumb", colors.scrollbar_thumb),
          ("scrollbar_thumb_hover", colors.scrollbar_thumb_hover),
        ],
      ),
    ]
    .into_iter()
    .chain(syntax_sections(theme))
    .collect::<Vec<_>>();

    window_border().child(
      v_flex()
        .size_full()
        .min_h_0()
        .child(
          TitleBar::new()
            .title("Woocraft Theme Palette")
            .zoom_button(true),
        )
        .child(
          v_flex()
            .p_6()
            .gap_6()
            .overflow_y_scrollbar()
            .flex_1()
            .min_h_0()
            .child(
              h_flex()
                .items_center()
                .justify_between()
                .child(div().text_xl().font_semibold().child("Theme Palette"))
                .child(
                  h_flex()
                    .gap_2()
                    .child(
                      Button::new("theme-light")
                        .label("Light")
                        .selected(!is_dark)
                        .small()
                        .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Light, cx)),
                    )
                    .child(
                      Button::new("theme-dark")
                        .label("Dark")
                        .selected(is_dark)
                        .small()
                        .on_click(|_, _, cx| Theme::set_mode(ThemeMode::Dark, cx)),
                    ),
                ),
            )
            .child(
              div()
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child(format!(
                  "tokens: primary={}° accent={}° secondary={}° lightness={} chroma={} \
                   bg(l)={}/c={} bg(d)={}/c={} | hues: error={}° warning={}° success={}° info={}°",
                  tokens.primary,
                  tokens.accent,
                  tokens.secondary,
                  tokens.lightness,
                  tokens.chroma,
                  tokens.light_bg_lightness,
                  tokens.light_bg_chroma,
                  tokens.dark_bg_lightness,
                  tokens.dark_bg_chroma,
                  tokens.error,
                  tokens.warning,
                  tokens.success,
                  tokens.info,
                )),
            )
            .children(
              sections
                .into_iter()
                .map(|(title, colors)| section(title, colors, cx)),
            ),
        ),
    )
  }
}

fn main() {
  gpui_platform::application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      woocraft::init(cx);
      cx.activate(true);

      let bounds = Bounds::centered(None, GpuiSize::new(px(1100.), px(760.)), cx);
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
          PaletteWindow::view,
        )
        .expect("open palette window failed");

      window
        .update(cx, |_, window, _| {
          window.activate_window();
          window.set_window_title("Woocraft Theme Palette");
        })
        .expect("update palette window failed");
    });
}
