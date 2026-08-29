//! The GPUI element that paints the terminal grid.
//!
//! Performance-critical path: cells are batched into background regions and
//! text runs before painting; no per-cell glyph painting happens here.

use gpui::{
  App, Bounds, ContentMask, CursorStyle, DispatchPhase, Element, ElementId, FontStyle, FontWeight,
  GlobalElementId, Hitbox, Hsla, InspectorElementId, InteractiveElement, Interactivity,
  IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point,
  ScrollWheelEvent, ShapedLine, SharedString, StatefulInteractiveElement, StrikethroughStyle,
  TextAlign, TextRun, UnderlineStyle, Window, fill, px,
};
use woocraft_terminal::{
  Cell, CellColor, CellFlags, Content, CursorShape, NamedColor, SelectionRange, TerminalBounds,
};

use super::{colors::TerminalPalette, view::TerminalView};
use crate::ActiveTheme as _;

/// Subcell columns per cell for block element glyphs (eighth blocks).
const BLOCK_SUBCELL_COLUMNS: i32 = 8;
/// Subcell lines per cell for block element glyphs (eighth/sextant blocks).
const BLOCK_SUBCELL_LINES: i32 = 24;

/// The terminal element; the view entity provides content and input handling.
pub struct TerminalElement {
  view: gpui::Entity<TerminalView>,
  focus: gpui::FocusHandle,
  focused: bool,
  interactivity: Interactivity,
}

impl InteractiveElement for TerminalElement {
  fn interactivity(&mut self) -> &mut Interactivity {
    &mut self.interactivity
  }
}

impl StatefulInteractiveElement for TerminalElement {}

/// A contiguous same-styled group of cells, positioned on the grid.
pub(crate) struct BatchedTextRun {
  pub line: i32,
  pub column: i32,
  pub text: String,
  pub cell_count: usize,
  pub style: TextRun,
  pub font_size: Pixels,
}

impl BatchedTextRun {
  fn new_from_cell(line: i32, column: i32, c: char, style: TextRun, font_size: Pixels) -> Self {
    let mut text = String::with_capacity(64);
    text.push(c);
    Self {
      line,
      column,
      text,
      cell_count: 1,
      style,
      font_size,
    }
  }

  fn can_append(&self, style: &TextRun) -> bool {
    self.style.font == style.font
      && self.style.color == style.color
      && self.style.underline == style.underline
      && self.style.strikethrough == style.strikethrough
  }

  fn append_cell(&mut self, c: char) {
    self.text.push(c);
    self.cell_count += 1;
  }

  fn append_zero_width(&mut self, chars: &[char]) {
    for &c in chars {
      self.text.push(c);
    }
  }

  fn paint(
    &self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window, cx: &mut App,
  ) {
    let position = Point::new(
      origin.x + px(self.column as f32 * dimensions.cell_width()),
      origin.y + px(self.line as f32 * dimensions.line_height()),
    );
    window
      .text_system()
      .shape_line(
        SharedString::from(self.text.clone()),
        self.font_size,
        std::slice::from_ref(&self.style),
        Some(px(dimensions.cell_width())),
      )
      .paint(
        position,
        px(dimensions.line_height()),
        TextAlign::Left,
        None,
        window,
        cx,
      )
      .ok();
  }
}

/// A merged background rectangle positioned on the grid.
pub(crate) struct LayoutRect {
  pub line: i32,
  pub column: i32,
  pub columns: i32,
  pub lines: i32,
  pub color: Hsla,
}

impl LayoutRect {
  fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
    let bounds = Bounds::new(
      Point::new(
        origin.x + px(self.column as f32 * dimensions.cell_width()),
        origin.y + px(self.line as f32 * dimensions.line_height()),
      ),
      gpui::size(
        px(self.columns as f32 * dimensions.cell_width()),
        px(self.lines as f32 * dimensions.line_height()),
      ),
    );
    window.paint_quad(fill(bounds, self.color));
  }
}

/// A merged rectangle on the block-element subcell grid.
struct BlockElementRect {
  line: i32,
  column: i32,
  columns: i32,
  lines: i32,
  color: Hsla,
}

impl BlockElementRect {
  fn paint(&self, origin: Point<Pixels>, dimensions: &TerminalBounds, window: &mut Window) {
    let subcell_width = dimensions.cell_width() / BLOCK_SUBCELL_COLUMNS as f32;
    let subcell_height = dimensions.line_height() / BLOCK_SUBCELL_LINES as f32;
    let bounds = Bounds::new(
      Point::new(
        origin.x + px(self.column as f32 * subcell_width),
        origin.y + px(self.line as f32 * subcell_height),
      ),
      gpui::size(
        px(self.columns as f32 * subcell_width),
        px(self.lines as f32 * subcell_height),
      ),
    );
    window.paint_quad(fill(bounds, self.color));
  }
}

/// A merged same-colored background region, in grid coordinates.
#[derive(Debug, Clone)]
struct BackgroundRegion {
  start_line: i32,
  start_column: i32,
  end_line: i32,
  end_column: i32,
  color: Hsla,
}

impl BackgroundRegion {
  fn new(line: i32, column: i32, color: Hsla) -> Self {
    Self {
      start_line: line,
      start_column: column,
      end_line: line,
      end_column: column,
      color,
    }
  }

  fn with_extents(
    start_line: i32, start_column: i32, end_line: i32, end_column: i32, color: Hsla,
  ) -> Self {
    Self {
      start_line,
      start_column,
      end_line,
      end_column,
      color,
    }
  }

  fn can_merge_with(&self, other: &Self) -> bool {
    if self.color != other.color {
      return false;
    }
    // Adjacent horizontally on the same line.
    if self.start_line == other.start_line && self.end_line == other.end_line {
      return self.end_column + 1 == other.start_column
        || other.end_column + 1 == self.start_column;
    }
    // Adjacent vertically with the same column span.
    if self.start_column == other.start_column && self.end_column == other.end_column {
      return self.end_line + 1 == other.start_line || other.end_line + 1 == self.start_line;
    }
    false
  }

  fn merge_with(&mut self, other: &Self) {
    self.start_line = self.start_line.min(other.start_line);
    self.start_column = self.start_column.min(other.start_column);
    self.end_line = self.end_line.max(other.end_line);
    self.end_column = self.end_column.max(other.end_column);
  }
}

/// Merges grid regions to minimize the number of painted rectangles.
fn merge_background_regions(regions: Vec<BackgroundRegion>) -> Vec<BackgroundRegion> {
  if regions.len() < 2 {
    return regions;
  }
  let mut merged = regions;
  let mut changed = true;
  while changed {
    changed = false;
    let mut index = 0;
    while index < merged.len() {
      let mut other = index + 1;
      while other < merged.len() {
        if merged[index].can_merge_with(&merged[other]) {
          let other_region = merged.remove(other);
          merged[index].merge_with(&other_region);
          changed = true;
        } else {
          other += 1;
        }
      }
      index += 1;
    }
  }
  merged
}

/// The cursor rectangle and its rendered shape.
struct CursorLayout {
  bounds: Bounds<Pixels>,
  shape: CursorShape,
  /// The character under the cursor, pre-shaped with inverse colors for block
  /// cursors.
  char_text: Option<ShapedLine>,
  line_height: Pixels,
}

impl CursorLayout {
  fn paint(&self, window: &mut Window, cx: &mut App) {
    let cursor_color = cx.theme().caret;
    match self.shape {
      CursorShape::Block => {
        window.paint_quad(fill(self.bounds, cursor_color));
        if let Some(text) = &self.char_text {
          text
            .paint(
              self.bounds.origin,
              self.line_height,
              TextAlign::Left,
              None,
              window,
              cx,
            )
            .ok();
        }
      }
      CursorShape::Underline => {
        let thickness = px(2.0);
        let bounds = Bounds::new(
          Point::new(
            self.bounds.origin.x,
            self.bounds.bottom_left().y - thickness,
          ),
          gpui::size(self.bounds.size.width, thickness),
        );
        window.paint_quad(fill(bounds, cursor_color));
      }
      CursorShape::Bar => {
        let width = px(2.0);
        let bounds = Bounds::new(
          self.bounds.origin,
          gpui::size(width, self.bounds.size.height),
        );
        window.paint_quad(fill(bounds, cursor_color));
      }
      CursorShape::HollowBlock => {
        let thickness = px(1.5);
        let bounds = self.bounds;
        // Top, bottom, left, right strokes.
        window.paint_quad(fill(
          Bounds::new(bounds.origin, gpui::size(bounds.size.width, thickness)),
          cursor_color,
        ));
        window.paint_quad(fill(
          Bounds::new(
            Point::new(bounds.origin.x, bounds.bottom_left().y - thickness),
            gpui::size(bounds.size.width, thickness),
          ),
          cursor_color,
        ));
        window.paint_quad(fill(
          Bounds::new(bounds.origin, gpui::size(thickness, bounds.size.height)),
          cursor_color,
        ));
        window.paint_quad(fill(
          Bounds::new(
            Point::new(bounds.right() - thickness, bounds.origin.y),
            gpui::size(thickness, bounds.size.height),
          ),
          cursor_color,
        ));
      }
      CursorShape::Hidden => {}
    }
  }
}

/// State computed during prepaint and consumed by paint.
pub struct PrepaintState {
  hitbox: Hitbox,
  dimensions: TerminalBounds,
  origin: Point<Pixels>,
  background_color: Hsla,
  font: gpui::Font,
  font_size: Pixels,
  background_rects: Vec<LayoutRect>,
  selection_rects: Vec<LayoutRect>,
  batched_runs: Vec<BatchedTextRun>,
  block_rects: Vec<BlockElementRect>,
  cursor: Option<CursorLayout>,
  ime_cursor_bounds: Option<Bounds<Pixels>>,
}

impl TerminalElement {
  pub fn new(view: gpui::Entity<TerminalView>, focus: gpui::FocusHandle, focused: bool) -> Self {
    Self {
      view,
      focus: focus.clone(),
      focused,
      interactivity: Default::default(),
    }
    .track_focus(&focus)
  }

  /// Groups cells into background regions, batched text runs, and block
  /// element rectangles.
  fn layout_grid(
    content: &Content, font: gpui::Font, font_size: Pixels, palette: &TerminalPalette,
  ) -> (Vec<LayoutRect>, Vec<BatchedTextRun>, Vec<BlockElementRect>) {
    let cells = &content.cells;
    let estimated = cells.len();
    let mut background_regions: Vec<BackgroundRegion> = Vec::with_capacity(estimated / 20);
    let mut block_regions: Vec<BackgroundRegion> = Vec::with_capacity(8);
    let mut batched_runs: Vec<BatchedTextRun> = Vec::with_capacity(estimated / 10);
    let mut current_batch: Option<BatchedTextRun> = None;

    for indexed in cells {
      let point = indexed.point;
      let cell = &indexed.cell;
      let display_line = point.line + content.display_offset as i32;
      let mut fg = cell.fg;
      let mut bg = cell.bg;
      if cell.flags.contains(CellFlags::INVERSE) {
        std::mem::swap(&mut fg, &mut bg);
      }

      // Collect non-default background regions.
      if !is_default_background(bg) {
        let color = palette.convert(&bg);
        let column = point.column as i32;
        match background_regions.last_mut() {
          Some(last)
            if last.color == color
              && last.start_line == display_line
              && last.end_line == display_line
              && last.end_column + 1 == column =>
          {
            last.end_column = column;
          }
          _ => background_regions.push(BackgroundRegion::new(display_line, column, color)),
        }
      }

      // Wide character spacers are placeholders for the other half of a wide
      // glyph; they must not paint text.
      if cell
        .flags
        .intersects(CellFlags::WIDE_CHAR_SPACER | CellFlags::LEADING_WIDE_CHAR_SPACER)
      {
        continue;
      }

      // Block element glyphs render more accurately as filled rectangles.
      if is_block_element(cell.c) {
        if let Some(batch) = current_batch.take() {
          batched_runs.push(batch);
        }
        let style = cell_style(cell, fg, palette, font.clone());
        collect_block_element_regions(
          display_line,
          point.column as i32,
          cell.c,
          style.color,
          &mut block_regions,
        );
        continue;
      }

      if is_blank(cell) {
        continue;
      }

      // Extend or start a text run.
      let style = cell_style(cell, fg, palette, font.clone());
      let line = display_line;
      let column = point.column as i32;
      match current_batch.as_mut() {
        Some(batch)
          if batch.can_append(&style)
            && batch.line == line
            && batch.column + batch.cell_count as i32 == column =>
        {
          batch.append_cell(cell.c);
          if !cell.zerowidth.is_empty() {
            batch.append_zero_width(&cell.zerowidth);
          }
        }
        _ => {
          if let Some(batch) = current_batch.take() {
            batched_runs.push(batch);
          }
          let mut batch = BatchedTextRun::new_from_cell(line, column, cell.c, style, font_size);
          if !cell.zerowidth.is_empty() {
            batch.append_zero_width(&cell.zerowidth);
          }
          current_batch = Some(batch);
        }
      }
    }
    if let Some(batch) = current_batch.take() {
      batched_runs.push(batch);
    }

    let background_rects = merge_background_regions(background_regions)
      .into_iter()
      .map(|region| LayoutRect {
        line: region.start_line,
        column: region.start_column,
        columns: region.end_column - region.start_column + 1,
        lines: region.end_line - region.start_line + 1,
        color: region.color,
      })
      .collect();

    let block_rects = merge_background_regions(block_regions)
      .into_iter()
      .map(|region| BlockElementRect {
        line: region.start_line,
        column: region.start_column,
        columns: region.end_column - region.start_column + 1,
        lines: region.end_line - region.start_line + 1,
        color: region.color,
      })
      .collect();

    (background_rects, batched_runs, block_rects)
  }

  /// Expands a selection range into per-line rectangles, clamped to the
  /// viewport.
  fn selection_rects(selection: SelectionRange, content: &Content, color: Hsla) -> Vec<LayoutRect> {
    let display_offset = i32::try_from(content.display_offset).unwrap_or(i32::MAX);
    let start_line = selection.start.line + display_offset;
    let end_line = selection.end.line + display_offset;
    if end_line < 0 || start_line > content.screen_lines as i32 {
      return Vec::new();
    }

    let mut rects = Vec::new();
    let last_column = content.columns.saturating_sub(1);
    for line in start_line.max(0)..=end_line.min(content.screen_lines as i32 - 1) {
      let start_column = if line == start_line {
        selection.start.column as i32
      } else {
        0
      };
      let end_column = if line == end_line {
        selection.end.column.min(last_column) as i32 + 1
      } else {
        content.columns as i32
      };
      if end_column <= start_column {
        continue;
      }
      rects.push(LayoutRect {
        line,
        column: start_column,
        columns: end_column - start_column,
        lines: 1,
        color,
      });
    }
    rects
  }

  fn register_mouse_listeners(
    &mut self, hitbox: Hitbox, origin: Point<Pixels>, window: &mut Window,
  ) {
    let view = self.view.clone();
    let focus = self.focus.clone();

    self.interactivity.on_mouse_down(MouseButton::Left, {
      let view = view.clone();
      let focus = focus.clone();
      move |event: &MouseDownEvent, window, cx| {
        window.focus(&focus, cx);
        view.update(cx, |view, cx| {
          view.pause_blink(cx);
          let position = event.position - origin;
          view.mouse_down(
            position,
            event.button,
            event.modifiers,
            event.click_count,
            cx,
          );
        });
      }
    });

    window.on_mouse_event({
      let view = view.clone();
      let hitbox = hitbox.clone();
      let focus = focus.clone();
      move |event: &MouseMoveEvent, phase: DispatchPhase, window, cx| {
        if phase != DispatchPhase::Bubble {
          return;
        }
        // Gate drag handling like Zed: only while the left button is held,
        // no GPUI drag is active, and the terminal owns focus. Without the
        // focus gate a stale selection would follow drags anywhere in the app.
        if event.pressed_button == Some(MouseButton::Left)
          && !cx.has_active_drag()
          && focus.is_focused(window)
        {
          view.update(cx, |view, cx| {
            if view.selection_started() || hitbox.is_hovered(window) {
              view.mouse_drag(
                event.position - origin,
                hitbox.bounds,
                event.modifiers.shift,
                cx,
              );
            }
          });
        } else if hitbox.is_hovered(window) && view.read(cx).mouse_mode_enabled() {
          view.update(cx, |view, cx| {
            view.mouse_move(
              event.position - origin,
              event.pressed_button,
              event.modifiers,
              cx,
            );
          });
        }
      }
    });

    self.interactivity.on_mouse_up(MouseButton::Left, {
      let view = view.clone();
      move |event: &MouseUpEvent, _window, cx| {
        view.update(cx, |view, _| {
          view.mouse_up(event.position - origin, event.button, event.modifiers);
        });
      }
    });

    // End selections released outside the terminal (e.g. over the dock or
    // while resizing the window): `on_mouse_up` only fires when hovered, so
    // without this the selection would stay active forever and hijack every
    // subsequent drag in the app.
    self.interactivity.on_mouse_up_out(MouseButton::Left, {
      let view = view.clone();
      move |event: &MouseUpEvent, _window, cx| {
        view.update(cx, |view, _| {
          view.mouse_up_outside(event.position, event.button, event.modifiers);
        });
      }
    });

    // Mouse-mode click reports for the other buttons.
    self.interactivity.on_mouse_down(MouseButton::Middle, {
      let view = view.clone();
      move |event: &MouseDownEvent, _window, cx| {
        view.update(cx, |view, cx| {
          view.mouse_down(
            event.position - origin,
            event.button,
            event.modifiers,
            event.click_count,
            cx,
          );
        });
      }
    });
    self.interactivity.on_mouse_up(MouseButton::Middle, {
      let view = view.clone();
      move |event: &MouseUpEvent, _window, cx| {
        view.update(cx, |view, _| {
          view.mouse_up(event.position - origin, event.button, event.modifiers);
        });
      }
    });
    self.interactivity.on_mouse_down(MouseButton::Right, {
      let view = view.clone();
      move |event: &MouseDownEvent, _window, cx| {
        view.update(cx, |view, cx| {
          view.mouse_down(
            event.position - origin,
            event.button,
            event.modifiers,
            event.click_count,
            cx,
          );
        });
      }
    });
    self.interactivity.on_mouse_up(MouseButton::Right, {
      let view = view.clone();
      move |event: &MouseUpEvent, _window, cx| {
        view.update(cx, |view, _| {
          view.mouse_up(event.position - origin, event.button, event.modifiers);
        });
      }
    });

    self.interactivity.on_scroll_wheel({
      let view = view.clone();
      move |event: &ScrollWheelEvent, _window, cx| {
        view.update(cx, |view, cx| {
          view.scroll_wheel(
            event.position - origin,
            event.delta,
            event.modifiers.shift,
            event.touch_phase,
            cx,
          );
        });
      }
    });
  }
}

impl Element for TerminalElement {
  type RequestLayoutState = ();
  type PrepaintState = PrepaintState;

  fn id(&self) -> Option<ElementId> {
    Some("terminal".into())
  }

  fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, global_id: Option<&GlobalElementId>, inspector_id: Option<&InspectorElementId>,
    window: &mut Window, cx: &mut App,
  ) -> (LayoutId, Self::RequestLayoutState) {
    let layout_id = self.interactivity.request_layout(
      global_id,
      inspector_id,
      window,
      cx,
      |mut style, window, cx| {
        style.size.width = gpui::relative(1.).into();
        style.size.height = gpui::relative(1.).into();
        window.request_layout(style, None, cx)
      },
    );
    (layout_id, ())
  }

  fn prepaint(
    &mut self, global_id: Option<&GlobalElementId>, inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>, _request_layout: &mut Self::RequestLayoutState, window: &mut Window,
    cx: &mut App,
  ) -> Self::PrepaintState {
    let theme = cx.theme().clone();
    let palette = TerminalPalette::from_theme(&theme);
    let background_color = palette.background;

    self.interactivity.prepaint(
      global_id,
      inspector_id,
      bounds,
      bounds.size,
      window,
      cx,
      |_, _, hitbox, window, cx| {
        let hitbox = hitbox.expect("terminal element prepaint must produce a hitbox");

        // Measure the monospace cell size.
        let font_size = self
          .view
          .read(cx)
          .font_size
          .unwrap_or(window.text_style().font_size.to_pixels(window.rem_size()));
        let family = self
          .view
          .read(cx)
          .font_family
          .clone()
          .unwrap_or_else(|| SharedString::from(super::element::default_monospace_family()));
        let font = gpui::Font {
          family,
          fallbacks: crate::platform_font_fallbacks(),
          ..window.text_style().font()
        };
        let font_id = window.text_system().resolve_font(&font);
        let cell_width = window
          .text_system()
          .advance(font_id, font_size, 'm')
          .map(|advance| advance.width)
          .unwrap_or(px(font_size.to_f64() as f32 * 0.6));
        // 1.3 is the conventional terminal line height multiplier.
        let line_height = px(font_size.to_f64() as f32 * 1.3);

        // Fit a whole number of lines and columns; anchor to the bottom so
        // output stays flush with the bottom edge.
        let scale_factor = window.scale_factor();
        let line_height_device_px = (f32::from(line_height) * scale_factor).floor().max(1.0) as i32;
        let cell_width_device_px = (f32::from(cell_width) * scale_factor).floor().max(1.0) as i32;
        let lines = ((f32::from(bounds.size.height) * scale_factor).floor() as i32
          / line_height_device_px)
          .max(1) as usize;
        let columns = ((f32::from(bounds.size.width) * scale_factor).floor() as i32
          / cell_width_device_px)
          .max(1) as usize;
        let snapped_height = px((lines as i32 * line_height_device_px) as f32 / scale_factor);
        let padding = (f32::from(bounds.size.height) - f32::from(snapped_height)).max(0.0);
        let snap = |value: Pixels| px((f32::from(value) * scale_factor).floor() / scale_factor);
        let origin = Point::new(snap(bounds.origin.x), snap(bounds.origin.y + px(padding)));
        let dimensions = TerminalBounds::new(
          f32::from(line_height),
          f32::from(cell_width),
          columns,
          lines,
        );

        // Resize the session when the viewport changed, then take a fresh
        // short-lock snapshot for this frame. The snapshot is stored on the
        // view behind an `Arc` and reused here, so each frame copies the
        // visible grid exactly once (Zed's `sync`/`last_content` pattern:
        // never hold the emulator lock across layout/paint, never clone the
        // grid twice).
        let content = self.view.update(cx, |view, _| {
          if view.session().bounds() != dimensions {
            view.session().resize(dimensions);
          }
          let snapshot = std::sync::Arc::new(view.session().snapshot());
          view.content = snapshot.clone();
          snapshot
        });
        let (background_rects, batched_runs, block_rects) =
          Self::layout_grid(&content, font.clone(), font_size, &palette);

        // Selection rectangles, in viewport coordinates.
        let selection_rects = content
          .selection
          .map(|selection| Self::selection_rects(selection, &content, theme.selection))
          .unwrap_or_default();

        // Cursor layout. The rectangle doubles as the IME anchor.
        let display_offset = content.display_offset as i32;
        let cursor_line = content.cursor.point.line + display_offset;
        let cursor_position = if content.cursor.shape != CursorShape::Hidden
          && cursor_line >= 0
          && (cursor_line as usize) < lines
        {
          Some(Point::new(
            origin.x + px(content.cursor.point.column as f32 * f32::from(cell_width)),
            origin.y + px(cursor_line as f32 * f32::from(line_height)),
          ))
        } else {
          None
        };
        let cursor_char = content.cursor_char;
        let char_text = cursor_position.map(|_| {
          let text = cursor_char.to_string();
          window.text_system().shape_line(
            SharedString::from(text),
            font_size,
            &[TextRun {
              len: cursor_char.len_utf8(),
              font: font.clone(),
              color: palette.background,
              ..Default::default()
            }],
            None,
          )
        });
        let cursor_width = if cursor_char.is_whitespace() {
          f32::from(cell_width)
        } else {
          char_text
            .as_ref()
            .map(|line| f32::from(line.width.max(cell_width)))
            .unwrap_or(f32::from(cell_width))
        };
        let ime_cursor_bounds = cursor_position
          .map(|position| Bounds::new(position, gpui::size(px(cursor_width.ceil()), line_height)));
        let cursor = if content.cursor.shape == CursorShape::Hidden {
          None
        } else {
          ime_cursor_bounds.map(|bounds| CursorLayout {
            bounds,
            // Unfocused block cursors render hollow.
            shape: if self.focused {
              content.cursor.shape
            } else {
              match content.cursor.shape {
                CursorShape::Bar => CursorShape::Bar,
                CursorShape::Underline => CursorShape::Underline,
                _ => CursorShape::HollowBlock,
              }
            },
            char_text: (self.focused && content.cursor.shape == CursorShape::Block)
              .then_some(char_text)
              .flatten(),
            line_height,
          })
        };

        PrepaintState {
          hitbox,
          dimensions,
          origin,
          background_color,
          font: font.clone(),
          font_size,
          background_rects,
          selection_rects,
          batched_runs,
          block_rects,
          cursor,
          ime_cursor_bounds,
        }
      },
    )
  }

  fn paint(
    &mut self, global_id: Option<&GlobalElementId>, inspector_id: Option<&InspectorElementId>,
    bounds: Bounds<Pixels>, _request_layout: &mut Self::RequestLayoutState,
    prepaint: &mut Self::PrepaintState, window: &mut Window, cx: &mut App,
  ) {
    let palette = TerminalPalette::from_theme(cx.theme());
    let selection_color = cx.theme().selection;

    window.with_content_mask(Some(ContentMask { bounds }), |window| {
      let cursor = prepaint.cursor.take();
      let ime_cursor_bounds = prepaint.ime_cursor_bounds;
      let marked_text = self.view.read(cx).marked_text().map(str::to_string);

      let input_handler = TerminalInputHandler {
        view: self.view.clone(),
        cursor_bounds: ime_cursor_bounds.map(|bounds| bounds + prepaint.origin),
        cell_width: prepaint.dimensions.cell_width(),
      };

      self.register_mouse_listeners(prepaint.hitbox.clone(), prepaint.origin, window);
      window.set_cursor_style(CursorStyle::IBeam, &prepaint.hitbox);

      self.interactivity.paint(
        global_id,
        inspector_id,
        bounds,
        Some(&prepaint.hitbox),
        window,
        cx,
        |_, window, cx| {
          window.handle_input(&self.focus, input_handler, cx);

          window.paint_quad(fill(bounds, prepaint.background_color));

          for rect in &prepaint.background_rects {
            rect.paint(prepaint.origin, &prepaint.dimensions, window);
          }
          for rect in &prepaint.selection_rects {
            rect.paint(prepaint.origin, &prepaint.dimensions, window);
          }
          for rect in &prepaint.block_rects {
            rect.paint(prepaint.origin, &prepaint.dimensions, window);
          }

          for run in &prepaint.batched_runs {
            run.paint(prepaint.origin, &prepaint.dimensions, window, cx);
          }

          // IME composition text, drawn on an opaque background at the cursor.
          if let (Some(marked_text), Some(ime_bounds)) = (marked_text.as_ref(), ime_cursor_bounds) {
            let ime_bounds = ime_bounds + prepaint.origin;
            let marked_style = TextRun {
              len: marked_text.len(),
              font: prepaint.font.clone(),
              color: palette.foreground,
              underline: Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(palette.foreground),
                wavy: false,
              }),
              ..Default::default()
            };
            let shaped = window.text_system().shape_line(
              SharedString::from(marked_text.clone()),
              prepaint.font_size,
              &[marked_style],
              None,
            );
            let marked_bounds = Bounds::new(
              ime_bounds.origin,
              gpui::size(shaped.width, px(prepaint.dimensions.line_height())),
            );
            window.paint_quad(fill(marked_bounds, palette.background));
            shaped
              .paint(
                ime_bounds.origin,
                px(prepaint.dimensions.line_height()),
                TextAlign::Left,
                None,
                window,
                cx,
              )
              .ok();
          }

          if let Some(cursor) = cursor {
            cursor.paint(window, cx);
          }
          let _ = selection_color;
        },
      );
    });
  }
}

impl IntoElement for TerminalElement {
  type Element = Self;

  fn into_element(self) -> Self {
    self
  }
}

/// The input handler registered with the platform for IME support.
struct TerminalInputHandler {
  view: gpui::Entity<TerminalView>,
  cursor_bounds: Option<Bounds<Pixels>>,
  cell_width: f32,
}

impl gpui::InputHandler for TerminalInputHandler {
  fn selected_text_range(
    &mut self, _ignore_disabled_input: bool, _window: &mut Window, _cx: &mut App,
  ) -> Option<gpui::UTF16Selection> {
    // Always return a valid (empty) selection so IME candidate windows can
    // position themselves, even in the alternate screen.
    Some(gpui::UTF16Selection {
      range: 0..0,
      reversed: false,
    })
  }

  fn marked_text_range(
    &mut self, _window: &mut Window, cx: &mut App,
  ) -> Option<std::ops::Range<usize>> {
    let marked = self.view.read(cx).marked_text()?;
    Some(0..marked.chars().map(char::len_utf16).sum())
  }

  fn text_for_range(
    &mut self, _range_utf16: std::ops::Range<usize>,
    _adjusted_range: &mut Option<std::ops::Range<usize>>, _window: &mut Window, _cx: &mut App,
  ) -> Option<String> {
    None
  }

  fn replace_text_in_range(
    &mut self, _replacement_range: Option<std::ops::Range<usize>>, text: &str,
    _window: &mut Window, cx: &mut App,
  ) {
    self.view.update(cx, |view, cx| {
      view.marked_text = None;
      view.session().paste(text);
      cx.notify();
    });
  }

  fn replace_and_mark_text_in_range(
    &mut self, _range_utf16: Option<std::ops::Range<usize>>, new_text: &str,
    _new_selected_range: Option<std::ops::Range<usize>>, _window: &mut Window, cx: &mut App,
  ) {
    self.view.update(cx, |view, cx| {
      view.marked_text = Some(new_text.to_string());
      cx.notify();
    });
  }

  fn unmark_text(&mut self, _window: &mut Window, cx: &mut App) {
    self.view.update(cx, |view, cx| {
      if view.marked_text.take().is_some() {
        cx.notify();
      }
    });
  }

  fn bounds_for_range(
    &mut self, range_utf16: std::ops::Range<usize>, _window: &mut Window, _cx: &mut App,
  ) -> Option<Bounds<Pixels>> {
    let mut bounds = self.cursor_bounds?;
    bounds.origin.x += px(self.cell_width * range_utf16.start as f32);
    Some(bounds)
  }

  fn apple_press_and_hold_enabled(&mut self) -> bool {
    false
  }

  fn character_index_for_point(
    &mut self, _point: Point<Pixels>, _window: &mut Window, _cx: &mut App,
  ) -> Option<usize> {
    None
  }
}

/// Whether a cell's background is the terminal default (no region painted).
fn is_default_background(color: CellColor) -> bool {
  matches!(color, CellColor::Named(NamedColor::Background))
}

/// Whether a cell draws nothing but an unstyled space.
fn is_blank(cell: &Cell) -> bool {
  cell.c == ' '
    && is_default_background(cell.bg)
    && !cell.flags.contains(CellFlags::INVERSE)
    && !cell.flags.intersects(
      CellFlags::BOLD
        | CellFlags::ITALIC
        | CellFlags::UNDERLINE
        | CellFlags::DIM
        | CellFlags::HIDDEN
        | CellFlags::STRIKEOUT,
    )
    && cell.hyperlink.is_none()
    && cell.zerowidth.is_empty()
}

/// Resolves a cell into a GPUI text run with terminal styling applied.
fn cell_style(cell: &Cell, fg: CellColor, palette: &TerminalPalette, font: gpui::Font) -> TextRun {
  let mut color = palette.convert(&fg);
  if cell.flags.contains(CellFlags::DIM) {
    color.a *= 0.7;
  }

  let underline =
    (cell.flags.contains(CellFlags::UNDERLINE) || cell.hyperlink.is_some()).then(|| {
      UnderlineStyle {
        thickness: px(1.0),
        color: Some(color),
        wavy: cell.flags.contains(CellFlags::UNDERCURL),
      }
    });

  let strikethrough = cell
    .flags
    .contains(CellFlags::STRIKEOUT)
    .then(|| StrikethroughStyle {
      thickness: px(1.0),
      color: Some(color),
    });

  // Bold/italic cells switch weight/style within the embedded multi-face
  // family (`DEFAULT_FONT_FAMILY` now ships all static weights and italics).
  // Requesting a face that exists keeps every run on the same metrics, so the
  // forced per-cell grid stays aligned. CJK runs resolve through the platform
  // fallback list; their glyph positions are still snapped to the cell grid.
  //
  // Do NOT request faces the family doesn't provide (e.g. synthetic-bold a
  // Regular-only family): gpui would fall back to another family whose glyph
  // advances mismatch `cell_width`, overlapping adjacent cells.
  let weight = if cell.flags.contains(CellFlags::BOLD) {
    FontWeight::BOLD
  } else {
    font.weight
  };

  let style = if cell.flags.contains(CellFlags::ITALIC) {
    FontStyle::Italic
  } else {
    FontStyle::Normal
  };

  TextRun {
    len: cell.c.len_utf8(),
    font: gpui::Font {
      weight,
      style,
      ..font
    },
    color,
    background_color: None,
    underline,
    strikethrough,
  }
}

/// Whether the character renders more accurately as filled rectangles than as
/// a font glyph.
fn is_block_element(c: char) -> bool {
  matches!(c as u32, 0x2580..=0x259F | 0x1FB00..=0x1FB3B)
}

/// Returns `(column, line, columns, lines)` in subcell units for block
/// characters that are a single filled rectangle.
fn block_char_to_rect(c: char) -> Option<(i32, i32, i32, i32)> {
  let codepoint = c as u32;
  match codepoint {
    // ▀ upper half
    0x2580 => Some((0, 0, 8, 12)),
    // ▁▂▃▄▅▆▇█ lower blocks of 1..=8 eighths
    0x2581..=0x2588 => {
      let eighths = (codepoint - 0x2580) as i32;
      Some((0, 24 - eighths * 3, 8, eighths * 3))
    }
    // ▉▊▋▌▍▎▏ left blocks of 7..=1 eighths
    0x2589..=0x258F => Some((0, 0, (0x2590 - codepoint) as i32, 24)),
    // ▐ right half
    0x2590 => Some((4, 0, 4, 24)),
    // ▔ upper eighth
    0x2594 => Some((0, 0, 8, 3)),
    // ▕ right eighth
    0x2595 => Some((7, 0, 1, 24)),
    _ => None,
  }
}

/// Returns the filled quadrants of a quadrant character, bit `row * 2 +
/// column`.
fn quadrant_char_to_filled_bits(c: char) -> Option<u8> {
  Some(match c {
    '▘' => 0b0001,
    '▝' => 0b0010,
    '▖' => 0b0100,
    '▗' => 0b1000,
    '▚' => 0b1001,
    '▞' => 0b0110,
    '▛' => 0b0111,
    '▜' => 0b1011,
    '▙' => 0b1101,
    '▟' => 0b1110,
    _ => return None,
  })
}

/// Returns the filled subcells of a sextant character, bit `row * 2 + column`.
///
/// U+1FB00..=U+1FB3B enumerate all 2x3 fill combinations except the four that
/// already exist as block elements.
fn sextant_char_to_filled_bits(c: char) -> Option<u8> {
  let offset = (c as u32).checked_sub(0x1FB00)?;
  if offset > 0x3B {
    return None;
  }
  Some((offset + 1 + u32::from(offset >= 20) + u32::from(offset >= 40)) as u8)
}

/// Approximates the shade characters `░▒▓` with the foreground color at
/// reduced opacity.
fn shade_char_to_opacity(c: char) -> Option<f32> {
  match c {
    '░' => Some(0.25),
    '▒' => Some(0.5),
    '▓' => Some(0.75),
    _ => None,
  }
}

#[allow(clippy::too_many_arguments)]
fn push_block_element_region(
  cell_line: i32, cell_column: i32, column: i32, line: i32, columns: i32, lines: i32, color: Hsla,
  regions: &mut Vec<BackgroundRegion>,
) {
  let start_line = cell_line * BLOCK_SUBCELL_LINES + line;
  let start_column = cell_column * BLOCK_SUBCELL_COLUMNS + column;
  let end_line = start_line + lines - 1;
  let end_column = start_column + columns - 1;
  regions.push(BackgroundRegion::with_extents(
    start_line,
    start_column,
    end_line,
    end_column,
    color,
  ));
}

fn collect_block_element_regions(
  cell_line: i32, cell_column: i32, c: char, color: Hsla, regions: &mut Vec<BackgroundRegion>,
) {
  if let Some((column, line, columns, lines)) = block_char_to_rect(c) {
    push_block_element_region(
      cell_line,
      cell_column,
      column,
      line,
      columns,
      lines,
      color,
      regions,
    );
    return;
  }
  if let Some(filled) = quadrant_char_to_filled_bits(c) {
    for row in 0..2 {
      for column in 0..2 {
        if filled & (1 << (row * 2 + column)) != 0 {
          push_block_element_region(
            cell_line,
            cell_column,
            column * 4,
            row * 12,
            4,
            12,
            color,
            regions,
          );
        }
      }
    }
    return;
  }
  if let Some(filled) = sextant_char_to_filled_bits(c) {
    for row in 0..3 {
      for column in 0..2 {
        if filled & (1 << (row * 2 + column)) != 0 {
          push_block_element_region(
            cell_line,
            cell_column,
            column * 4,
            row * 8,
            4,
            8,
            color,
            regions,
          );
        }
      }
    }
    return;
  }
  if let Some(opacity) = shade_char_to_opacity(c) {
    let mut shade_color = color;
    shade_color.a *= opacity;
    push_block_element_region(cell_line, cell_column, 0, 0, 8, 24, shade_color, regions);
  }
}

/// The default terminal font family.
///
/// Prefers the font embedded by the `resources` feature (registered by
/// `woocraft::init`); falls back to a platform system monospace when resources
/// are disabled.
pub fn default_monospace_family() -> &'static str {
  if cfg!(feature = "resources") {
    crate::DEFAULT_FONT_FAMILY
  } else if cfg!(target_os = "macos") {
    "Menlo"
  } else if cfg!(target_os = "windows") {
    "Consolas"
  } else {
    "monospace"
  }
}

#[cfg(test)]
mod tests {
  use woocraft_terminal::{IndexedCell, Modes, Point as GridPoint, SelectionKind};

  use super::{super::colors::rgb_to_hsla, *};

  fn palette() -> TerminalPalette {
    TerminalPalette::from_theme(&crate::Theme::default())
  }

  #[cfg(test)]
  fn default_font() -> gpui::Font {
    gpui::Font {
      family: default_monospace_family().into(),
      ..Default::default()
    }
  }

  #[cfg(test)]
  fn content_with(cells: Vec<(i32, usize, char, CellFlags)>) -> Content {
    Content {
      cells: cells
        .into_iter()
        .map(|(line, column, c, flags)| IndexedCell {
          point: GridPoint::new(line, column),
          cell: Cell {
            c,
            flags,
            ..Default::default()
          },
        })
        .collect(),
      ..Default::default()
    }
  }

  #[test]
  fn blank_cells_are_skipped() {
    let mut blank = Cell {
      c: ' ',
      ..Default::default()
    };
    assert!(is_blank(&blank));
    blank.flags.insert(CellFlags::BOLD);
    assert!(!is_blank(&blank));
    blank.flags = CellFlags::empty();
    blank.bg = CellColor::Spec(woocraft_terminal::Rgb { r: 1, g: 2, b: 3 });
    assert!(!is_blank(&blank));
  }

  #[test]
  fn adjacent_same_style_cells_batch_into_one_run() {
    let content = content_with(vec![
      (0, 0, 'a', CellFlags::BOLD),
      (0, 1, 'b', CellFlags::BOLD),
      (0, 2, 'c', CellFlags::BOLD),
    ]);
    let (_, runs, _) = TerminalElement::layout_grid(&content, default_font(), px(14.), &palette());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "abc");
    assert_eq!(runs[0].cell_count, 3);
  }

  #[test]
  fn style_changes_split_runs() {
    let content = content_with(vec![
      (0, 0, 'a', CellFlags::empty()),
      (0, 1, 'b', CellFlags::BOLD),
      (0, 2, 'c', CellFlags::BOLD),
    ]);
    let (_, runs, _) = TerminalElement::layout_grid(&content, default_font(), px(14.), &palette());
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].text, "a");
    assert_eq!(runs[1].text, "bc");
  }

  #[test]
  fn wide_char_spacers_are_skipped() {
    let content = content_with(vec![
      (0, 0, '界', CellFlags::WIDE_CHAR),
      (0, 1, ' ', CellFlags::WIDE_CHAR_SPACER),
    ]);
    let (_, runs, _) = TerminalElement::layout_grid(&content, default_font(), px(14.), &palette());
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].text, "界");
    assert_eq!(runs[0].cell_count, 1);
  }

  #[test]
  fn block_elements_become_rects() {
    let content = content_with(vec![
      (0, 0, '█', CellFlags::empty()),
      (0, 1, '█', CellFlags::empty()),
    ]);
    let (_, runs, rects) =
      TerminalElement::layout_grid(&content, default_font(), px(14.), &palette());
    assert!(runs.is_empty());
    // Two adjacent full blocks merge into a single 2-column rect.
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].line, 0);
    assert_eq!(rects[0].column, 0);
    assert_eq!(rects[0].columns, 16);
    assert_eq!(rects[0].lines, 24);
  }

  #[test]
  fn background_regions_merge() {
    let region =
      |line: i32, start_column: i32| BackgroundRegion::new(line, start_column, gpui::black());
    // Adjacent horizontal cells merge; a gap splits regions.
    let merged = merge_background_regions(vec![region(0, 0), region(0, 1), region(0, 3)]);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].start_column, 0);
    assert_eq!(merged[0].end_column, 1);

    // Vertically stacked regions with the same span merge into one.
    let merged = merge_background_regions(vec![region(0, 0), region(1, 0), region(0, 5)]);
    assert_eq!(merged.len(), 2);
    assert_eq!(merged[0].start_line, 0);
    assert_eq!(merged[0].end_line, 1);
    assert_eq!(merged[0].end_column, 0);
  }

  #[test]
  fn inverse_cells_paint_swapped_background() {
    let content = content_with(vec![(0, 0, 'x', CellFlags::INVERSE)]);
    let (rects, ..) = TerminalElement::layout_grid(&content, default_font(), px(14.), &palette());
    assert_eq!(rects.len(), 1);
    // The swapped background is the default foreground color.
    assert_eq!(rects[0].color, palette().foreground);
  }

  #[test]
  fn sextant_mapping_gaps() {
    assert_eq!(sextant_char_to_filled_bits('\u{1FB00}'), Some(1));
    assert_eq!(sextant_char_to_filled_bits('█'), None);
    assert_eq!(sextant_char_to_filled_bits('▌'), None);
    assert_eq!(sextant_char_to_filled_bits('▐'), None);
  }

  #[test]
  fn quadrant_mapping() {
    assert_eq!(quadrant_char_to_filled_bits('▚'), Some(0b1001));
    assert_eq!(quadrant_char_to_filled_bits('▟'), Some(0b1110));
  }

  #[test]
  fn block_rect_mapping() {
    assert_eq!(block_char_to_rect('▀'), Some((0, 0, 8, 12)));
    assert_eq!(block_char_to_rect('█'), Some((0, 0, 8, 24)));
    assert_eq!(block_char_to_rect('▏'), Some((0, 0, 1, 24)));
    assert_eq!(block_char_to_rect('▔'), Some((0, 0, 8, 3)));
  }

  #[test]
  fn default_background_detection() {
    assert!(is_default_background(CellColor::Named(
      NamedColor::Background
    )));
    assert!(!is_default_background(CellColor::Named(NamedColor::Red)));
  }

  #[test]
  fn rgb_conversion() {
    let color = rgb_to_hsla(255, 128, 0);
    let rgba = gpui::Rgba::from(color);
    assert_eq!(rgba.r, 1.0);
    assert_eq!((rgba.g * 255.0).round(), 128.0);
  }

  #[test]
  fn selection_rects_clamp_to_viewport() {
    let mut content = Content::empty();
    content.screen_lines = 5;
    content.columns = 10;
    content.display_offset = 2;
    // Selection from two lines above the viewport to one line down.
    let selection = SelectionRange {
      start: GridPoint::new(-3, 4),
      end: GridPoint::new(1, 6),
      is_block: false,
    };
    let rects = TerminalElement::selection_rects(selection, &content, gpui::black());
    // Viewport lines 0..=3 after applying the display offset of 2. Lines
    // 0..=2 are middle rows (the selection starts above the viewport); line
    // 3 is the tail row.
    assert_eq!(rects.len(), 4);
    assert_eq!(rects[0].line, 0);
    assert_eq!(rects[0].column, 0);
    assert_eq!(rects[0].columns, 10);
    assert_eq!(rects[3].line, 3);
    assert_eq!(rects[3].column, 0);
    assert_eq!(rects[3].columns, 7);
  }

  #[test]
  fn selection_kind_semantics_round_trip() {
    // Words/Lines/Block map onto the emulator's selection types; ensure the
    // kinds are distinct so double/triple-click keep their semantics.
    assert_ne!(SelectionKind::Characters, SelectionKind::Words);
    assert_ne!(SelectionKind::Words, SelectionKind::Lines);
    assert_ne!(SelectionKind::Lines, SelectionKind::Block);
  }

  #[test]
  fn mouse_mode_detection() {
    let modes = Modes::SGR_MOUSE | Modes::MOUSE_REPORT_CLICK;
    assert!(modes.intersects(Modes::MOUSE_MODE));
  }
}
