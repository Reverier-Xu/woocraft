//! Theme palette inspector.
//!
//! Renders every color derived from the current `ThemeTokens` (light and
//! dark), the raw token values, and the syntax highlight hues. Use it to
//! eyeball palette tweaks before committing them to `ThemeTokens::default`.

use gpui::{
  App, AppContext, Bounds, Context, Entity, Hsla, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, black, canvas, div, fill, point,
  px, size, white,
};
use woocraft::{
  ActiveTheme, Button, ScrollableElement as _, Selectable, Sizable, StyledExt, SyntaxTokenHues,
  Theme, ThemeMode, TitleBar, h_flex, v_flex, window_border,
};
use woocraft_colors::{Hct, Tone};

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

/// Picks black or white text for readability on `background`.
///
/// The theme reasons in HCT, so this does too: black and white are the ends of
/// the tone scale, and the contrast ratio against the background follows from
/// the background's own tone. No threshold to tune.
fn readable_on(background: Hsla) -> Hsla {
  let tone = woocraft_colors::Hct::from_srgb(
    woocraft_colors::Hsl {
      h: f64::from(background.h) * 360.0,
      s: f64::from(background.s),
      l: f64::from(background.l),
      a: 1.0,
    }
    .to_srgb(),
  )
  .tone();

  let white_ratio = woocraft_colors::Contrast::ratio_of_tones(tone, woocraft_colors::Tone::MAX);
  let black_ratio = woocraft_colors::Contrast::ratio_of_tones(tone, woocraft_colors::Tone::MIN);
  if white_ratio >= black_ratio {
    white()
  } else {
    black()
  }
}

/// One labeled position on the hue bar.
struct HueMark {
  name: &'static str,
  hue: f64,
}

/// A 0..360° hue bar painted at the theme's fill chroma and tone, with the
/// semantic hues marked on it.
///
/// This is the acceptance tool for palette work: it shows where each role's hue
/// actually lands in CAM16, which is not where an evenly spaced hue wheel would
/// put it, and it shows the gamut ceiling as a dimmed band so it is obvious
/// which hues had to be gamut-mapped down.
fn hue_bar(tokens: &woocraft::ThemeTokens, tone: Tone, cx: &App) -> impl IntoElement {
  let chroma = f64::from(tokens.chroma);
  let marks: Vec<HueMark> = vec![
    HueMark {
      name: "primary",
      hue: f64::from(tokens.primary),
    },
    HueMark {
      name: "info",
      hue: f64::from(tokens.info),
    },
    HueMark {
      name: "error",
      hue: f64::from(tokens.error),
    },
    HueMark {
      name: "warning",
      hue: f64::from(tokens.warning),
    },
    HueMark {
      name: "success",
      hue: f64::from(tokens.success),
    },
    HueMark {
      name: "accent",
      hue: f64::from(tokens.accent),
    },
    HueMark {
      name: "secondary",
      hue: f64::from(tokens.secondary),
    },
    HueMark {
      name: "red",
      hue: f64::from(tokens.red),
    },
    HueMark {
      name: "green",
      hue: f64::from(tokens.green),
    },
    HueMark {
      name: "blue",
      hue: f64::from(tokens.blue),
    },
    HueMark {
      name: "yellow",
      hue: f64::from(tokens.yellow),
    },
    HueMark {
      name: "cyan",
      hue: f64::from(tokens.cyan),
    },
  ];

  // Sample the bar once per degree: enough to read the gamut wall as a smooth
  // edge rather than a staircase.
  let samples: Vec<(Hsla, bool)> = (0..360)
    .map(|hue| {
      let requested = Hct::new(f64::from(hue), chroma, tone);
      let clipped = requested.chroma() < chroma - 0.5;
      let mut hsla = color_of(requested);
      // Dim the clipped arcs so the gamut wall is visible at a glance.
      if clipped {
        hsla = hsla.opacity(0.45);
      }
      (hsla, clipped)
    })
    .collect();

  let label_marks: Vec<(String, Hsla)> = marks
    .iter()
    .map(|mark| {
      let color = color_of(Hct::new(mark.hue, chroma, tone));
      (format!("{} {:.0}°", mark.name, mark.hue), color)
    })
    .collect();
  let tick_marks: Vec<f64> = marks.iter().map(|mark| mark.hue).collect();

  v_flex()
    .gap_2()
    .child(
      h_flex()
        .gap_2()
        .items_center()
        .child(div().text_sm().font_semibold().child("Hue bar"))
        .child(
          div()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(format!(
              "chroma {:.0} · tone {:.0} · dimmed arcs are gamut-mapped",
              chroma,
              tone.value()
            )),
        ),
    )
    .child(
      div()
        .relative()
        .h(px(56.0))
        .w_full()
        .rounded(cx.theme().radius)
        .border_1()
        .border_color(cx.theme().border)
        .overflow_hidden()
        .child(
          canvas(
            move |_, _, _| {},
            move |bounds, _, window, cx| {
              let width = bounds.size.width / 360.0;
              for (hue, (color, _)) in samples.iter().enumerate() {
                let segment = Bounds::new(
                  point(bounds.origin.x + width * hue as f32, bounds.origin.y),
                  size(width.max(px(1.)), bounds.size.height),
                );
                window.paint_quad(fill(segment, *color));
              }

              // Ticks for each marked hue, drawn from the top edge down.
              let tick_color = cx.theme().foreground;
              for hue in &tick_marks {
                let x = bounds.origin.x + bounds.size.width * (*hue as f32 / 360.0);
                let tick = Bounds::new(
                  point(x - px(0.5), bounds.origin.y),
                  size(px(1.), bounds.size.height * 0.45),
                );
                window.paint_quad(fill(tick, tick_color));
              }
            },
          )
          .absolute()
          .size_full(),
        ),
    )
    .child(
      h_flex()
        .flex_wrap()
        .gap_x_3()
        .gap_y_1()
        .children(label_marks.into_iter().map(|(label, color)| {
          h_flex()
            .gap_1()
            .items_center()
            .child(div().size(px(8.0)).rounded_full().bg(color))
            .child(div().text_xs().child(label))
        })),
    )
}

/// The theme's `Hsla` for an HCT color.
fn color_of(color: Hct) -> Hsla {
  let hsl = color.hsl();
  Hsla {
    h: hsl.h as f32 / 360.0,
    s: hsl.s as f32,
    l: hsl.l as f32,
    a: 1.0,
  }
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
          ("info_foreground", colors.info_foreground),
          ("selection", colors.selection),
          ("caret", colors.caret),
        ],
      ),
      (
        "Semantic",
        vec![
          ("success", colors.success),
          ("success_foreground", colors.success_foreground),
          ("warning", colors.warning),
          ("warning_foreground", colors.warning_foreground),
          ("danger", colors.danger),
          ("danger_foreground", colors.danger_foreground),
          ("info", colors.info),
          ("info_foreground", colors.info_foreground),
          ("red", colors.red),
          ("red_foreground", colors.red_foreground),
          ("green", colors.green),
          ("green_foreground", colors.green_foreground),
          ("blue", colors.blue),
          ("blue_foreground", colors.blue_foreground),
          ("yellow", colors.yellow),
          ("yellow_foreground", colors.yellow_foreground),
          ("cyan", colors.cyan),
          ("cyan_foreground", colors.cyan_foreground),
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
                  "tokens: primary={}° accent={}° secondary={}° chroma={} \
                   fill tone(l/d)={}/{} bg tone(l/d)={}/{} surface chroma={} \
                   | hues: error={}° warning={}° success={}° info={}°",
                  tokens.primary,
                  tokens.accent,
                  tokens.secondary,
                  tokens.chroma,
                  tokens.light_fill_tone,
                  tokens.dark_fill_tone,
                  tokens.light_bg_tone,
                  tokens.dark_bg_tone,
                  tokens.surface_chroma,
                  tokens.error,
                  tokens.warning,
                  tokens.success,
                  tokens.info,
                )),
            )
            .child(hue_bar(&tokens, tokens.fill_tone(is_dark), cx))
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
