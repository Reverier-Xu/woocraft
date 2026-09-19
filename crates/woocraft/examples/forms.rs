//! forms gallery — checkbox, switch, toggle, radio, avatar, progress, link,
//! and collapsible reviewed against the active theme with fully controlled
//! state, light/dark switching, and rem scaling.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example forms
//! ```

use std::f32::consts::PI;

use gpui::{
  App, AppContext, Bounds, Context, Entity, Global, InteractiveElement, IntoElement, ParentElement,
  Pixels, Point, Radians, Render, SharedString, StatefulInteractiveElement, Styled, Window,
  WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, Assets, Avatar, Checkbox, CheckboxState, Collapsible, Icon, IconName, Link,
  Progress, Radio, RadioGroup, Switch, Theme, ThemeMode, Toggle, ToggleGroup, application, init,
  logging,
};

/// the current ui font size, adjustable from the gallery header.
struct UiScale(Pixels);

impl Global for UiScale {}

const MIN_REM: f32 = 12.0;
const MAX_REM: f32 = 28.0;
const REM_STEP: f32 = 2.0;

/// plan options for the radio group demo.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Plan {
  Basic,
  Pro,
  Team,
}

struct FormsGallery {
  notifications: bool,
  tasks: [bool; 3],
  wifi: bool,
  bluetooth: bool,
  hotspot: bool,
  bold: bool,
  italic: bool,
  underline: bool,
  align: usize,
  plan: Plan,
  details_open: bool,
}

impl Default for FormsGallery {
  fn default() -> Self {
    Self {
      notifications: true,
      tasks: [true, false, false],
      wifi: true,
      bluetooth: false,
      hotspot: true,
      bold: true,
      italic: false,
      underline: false,
      align: 1,
      plan: Plan::Pro,
      details_open: false,
    }
  }
}

fn main() {
  let _ = logging::init();

  application().with_assets(Assets).run(|cx: &mut App| {
    if let Err(err) = init(cx) {
      eprintln!("woocraft init failed: {err}");
      return;
    }
    cx.set_global(UiScale(px(16.)));

    let bounds = Bounds {
      origin: Point {
        x: px(120.),
        y: px(120.),
      },
      size: size(px(980.), px(860.)),
    };
    let options = WindowOptions {
      window_bounds: Some(WindowBounds::Windowed(bounds)),
      ..Default::default()
    };

    cx.spawn(async move |cx| {
      cx.open_window(options, |_window, cx| cx.new(|_| FormsGallery::default()))
        .expect("failed to open the forms gallery window");
    })
    .detach();
  });
}

impl Render for FormsGallery {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let theme = cx.theme();
    let scale = cx.global::<UiScale>().0;

    div()
      .id("forms-gallery")
      .size_full()
      .overflow_y_scroll()
      .font_family(theme.font_family.clone())
      .bg(theme.background)
      .text_color(theme.foreground)
      .flex()
      .flex_col()
      .gap_6()
      .p_6()
      .child(header(theme, scale))
      .child(section(
        theme,
        "checkbox",
        checkboxes(&cx.entity(), self, theme),
      ))
      .child(section(
        theme,
        "switch",
        switches(&cx.entity(), self, theme),
      ))
      .child(section(theme, "toggle", toggles(&cx.entity(), self, theme)))
      .child(section(theme, "radio", radios(&cx.entity(), self, theme)))
      .child(section(theme, "avatar", avatars(theme)))
      .child(section(theme, "progress", progress(theme)))
      .child(section(theme, "link", links(theme)))
      .child(section(
        theme,
        "collapsible",
        collapsible(&cx.entity(), self, theme),
      ))
  }
}

fn checkboxes(
  gallery: &Entity<FormsGallery>, state: &FormsGallery, theme: &Theme,
) -> impl IntoElement {
  let tasks_checked = state.tasks.iter().all(|task| *task);
  let tasks_indeterminate = !tasks_checked && state.tasks.iter().any(|task| *task);
  let readout = SharedString::from(format!(
    "tasks: {:?}",
    state.tasks.iter().map(|t| u8::from(*t)).collect::<Vec<_>>()
  ));

  let parent = gallery.clone();
  let child_a = gallery.clone();
  let child_b = gallery.clone();
  let child_c = gallery.clone();
  let notify = gallery.clone();

  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(labeled(
      theme,
      "controlled",
      Checkbox::new("notifications")
        .label("enable notifications")
        .checked(state.notifications)
        .on_change(move |state: CheckboxState, _, _, cx| {
          notify.update(cx, |gallery, _| {
            gallery.notifications = state == CheckboxState::Checked
          });
        }),
    ))
    .child(labeled(
      theme,
      "tri-state parent",
      Checkbox::new("tasks-parent")
        .label("all tasks")
        .checked(tasks_checked)
        .indeterminate(tasks_indeterminate)
        .on_change(move |state: CheckboxState, _, _, cx| {
          let checked = state == CheckboxState::Checked;
          parent.update(cx, |gallery, _| gallery.tasks = [checked; 3]);
        }),
    ))
    .child(labeled(
      theme,
      "readout",
      div().text_color(theme.muted_foreground).child(readout),
    ))
    .child(
      div().pl(rems(2.)).flex().flex_col().gap_2().children(
        [child_a, child_b, child_c]
          .into_iter()
          .enumerate()
          .map(|(index, gallery)| {
            Checkbox::new(("task", index))
              .label(["design", "implement", "test"][index])
              .checked(state.tasks[index])
              .on_change(move |state: CheckboxState, _, _, cx| {
                let checked = state == CheckboxState::Checked;
                gallery.update(cx, move |gallery, _| gallery.tasks[index] = checked);
              })
          }),
      ),
    )
    .child(labeled(
      theme,
      "disabled",
      Checkbox::new("disabled")
        .label("locked choice")
        .disabled(true),
    ))
    .child(labeled(
      theme,
      "disabled checked",
      Checkbox::new("disabled-checked")
        .label("locked agreement")
        .checked(true)
        .disabled(true),
    ))
}

fn switches(
  gallery: &Entity<FormsGallery>, state: &FormsGallery, theme: &Theme,
) -> impl IntoElement {
  let wifi = gallery.clone();
  let bluetooth = gallery.clone();
  let hotspot = gallery.clone();

  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(labeled(
      theme,
      "on",
      Switch::new("wifi")
        .label("wi-fi")
        .checked(state.wifi)
        .on_change(move |checked, _, _, cx| {
          wifi.update(cx, |gallery, _| gallery.wifi = checked);
        }),
    ))
    .child(labeled(
      theme,
      "off",
      Switch::new("bluetooth")
        .label("bluetooth")
        .checked(state.bluetooth)
        .on_change(move |checked, _, _, cx| {
          bluetooth.update(cx, |gallery, _| gallery.bluetooth = checked);
        }),
    ))
    .child(labeled(
      theme,
      "success",
      Switch::new("hotspot")
        .label("hotspot")
        .color(theme.success)
        .checked(state.hotspot)
        .on_change(move |checked, _, _, cx| {
          hotspot.update(cx, |gallery, _| gallery.hotspot = checked);
        }),
    ))
    .child(labeled(
      theme,
      "disabled",
      Switch::new("switch-disabled")
        .label("managed by org")
        .disabled(true),
    ))
    .child(labeled(
      theme,
      "disabled on",
      Switch::new("switch-locked")
        .label("enforced")
        .checked(true)
        .disabled(true),
    ))
}

fn toggles(
  gallery: &Entity<FormsGallery>, state: &FormsGallery, theme: &Theme,
) -> impl IntoElement {
  let bold = gallery.clone();
  let italic = gallery.clone();
  let underline = gallery.clone();
  let align_left = gallery.clone();
  let align_center = gallery.clone();
  let align_right = gallery.clone();

  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(labeled(
      theme,
      "rich text",
      ToggleGroup::new("rich-text")
        .child(
          Toggle::new("bold")
            .icon(Icon::new(IconName::TextBold))
            .pressed(state.bold)
            .on_change(move |pressed, _, _, cx| {
              bold.update(cx, |gallery, _| gallery.bold = pressed);
            }),
        )
        .child(
          Toggle::new("italic")
            .icon(Icon::new(IconName::TextItalic))
            .pressed(state.italic)
            .on_change(move |pressed, _, _, cx| {
              italic.update(cx, |gallery, _| gallery.italic = pressed);
            }),
        )
        .child(
          Toggle::new("underline")
            .icon(Icon::new(IconName::TextUnderline))
            .pressed(state.underline)
            .on_change(move |pressed, _, _, cx| {
              underline.update(cx, |gallery, _| gallery.underline = pressed);
            }),
        ),
    ))
    .child(labeled(
      theme,
      "alignment",
      ToggleGroup::new("alignment")
        .child(
          Toggle::new("align-left")
            .icon(Icon::new(IconName::AlignLeft))
            .pressed(state.align == 0)
            .on_change(move |pressed, _, _, cx| {
              if pressed {
                align_left.update(cx, |gallery, _| gallery.align = 0);
              }
            }),
        )
        .child(
          Toggle::new("align-center")
            .icon(Icon::new(IconName::AlignCenterHorizontal))
            .pressed(state.align == 1)
            .on_change(move |pressed, _, _, cx| {
              if pressed {
                align_center.update(cx, |gallery, _| gallery.align = 1);
              }
            }),
        )
        .child(
          Toggle::new("align-right")
            .icon(Icon::new(IconName::AlignRight))
            .pressed(state.align == 2)
            .on_change(move |pressed, _, _, cx| {
              if pressed {
                align_right.update(cx, |gallery, _| gallery.align = 2);
              }
            }),
        ),
    ))
    .child(labeled(
      theme,
      "disabled",
      Toggle::new("toggle-disabled")
        .label("archived")
        .disabled(true),
    ))
}

fn radios(gallery: &Entity<FormsGallery>, state: &FormsGallery, theme: &Theme) -> impl IntoElement {
  let plans = [
    ("basic", Plan::Basic),
    ("pro", Plan::Pro),
    ("team", Plan::Team),
  ];
  let count = plans.len();

  let radios = plans
    .iter()
    .enumerate()
    .map(|(index, (name, plan))| {
      let gallery = gallery.clone();
      let (name, plan) = (*name, *plan);
      Radio::new(("plan", index))
        .label(name)
        .checked(state.plan == plan)
        .set_position(index + 1, count)
        .on_change(move |checked, _, _, cx| {
          if checked {
            gallery.update(cx, move |gallery, _| gallery.plan = plan);
          }
        })
        .into_any_element()
    })
    .collect::<Vec<_>>();

  div().flex().flex_col().gap_2().child(labeled(
    theme,
    "plan",
    RadioGroup::new("plan").children(radios),
  ))
}

fn avatars(theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_3()
    .child(Avatar::new().name("alice johnson"))
    .child(Avatar::new().name("reverier"))
    .child(Avatar::new().name("bob marley").bg_color(theme.success))
    .child(Avatar::new())
    .child(Avatar::new().src(SharedString::from(
      "tech.woooo.craft/assets/icons/person.svg",
    )))
}

fn progress(theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_4()
    .w(rems(20.))
    .child(
      Progress::new("progress-quarter")
        .value(25.)
        .accessibility_label("quarter"),
    )
    .child(Progress::new("progress-half").value(60.))
    .child(Progress::new("progress-done").value(100.))
    .child(
      Progress::new("progress-success")
        .value(80.)
        .color(theme.success),
    )
    .child(Progress::new("progress-indeterminate").indeterminate(true))
}

fn links(theme: &Theme) -> impl IntoElement {
  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(labeled(
      theme,
      "external",
      Link::new("homepage")
        .href("https://craft.woooo.tech")
        .child("woocraft homepage"),
    ))
    .child(labeled(
      theme,
      "in a sentence",
      div().child(
        div()
          .flex()
          .gap_1()
          .child("read the")
          .child(
            Link::new("guide")
              .href("https://github.com/Reverier-Xu/woocraft")
              .child("repository"),
          )
          .child("for details"),
      ),
    ))
    .child(labeled(
      theme,
      "disabled",
      Link::new("link-disabled")
        .href("https://craft.woooo.tech")
        .child("unavailable"),
    ))
}

fn collapsible(
  gallery: &Entity<FormsGallery>, state: &FormsGallery, theme: &Theme,
) -> impl IntoElement {
  let toggle = gallery.clone();
  let open = state.details_open;

  div()
    .flex()
    .flex_col()
    .gap_2()
    .child(
      div()
        .id("details-trigger")
        .cursor_pointer()
        .flex()
        .items_center()
        .gap_2()
        .text_color(theme.foreground)
        .child(Icon::new(IconName::ChevronDown).rotate(Radians(if open { PI } else { 0.0 })))
        .child("details")
        .on_click(move |_, _, cx| {
          toggle.update(cx, |gallery, _| {
            gallery.details_open = !gallery.details_open
          });
        }),
    )
    .child(
      Collapsible::new("details").open(open).content(
        div()
          .flex()
          .flex_col()
          .gap_2()
          .pt_2()
          .pl(rems(3.))
          .text_color(theme.muted_foreground)
          .child("the collapsible content unmounts once the close")
          .child("transition settles, and animates its height on the")
          .child("way in and out."),
      ),
    )
}

fn header(theme: &Theme, scale: Pixels) -> impl IntoElement {
  let dark = theme.mode.is_dark();

  div()
    .flex()
    .flex_wrap()
    .items_center()
    .gap_3()
    .child(div().child("woocraft forms"))
    .child(
      div()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(
          format!("{:?}", theme.mode).to_lowercase(),
        )),
    )
    .child(
      div()
        .flex()
        .items_center()
        .gap_2()
        .child(
          div()
            .text_color(theme.muted_foreground)
            .child(SharedString::from(format!("rem {scale}"))),
        )
        .child(scale_button("scale-down", "\u{2212}", -REM_STEP))
        .child(scale_button("scale-up", "+", REM_STEP)),
    )
    .child(div().flex_1())
    .child(mode_button(theme, ThemeMode::Light, !dark))
    .child(mode_button(theme, ThemeMode::Dark, dark))
}

fn scale_button(id: &'static str, label: &'static str, delta: f32) -> woocraft::Button {
  woocraft::Button::new(id)
    .label(label)
    .on_click(move |_, window, cx| {
      let next = (cx.global::<UiScale>().0 + px(delta))
        .max(px(MIN_REM))
        .min(px(MAX_REM));
      cx.set_global(UiScale(next));
      window.set_rem_size(next);
      cx.refresh_windows();
    })
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

fn labeled(theme: &Theme, label: &str, widget: impl IntoElement) -> impl IntoElement {
  div()
    .flex()
    .items_center()
    .gap_3()
    .child(
      div()
        .w(rems(10.))
        .flex_none()
        .text_color(theme.muted_foreground)
        .child(SharedString::from(label.to_string())),
    )
    .child(widget)
}

fn section(theme: &Theme, title: &'static str, content: impl IntoElement) -> impl IntoElement {
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
