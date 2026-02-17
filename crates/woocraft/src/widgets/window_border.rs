use gpui::{
  AnyElement, App, Bounds, CursorStyle, Decorations, Edges, HitboxBehavior, Hsla,
  InteractiveElement as _, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce,
  ResizeEdge, Size, Styled as _, Window, canvas, div, point, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme, default_font, v_flex};

#[cfg(not(target_os = "linux"))]
const SHADOW_SIZE: Pixels = px(0.0);
#[cfg(target_os = "linux")]
const SHADOW_SIZE: Pixels = px(12.0);

const BORDER_SIZE: Pixels = px(1.0);

pub fn window_border() -> WindowBorder {
  WindowBorder::new()
}

#[derive(IntoElement, Default)]
pub struct WindowBorder {
  children: Vec<AnyElement>,
}

impl WindowBorder {
  pub fn new() -> Self {
    Self::default()
  }
}

pub fn window_paddings(window: &Window) -> Edges<Pixels> {
  match window.window_decorations() {
    Decorations::Server => Edges::all(px(0.0)),
    Decorations::Client { tiling } => {
      let mut paddings = Edges::all(SHADOW_SIZE);
      if tiling.top {
        paddings.top = px(0.0);
      }
      if tiling.bottom {
        paddings.bottom = px(0.0);
      }
      if tiling.left {
        paddings.left = px(0.0);
      }
      if tiling.right {
        paddings.right = px(0.0);
      }
      paddings
    }
  }
}

impl ParentElement for WindowBorder {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
}

impl RenderOnce for WindowBorder {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let decorations = window.window_decorations();
    let border_radius = cx.theme().radius_container;
    window.set_client_inset(SHADOW_SIZE);

    div()
      .id("window-backdrop")
      .font(default_font())
      .bg(gpui::transparent_black())
      .map(|this| match decorations {
        Decorations::Server => this,
        Decorations::Client { tiling, .. } => this
          .bg(gpui::transparent_black())
          .child(
            canvas(
              |_bounds, window, _| {
                window.insert_hitbox(
                  Bounds::new(
                    point(px(0.0), px(0.0)),
                    window.window_bounds().get_bounds().size,
                  ),
                  HitboxBehavior::Normal,
                )
              },
              move |_bounds, hitbox, window, _| {
                let mouse = window.mouse_position();
                let size = window.window_bounds().get_bounds().size;
                let Some(edge) = resize_edge(mouse, SHADOW_SIZE, size) else {
                  return;
                };
                window.set_cursor_style(
                  match edge {
                    ResizeEdge::Top | ResizeEdge::Bottom => CursorStyle::ResizeUpDown,
                    ResizeEdge::Left | ResizeEdge::Right => CursorStyle::ResizeLeftRight,
                    ResizeEdge::TopLeft | ResizeEdge::BottomRight => {
                      CursorStyle::ResizeUpLeftDownRight
                    }
                    ResizeEdge::TopRight | ResizeEdge::BottomLeft => {
                      CursorStyle::ResizeUpRightDownLeft
                    }
                  },
                  &hitbox,
                );
              },
            )
            .size_full()
            .absolute(),
          )
          .when(!(tiling.top || tiling.right), |div| {
            div.rounded_tr(border_radius)
          })
          .when(!(tiling.top || tiling.left), |div| {
            div.rounded_tl(border_radius)
          })
          .when(!(tiling.bottom || tiling.left), |div| {
            div.rounded_bl(border_radius)
          })
          .when(!(tiling.bottom || tiling.right), |div| {
            div.rounded_br(border_radius)
          })
          .when(!tiling.top, |div| div.pt(SHADOW_SIZE))
          .when(!tiling.bottom, |div| div.pb(SHADOW_SIZE))
          .when(!tiling.left, |div| div.pl(SHADOW_SIZE))
          .when(!tiling.right, |div| div.pr(SHADOW_SIZE))
          .on_mouse_down(MouseButton::Left, move |_, window, _| {
            let size = window.window_bounds().get_bounds().size;
            let pos = window.mouse_position();

            if let Some(edge) = resize_edge(pos, SHADOW_SIZE, size)
              && !window.is_maximized()
              && !window.is_fullscreen()
            {
              window.start_window_resize(edge);
            }
          }),
      })
      .size_full()
      .child(
        v_flex()
          .cursor(CursorStyle::default())
          .map(|this| match decorations {
            Decorations::Server => this
              .rounded(border_radius)
              .border_color(cx.theme().border)
              .border_1(),
            Decorations::Client { tiling } => this
              .when(!(tiling.top || tiling.right), |div| {
                div.rounded_tr(border_radius)
              })
              .when(!(tiling.top || tiling.left), |div| {
                div.rounded_tl(border_radius)
              })
              .when(!(tiling.bottom || tiling.left), |div| {
                div.rounded_bl(border_radius)
              })
              .when(!(tiling.bottom || tiling.right), |div| {
                div.rounded_br(border_radius)
              })
              .border_color(cx.theme().border)
              .when(!tiling.top, |div| div.border_t(BORDER_SIZE))
              .when(!tiling.bottom, |div| div.border_b(BORDER_SIZE))
              .when(!tiling.left, |div| div.border_l(BORDER_SIZE))
              .when(!tiling.right, |div| div.border_r(BORDER_SIZE))
              .when(!tiling.is_tiled(), |div| {
                div.shadow(vec![gpui::BoxShadow {
                  color: Hsla {
                    h: 0.,
                    s: 0.,
                    l: 0.,
                    a: 0.3,
                  },
                  blur_radius: SHADOW_SIZE / 2.,
                  spread_radius: px(0.),
                  offset: point(px(0.0), px(0.0)),
                }])
              }),
          })
          .on_mouse_move(|_e, _, cx| {
            cx.stop_propagation();
          })
          .overflow_hidden()
          .bg(cx.theme().background)
          .text_color(cx.theme().foreground)
          .size_full()
          .children(self.children),
      )
  }
}

fn resize_edge(pos: Point<Pixels>, shadow_size: Pixels, size: Size<Pixels>) -> Option<ResizeEdge> {
  let edge = if pos.y < shadow_size && pos.x < shadow_size {
    ResizeEdge::TopLeft
  } else if pos.y < shadow_size && pos.x > size.width - shadow_size {
    ResizeEdge::TopRight
  } else if pos.y < shadow_size {
    ResizeEdge::Top
  } else if pos.y > size.height - shadow_size && pos.x < shadow_size {
    ResizeEdge::BottomLeft
  } else if pos.y > size.height - shadow_size && pos.x > size.width - shadow_size {
    ResizeEdge::BottomRight
  } else if pos.y > size.height - shadow_size {
    ResizeEdge::Bottom
  } else if pos.x < shadow_size {
    ResizeEdge::Left
  } else if pos.x > size.width - shadow_size {
    ResizeEdge::Right
  } else {
    return None;
  };

  Some(edge)
}
