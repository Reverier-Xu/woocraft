use std::rc::Rc;

use gpui::{
  AnyElement, App, ClickEvent, Context, Decorations, InteractiveElement as _, IntoElement,
  MouseButton, ParentElement, Render, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, TitlebarOptions, Window, WindowControlArea, div,
  prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, DropdownMenu as _, Icon, IconLabel, IconName, PopupMenu,
  PopupMenuItem, Sizable as _, Size, StyleSized, StyledExt, Theme, ThemeMode, available_locales,
  h_flex, locale, locale_display_name, set_locale, translate,
};

type CloseWindowHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ToolbarButtonHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type TitleMenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

const TITLE_BAR_SIZE: Size = Size::Medium;

#[derive(IntoElement)]
pub struct TitleBar {
  style: StyleRefinement,
  children: Vec<AnyElement>,
  title: Option<SharedString>,
  icon: Option<Icon>,
  app_menu_bar_slot: Option<AnyElement>,
  title_menu_builder: Option<TitleMenuBuilder>,
  theme_button_enabled: bool,
  language_button_enabled: bool,
  on_theme_button_click: Option<ToolbarButtonHandler>,
  on_language_button_click: Option<ToolbarButtonHandler>,
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
      title_menu_builder: None,
      theme_button_enabled: false,
      language_button_enabled: false,
      on_theme_button_click: None,
      on_language_button_click: None,
      on_close_window: None,
    }
  }

  pub fn title(mut self, title: impl Into<SharedString>) -> Self {
    self.title = Some(title.into());
    self
  }

  pub fn icon(mut self, icon: Icon) -> Self {
    self.icon = Some(icon);
    self
  }

  pub fn app_menu_bar(mut self, app_menu_bar: impl IntoElement) -> Self {
    self.app_menu_bar_slot = Some(app_menu_bar.into_any_element());
    self
  }

  pub fn title_menu(
    mut self,
    builder: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
  ) -> Self {
    self.title_menu_builder = Some(Rc::new(builder));
    self
  }

  pub fn theme_button(mut self, enabled: bool) -> Self {
    self.theme_button_enabled = enabled;
    self
  }

  pub fn language_button(mut self, enabled: bool) -> Self {
    self.language_button_enabled = enabled;
    self
  }

  pub fn on_theme_button_click(
    mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_theme_button_click = Some(Rc::new(f));
    self
  }

  pub fn on_language_button_click(
    mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
  ) -> Self {
    self.on_language_button_click = Some(Rc::new(f));
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

impl_styled!(TitleBar);

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
    let Self {
      style,
      children,
      title,
      icon,
      app_menu_bar_slot,
      title_menu_builder,
      theme_button_enabled,
      language_button_enabled,
      on_theme_button_click,
      on_language_button_click,
      on_close_window,
    } = self;
    let decorations = window.window_decorations();
    let is_client_decorated = matches!(decorations, Decorations::Client { .. });
    let is_linux = cfg!(target_os = "linux");
    let is_macos = cfg!(target_os = "macos");
    let window_radius = cx.theme().radius_container;
    let title = title.unwrap_or_else(|| {
      let window_title = window.window_title();
      if window_title.is_empty() {
        translate("title_bar.untitled").into()
      } else {
        window_title.into()
      }
    });
    let icon = icon.unwrap_or_else(|| Icon::new(IconName::Apps));
    let theme_icon = if cx.theme().mode.is_dark() {
      IconName::WeatherSunny
    } else {
      IconName::WeatherMoon
    };
    let title_display = if let Some(title_menu_builder) = title_menu_builder {
      Button::new("title-bar-label-menu")
        .flat()
        .medium()
        .icon(icon)
        .label(title.clone())
        .dropdown_menu(move |menu, window, cx| title_menu_builder(menu, window, cx))
        .into_any_element()
    } else {
      IconLabel::new("title-bar-label")
        .icon(icon)
        .label(title.clone())
        .into_any_element()
    };

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
        .refine_style(&style)
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
            .when(language_button_enabled, |this| {
              let current_locale = locale().to_string();
              let on_language_button_click = on_language_button_click.clone();
              this.child(
                Button::new("title-bar-language")
                  .flat()
                  .medium()
                  .icon(Icon::new(IconName::Translate))
                  .dropdown_menu(move |menu, _, _| {
                    let mut menu = menu;
                    for locale_name in available_locales() {
                      let is_selected = current_locale == locale_name
                        || current_locale.starts_with(&(locale_name.clone() + "-"));
                      let label = locale_display_name(&locale_name);
                      let on_language_button_click = on_language_button_click.clone();
                      menu = menu.item(PopupMenuItem::new(label).checked(is_selected).on_click(
                        move |event, window, cx| {
                          set_locale(&locale_name);
                          if let Some(handler) = on_language_button_click.as_ref() {
                            handler(event, window, cx);
                          }
                          window.refresh();
                        },
                      ));
                    }
                    menu
                  }),
              )
            })
            .when(theme_button_enabled, |this| {
              this.child(
                Button::new("title-bar-theme")
                  .flat()
                  .medium()
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
