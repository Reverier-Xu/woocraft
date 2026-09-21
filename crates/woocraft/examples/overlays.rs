//! overlays gallery — popover, tooltip, dialog, alert dialog, and
//! toast reviewed against the active theme with controlled state, focus
//! traps, and the managed tooltip overlay.
//!
//! run with:
//!
//! ```text
//! cargo run -p woocraft --example overlays
//! ```

use std::time::{Duration, Instant};

use gpui::{
  Anchor, App, AppContext, Bounds, Context, Entity, FocusHandle, Global, InteractiveElement,
  IntoElement, Keystroke, ParentElement, Point, Render, SharedString, StatefulInteractiveElement,
  Styled, Window, WindowBounds, WindowOptions, div, prelude::FluentBuilder as _, px, rems, size,
};
use woocraft::{
  ActiveTheme, AlertDialog, AlertDialogAction, AlertDialogCancel, Button, ButtonVariants as _,
  Dialog, DialogClose, DialogDescription, DialogHandle, DialogStack, DialogTitle, Kbd, Popover,
  Theme, ThemeMode, Toast, ToastManager, ToastOptions, ToastStackState, ToastVariant, Toaster,
  Tooltip, WindowExt as _, application, base, h_flex, init, logging, v_flex,
};

/// the current ui font size, adjustable from the gallery header.
struct UiScale(f32);

impl Global for UiScale {}

/// one entry in the demo toast stack.
#[derive(Clone)]
struct Draft {
  variant: ToastVariant,
  title: &'static str,
  description: Option<&'static str>,
  timeout: Option<Duration>,
  action: bool,
  pushed_at: Option<Instant>,
}

struct OverlaysGallery {
  dialog_handle: DialogHandle,
  dialogs: Entity<DialogStack>,
  alert_open: bool,
  popover_open: bool,
  decision: Option<&'static str>,
  toasts: ToastManager<SharedString, Draft>,
  toast_state: ToastStackState,
  toast_focus: FocusHandle,
  toast_serial: usize,
}

impl OverlaysGallery {
  fn new(window: &Window, cx: &mut Context<Self>) -> Self {
    let gallery = Self {
      dialog_handle: DialogHandle::new(false),
      dialogs: DialogStack::new(cx),
      alert_open: false,
      popover_open: false,
      decision: None,
      toasts: ToastManager::new(Default::default()),
      toast_state: ToastStackState::default(),
      toast_focus: cx.focus_handle(),
      toast_serial: 0,
    };
    gallery.spawn_tick(window, cx);
    gallery
  }

  /// advances the toast lifecycle clock; the stack pauses auto-hide while
  /// it is hovered or focused.
  fn spawn_tick(&self, window: &Window, cx: &mut Context<Self>) {
    cx.spawn_in(window, async move |gallery, cx| {
      loop {
        cx.background_executor()
          .timer(Duration::from_millis(120))
          .await;
        let _ = gallery.update_in(cx, |gallery, _, cx| {
          let paused = gallery.toast_state.is_expanded();
          let advance = gallery.toasts.advance(Instant::now(), paused);
          if advance.changed {
            cx.notify();
          }
        });
      }
    })
    .detach();
  }

  fn push_toast(&mut self, draft: Draft) {
    self.toast_serial += 1;
    let id = SharedString::from(format!("toast-{}", self.toast_serial));
    let mut draft = draft;
    draft.pushed_at = Some(Instant::now());
    let timeout = draft.timeout;
    self
      .toasts
      .push(id, draft, ToastOptions { timeout }, Instant::now());
  }

  /// the mounted toast cards, in display order with their live status.
  fn toast_elements(&self, gallery: &Entity<Self>) -> Vec<gpui::AnyElement> {
    self
      .toasts
      .iter()
      .map(|(id, draft, status)| {
        let gallery = gallery.clone();
        let close_id = id.clone();
        let remaining = draft
          .timeout
          .zip(draft.pushed_at)
          .map(|(timeout, pushed_at)| {
            (1. - pushed_at.elapsed().as_secs_f32() / timeout.as_secs_f32()).clamp(0., 1.)
          });
        Toast::new(id.clone())
          .variant(draft.variant)
          .title(draft.title)
          .when_some(draft.description, |toast, description| {
            toast.description(description)
          })
          .when_some(remaining, |toast, remaining| {
            toast.timeout_progress(remaining)
          })
          .transition_status(status)
          .when(draft.action, |toast| {
            let undo_id = id.clone();
            let undo_gallery = gallery.clone();
            toast.action("undo", move |_, window, cx| {
              undo_gallery.update(cx, |gallery, _| {
                gallery.toasts.dismiss(&undo_id, Instant::now());
              });
              window.refresh();
            })
          })
          .on_close(move |window, cx| {
            gallery.update(cx, |gallery, _| {
              gallery.toasts.dismiss(&close_id, Instant::now());
            });
            window.refresh();
          })
          .into_any_element()
      })
      .collect()
  }
}

fn main() {
  let _ = logging::init();

  application()
    .with_assets(woocraft::Assets)
    .run(|cx: &mut App| {
      if let Err(err) = init(cx) {
        eprintln!("woocraft init failed: {err}");
        return;
      }
      cx.set_global(UiScale(16.0));

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
        cx.open_window(options, |window, cx| {
          cx.new(|cx| OverlaysGallery::new(window, cx))
        })
        .expect("failed to open the overlays gallery window");
      })
      .detach();
    });
}

impl Render for OverlaysGallery {
  fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let gallery = cx.entity();
    let scale = cx.global::<UiScale>().0;
    // the few theme values the root itself needs, copied out so the modal
    // hosts below can take `cx` mutably.
    let (root_font, root_bg, root_fg) = {
      let theme = cx.theme();
      (
        theme.font_family.clone(),
        theme.background,
        theme.foreground,
      )
    };

    // display sections own a theme snapshot; modal hosts need `cx` mutably,
    // so the borrow ends before they are built.
    let sections = {
      let theme = cx.theme().clone();
      v_flex()
        .gap_6()
        .child(header(&theme, scale))
        .child(section(
          &theme,
          "popover",
          popovers(&gallery, self.popover_open, &theme),
        ))
        .child(section(&theme, "tooltip", tooltips(&theme)))
        .child(section(
          &theme,
          "dialog",
          dialogs(&gallery, self.decision, &theme),
        ))
        .child(section(
          &theme,
          "alert dialog",
          alerts(&gallery, self.decision, &theme),
        ))
        .child(section(&theme, "toast", toasts(&gallery, &theme)))
    };

    let dialog = Dialog::new(cx)
      .handle(self.dialog_handle.clone())
      .on_ok({
        let gallery = gallery.clone();
        move |_, _, cx| {
          gallery.update(cx, |gallery, _| {
            gallery.decision = Some("dialog: confirmed")
          });
          true
        }
      })
      .on_cancel({
        let gallery = gallery.clone();
        move |_, _, cx| {
          gallery.update(cx, |gallery, _| {
            gallery.decision = Some("dialog: cancelled")
          });
          true
        }
      })
      .child(
        DialogTitle::new()
          .child("rename workspace")
          .action(DialogClose::new()),
      )
      .child(DialogDescription::new().child(
        "pick a short, lowercase name. escape runs the cancel decision, \
           enter the ok decision.",
      ))
      .child(
        h_flex()
          .justify_end()
          .p(rems(0.25))
          .gap(rems(0.25))
          .child(
            Button::new("dialog-cancel")
              .label("cancel")
              .on_click(|_, window, cx| {
                window.dispatch_action(Box::new(base::actions::Cancel), cx);
              }),
          )
          .child(
            Button::new("dialog-ok")
              .label("rename")
              .primary()
              .on_click(|_, window, cx| {
                window.dispatch_action(Box::new(base::actions::Confirm { secondary: false }), cx);
              }),
          ),
      );

    let alert = AlertDialog::new(cx)
      .open(self.alert_open)
      .on_open_change({
        let gallery = gallery.clone();
        move |open, _, _, cx| {
          gallery.update(cx, |gallery, _| gallery.alert_open = open);
        }
      })
      .on_ok({
        let gallery = gallery.clone();
        move |_, _, cx| {
          gallery.update(cx, |gallery, _| gallery.decision = Some("alert: deleted"));
          true
        }
      })
      .on_cancel({
        let gallery = gallery.clone();
        move |_, _, cx| {
          gallery.update(cx, |gallery, _| gallery.decision = Some("alert: kept"));
          true
        }
      })
      .child(DialogTitle::new().child("delete branch?"))
      .child(
        DialogDescription::new()
          .child("12 commits land only on feat/v1-migration. this cannot be undone."),
      )
      .child(
        h_flex()
          .justify_end()
          .p(rems(0.25))
          .gap(rems(0.25))
          .child(AlertDialogCancel::new().child("keep branch"))
          .child(AlertDialogAction::new().child("delete").danger()),
      );

    // the toaster lives outside the scroll container: a scroll ancestor
    // clips absolutely positioned children, which would hide the stack.
    let toast_elements = self.toast_elements(&gallery);
    let mut toaster = Toaster::new("gallery-toaster", self.toast_state.clone())
      .focus_handle(self.toast_focus.clone())
      .absolute()
      .top(rems(1.))
      .right(rems(1.))
      .w(rems(20.));
    for (index, element) in toast_elements.into_iter().enumerate() {
      toaster = toaster.item(("toast-item", index as u64), element);
    }

    div()
      .id("overlays-gallery")
      .relative()
      .size_full()
      .font_family(root_font)
      .bg(root_bg)
      .text_color(root_fg)
      .child(
        div()
          .id("overlays-scroll")
          .size_full()
          .overflow_y_scroll()
          .flex()
          .flex_col()
          .child(sections),
      )
      .child(dialog)
      .child(alert)
      .child(toaster)
      .child(self.dialogs.clone())
  }
}

fn header(theme: &Theme, scale: f32) -> impl IntoElement {
  let mode = match theme.mode {
    ThemeMode::Light => "light",
    ThemeMode::Dark => "dark",
  };
  h_flex()
    .justify_between()
    .px(rems(1.5))
    .py(rems(1.))
    .border_b_1()
    .border_color(theme.border)
    .child(
      v_flex()
        .gap_1()
        .child(
          div()
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .child("overlays gallery"),
        )
        .child(
          div()
            .text_color(theme.muted_foreground)
            .child("popover · tooltip · dialog · alert · toast"),
        ),
    )
    .child(
      h_flex()
        .gap_2()
        .child(
          Button::new("theme-mode")
            .label(mode)
            .outline(true)
            .on_click(|_, _, cx| {
              let theme = Theme::global(cx);
              let next = match theme.mode {
                ThemeMode::Light => ThemeMode::Dark,
                ThemeMode::Dark => ThemeMode::Light,
              };
              Theme::set_mode(next, cx);
            }),
        )
        .child(
          div()
            .text_color(theme.muted_foreground)
            .child(format!("{scale:.0}px")),
        )
        .child(
          Button::new("scale-down")
            .label("a−")
            .outline(true)
            .on_click(|_, window, cx| {
              let next = (cx.global::<UiScale>().0 - 2.).clamp(12., 28.);
              cx.set_global(UiScale(next));
              window.set_rem_size(px(next));
              cx.refresh_windows();
            }),
        )
        .child(
          Button::new("scale-up")
            .label("a+")
            .outline(true)
            .on_click(|_, window, cx| {
              let next = (cx.global::<UiScale>().0 + 2.).clamp(12., 28.);
              cx.set_global(UiScale(next));
              window.set_rem_size(px(next));
              cx.refresh_windows();
            }),
        ),
    )
}

fn section(theme: &Theme, name: &'static str, body: impl IntoElement) -> impl IntoElement {
  v_flex()
    .px(rems(1.5))
    .py(rems(1.25))
    .gap_3()
    .child(
      div()
        .font_weight(gpui::FontWeight::MEDIUM)
        .text_color(theme.muted_foreground)
        .child(name),
    )
    .child(body)
}

fn labeled(theme: &Theme, label: &'static str, body: impl IntoElement) -> impl IntoElement {
  v_flex()
    .gap_1()
    .child(div().text_color(theme.muted_foreground).child(label))
    .child(body)
}

fn readout(theme: &Theme, text: &'static str) -> impl IntoElement {
  div().text_color(theme.muted_foreground).child(text)
}

fn popovers(gallery: &Entity<OverlaysGallery>, open: bool, theme: &Theme) -> impl IntoElement {
  let toggler = gallery.clone();
  let change_watcher = gallery.clone();
  let (target_radius, target_border) = (theme.radius, theme.border);

  v_flex()
    .gap_3()
    .child(labeled(
      theme,
      "uncontrolled · content can dismiss through the popover state",
      Popover::new("export-popover")
        .anchor(Anchor::BottomLeft)
        .trigger(Button::new("export-trigger").label("export…").outline(true))
        .content(|state, _, cx| {
          let popover = cx.entity();
          let _ = state;
          v_flex()
            .w(rems(11.))
            .gap_1()
            .child(Button::new("export-pdf").label("pdf").flat().on_click({
              let popover = popover.clone();
              move |_, window, cx| {
                popover.update(cx, |state, cx| state.dismiss(window, cx));
              }
            }))
            .child(Button::new("export-svg").label("svg").flat().on_click({
              let popover = popover.clone();
              move |_, window, cx| {
                popover.update(cx, |state, cx| state.dismiss(window, cx));
              }
            }))
        }),
    ))
    .child(labeled(
      theme,
      if open {
        "controlled · open"
      } else {
        "controlled · closed"
      },
      h_flex()
        .gap_2()
        .child(
          Button::new("controlled-toggle")
            .label("toggle from outside")
            .outline(true)
            .on_click(move |_, _, cx| {
              toggler.update(cx, |gallery, _| {
                gallery.popover_open = !gallery.popover_open
              });
            }),
        )
        .child(
          Popover::new("controlled-popover")
            .open(open)
            .anchor(Anchor::BottomLeft)
            .trigger(
              Button::new("controlled-anchor")
                .label("anchor")
                .outline(true),
            )
            .on_open_change(move |open, _, cx| {
              change_watcher.update(cx, |gallery, _| gallery.popover_open = *open);
            })
            .content(|_, _, _| {
              v_flex()
                .w(rems(11.))
                .child(div().child("state is pinned by the prop; escape still cancels"))
            }),
        ),
    ))
    .child(labeled(
      theme,
      "right click",
      Popover::new("context-popover")
        .anchor(Anchor::BottomLeft)
        .mouse_button(gpui::MouseButton::Right)
        .trigger_with(move |_, _, _| {
          div()
            .id("context-target")
            .px(rems(3.))
            .py(rems(1.5))
            .rounded(target_radius)
            .border_1()
            .border_color(target_border)
            .child("right click this box")
            .into_any_element()
        })
        .content(|_, _, _| {
          v_flex()
            .gap_1()
            .child(Button::new("context-copy").label("copy").flat())
        }),
    ))
}

fn tooltips(theme: &Theme) -> impl IntoElement {
  let save = Keystroke::parse("ctrl-s").expect("valid keystroke");
  v_flex()
    .gap_3()
    .child(labeled(
      theme,
      "text",
      Button::new("tooltip-save")
        .label("save")
        .tooltip(|window, cx| Tooltip::new("save the current file").build(window, cx)),
    ))
    .child(labeled(
      theme,
      "key hint",
      Button::new("tooltip-kbd")
        .label("export")
        .tooltip(move |window, cx| {
          Tooltip::new("export")
            .key_binding(Some(Kbd::new(save.clone())))
            .build(window, cx)
        }),
    ))
    .child(labeled(
      theme,
      "custom element",
      Button::new("tooltip-rich")
        .label("share")
        .tooltip(|window, cx| {
          Tooltip::element(|_, _| h_flex().gap_1().child("shared with").child("3 people"))
            .build(window, cx)
        }),
    ))
}

fn dialogs(
  gallery: &Entity<OverlaysGallery>, decision: Option<&'static str>, theme: &Theme,
) -> impl IntoElement {
  let opener = gallery.clone();
  let decision = decision.unwrap_or("no decision yet");
  v_flex()
    .gap_3()
    .child(labeled(
      theme,
      "handle driven · escape cancels, enter confirms",
      Button::new("open-dialog")
        .label("open dialog")
        .primary()
        .on_click(move |_, window, cx| {
          let handle = opener.read(cx).dialog_handle.clone();
          handle.open(window, cx);
        }),
    ))
    .child(labeled(
      theme,
      "stack driven · layers pile up, only the topmost dims",
      Button::new("open-stacked-dialog")
        .label("stack a dialog")
        .outline(true)
        .on_click(move |_, window, cx| {
          window.open_dialog(cx, |dialog, _, _| {
            dialog
              .child(DialogTitle::new().child("layered"))
              .child(
                DialogDescription::new()
                  .child("escape closes only this layer; focus returns underneath."),
              )
              .child(
                h_flex().justify_end().p(rems(0.25)).child(
                  Button::new("stack-another")
                    .label("stack another")
                    .outline(true)
                    .on_click(|_, window, cx| {
                      window.open_alert_dialog(cx, |alert, _, _| {
                        alert
                          .child(DialogTitle::new().child("topmost"))
                          .child(
                            DialogDescription::new()
                              .child("an alert over a dialog; only this layer answers."),
                          )
                          .child(
                            h_flex()
                              .justify_end()
                              .p(rems(0.25))
                              .child(AlertDialogAction::new().child("done")),
                          )
                      });
                    }),
                ),
              )
          });
        }),
    ))
    .child(labeled(theme, "decision readout", readout(theme, decision)))
}

fn alerts(
  gallery: &Entity<OverlaysGallery>, decision: Option<&'static str>, theme: &Theme,
) -> impl IntoElement {
  let opener = gallery.clone();
  let decision = decision.unwrap_or("no decision yet");
  v_flex()
    .gap_3()
    .child(labeled(
      theme,
      "backdrop press disabled · requires an explicit decision",
      Button::new("open-alert")
        .label("delete branch…")
        .danger()
        .on_click(move |_, _, cx| {
          opener.update(cx, |gallery, _| gallery.alert_open = true);
        }),
    ))
    .child(labeled(theme, "decision readout", readout(theme, decision)))
}

fn toasts(gallery: &Entity<OverlaysGallery>, theme: &Theme) -> impl IntoElement {
  let default_push = gallery.clone();
  let success_push = gallery.clone();
  let warning_push = gallery.clone();
  let danger_push = gallery.clone();
  let info_push = gallery.clone();
  let action_push = gallery.clone();
  let long_push = gallery.clone();

  v_flex()
    .gap_3()
    .child(labeled(
      theme,
      "push · auto-hide after 5s; hover or focus the stack to pause",
      h_flex()
        .flex_wrap()
        .gap_2()
        .child(
          Button::new("push-default")
            .label("default")
            .on_click(move |_, _, cx| {
              default_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Default,
                  title: "workspace synced",
                  description: None,
                  action: false,
                  pushed_at: None,
                  timeout: Some(Duration::from_secs(5)),
                });
              });
            }),
        )
        .child(
          Button::new("push-success")
            .label("success")
            .success()
            .on_click(move |_, _, cx| {
              success_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Success,
                  title: "project saved",
                  description: Some("all changes written to disk"),
                  action: false,
                  pushed_at: None,
                  timeout: Some(Duration::from_secs(5)),
                });
              });
            }),
        )
        .child(
          Button::new("push-warning")
            .label("warning")
            .warning()
            .on_click(move |_, _, cx| {
              warning_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Warning,
                  title: "deprecated dependency",
                  description: Some("left-pad 1.3.0 is deprecated"),
                  action: false,
                  pushed_at: None,
                  timeout: Some(Duration::from_secs(5)),
                });
              });
            }),
        )
        .child(
          Button::new("push-danger")
            .label("danger")
            .danger()
            .on_click(move |_, _, cx| {
              danger_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Danger,
                  title: "build failed",
                  description: Some("2 errors in main.rs"),
                  action: false,
                  pushed_at: None,
                  timeout: Some(Duration::from_secs(5)),
                });
              });
            }),
        )
        .child(
          Button::new("push-info")
            .label("info · sticky")
            .info()
            .on_click(move |_, _, cx| {
              info_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Info,
                  title: "tip",
                  description: Some("hover the stack to pause timers"),
                  action: false,
                  pushed_at: None,
                  timeout: None,
                });
              });
            }),
        )
        .child(
          Button::new("push-action")
            .label("action")
            .outline(true)
            .on_click(move |_, _, cx| {
              action_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Default,
                  title: "file deleted",
                  description: Some("notes/draft.md moved to trash"),
                  action: true,
                  pushed_at: None,
                  timeout: Some(Duration::from_secs(5)),
                });
              });
            }),
        )
        .child(
          Button::new("push-long")
            .label("long copy")
            .outline(true)
            .on_click(move |_, _, cx| {
              long_push.update(cx, |gallery, _| {
                gallery.push_toast(Draft {
                  variant: ToastVariant::Info,
                  title: "a title that runs well past the width the stack gives the card",
                  description: Some(
                    "a description long enough to wrap past three lines of body copy, so the \
                     card clamps the block and lets the rest scroll: one, two, three, four, \
                     five, six, seven, eight, nine, ten, eleven, twelve",
                  ),
                  action: false,
                  pushed_at: None,
                  timeout: None,
                });
              });
            }),
        ),
    ))
    .child(labeled(
      theme,
      "the stack renders top-right, layered with peek and expansion motion",
      readout(theme, "push a few toasts to see the stack reflow"),
    ))
}
