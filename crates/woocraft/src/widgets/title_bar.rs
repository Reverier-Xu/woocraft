use std::{
  rc::Rc,
  sync::{LazyLock, RwLock},
};

use gpui::{
  AnyElement, App, AppContext as _, ClickEvent, Context, Decorations, InteractiveElement as _,
  IntoElement, MouseButton, ParentElement, Pixels, Point, Render, RenderOnce, SharedString,
  StatefulInteractiveElement as _, StyleRefinement, Styled, TitlebarOptions, Window,
  WindowControlArea, div, point, prelude::FluentBuilder as _, px,
};

use crate::{
  ActiveTheme, Button, ButtonVariants, DropdownMenu as _, Icon, IconLabel, IconName, PopupMenu,
  PopupMenuItem, Sizable as _, Size, Slider, SliderEvent, SliderState, StyleSized, StyledExt,
  Theme, ThemeMode, available_locales, h_flex, locale, locale_display_name, set_locale,
  translate_woocraft,
};

type CloseWindowHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type ToolbarButtonHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type TitleMenuBuilder = Rc<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;
type ZoomChangeHandler = Rc<dyn Fn(f32, &mut Window, &mut App)>;

const TITLE_BAR_SIZE: Size = Size::Medium;

/// Interface zoom bounds, as multipliers of the base rem size (50%–200%).
const ZOOM_FACTOR_MIN: f32 = 0.5;
const ZOOM_FACTOR_MAX: f32 = 2.0;
/// Step of the zoom slider inside the title-bar zoom menu, in percent.
const ZOOM_SLIDER_STEP: f32 = 5.0;
/// Step of [`TitleBar::zoom_in`] and [`TitleBar::zoom_out`], in percent.
const ZOOM_STEP: f32 = 10.0;

/// App-wide interface zoom multiplier applied on top of the base rem size.
///
/// `1.0` keeps the UI at its base scale; the title-bar zoom menu slider moves
/// this value between `ZOOM_FACTOR_MIN` and `ZOOM_FACTOR_MAX`. Every window
/// hosting a [`TitleBar`] applies the multiplier on render, so a change takes
/// effect everywhere on the next refresh.
static ZOOM_FACTOR: LazyLock<RwLock<f32>> = LazyLock::new(|| RwLock::new(1.0));

/// App-wide rem-size override for apps that configure the interface scale
/// programmatically (e.g. restored from persisted settings).
///
/// `None` (the default) lets each title bar fall back to its own
/// [`TitleBar::rem_size`] argument or the platform default (see
/// [`TitleBar::default_rem_size`]).
static REM_SIZE_OVERRIDE: LazyLock<RwLock<Option<Pixels>>> = LazyLock::new(|| RwLock::new(None));

fn zoom_factor_locked() -> f32 {
  *ZOOM_FACTOR.read().expect("zoom factor lock poisoned")
}

fn rem_size_override_locked() -> Option<Pixels> {
  *REM_SIZE_OVERRIDE
    .read()
    .expect("rem size override lock poisoned")
}

/// Returns the zoom factor stepped towards `up` by [`ZOOM_STEP`], clamped to
/// the zoom bounds. Pure companion of [`TitleBar::zoom_in`] and
/// [`TitleBar::zoom_out`].
fn stepped_zoom_factor(factor: f32, up: bool) -> f32 {
  let delta = if up { ZOOM_STEP } else { -ZOOM_STEP } / 100.;
  (factor + delta).clamp(ZOOM_FACTOR_MIN, ZOOM_FACTOR_MAX)
}

/// Left padding reserved for the native macOS traffic-light buttons
/// (close / minimize / zoom) so the title-bar content never overlaps them.
const TRAFFIC_LIGHT_PADDING: f32 = 64.0;
const TRAFFIC_LIGHT_MARGIN: f32 = 0.8;

fn traffic_light_position(rem_size: Pixels) -> Point<Pixels> {
  point(
    TITLE_BAR_SIZE.em(TRAFFIC_LIGHT_MARGIN).to_pixels(rem_size),
    TITLE_BAR_SIZE.em(TRAFFIC_LIGHT_MARGIN).to_pixels(rem_size),
  )
}

/// Title bar with optional trailing toolbar buttons (language, interface
/// zoom, theme) and cross-platform window controls.
///
/// # Interface scaling (zoom)
///
/// The title bar is the owner of the interface scale for its window. On every
/// render it resolves a base rem size — an explicit [`TitleBar::rem_size`]
/// argument, then the app-wide [`TitleBar::set_rem_size_override`], then the
/// platform default ([`TitleBar::default_rem_size`]: 13px on macOS, 16px on
/// Windows and Linux) — multiplies it by the current zoom multiplier and
/// applies the result via `window.set_rem_size`, so the whole UI scales like
/// zooming a web page. Windows without a title bar keep whatever rem size
/// the app set itself.
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
  zoom_button_enabled: bool,
  rem_size: Option<Pixels>,
  on_theme_button_click: Option<ToolbarButtonHandler>,
  on_language_button_click: Option<ToolbarButtonHandler>,
  on_zoom_change: Option<ZoomChangeHandler>,
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
      zoom_button_enabled: false,
      rem_size: None,
      on_theme_button_click: None,
      on_language_button_click: None,
      on_zoom_change: None,
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

  /// Enables the interface-zoom button in the title bar (disabled by
  /// default).
  ///
  /// The button opens a dropdown menu with a zoom slider (50%–200%, live
  /// preview while dragging) and a reset action, implemented on top of the
  /// window rem size (see the [`TitleBar`] type-level docs on interface
  /// scaling).
  pub fn zoom_button(mut self, enabled: bool) -> Self {
    self.zoom_button_enabled = enabled;
    self
  }

  /// Sets the base rem size for the window hosting this title bar.
  ///
  /// The base is the rem size at zoom level 100%; the interface zoom
  /// multiplier is applied on top of it. When unset, the app-wide override
  /// (see [`TitleBar::set_rem_size_override`]) or the platform default (see
  /// [`TitleBar::default_rem_size`]) is used as a fallback.
  pub fn rem_size(mut self, rem_size: impl Into<Pixels>) -> Self {
    self.rem_size = Some(rem_size.into());
    self
  }

  /// Registers a handler invoked whenever the zoom level changes through the
  /// title-bar zoom menu, e.g. to persist the chosen level.
  ///
  /// The handler receives the new zoom multiplier (1.0 = 100%).
  pub fn on_zoom_change(mut self, f: impl Fn(f32, &mut Window, &mut App) + 'static) -> Self {
    self.on_zoom_change = Some(Rc::new(f));
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

  /// The platform default rem size used when neither the title bar nor the
  /// app provides one: 13px on macOS (matching the system base text size)
  /// and 16px on Windows and Linux (the CSS default).
  pub fn default_rem_size() -> Pixels {
    if cfg!(target_os = "macos") {
      px(13.)
    } else {
      px(16.)
    }
  }

  /// Returns the app-wide interface zoom multiplier (1.0 by default).
  pub fn zoom_factor() -> f32 {
    zoom_factor_locked()
  }

  /// Returns the app-wide rem-size override, if the app set one.
  pub fn rem_size_override() -> Option<Pixels> {
    rem_size_override_locked()
  }

  /// Replaces the app-wide rem-size override (the base the zoom multiplier
  /// is applied to). Passing `None` restores the platform default fallback.
  pub fn set_rem_size_override(rem_size: Option<Pixels>) {
    *REM_SIZE_OVERRIDE
      .write()
      .expect("rem size override lock poisoned") = rem_size;
  }

  /// Stores the interface zoom multiplier without touching any window.
  ///
  /// The factor is clamped to a safe range; every window hosting a
  /// [`TitleBar`] picks the new value up on its next render. Use this at
  /// startup to restore a persisted zoom level before windows are drawn; use
  /// [`TitleBar::zoom_in`], [`TitleBar::zoom_out`] or
  /// [`TitleBar::reset_zoom`] to zoom a live window.
  pub fn set_zoom_factor(factor: f32) {
    *ZOOM_FACTOR.write().expect("zoom factor lock poisoned") =
      factor.clamp(ZOOM_FACTOR_MIN, ZOOM_FACTOR_MAX);
  }

  /// Zooms the given window one step in (towards larger UI) and refreshes
  /// every window.
  pub fn zoom_in(window: &mut Window, cx: &mut App) {
    let factor = stepped_zoom_factor(zoom_factor_locked(), true);
    Self::apply_zoom_factor(factor, window, cx);
  }

  /// Zooms the given window one step out (towards smaller UI) and refreshes
  /// every window.
  pub fn zoom_out(window: &mut Window, cx: &mut App) {
    let factor = stepped_zoom_factor(zoom_factor_locked(), false);
    Self::apply_zoom_factor(factor, window, cx);
  }

  /// Resets the interface zoom to 100% and refreshes every window.
  pub fn reset_zoom(window: &mut Window, cx: &mut App) {
    Self::apply_zoom_factor(1.0, window, cx);
  }

  /// Stores the zoom multiplier and rescales the given window relative to
  /// its current zoom, then refreshes every window so all title bars pick
  /// up the new multiplier. No-op when the factor is unchanged (the slider
  /// emits a change event for every drag tick).
  fn apply_zoom_factor(factor: f32, window: &mut Window, cx: &mut App) {
    let previous = zoom_factor_locked();
    let factor = factor.clamp(ZOOM_FACTOR_MIN, ZOOM_FACTOR_MAX);
    *ZOOM_FACTOR.write().expect("zoom factor lock poisoned") = factor;
    window.set_rem_size(window.rem_size() * (factor / previous));
    cx.refresh_windows();
  }

  pub fn title_bar_options() -> TitlebarOptions {
    TitlebarOptions {
      title: None,
      appears_transparent: true,
      traffic_light_position: Some(traffic_light_position(Self::default_rem_size())),
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

/// Drag-gesture flag for the title bar.
///
/// This deliberately stays an `Entity` instead of an `Rc<Cell<bool>>`
/// recreated on every render: the flag is set on mouse-down and consumed on a
/// later mouse-move, and a re-render between those events (hover, animation,
/// any `cx.notify()`) would reset a per-render value and silently cancel an
/// in-progress window drag.
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
      zoom_button_enabled,
      rem_size,
      on_theme_button_click,
      on_language_button_click,
      on_zoom_change,
      on_close_window,
    } = self;
    // Interface-scaling fallback: resolve the base rem size (explicit title
    // bar value, then the app-wide override, then the platform default:
    // 13px on macOS, 16px elsewhere) and apply it scaled by the current zoom
    // multiplier. This must happen before anything below reads
    // `window.rem_size()` (e.g. the macOS traffic-light position).
    let base_rem_size = rem_size
      .or_else(Self::rem_size_override)
      .unwrap_or_else(Self::default_rem_size);
    window.set_rem_size(base_rem_size * zoom_factor_locked());
    let decorations = window.window_decorations();
    let is_client_decorated = matches!(decorations, Decorations::Client { .. });
    let is_linux = cfg!(target_os = "linux");
    let is_macos = cfg!(target_os = "macos");
    #[cfg(target_os = "macos")]
    if is_macos {
      window.set_traffic_light_position(traffic_light_position(window.rem_size()));
    }
    let window_radius = cx.theme().radius_container;
    let title = title.unwrap_or_else(|| {
      let window_title = window.window_title();
      if window_title.is_empty() {
        translate_woocraft("title_bar.untitled").into()
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
            .when(
              is_macos && !window.is_fullscreen() && !window.is_simple_fullscreen(),
              |this| {
                this.pl(
                  px(TRAFFIC_LIGHT_PADDING)
                    + TITLE_BAR_SIZE
                      .em(TRAFFIC_LIGHT_MARGIN * 2.0)
                      .to_pixels(window.rem_size()),
                )
              },
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
            .when(zoom_button_enabled, |this| {
              let on_zoom_change = on_zoom_change.clone();
              this.child(
                Button::new("title-bar-zoom")
                  .flat()
                  .medium()
                  .icon(Icon::new(IconName::ZoomIn))
                  .dropdown_menu(move |menu, window, cx| {
                    let menu = menu;
                    let menu_handle = cx.entity();

                    // The slider state lives as long as the open menu (the
                    // popup-menu host caches the built menu entity), and is
                    // recreated from the current zoom every time the dropdown
                    // is (re)opened.
                    let slider_state = cx.new(|_| {
                      SliderState::new()
                        .min(ZOOM_FACTOR_MIN * 100.)
                        .max(ZOOM_FACTOR_MAX * 100.)
                        .step(ZOOM_SLIDER_STEP)
                        .default_value(zoom_factor_locked() * 100.)
                    });

                    window
                      .subscribe(&slider_state, cx, {
                        let on_zoom_change = on_zoom_change.clone();
                        move |_, event: &SliderEvent, window, cx| {
                          let SliderEvent::Change(value) = event;
                          let factor = value.end() / 100.;
                          TitleBar::apply_zoom_factor(factor, window, cx);
                          if let Some(handler) = on_zoom_change.as_ref() {
                            handler(factor, window, cx);
                          }
                          // Re-render the menu so the percentage readout
                          // follows the dragged thumb.
                          menu_handle.update(cx, |_, cx| cx.notify());
                        }
                      })
                      .detach();
                    menu.item(PopupMenuItem::element({
                      let slider_state = slider_state.clone();
                      move |_, cx| {
                        let percent = slider_state.read(cx).value().end();
                        h_flex()
                          .id("title-bar-zoom-slider-row")
                          .items_center()
                          .gap_2()
                          .w(TITLE_BAR_SIZE.em(20.0))
                          // Interactive menu rows confirm (and dismiss) the
                          // menu on click; swallow the gesture so the slider
                          // can be dragged without closing the dropdown.
                          .on_mouse_down(MouseButton::Left, |_, _, cx| {
                            cx.stop_propagation();
                          })
                          .on_click(|_, _, cx| cx.stop_propagation())
                          .child(Slider::new("title-bar-zoom-slider", &slider_state))
                          .child(div().flex_shrink_0().w_10().child(format!("{percent:.0}%")))
                      }
                    }))
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
                      window.refresh();
                    } else {
                      let next = if cx.theme().mode.is_dark() {
                        ThemeMode::Light
                      } else {
                        ThemeMode::Dark
                      };
                      // Theme::set_mode already calls cx.refresh_windows();
                      // an extra window.refresh() here would just trigger a
                      // second full redraw of the same frame.
                      Theme::set_mode(next, cx);
                    }
                  }),
              )
            })
            .child(WindowControls { on_close_window }),
        ),
    )
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_rem_size_follows_platform_convention() {
    let expected = if cfg!(target_os = "macos") {
      px(13.)
    } else {
      px(16.)
    };
    assert_eq!(TitleBar::default_rem_size(), expected);
  }

  #[test]
  fn stepped_zoom_factor_steps_and_clamps() {
    assert_eq!(stepped_zoom_factor(1.0, true), 1.1);
    assert_eq!(stepped_zoom_factor(1.0, false), 0.9);
    assert_eq!(stepped_zoom_factor(ZOOM_FACTOR_MAX, true), ZOOM_FACTOR_MAX);
    assert_eq!(stepped_zoom_factor(ZOOM_FACTOR_MIN, false), ZOOM_FACTOR_MIN);
  }

  #[test]
  fn set_zoom_factor_clamps_to_safe_bounds() {
    let saved = TitleBar::zoom_factor();
    TitleBar::set_zoom_factor(9.0);
    assert_eq!(TitleBar::zoom_factor(), ZOOM_FACTOR_MAX);
    TitleBar::set_zoom_factor(0.0);
    assert_eq!(TitleBar::zoom_factor(), ZOOM_FACTOR_MIN);
    TitleBar::set_zoom_factor(1.25);
    assert_eq!(TitleBar::zoom_factor(), 1.25);
    TitleBar::set_zoom_factor(saved);
  }

  #[test]
  fn rem_size_override_roundtrips() {
    let saved = TitleBar::rem_size_override();
    TitleBar::set_rem_size_override(Some(px(18.)));
    assert_eq!(TitleBar::rem_size_override(), Some(px(18.)));
    TitleBar::set_rem_size_override(None);
    assert_eq!(TitleBar::rem_size_override(), None);
    TitleBar::set_rem_size_override(saved);
  }
}
