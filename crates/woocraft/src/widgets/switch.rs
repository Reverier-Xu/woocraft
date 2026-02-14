use std::rc::Rc;

use gpui::{
  App, ClickEvent, ElementId, InteractiveElement as _, IntoElement, ParentElement, RenderOnce,
  SharedString, StatefulInteractiveElement as _, StyleRefinement, Styled, Window, div,
  prelude::FluentBuilder as _,
};

use crate::{ActiveTheme, Disableable, Sizable, Size, StyleSized, StyledExt, h_flex};

#[derive(IntoElement)]
pub struct Switch {
  id: ElementId,
  style: StyleRefinement,
  checked: bool,
  disabled: bool,
  label: Option<SharedString>,
  on_click: Option<Rc<dyn Fn(&bool, &mut Window, &mut App)>>,
  size: Size,
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
  fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
    let track_h = self.size.component_height();
    let track_w = match self.size {
      Size::Small => gpui::px(32.0),
      Size::Medium => gpui::px(40.0),
      Size::Large => gpui::px(48.0),
    };
    let thumb_size = gpui::px(16.0);
    let thumb_offset = gpui::px(8.0);
    let track_thickness = gpui::px(2.0);
    let track_radius = self.size.component_radius();

    let track_bg = cx.theme().muted;
    let track_active = cx.theme().primary;
    let thumb_bg = cx.theme().background;

    let max_x = track_w - thumb_size;
    let thumb_x = if self.checked { max_x } else { gpui::px(0.0) };
    let filled_w = if self.checked {
      thumb_x + thumb_offset
    } else {
      gpui::px(0.0)
    };

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
              .bg(if self.disabled { track_bg.opacity(0.6) } else { track_bg }),
          )
          .child(
            div()
              .absolute()
              .left_0()
              .top((track_h - track_thickness) / 2.0)
              .h(track_thickness)
              .rounded_full()
              .bg(if self.disabled { track_active.opacity(0.6) } else { track_active })
              .w(filled_w),
          )
          .child(
            div()
              .absolute()
              .left(thumb_x)
              .top((track_h - thumb_size) / 2.0)
              .size(thumb_size)
              .rounded_full()
              .border_2()
              .border_color(if self.disabled {
                track_active.opacity(0.6)
              } else {
                track_active
              })
              .bg(if self.disabled { thumb_bg.opacity(0.7) } else { thumb_bg }),
          )
          .when(!self.disabled, |this| {
            this.cursor_pointer().on_click({
              let on_click = self.on_click.clone();
              move |_: &ClickEvent, window, cx| {
                if let Some(on_click) = on_click.as_ref() {
                  on_click(&!self.checked, window, cx);
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
