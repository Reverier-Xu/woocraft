//! client-side window frame: shadow, resize edges, and the rounded border
//! drawn around the whole application surface on linux.
//!
//! pairs with [`TitleBar`](crate::TitleBar): the example composes
//! `window_border().child(TitleBar::new()...)` as the window root.

use gpui::{
  AnyElement, App, Bounds, CursorStyle, Decorations, Edges, HitboxBehavior, Hsla,
  InteractiveElement as _, IntoElement, MouseButton, ParentElement, Pixels, Point, RenderOnce,
  ResizeEdge, Size, Styled as _, Window, canvas, div, point, prelude::FluentBuilder as _, px,
};

use crate::{ActiveTheme, base::v_flex};

/// the outer margin client-decorated windows reserve for the drop shadow,
/// rem-driven like every other size in the design system; zero where the
/// platform keeps server-side decorations.
#[cfg(not(target_os = "linux"))]
pub(crate) fn window_shadow_size(_: &Window) -> Pixels {
  px(0.0)
}

/// the outer margin client-decorated windows reserve for the drop shadow,
/// rem-driven like every other size in the design system.
#[cfg(target_os = "linux")]
pub(crate) fn window_shadow_size(window: &Window) -> Pixels {
  gpui::rems(0.75).to_pixels(window.rem_size())
}

/// returns the extra outer padding the window needs under client-side
/// decorations: room for the drop shadow, collapsing to zero on tiled edges.
pub fn window_paddings(window: &Window) -> Edges<Pixels> {
  match window.window_decorations() {
    Decorations::Server => Edges::all(px(0.0)),
    Decorations::Client { tiling } => {
      let shadow = window_shadow_size(window);
      let mut paddings = Edges::all(shadow);
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

pub fn window_border() -> WindowBorder {
  WindowBorder::new()
}

impl ParentElement for WindowBorder {
  fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
    self.children.extend(elements);
  }
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

impl RenderOnce for WindowBorder {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let decorations = window.window_decorations();
    let border_radius = cx.theme().radius_container;
    let border_size = cx.theme().border_width;
    let shadow_size = window_shadow_size(window);
    window.set_client_inset(shadow_size);

    div()
      .id("window-backdrop")
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
                  HitboxBehavior::BlockMouseExceptScroll,
                )
              },
              move |_bounds, hitbox, window, _| {
                let mouse = window.mouse_position();
                let size = window.window_bounds().get_bounds().size;
                let Some(edge) = resize_edge(mouse, shadow_size, size) else {
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
          .when(!tiling.top, |div| div.pt(shadow_size))
          .when(!tiling.bottom, |div| div.pb(shadow_size))
          .when(!tiling.left, |div| div.pl(shadow_size))
          .when(!tiling.right, |div| div.pr(shadow_size))
          .on_mouse_down(MouseButton::Left, move |_, window, _| {
            let size = window.window_bounds().get_bounds().size;
            let pos = window.mouse_position();

            if let Some(edge) = resize_edge(pos, shadow_size, size)
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
              .border(border_size),
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
              .when(!tiling.top, |div| div.border_t(border_size))
              .when(!tiling.bottom, |div| div.border_b(border_size))
              .when(!tiling.left, |div| div.border_l(border_size))
              .when(!tiling.right, |div| div.border_r(border_size))
              .when(!tiling.is_tiled(), |div| {
                div.shadow(vec![gpui::BoxShadow {
                  color: Hsla {
                    h: 0.,
                    s: 0.,
                    l: 0.,
                    a: 0.3,
                  },
                  blur_radius: shadow_size / 2.,
                  spread_radius: px(0.),
                  offset: point(px(0.0), px(0.0)),
                  inset: false,
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
