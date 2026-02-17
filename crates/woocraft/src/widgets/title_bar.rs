use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, Context, Decorations, InteractiveElement as _, IntoElement,
  MouseButton, ParentElement, Render, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, TitlebarOptions, Window, WindowControlArea, div,
  prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, IconLabel, IconName, Size, StyleSized, StyledExt, h_flex,
};

type CloseWindowHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

const TITLE_BAR_SIZE: Size = Size::Medium;

#[derive(IntoElement)]
pub struct TitleBar {
  style: StyleRefinement,
  children: Vec<AnyElement>,
  title: Option<SharedString>,
  icon: Option<IconName>,
  on_close_window: Option<CloseWindowHandler>,
}

impl TitleBar {
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      children: Vec::new(),
      title: None,
      icon: None,
      on_close_window: None,
    }
  }

  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  pub fn icon(mut self, icon: IconName) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn title_bar_options() -> TitlebarOptions {
    TitlebarOptions {
      title: None,
      appears_transparent: true,
      traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
    }
  }

  pub fn on_close_window(
    mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    if cfg!(target_os = "linux") {
      self.on_close_window = Some(Rc::new(f));
    }
    self
  }
}

impl Default for TitleBar {
  fn default() -> Self {
    Self::new()
  }
}

#[derive(IntoElement)]
struct WindowControls {
  on_close_window: Option<CloseWindowHandler>,
}

impl RenderOnce for WindowControls {
  fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
    let is_linux = cfg!(target_os = "linux");
    let is_windows = cfg!(target_os = "windows");

    if cfg!(target_os = "macos") {
      return div().id("window-controls");
    }

    let minimize_button = div()
      .id("minimize")
      .flex()
      .flex_shrink_0()
      .justify_center()
      .content_center()
      .items_center()
      .when(is_windows, |this| {
        this.window_control_area(WindowControlArea::Min)
      })
      .when(is_linux, |this| {
        this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
          window.prevent_default();
          cx.stop_propagation();
        })
      })
      .child(
        Button::new("window-control-minimize")
          .flat()
          .icon(IconName::Subtract)
          .on_click(|_, window, cx| {
            cx.stop_propagation();
            window.minimize_window();
          }),
      );

    let maximize_button = div()
      .id("maximize")
      .flex()
      .flex_shrink_0()
      .justify_center()
      .content_center()
      .items_center()
      .when(is_windows, |this| {
        this.window_control_area(WindowControlArea::Max)
      })
      .when(is_linux, |this| {
        this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
          window.prevent_default();
          cx.stop_propagation();
        })
      })
      .child(
        Button::new("window-control-maximize")
          .flat()
          .icon(if window.is_maximized() {
            IconName::SquareMultiple
          } else {
            IconName::Maximize
          })
          .on_click(|_, window, cx| {
            cx.stop_propagation();
            window.zoom_window();
          }),
      );

    let on_close_window = self.on_close_window;
    let close_button = div()
      .id("close")
      .flex()
      .flex_shrink_0()
      .justify_center()
      .content_center()
      .items_center()
      .when(is_windows, |this| {
        this.window_control_area(WindowControlArea::Close)
      })
      .when(is_linux, |this| {
        this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
          window.prevent_default();
          cx.stop_propagation();
        })
      })
      .child(
        Button::new("window-control-close")
          .flat()
          .icon(IconName::Dismiss)
          .on_click(move |event, window, cx| {
            cx.stop_propagation();
            if let Some(f) = on_close_window.as_ref() {
              f(event, window, cx);
            } else {
              window.remove_window();
            }
          }),
      );

    h_flex()
      .id("window-controls")
      .items_center()
      .flex_shrink_0()
      .gap_1()
      .child(minimize_button)
      .child(maximize_button)
      .child(close_button)
  }
}

impl Styled for TitleBar {
  fn style(&mut self) -> &mut gpui::StyleRefinement {
    &mut self.style
  }
}

impl ParentElement for TitleBar {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

struct TitleBarState {
  should_move: bool,
}

impl Render for TitleBarState {
  fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
    div()
  }
}

impl RenderOnce for TitleBar {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let decorations = window.window_decorations();
    let is_client_decorated = matches!(decorations, Decorations::Client { .. });
    let is_linux = cfg!(target_os = "linux");
    let is_macos = cfg!(target_os = "macos");
    let window_radius = cx.theme().radius_container;
    let title = self.title.unwrap_or_else(|| {
      let window_title = window.window_title();
      if window_title.is_empty() {
        rust_i18n::t!("title_bar.untitled").into()
      } else {
        window_title.into()
      }
    });
    let icon = self.icon.unwrap_or(IconName::Apps);

    let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });

    div().flex_shrink_0().child(
      div()
        .id("title-bar")
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .container_size(TITLE_BAR_SIZE)
        .container_h(TITLE_BAR_SIZE)
        .border_color(cx.theme().border)
        .bg(cx.theme().card)
        .refine_style(&self.style)
        .map(|this| match decorations {
          Decorations::Server => this.rounded_tl(window_radius).rounded_tr(window_radius),
          Decorations::Client { tiling, .. } => this
            .when(!(tiling.top || tiling.left), |div| {
              div.rounded_tl(window_radius)
            })
            .when(!(tiling.top || tiling.right), |div| {
              div.rounded_tr(window_radius)
            }),
        })
        .when(is_linux, |this| {
          this.on_click(|event, window, _| {
            if event.click_count() == 2 {
              window.zoom_window();
            }
          })
        })
        .when(is_macos, |this| {
          this.on_click(|event, window, _| {
            if event.click_count() == 2 {
              window.titlebar_double_click();
            }
          })
        })
        .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
          state.should_move = false;
        }))
        .on_mouse_down(
          MouseButton::Left,
          window.listener_for(&state, |state, _, _, _| {
            state.should_move = true;
          }),
        )
        .on_mouse_up(
          MouseButton::Left,
          window.listener_for(&state, |state, _, _, _| {
            state.should_move = false;
          }),
        )
        .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
          if state.should_move {
            state.should_move = false;
            window.start_window_move();
          }
        }))
        .child(
          h_flex()
            .id("bar")
            .window_control_area(WindowControlArea::Drag)
            .when(window.is_fullscreen(), |this| this.pl_3())
            .h_full()
            .justify_start()
            .gap_2()
            .flex_shrink_0()
            .flex_1()
            .child(IconLabel::new("title-bar-label").icon(icon).label(title))
            .when(is_linux && is_client_decorated, |this| {
              this.child(
                div()
                  .top_0()
                  .left_0()
                  .absolute()
                  .size_full()
                  .h_full()
                  .on_mouse_down(MouseButton::Right, move |ev, window, _| {
                    window.show_window_menu(ev.position)
                  }),
              )
            })
            .children(self.children),
        )
        .child(WindowControls {
          on_close_window: self.on_close_window,
        }),
    )
  }
}
