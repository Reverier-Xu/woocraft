use std::sync::Arc;

use gpui::{
  App, AppContext, Application, Bounds, Context, Edges, Entity, FocusHandle, Focusable,
  IntoElement, ParentElement, Render, SharedString, Size as GpuiSize, Styled, Window, WindowBounds,
  WindowOptions, div, px,
};
use woocraft::{
  ActiveTheme, Button, ButtonVariants as _, DockArea, DockItem, DockPlacement, IconName, Panel,
  PanelEvent, StyledExt as _, Theme, ThemeMode, h_flex, v_flex, window_border,
};

struct ExamplePanel {
  title: SharedString,
  focus_handle: FocusHandle,
}

impl ExamplePanel {
  fn new(title: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
    Self {
      title: title.into(),
      focus_handle: cx.focus_handle(),
    }
  }
}

impl Panel for ExamplePanel {
  fn panel_name(&self) -> &'static str {
    "ExamplePanel"
  }

  fn tab_name(&self, _cx: &App) -> Option<SharedString> {
    Some(self.title.clone())
  }

  fn title(&self, _cx: &App) -> SharedString {
    self.title.clone()
  }

  fn icon(&self, _cx: &App) -> IconName {
    IconName::Grid
  }
}

impl gpui::EventEmitter<PanelEvent> for ExamplePanel {}

impl Focusable for ExamplePanel {
  fn focus_handle(&self, _cx: &App) -> FocusHandle {
    self.focus_handle.clone()
  }
}

impl Render for ExamplePanel {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    v_flex()
      .size_full()
      .p_4()
      .gap_3()
      .bg(cx.theme().background)
      .text_color(cx.theme().foreground)
      .child(div().text_lg().font_semibold().child(self.title.clone()))
      .child(
        div()
          .text_sm()
          .text_color(cx.theme().muted_foreground)
          .child("This panel is rendered by the migrated dock system."),
      )
      .child(
        div()
          .flex_1()
          .rounded_md()
          .border_1()
          .border_color(cx.theme().border)
          .bg(cx.theme().card),
      )
  }
}

struct DockExample {
  dock_area: Entity<DockArea>,
}

impl DockExample {
  fn panel(
    title: impl Into<SharedString>, _window: &mut Window, cx: &mut App,
  ) -> Entity<ExamplePanel> {
    let title: SharedString = title.into();
    cx.new(move |cx| ExamplePanel::new(title.clone(), cx))
  }

  fn view(window: &mut Window, cx: &mut App) -> Entity<Self> {
    let dock_area = cx.new(|cx| DockArea::new("dock-example", Some(1), window, cx));
    let weak = dock_area.downgrade();

    let editor = Self::panel("Editor", window, cx);
    let preview = Self::panel("Preview", window, cx);
    let inspector = Self::panel("Inspector", window, cx);
    let explorer = Self::panel("Explorer", window, cx);
    let outline = Self::panel("Outline", window, cx);
    let terminal = Self::panel("Terminal", window, cx);
    let problems = Self::panel("Problems", window, cx);
    let references = Self::panel("References", window, cx);

    let center = DockItem::h_split(
      vec![
        DockItem::tab(editor, &weak, window, cx).size(px(540.)),
        DockItem::v_split(
          vec![
            DockItem::tab(preview, &weak, window, cx),
            DockItem::tab(inspector, &weak, window, cx).size(px(220.)),
          ],
          &weak,
          window,
          cx,
        )
        .size(px(360.)),
      ],
      &weak,
      window,
      cx,
    );

    let left = DockItem::tabs(
      vec![Arc::new(explorer.clone()), Arc::new(outline.clone())],
      &weak,
      window,
      cx,
    );

    let bottom = DockItem::tabs(
      vec![Arc::new(terminal.clone()), Arc::new(problems.clone())],
      &weak,
      window,
      cx,
    );

    let right = DockItem::tabs(vec![Arc::new(references.clone())], &weak, window, cx);

    dock_area.update(cx, |dock, cx| {
      dock.set_center(center, window, cx);
      dock.set_left_dock(left, Some(px(260.)), true, window, cx);
      dock.set_bottom_dock(bottom, Some(px(220.)), true, window, cx);
      dock.set_right_dock(right, Some(px(280.)), true, window, cx);
      dock.set_dock_collapsible(
        Edges {
          left: true,
          bottom: true,
          right: true,
          ..Default::default()
        },
        window,
        cx,
      );
    });

    cx.new(|_| Self { dock_area })
  }
}

impl Render for DockExample {
  fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
    let view = cx.entity().clone();

    window_border().child(
      v_flex()
        .size_full()
        .bg(cx.theme().background)
        .text_color(cx.theme().foreground)
        .child(
          h_flex()
            .h(px(48.))
            .px_4()
            .gap_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(
              Button::new("dock-theme")
                .label("Toggle Theme")
                .on_click(|_, _, cx| {
                  let next = if cx.theme().mode.is_dark() {
                    ThemeMode::Light
                  } else {
                    ThemeMode::Dark
                  };
                  Theme::set_mode(next, cx);
                }),
            )
            .child(Button::new("toggle-left").label("Left").flat().on_click({
              let view = view.clone();
              move |_, window, cx| {
                view.update(cx, |this, cx| {
                  this.dock_area.update(cx, |dock, cx| {
                    dock.toggle_dock(DockPlacement::Left, window, cx);
                  });
                });
              }
            }))
            .child(
              Button::new("toggle-bottom")
                .label("Bottom")
                .flat()
                .on_click({
                  let view = view.clone();
                  move |_, window, cx| {
                    view.update(cx, |this, cx| {
                      this.dock_area.update(cx, |dock, cx| {
                        dock.toggle_dock(DockPlacement::Bottom, window, cx);
                      });
                    });
                  }
                }),
            )
            .child(Button::new("toggle-right").label("Right").flat().on_click(
              move |_, window, cx| {
                view.update(cx, |this, cx| {
                  this.dock_area.update(cx, |dock, cx| {
                    dock.toggle_dock(DockPlacement::Right, window, cx);
                  });
                });
              },
            )),
        )
        .child(self.dock_area.clone()),
    )
  }
}

fn main() {
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    cx.activate(true);

    let bounds = Bounds::centered(None, GpuiSize::new(px(1200.), px(820.)), cx);
    let window = cx
      .open_window(
        WindowOptions {
          window_bounds: Some(WindowBounds::Windowed(bounds)),
          ..Default::default()
        },
        |window, cx| DockExample::view(window, cx),
      )
      .expect("open dock example window failed");

    window
      .update(cx, |_, window, _| {
        window.activate_window();
        window.set_window_title("Woocraft Dock Example");
      })
      .expect("update dock example window failed");
  });
}
