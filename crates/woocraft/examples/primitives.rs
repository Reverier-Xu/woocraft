//! display primitives gallery — label, divider, badge, tag, kbd, spinner,
//! and icon label, reviewed against the active theme.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example primitives
//! ```

use gpui::{
  App, AppContext, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Point, Render,
  SharedString, StatefulInteractiveElement, Styled, Window, WindowBounds, WindowOptions, div,
  prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Badge, Divider, Icon, IconLabel, IconName, Kbd, Label, Spinner, Tag, Theme,
  ThemeMode, application, init, logging,
};

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
      size: size(px(920.), px(720.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_window, cx| cx.new(|_| PrimitivesGallery))
        .expect("failed to open the primitives gallery window");
    })
    .detach();
  });
}

struct PrimitivesGallery;

impl Render for PrimitivesGallery {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();

    div()
      .id("primitives-gallery")
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
      .child(section("labels", theme, labels()))
      .child(section("dividers", theme, dividers(theme)))
      .child(section("tags", theme, tags(theme)))
      .child(section("keyboard", theme, kbds()))
      .child(section("badges", theme, badges(theme)))
      .child(section("spinners & rows", theme, spinners_and_rows()))
  }
}

fn header(theme: &Theme) -> impl IntoElement {
  let dark = theme.mode.is_dark();

  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_3()
    .child(div().child("woocraft primitives"))
    .child(
      div()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(
          format!("{:?}", theme.mode).to_lowercase(),
        )),
    )
    .child(div().flex_1())
    .child(mode_button(theme, ThemeMode::Light, !dark))
    .child(mode_button(theme, ThemeMode::Dark, dark))
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
    .on_click(move |_: &gpui::ClickEvent, _, cx| Theme::set_mode(mode, cx))
    .child(SharedString::from(label))
}

fn section(title: &'static str, theme: &Theme, content: impl IntoElement) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(
      div()
        .text_color(theme.muted_foreground)
        .child(title.to_uppercase()),
    )
    .child(
      div()
        .flex()
        .flex_col()
        .gap_4()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(theme.card)
        .p_4()
        .child(content),
    )
}

fn labels() -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(Label::new("the quick brown fox jumps over the lazy dog"))
    .child(Label::new("search results for").secondary("woocraft theme"))
    .child(Label::new("the quick brown fox").highlights("brown"))
    .child(Label::new("s3cret-value").masked(true))
}

fn dividers(theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_4()
    .child(Divider::horizontal())
    .child(Divider::horizontal().dashed())
    .child(Divider::horizontal().label("section").color(theme.primary))
    .child(
      div()
        .flex()
        .items_center()
        .gap_4()
        .child(div().h(rems(2.)).child("left"))
        .child(Divider::vertical())
        .child(div().h(rems(2.)).child("right")),
    )
}

fn tags(theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_wrap()
    .gap_2()
    .child(Tag::secondary().child("secondary"))
    .child(Tag::primary().child("primary"))
    .child(Tag::success().child("success"))
    .child(Tag::warning().child("warning"))
    .child(Tag::danger().child("danger"))
    .child(Tag::info().child("info"))
    .child(Tag::primary().outline().child("outline"))
    .child(Tag::success().rounded_full().child("pill"))
    .child({
      let (bg, fg, border) = (theme.primary, theme.primary_foreground, theme.border);
      Tag::custom(bg, fg, border).child("custom")
    })
}

fn kbds() -> impl IntoElement {
  let ctrl_delete = gpui::Keystroke::parse("ctrl-alt-delete").unwrap();
  let enter = gpui::Keystroke::parse("enter").unwrap();
  let cmd_shift_p = gpui::Keystroke::parse("cmd-shift-p").unwrap();

  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_2()
    .child(Kbd::new(ctrl_delete))
    .child(Kbd::new(enter.clone()))
    .child(Kbd::new(cmd_shift_p).outline())
    .child(Kbd::new(enter).appearance(false))
}

fn badges(theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_6()
    .child(
      div()
        .relative()
        .p_2()
        .child(Icon::new(IconName::Alert))
        .child(Badge::new().count(5)),
    )
    .child(
      div()
        .relative()
        .p_2()
        .child(Icon::new(IconName::Alert))
        .child(Badge::new().count(150).max(99)),
    )
    .child(
      div()
        .relative()
        .p_2()
        .child(Icon::new(IconName::Person))
        .child(Badge::new().dot().color(theme.success)),
    )
    .child(
      div()
        .relative()
        .p_2()
        .child(Icon::new(IconName::Chat))
        .child(
          Badge::new()
            .icon(Icon::new(IconName::Pin))
            .color(theme.primary),
        ),
    )
}

fn spinners_and_rows() -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_3()
    .child(
      div().flex().items_center().gap_4().child(
        div()
          .flex()
          .items_center()
          .gap_2()
          .child(Spinner::new())
          .child(Label::new("loading…")),
      ),
    )
    .child(
      IconLabel::new("row-default")
        .icon(Icon::new(IconName::Star))
        .label("default row"),
    )
    .child(
      IconLabel::new("row-selected")
        .icon(Icon::new(IconName::Star))
        .label("selected row")
        .selected(true),
    )
    .child(
      IconLabel::new("row-disabled")
        .icon(Icon::new(IconName::Star))
        .label("disabled row")
        .disabled(true),
    )
    .child(
      IconLabel::new("row-clickable")
        .icon(Icon::new(IconName::Rocket))
        .label("clickable row")
        .on_click(|_, _, _| tracing::info!("row clicked")),
    )
}
