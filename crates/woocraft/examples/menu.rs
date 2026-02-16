use gpui::{
  App, AppContext, Application, Bounds, Context, Entity, IntoElement, ParentElement, Render,
  Size as GpuiSize, Styled, Window, WindowBounds, WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ContextMenuExt as _, DropdownMenu as _, IconName, PopupMenuItem,
  StyledExt as _, ThemeMode, h_flex, v_flex, window_border,
};

#[derive(Default)]
struct MenuWindow {
  selected: String,
  checked: bool,
}

impl MenuWindow {
  fn view(cx: &mut App) -> Entity<Self> {
    cx.new(|_| Self {
      selected: "None".to_string(),
      checked: true,
    })
  }
}

impl Render for MenuWindow {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().clone();

    let menu_trigger = Button::new("menu-trigger").label("Open Menu").dropdown_menu(
      move |menu, _, _| {
        let view = view.clone();
        let view_refresh = view.clone();
        let view_pin = view.clone();
        menu
          .item(
            PopupMenuItem::new("Refresh")
              .icon(IconName::ArrowSync)
              .on_click(move |_, _, cx| {
                view_refresh.update(cx, |state, cx| {
                  state.selected = "Refresh".to_string();
                  cx.notify();
                });
              }),
          )
          .item(
            PopupMenuItem::new("Pinned")
              .checked(true)
              .on_click(move |_, _, cx| {
                view_pin.update(cx, |state, cx| {
                  state.selected = "Pinned".to_string();
                  state.checked = !state.checked;
                  cx.notify();
                });
              }),
          )
          .item(PopupMenuItem::new("Disabled Item").disabled(true))
          .separator()
          .item(PopupMenuItem::link("Open Project", "https://github.com/Reverier-Xu/woocraft"))
      },
    );

    let view_for_context = cx.entity().clone();
    let context_area = div()
      .w(px(360.))
      .h(px(180.))
      .rounded(cx.theme().radius_container)
      .border_1()
      .border_color(cx.theme().border)
      .bg(cx.theme().card)
      .text_color(cx.theme().card_foreground)
      .items_center()
      .justify_center()
      .child("Right click this area")
      .context_menu(move |menu, _, _| {
        let view = view_for_context.clone();
        let view_copy = view.clone();
        let view_inspect = view.clone();
        menu
          .item(PopupMenuItem::new("Copy").on_click(move |_, _, cx| {
            view_copy.update(cx, |state, cx| {
              state.selected = "Copy".to_string();
              cx.notify();
            });
          }))
          .item(PopupMenuItem::new("Inspect").on_click(move |_, _, cx| {
            view_inspect.update(cx, |state, cx| {
              state.selected = "Inspect".to_string();
              cx.notify();
            });
          }))
      });

    window_border().child(
      v_flex()
        .size_full()
        .p_6()
        .gap_4()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(div().text_xl().font_semibold().child("Woocraft Menu Example"))
        .child(
          h_flex()
            .gap_3()
            .child(menu_trigger)
            .child(
              Button::new("theme-toggle")
                .label("Toggle Theme")
                .on_click(|_, _, cx| {
                  let mode = if cx.theme().mode.is_dark() {
                    ThemeMode::Light
                  } else {
                    ThemeMode::Dark
                  };
                  woocraft::Theme::set_mode(mode, cx);
                }),
            ),
        )
        .child(
          div()
            .text_sm()
            .text_color(cx.theme().muted_foreground)
            .child(format!("Last selected: {}", self.selected)),
        )
        .child(context_area),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(920.), px(640.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |_window, cx| MenuWindow::view(cx),
      )
      .expect("open menu demo window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Menu Example");
      })
      .expect("update menu demo window failed");
  });
}