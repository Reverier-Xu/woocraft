use std::{rc::Rc, time::Duration};

use gpui::{
  Animation, AnimationExt as _, AnyElement, App, ClickEvent, ElementId, InteractiveElement as _,
  IntoElement, ParentElement, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, Window, div, prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, Disableable, Sizable, Size, StyleSized, StyledExt, h_flex};

type SwitchClickHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum SwitchVariant {
  #[default]
  Primary,
  Success,
  Warning,
  Danger,
}

pub trait SwitchVariants: Sized {
  fn with_variant(self, variant: SwitchVariant) -> Self;

  fn primary(self) -> Self {
    self.with_variant(SwitchVariant::Primary)
  }

  fn success(self) -> Self {
    self.with_variant(SwitchVariant::Success)
  }

  fn warning(self) -> Self {
    self.with_variant(SwitchVariant::Warning)
  }

  fn danger(self) -> Self {
    self.with_variant(SwitchVariant::Danger)
  }
}

#[derive(IntoElement)]
pub struct Switch {
  id: ElementId,
  style: StyleRefinement,
  checked: bool,
  disabled: bool,
  label: Option<SharedString>,
  on_click: Option<SwitchClickHandler>,
  size: Size,
  variant: SwitchVariant,
}

impl Switch {
  pub fn new(id: impl Into<ElementId>) -> Self {
    Self {
      id: id.into(),
      style: StyleRefinement::default(),
      checked: false,
      disabled: false,
      label: None,
      on_click: None,
      size: Size::Medium,
      variant: SwitchVariant::Primary,
    }
  }

  pub fn checked(mut self, checked: bool) -> Self {
    self.checked = checked;
    self
  }

  pub fn label(mut self, label: impl Into<SharedString>) -> Self {
    self.label = Some(label.into());
    self
  }

  pub fn on_click<F>(mut self, handler: F) -> Self
  where
    F: Fn(&bool, &mut Window, &mut App) + 'static,
  {
    self.on_click = Some(Rc::new(handler));
    self
  }
}

impl SwitchVariants for Switch {
  fn with_variant(mut self, variant: SwitchVariant) -> Self {
    self.variant = variant;
    self
  }
}

impl Styled for Switch {
  fn style(&mut self) -> &mut StyleRefinement {
    &mut self.style
  }
}

impl Sizable for Switch {
  fn with_size(mut self, size: impl Into<Size>) -> Self {
    self.size = size.into();
    self
  }
}

impl Disableable for Switch {
  fn disabled(mut self, disabled: bool) -> Self {
    self.disabled = disabled;
    self
  }
}

impl RenderOnce for Switch {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let checked = self.checked;
    let toggle_state = window.use_keyed_state(self.id.clone(), cx, |_, _| checked);
    let prev_checked = *toggle_state.read(cx);
    let should_animate = !self.disabled && prev_checked != checked;
    let animation_duration = Duration::from_secs_f64(0.15);

    if should_animate {
      cx.spawn({
        let toggle_state = toggle_state.clone();
        async move |cx| {
          cx.background_executor().timer(animation_duration).await;
          _ = toggle_state.update(cx, |state, _| *state = checked);
        }
      })
      .detach();
    }

    let track_h = self.size.component_height();
    let track_w = match self.size {
      Size::Small => gpui::px(24.0),
      Size::Medium => gpui::px(28.0),
      Size::Large => gpui::px(32.0),
    };
    let thumb_size = gpui::px(16.0);
    let thumb_offset = gpui::px(8.0);
    let track_thickness = gpui::px(2.0);
    let track_radius = self.size.component_radius();

    let track_bg = cx.theme().muted;
    let active_color = match self.variant {
      SwitchVariant::Primary => cx.theme().primary,
      SwitchVariant::Success => cx.theme().success,
      SwitchVariant::Warning => cx.theme().warning,
      SwitchVariant::Danger => cx.theme().danger,
    };

    let thumb_border_color = if checked {
      active_color
    } else {
      cx.theme().muted_foreground.opacity(0.7)
    };

    let thumb_bg = if checked {
      cx.theme().background
    } else {
      cx.theme().muted.opacity(0.85)
    };

    let max_x = track_w - thumb_size;
    let thumb_x = if checked { max_x } else { gpui::px(0.0) };
    let filled_w = if checked {
      thumb_x + thumb_offset
    } else {
      gpui::px(0.0)
    };

    let filled_track: AnyElement = div()
      .absolute()
      .left_0()
      .top((track_h - track_thickness) / 2.0)
      .h(track_thickness)
      .rounded_full()
      .bg(if self.disabled {
        active_color.opacity(0.6)
      } else {
        active_color
      })
      .map(|this| {
        if should_animate {
          let total_w = max_x + thumb_offset;
          this
            .with_animation(
              ElementId::NamedInteger("switch-fill".into(), checked as u64),
              Animation::new(animation_duration).with_easing(gpui::ease_out_quint()),
              move |this, delta| {
                let width = if checked {
                  total_w * delta
                } else {
                  total_w - total_w * delta
                };
                this.w(width)
              },
            )
            .into_any_element()
        } else {
          this.w(filled_w).into_any_element()
        }
      });

    let thumb: AnyElement = div()
      .absolute()
      .top((track_h - thumb_size) / 2.0)
      .size(thumb_size)
      .rounded_full()
      .border_2()
      .border_color(if self.disabled {
        thumb_border_color.opacity(0.35)
      } else {
        thumb_border_color
      })
      .bg(thumb_bg)
      .when(self.disabled, |this| this.opacity(0.7))
      .map(|this| {
        if should_animate {
          this
            .with_animation(
              ElementId::NamedInteger("switch-thumb".into(), checked as u64),
              Animation::new(animation_duration),
              move |this, delta| {
                let x = if checked {
                  max_x * delta
                } else {
                  max_x - max_x * delta
                };
                this.left(x)
              },
            )
            .into_any_element()
        } else {
          this.left(thumb_x).into_any_element()
        }
      });

    h_flex()
      .id(self.id.clone())
      .h(track_h)
      .items_center()
      .gap_2()
      .child(
        div()
          .id((self.id.clone(), "track"))
          .relative()
          .w(track_w)
          .h(track_h)
          .rounded(track_radius)
          .child(
            div()
              .absolute()
              .left_0()
              .right_0()
              .top((track_h - track_thickness) / 2.0)
              .h(track_thickness)
              .rounded_full()
              .bg(if self.disabled {
                track_bg.opacity(0.6)
              } else {
                track_bg
              }),
          )
          .child(filled_track)
          .child(thumb)
          .when(!self.disabled, |this| {
            this.cursor_pointer().on_click({
              let on_click = self.on_click.clone();
              move |_: &ClickEvent, window, cx| {
                if let Some(on_click) = on_click.as_ref() {
                  on_click(&!checked, window, cx);
                }
              }
            })
          }),
      )
      .when_some(self.label, |this, label| {
        this.child(
          div()
            .input_text_size(self.size)
            .text_color(if self.disabled {
              cx.theme().muted_foreground
            } else {
              cx.theme().foreground
            })
            .child(label),
        )
      })
      .refine_style(&self.style)
  }
}
