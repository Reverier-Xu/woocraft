//! window title bar with client-side window controls.
//!
//! renders a draggable title bar that adapts to the platform decoration
//! model: on linux it draws its own minimize/maximize/close controls for
//! client-side-decorated windows, on windows it exposes `WindowControlArea`s
//! to the native hit-testing, and on macos it reserves space for the traffic
//! lights. the theme toggle defaults to flipping light/dark mode.

use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, Context, Decorations, InteractiveElement as _, IntoElement,
  MouseButton, ParentElement, Refineable as _, Render, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, TitlebarOptions, Window,
  WindowControlArea, div, prelude::FluentBuilder as _, px, rems,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, Icon, IconName, Theme, ThemeMode, base::h_flex,
  translate_woocraft,
};

type CloseWindowHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ToolbarButtonHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// Left padding reserved for the native macOS traffic-light buttons
/// (close / minimize / zoom) so the title-bar content never overlaps them.
const TRAFFIC_LIGHT_PADDING: f32 = 68.0;

/// the rendered height of [`TitleBar`]: `2rem` of control row inside `0.25rem`
/// of vertical padding. modal overlays default their backdrop dismissal
/// cutoff to this value so the title bar never doubles as a close button.
pub const TITLE_BAR_HEIGHT: gpui::Rems = rems(2.5);

#[derive(IntoElement)]
pub struct TitleBar {
  style: StyleRefinement,
  children: Vec<AnyElement>,
  title: Option<SharedString>,
  icon: Option<Icon>,
  app_menu_bar_slot: Option<AnyElement>,
  theme_button_enabled: bool,
  on_theme_button_click: Option<ToolbarButtonHandler>,
  on_close_window: Option<CloseWindowHandler>,
}

impl TitleBar {
  pub fn new() -> Self {
    Self {
      style: StyleRefinement::default(),
      children: Vec::new(),
      title: None,
      icon: None,
      app_menu_bar_slot: None,
      theme_button_enabled: false,
      on_theme_button_click: None,
      on_close_window: None,
    }
  }

  /// sets the displayed title. falls back to the window title, then to the
  /// localized "untitled" string.
  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  /// sets the leading app icon.
  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  /// slots an arbitrary element (e.g. an app menu bar) after the title.
  pub fn app_menu_bar(mut self, app_menu_bar: impl IntoElement) -> Self {
    self.app_menu_bar_slot = Some(app_menu_bar.into_any_element());
    self
  }

  /// shows the light/dark toggle in the actions area.
  pub fn theme_button(mut self, enabled: bool) -> Self {
    self.theme_button_enabled = enabled;
    self
  }

  /// overrides the theme toggle behavior.
  pub fn on_theme_button_click(
    mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_theme_button_click = Some(Rc::new(f));
    self
  }

  /// returns the window options that make [`TitleBar`] render correctly:
  /// a transparent native titlebar with macos traffic lights nudged inward.
  pub fn title_bar_options() -> TitlebarOptions {
    TitlebarOptions {
      title: None,
      appears_transparent: true,
      traffic_light_position: Some(gpui::point(px(9.0), px(13.0))),
    }
  }

  /// registers an extra close handler on linux, where the close button is
  /// drawn client-side. other platforms route close through the native
  /// controls and ignore this handler.
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
          .icon(Icon::new(IconName::Subtract))
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
          .icon(Icon::new(if window.is_maximized() {
            IconName::SquareMultiple
          } else {
            IconName::Maximize
          }))
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
          .icon(Icon::new(IconName::Dismiss))
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
  fn style(&mut self) -> &mut StyleRefinement {
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
    let style = self.style;
    let TitleBar {
      children,
      title,
      icon,
      app_menu_bar_slot,
      theme_button_enabled,
      on_theme_button_click,
      on_close_window,
      ..
    } = self;
    let decorations = window.window_decorations();
    let is_client_decorated = matches!(decorations, Decorations::Client { .. });
    let is_linux = cfg!(target_os = "linux");
    let is_macos = cfg!(target_os = "macos");
    let window_radius = cx.theme().radius_container;
    let title = title.unwrap_or_else(|| {
      let window_title = window.window_title();
      if window_title.is_empty() {
        translate_woocraft("title_bar.untitled").into()
      } else {
        window_title.into()
      }
    });
    let icon = icon.unwrap_or_else(|| Icon::new(IconName::AddCircle));
    let theme_icon = if cx.theme().mode.is_dark() {
      IconName::WeatherSunny
    } else {
      IconName::WeatherMoon
    };
    let title_display = Button::new("title-bar-label")
      .flat()
      .icon(icon)
      .label(title.clone());

    let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });

    let mut title_bar = div()
      .id("title-bar")
      .flex()
      .flex_row()
      .items_center()
      .justify_between()
      .p(rems(0.25))
      .bg(cx.theme().card)
      .font_family(cx.theme().font_family.clone());

    // caller refinements win over the defaults above.
    title_bar.style().refine(&style);

    div().flex_shrink_0().child(
      title_bar
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
            .when(
              is_macos && !window.is_fullscreen() && !window.is_simple_fullscreen(),
              |this| this.pl(px(TRAFFIC_LIGHT_PADDING)),
            )
            .h_full()
            .justify_start()
            .gap_2()
            .flex_shrink_0()
            .flex_1()
            .child(title_display)
            .when_some(app_menu_bar_slot, |this, app_menu_bar| {
              this.child(app_menu_bar)
            })
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
            .children(children),
        )
        .child(
          h_flex()
            .id("title-bar-actions")
            .items_center()
            .gap_1()
            .flex_shrink_0()
            .when(theme_button_enabled, |this| {
              this.child(
                Button::new("title-bar-theme")
                  .flat()
                  .icon(Icon::new(theme_icon))
                  .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    if let Some(handler) = on_theme_button_click.as_ref() {
                      handler(event, window, cx);
                    } else {
                      let next = if cx.theme().mode.is_dark() {
                        ThemeMode::Light
                      } else {
                        ThemeMode::Dark
                      };
                      Theme::set_mode(next, cx);
                    }
                    window.refresh();
                  }),
              )
            })
            .child(WindowControls { on_close_window }),
        ),
    )
  }
}
