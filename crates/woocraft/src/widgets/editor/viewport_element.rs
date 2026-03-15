use std::{ops::Range, rc::Rc};

use gpui::{
  App, Bounds, ContentMask, Element, ElementId, ElementInputHandler, Entity, GlobalElementId,
  HighlightStyle, Hsla, IntoElement, LayoutId, MouseButton, MouseMoveEvent, MouseUpEvent, Path,
  Pixels, ShapedLine, SharedString, Style, TextRun, UnderlineStyle, Window, fill, point, px,
  relative, size,
};
use ropey::Rope;
use smallvec::SmallVec;

use super::{
  blink_cursor::CURSOR_WIDTH,
  highlighter::HighlightTheme,
  mode::InputMode,
  rope_ext::RopeExt as _,
  state::{InputState, LastLayout, VisibleLine, WhitespaceIndicators},
  text_wrapper::{LineLayout, break_all_ranges},
  viewport,
};
use crate::{ActiveTheme as _, ColorExt as _, paint_caret};

const OVERSCAN_ROWS: usize = 2;
const LINE_NUMBER_LEFT_MARGIN: Pixels = px(24.);
const LINE_NUMBER_GUTTER_RIGHT_PADDING: Pixels = px(8.);
const LINE_NUMBER_TEXT_GAP: Pixels = px(16.);

pub(super) struct ViewportElement {
  pub(crate) state: Entity<InputState>,
  placeholder: SharedString,
}

pub(super) struct PrepaintState {
  last_layout: LastLayout,
  line_numbers: Option<Vec<SmallVec<[ShapedLine; 1]>>>,
  cursor_bounds: Option<Bounds<Pixels>>,
  current_row: Option<usize>,
  selection_path: Option<Path<Pixels>>,
  hover_highlight_path: Option<Path<Pixels>>,
  search_match_paths: Vec<(Path<Pixels>, bool)>,
  document_color_paths: Vec<(Path<Pixels>, Hsla)>,
  hover_definition_hitbox: Option<gpui::Hitbox>,
  indent_guides_path: Option<Path<Pixels>>,
  active_indent_guide_path: Option<Path<Pixels>>,
  bounds: Bounds<Pixels>,
}

struct VisibleRowLayout {
  lines: Vec<VisibleLine>,
  logical_range: Range<usize>,
  byte_range: Range<usize>,
}

struct LayoutLinesParams<'a> {
  display_text: &'a Rope,
  last_layout: &'a LastLayout,
  font_size: Pixels,
  runs: &'a [TextRun],
  bg_segments: &'a [(Range<usize>, Hsla)],
  whitespace_indicators: Option<WhitespaceIndicators>,
}

impl ViewportElement {
  pub(super) fn new(state: Entity<InputState>) -> Self {
    Self {
      state,
      placeholder: SharedString::default(),
    }
  }

  pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
    self.placeholder = placeholder.into();
    self
  }

  fn paint_mouse_listeners(&mut self, window: &mut Window, _: &mut App) {
    window.on_mouse_event({
      let state = self.state.clone();

      move |event: &MouseMoveEvent, _, window, cx| {
        state.update(cx, |state, cx| {
          if state.scrollbar_dragging {
            if event.pressed_button != Some(MouseButton::Left) {
              state.scrollbar_dragging = false;
              return;
            }

            if state.drag_scrollbar_to(event.position, window) {
              cx.notify();
            }
            cx.stop_propagation();
            return;
          }

          if event.pressed_button == Some(MouseButton::Left) {
            state.on_drag_move(event, window, cx);
          }
        });
      }
    });

    window.on_mouse_event({
      let state = self.state.clone();

      move |_event: &MouseUpEvent, _, _window, cx| {
        state.update(cx, |state, _| {
          state.scrollbar_dragging = false;
        });
      }
    });
  }

  fn visible_layout(
    &self, state: &InputState, viewport_height: Pixels, line_height: Pixels,
  ) -> VisibleRowLayout {
    let total_logical_rows = state.line_count_u64() as usize;
    if total_logical_rows == 0 {
      return VisibleRowLayout {
        lines: Vec::new(),
        logical_range: 0..0,
        byte_range: 0..0,
      };
    }

    let total_display_rows = state.display_row_count();
    let viewport_rows = viewport::viewport_rows(viewport_height, line_height);
    let start_display_row =
      viewport::clamp_top_row(state.top_row, total_display_rows, viewport_rows);
    let mut remaining_display_rows = viewport_rows.saturating_add(OVERSCAN_ROWS);
    let mut display_row = start_display_row;
    let mut visible_lines = Vec::new();

    while display_row < total_display_rows && remaining_display_rows > 0 {
      let (row, local_row) = state.display_row_to_line_row(display_row);
      let Some(line) = state.text_wrapper.line(row) else {
        break;
      };

      let take = remaining_display_rows.min(line.lines_len().saturating_sub(local_row));
      let wrap_end = local_row + take;
      let line_start_offset = state.text.line_start_offset(row);
      let byte_start = line
        .wrapped_lines
        .get(local_row)
        .map(|range| line_start_offset + range.start)
        .unwrap_or(line_start_offset);
      let byte_end = line
        .wrapped_lines
        .get(wrap_end.saturating_sub(1))
        .map(|range| line_start_offset + range.end)
        .unwrap_or(line_start_offset);

      visible_lines.push(VisibleLine {
        row,
        line_start_offset,
        wrap_range: local_row..wrap_end,
        byte_range: byte_start..byte_end,
      });

      display_row += take;
      remaining_display_rows = remaining_display_rows.saturating_sub(take);
    }

    let logical_start = visible_lines.first().map(|line| line.row).unwrap_or(0);
    let logical_end = visible_lines
      .last()
      .map(|line| line.row + 1)
      .unwrap_or(logical_start);
    let byte_start = visible_lines
      .first()
      .map(|line| line.byte_range.start)
      .unwrap_or(0);
    let byte_end = visible_lines
      .last()
      .map(|line| line.byte_range.end)
      .unwrap_or(byte_start);

    VisibleRowLayout {
      lines: visible_lines,
      logical_range: logical_start..logical_end,
      byte_range: byte_start..byte_end,
    }
  }

  fn layout_cursor(
    &self, last_layout: &LastLayout, bounds: Bounds<Pixels>, _window: &mut Window, cx: &mut App,
  ) -> (Option<Bounds<Pixels>>, Option<usize>) {
    let state = self.state.read(cx);
    let line_height = last_layout.line_height;
    let line_number_width = last_layout.line_number_width;

    let mut cursor = state.cursor();
    if state.masked {
      cursor = state.text.offset_to_char_index(cursor);
    }

    let row = state.text.offset_to_point(cursor).row;
    let Some(line_ix) = last_layout
      .visible_lines
      .iter()
      .position(|line| line.row == row)
    else {
      return (None, None);
    };
    let Some(line) = last_layout.lines.get(line_ix) else {
      return (None, None);
    };

    let mut offset_y = last_layout.visible_top;
    for line in last_layout.lines.iter().take(line_ix) {
      offset_y += line.size(line_height).height;
    }

    let row_start = state.text.line_start_offset(row);
    let local_offset = cursor.saturating_sub(row_start);
    let Some(pos) = line.position_for_index(local_offset, last_layout) else {
      return (None, None);
    };

    let cursor_height = (line_height - px(1.)).max(px(1.));
    let cursor_bounds = Bounds::new(
      point(
        bounds.left() + pos.x + line_number_width,
        bounds.top() + offset_y + pos.y + ((line_height - cursor_height) / 2.),
      ),
      size(CURSOR_WIDTH, cursor_height),
    );

    (Some(cursor_bounds), Some(row))
  }

  fn layout_match_range(
    range: Range<usize>, last_layout: &LastLayout, bounds: &Bounds<Pixels>,
  ) -> Option<Path<Pixels>> {
    if range.is_empty() {
      return None;
    }

    if range.start < last_layout.visible_range_offset.start
      || range.end > last_layout.visible_range_offset.end
    {
      return None;
    }

    let line_height = last_layout.line_height;
    let lines = &last_layout.lines;
    let line_number_width = last_layout.line_number_width;
    let path_origin = bounds.origin + point(line_number_width, px(0.));

    let start_ix = range.start;
    let end_ix = range.end;

    let mut offset_y = last_layout.visible_top;
    let mut builder = gpui::PathBuilder::fill();
    let mut has_rect = false;

    for (line, visible_line) in lines.iter().zip(last_layout.visible_lines.iter()) {
      let line_size = line.size(line_height);
      let line_origin = point(px(0.), offset_y);

      let line_cursor_start = line.position_for_index(
        start_ix.saturating_sub(visible_line.line_start_offset),
        last_layout,
      );
      let line_cursor_end = line.position_for_index(
        end_ix.saturating_sub(visible_line.line_start_offset),
        last_layout,
      );

      if line_cursor_start.is_some() || line_cursor_end.is_some() {
        let start = line_cursor_start.unwrap_or_else(|| line.first_visible_position(last_layout));
        let end = line_cursor_end.unwrap_or_else(|| line.last_visible_position(last_layout));

        let start_wrap = (start.y / line_height).floor() as usize;
        let end_wrap = (end.y / line_height).floor() as usize;

        for wrap_ix in start_wrap..=end_wrap {
          let left = if wrap_ix == start_wrap {
            start.x
          } else {
            px(0.)
          };
          let right = if wrap_ix == end_wrap {
            end.x
          } else {
            line
              .wrapped_lines
              .get(wrap_ix)
              .map(|wrapped| wrapped.width)
              .unwrap_or(line_size.width)
          }
          .max(left + px(6.));
          let top = line_origin.y + wrap_ix as f32 * line_height;
          let rect_top_left = path_origin + point(left, top);
          let rect_top_right = path_origin + point(right, top);
          let rect_bottom_right = rect_top_right + point(px(0.), line_height);
          let rect_bottom_left = rect_top_left + point(px(0.), line_height);

          builder.move_to(rect_top_left);
          builder.line_to(rect_top_right);
          builder.line_to(rect_bottom_right);
          builder.line_to(rect_bottom_left);
          builder.close();
          has_rect = true;
        }
      }

      if line_cursor_start.is_some() && line_cursor_end.is_some() {
        break;
      }

      offset_y += line_size.height;
    }

    if !has_rect {
      return None;
    }

    builder.build().ok()
  }

  fn layout_search_matches(
    &self, last_layout: &LastLayout, bounds: &Bounds<Pixels>, cx: &mut App,
  ) -> Vec<(Path<Pixels>, bool)> {
    let search_panel = self.state.read(cx).search_panel.clone();
    let Some((ranges, current_match_ix)) = search_panel.and_then(|panel| {
      panel
        .read(cx)
        .matcher()
        .map(|matcher| (matcher.matched_ranges.clone(), matcher.current_match_ix))
    }) else {
      return vec![];
    };

    let mut paths = Vec::new();
    for (index, range) in ranges.as_ref().iter().enumerate() {
      if let Some(path) = Self::layout_match_range(range.clone(), last_layout, bounds) {
        paths.push((path, current_match_ix == index));
      }
    }

    paths
  }

  fn layout_hover_highlight(
    &self, last_layout: &LastLayout, bounds: &Bounds<Pixels>, cx: &mut App,
  ) -> Option<Path<Pixels>> {
    let hover_popover = self.state.read(cx).hover_popover.clone();
    let symbol_range = hover_popover.map(|popover| popover.read(cx).symbol_range.clone())?;

    Self::layout_match_range(symbol_range, last_layout, bounds)
  }

  fn layout_document_colors(
    &self, document_colors: &[(Range<usize>, Hsla)], last_layout: &LastLayout,
    bounds: &Bounds<Pixels>,
  ) -> Vec<(Path<Pixels>, Hsla)> {
    let mut paths = vec![];
    for (range, color) in document_colors.iter() {
      if let Some(path) = Self::layout_match_range(range.clone(), last_layout, bounds) {
        paths.push((path, *color));
      }
    }

    paths
  }

  fn layout_selections(
    &self, last_layout: &LastLayout, bounds: &Bounds<Pixels>, window: &mut Window, cx: &mut App,
  ) -> Option<Path<Pixels>> {
    let state = self.state.read(cx);
    if !state.focus_handle.is_focused(window) && !state.shared_context_menu_open {
      return None;
    }

    let mut selected_range = state.selected_range;
    if let Some(ime_marked_range) = &state.ime_marked_range
      && !ime_marked_range.is_empty()
    {
      selected_range = (ime_marked_range.end..ime_marked_range.end).into();
    }
    if selected_range.is_empty() {
      return None;
    }

    if state.masked {
      selected_range.start = state.text.offset_to_char_index(selected_range.start);
      selected_range.end = state.text.offset_to_char_index(selected_range.end);
    }

    let (start_ix, end_ix) = if selected_range.start < selected_range.end {
      (selected_range.start, selected_range.end)
    } else {
      (selected_range.end, selected_range.start)
    };

    let range = start_ix.max(last_layout.visible_range_offset.start)
      ..end_ix.min(last_layout.visible_range_offset.end);

    Self::layout_match_range(range, last_layout, bounds)
  }

  fn layout_lines(
    state: &InputState, params: LayoutLinesParams<'_>, window: &mut Window,
  ) -> Vec<LineLayout> {
    let LayoutLinesParams {
      display_text,
      last_layout,
      font_size,
      runs,
      bg_segments,
      whitespace_indicators,
    } = params;
    let mut lines = vec![];
    let wrap_width = last_layout.wrap_width.unwrap_or(last_layout.content_width);
    for visible_line in last_layout.visible_lines.iter() {
      let row = visible_line.row;
      let slice_start_offset = visible_line
        .byte_range
        .start
        .saturating_sub(visible_line.line_start_offset);
      let slice_end_offset = visible_line
        .byte_range
        .end
        .saturating_sub(visible_line.line_start_offset);
      let visible_offset = visible_line
        .byte_range
        .start
        .saturating_sub(last_layout.visible_range_offset.start);
      let wrapped_ranges = if !state.masked {
        state
          .text_wrapper
          .line(row)
          .map(|line| {
            line.wrapped_lines[visible_line.wrap_range.clone()]
              .iter()
              .map(|range| {
                range.start.saturating_sub(slice_start_offset)
                  ..range.end.saturating_sub(slice_start_offset)
              })
              .collect::<Vec<_>>()
          })
          .unwrap_or_else(|| {
            std::iter::once(0..slice_end_offset.saturating_sub(slice_start_offset)).collect()
          })
      } else {
        let line = display_text.slice_line(row).to_string();
        let full_line_runs = runs_for_range(runs, visible_offset, &(0..line.len()));
        let wrapped = break_all_ranges(
          &line,
          &window.text_system().shape_line(
            line.to_string().into(),
            font_size,
            &full_line_runs,
            None,
          ),
          wrap_width,
        );

        wrapped
          .get(visible_line.wrap_range.clone())
          .map(ToOwned::to_owned)
          .unwrap_or_else(|| std::iter::once(0..line.len()).collect())
      };

      let line = if state.masked {
        display_text.slice_line(row).to_string()
      } else {
        display_text
          .slice(visible_line.byte_range.start..visible_line.byte_range.end)
          .to_string()
      };
      let mut wrapped_lines = SmallVec::with_capacity(1);
      for range in &wrapped_ranges {
        let sub_line_runs = runs_for_range(runs, visible_offset, range);
        let sub_line_runs = if bg_segments.is_empty() {
          sub_line_runs
        } else {
          split_runs_by_bg_segments(visible_line.byte_range.start, &sub_line_runs, bg_segments)
        };

        let sub_line: SharedString = line[range.clone()].to_string().into();
        let shaped_line =
          window
            .text_system()
            .shape_line(sub_line, font_size, &sub_line_runs, None);
        wrapped_lines.push(shaped_line);
      }

      lines.push(
        LineLayout::new()
          .visible_slice(
            display_text.slice_line(row).len(),
            slice_start_offset,
            wrapped_lines,
          )
          .with_whitespaces(whitespace_indicators.clone()),
      );
    }

    lines
  }

  fn highlight_lines(
    &self, _visible_range: &Range<usize>, visible_byte_range: Range<usize>, cx: &mut App,
  ) -> Option<Vec<(Range<usize>, HighlightStyle)>> {
    let state = self.state.read(cx);
    let snapshot = state.current_backend_snapshot()?;
    let diagnostics = match &state.mode {
      InputMode::CodeEditor { diagnostics, .. } => diagnostics,
      _ => return None,
    };
    let highlighter = state.highlighter.as_ref()?;

    let highlight_theme = HighlightTheme::from_theme(cx.theme());
    let mut styles = highlighter
      .highlight_range(
        snapshot.as_ref(),
        visible_byte_range.start as u64..visible_byte_range.end as u64,
        &highlight_theme,
      )
      .into_iter()
      .map(|(range, style)| (range.start as usize..range.end as usize, style))
      .collect::<Vec<_>>();

    let diagnostic_styles = diagnostics.styles_for_range(&visible_byte_range, cx);
    if let Some(hover_style) = self.layout_hover_definition(cx) {
      styles.push(hover_style);
    }
    styles = gpui::combine_highlights(diagnostic_styles, styles).collect();
    Some(styles)
  }
}

impl IntoElement for ViewportElement {
  type Element = Self;

  fn into_element(self) -> Self::Element {
    self
  }
}

impl Element for ViewportElement {
  type RequestLayoutState = ();
  type PrepaintState = PrepaintState;

  fn id(&self) -> Option<ElementId> {
    None
  }

  fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
    None
  }

  fn request_layout(
    &mut self, _id: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    window: &mut Window, cx: &mut App,
  ) -> (LayoutId, Self::RequestLayoutState) {
    let state = self.state.read(cx);
    let line_height = window.line_height();

    let mut style = Style::default();
    style.size.width = relative(1.).into();
    style.flex_grow = 1.0;
    style.size.height = relative(1.).into();
    if state.mode.is_auto_grow() {
      let rows = state.mode.max_rows().min(state.mode.rows());
      style.min_size.height = (rows * line_height).into();
    } else {
      style.min_size.height = line_height.into();
    }

    (window.request_layout(style, [], cx), ())
  }

  fn prepaint(
    &mut self, _id: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    bounds: Bounds<Pixels>, _request_layout: &mut Self::RequestLayoutState, window: &mut Window,
    cx: &mut App,
  ) -> Self::PrepaintState {
    let style = window.text_style();
    let text_size = style.font_size.to_pixels(window.rem_size());
    let font = style.font();
    let invisible_color = cx.theme().editor_invisible;
    let (line_number_width, whitespace_indicators) = self.state.update(cx, |state, _| {
      (
        state.cached_line_number_width(font.clone(), text_size, window),
        state.cached_whitespace_indicators(font.clone(), text_size, invisible_color, window),
      )
    });
    let state = self.state.read(cx);
    let line_height = window.line_height();
    let wrap_width = (bounds.size.width - line_number_width).max(px(1.0));
    let _ = state;
    self.state.update(cx, |state, cx| {
      state.sync_wrap_metrics_for_view(wrap_width, window, cx);
      state.clamp_top_row(line_height);
    });

    let state = self.state.read(cx);
    let visible_layout = self.visible_layout(state, bounds.size.height, line_height);
    let visible_range = visible_layout.logical_range.clone();
    let visible_start_offset = visible_layout.byte_range.start;
    let visible_end_offset = visible_layout.byte_range.end;

    let is_text_empty = state.text.len() == 0;
    let ime_marked_range = state.ime_marked_range;
    let (display_text, text_color) = if is_text_empty {
      (
        Rope::from(self.placeholder.as_str()),
        cx.theme().muted_foreground,
      )
    } else if state.masked {
      (
        Rope::from(
          state
            .text
            .chars()
            .map(|ch| if matches!(ch, '\n' | '\r') { ch } else { '*' })
            .collect::<String>(),
        ),
        cx.theme().foreground,
      )
    } else {
      (state.text.clone(), cx.theme().foreground)
    };

    let text_style = window.text_style();
    let wrap_width = Some(wrap_width);

    let mut last_layout = LastLayout {
      visible_range: visible_range.clone(),
      visible_top: px(0.),
      visible_range_offset: visible_start_offset..visible_end_offset,
      line_height,
      wrap_width,
      line_number_width,
      lines: Rc::new(vec![]),
      visible_lines: Rc::new(visible_layout.lines.clone()),
      cursor_bounds: None,
      text_align: state.text_align,
      content_width: bounds.size.width - line_number_width,
    };

    let run = TextRun {
      len: display_text.len(),
      font: style.font(),
      color: text_color,
      background_color: None,
      underline: None,
      strikethrough: None,
    };
    let marked_run = TextRun {
      len: 0,
      font: style.font(),
      color: text_color,
      background_color: None,
      underline: Some(UnderlineStyle {
        thickness: px(1.),
        color: Some(text_color),
        wavy: false,
      }),
      strikethrough: None,
    };

    let document_colors = state
      .lsp
      .document_colors_for_range(&display_text, &last_layout.visible_range);
    let _ = state;

    let highlight_styles =
      self.highlight_lines(&visible_range, visible_start_offset..visible_end_offset, cx);
    let runs = if !is_text_empty {
      if let Some(highlight_styles) = highlight_styles {
        let mut runs = vec![];
        for (range, style) in highlight_styles {
          let mut run = text_style.clone().highlight(style).to_run(range.len());
          if let Some(ime_marked_range) = &ime_marked_range
            && range.start >= ime_marked_range.start
            && range.end <= ime_marked_range.end
          {
            run.color = marked_run.color;
            run.strikethrough = marked_run.strikethrough;
            run.underline = marked_run.underline;
          }
          runs.push(run);
        }
        runs.into_iter().filter(|run| run.len > 0).collect()
      } else {
        vec![run]
      }
    } else if let Some(ime_marked_range) = &ime_marked_range {
      vec![
        TextRun {
          len: ime_marked_range.start,
          ..run.clone()
        },
        TextRun {
          len: ime_marked_range.end - ime_marked_range.start,
          underline: marked_run.underline,
          ..run.clone()
        },
        TextRun {
          len: display_text.len() - ime_marked_range.end,
          ..run.clone()
        },
      ]
      .into_iter()
      .filter(|run| run.len > 0)
      .collect()
    } else {
      vec![run]
    };

    let lines = Self::layout_lines(
      self.state.read(cx),
      LayoutLinesParams {
        display_text: &display_text,
        last_layout: &last_layout,
        font_size: text_size,
        runs: &runs,
        bg_segments: &document_colors,
        whitespace_indicators,
      },
      window,
    );
    last_layout.lines = Rc::new(lines);

    let (cursor_bounds, current_row) = self.layout_cursor(&last_layout, bounds, window, cx);
    last_layout.cursor_bounds = cursor_bounds;

    let search_match_paths = self.layout_search_matches(&last_layout, &bounds, cx);
    let selection_path = self.layout_selections(&last_layout, &bounds, window, cx);
    let hover_highlight_path = self.layout_hover_highlight(&last_layout, &bounds, cx);
    let document_color_paths = self.layout_document_colors(&document_colors, &last_layout, &bounds);

    let state = self.state.read(cx);
    let hover_definition_hitbox = self.layout_hover_definition_hitbox(state, window, cx);
    let (indent_guides_path, active_indent_guide_path) =
      self.layout_indent_guides(state, &bounds, &last_layout, &text_style, window);
    let line_numbers = if state.mode.line_number() {
      let mut line_numbers = vec![];
      for (ix, line) in last_layout.lines.iter().enumerate() {
        let row = last_layout.visible_lines[ix].row;
        let line_number_text = state.line_number_text_for_row(row as u64);
        let line_no: SharedString = line_number_text.into();
        let line_no_len = line_no.len();
        let color = if current_row == Some(row) {
          cx.theme().primary
        } else {
          cx.theme().muted_foreground
        };
        let runs = vec![TextRun {
          len: line_no_len,
          font: style.font(),
          color,
          background_color: None,
          underline: None,
          strikethrough: None,
        }];
        let mut sub_lines: SmallVec<[ShapedLine; 1]> = SmallVec::new();
        sub_lines.push(
          window
            .text_system()
            .shape_line(line_no, text_size, &runs, None),
        );
        for _ in 0..line.wrapped_lines.len().saturating_sub(1) {
          sub_lines.push(ShapedLine::default());
        }
        line_numbers.push(sub_lines);
      }
      Some(line_numbers)
    } else {
      None
    };

    PrepaintState {
      last_layout,
      line_numbers,
      cursor_bounds,
      current_row,
      selection_path,
      hover_highlight_path,
      search_match_paths,
      document_color_paths,
      hover_definition_hitbox,
      indent_guides_path,
      active_indent_guide_path,
      bounds,
    }
  }

  fn paint(
    &mut self, _id: Option<&GlobalElementId>, _: Option<&gpui::InspectorElementId>,
    input_bounds: Bounds<Pixels>, _request_layout: &mut Self::RequestLayoutState,
    prepaint: &mut Self::PrepaintState, window: &mut Window, cx: &mut App,
  ) {
    let state_for_caret = self.state.read(cx);
    let focus_handle = state_for_caret.focus_handle.clone();
    let show_cursor = state_for_caret.show_cursor(window, cx);
    let caret_opacity = state_for_caret.caret_opacity(cx);
    let _ = state_for_caret;
    let focused = focus_handle.is_focused(window);
    let bounds = prepaint.bounds;
    let selected_range = self.state.read(cx).selected_range;

    window.handle_input(
      &focus_handle,
      ElementInputHandler::new(bounds, self.state.clone()),
      cx,
    );

    window.with_content_mask(
      Some(ContentMask {
        bounds: input_bounds,
      }),
      |window| {
        let line_height = window.line_height();
        let origin = bounds.origin;
        let active_line_color = Some(cx.theme().editor_active_line);

        let mut offset_y = prepaint.last_layout.visible_top;
        if let Some(line_numbers) = prepaint.line_numbers.as_ref() {
          for (ix, lines) in line_numbers.iter().enumerate() {
            let row = prepaint.last_layout.visible_lines[ix].row;
            let is_active = prepaint.current_row == Some(row);
            let p = point(input_bounds.origin.x, origin.y + offset_y);
            let height = line_height * lines.len() as f32;
            if is_active && let Some(bg_color) = active_line_color {
              window.paint_quad(fill(
                Bounds::new(p, size(bounds.size.width, height)),
                bg_color,
              ));
            }
            offset_y += height;
          }
        }

        if let Some(path) = prepaint.selection_path.take() {
          window.paint_path(path, cx.theme().selection);
        }
        if let Some(path) = prepaint.hover_highlight_path.take() {
          window.paint_path(path, cx.theme().selection.opacity(0.35));
        }
        for (path, is_current) in prepaint.search_match_paths.drain(..) {
          let color = if is_current {
            cx.theme().warning
          } else {
            cx.theme().warning.opacity(0.35)
          };
          window.paint_path(path, color);
        }
        for (path, color) in prepaint.document_color_paths.drain(..) {
          window.paint_path(path, color.opacity(0.45));
        }
        if let Some(path) = prepaint.indent_guides_path.take() {
          window.paint_path(path, cx.theme().foreground.opacity(0.2));
        }
        if let Some(path) = prepaint.active_indent_guide_path.take() {
          window.paint_path(path, cx.theme().foreground.opacity(0.3));
        }

        let mut offset_y = prepaint.last_layout.visible_top;
        for line in prepaint.last_layout.lines.iter() {
          let p = point(
            origin.x + prepaint.last_layout.line_number_width,
            origin.y + offset_y,
          );

          line.paint(
            p,
            line_height,
            prepaint.last_layout.text_align,
            Some(prepaint.last_layout.content_width),
            window,
            cx,
          );

          offset_y += line.size(line_height).height;
        }

        if let Some(line_numbers) = prepaint.line_numbers.as_ref() {
          let gutter_width =
            (prepaint.last_layout.line_number_width - LINE_NUMBER_TEXT_GAP).max(px(0.));
          let mut offset_y = prepaint.last_layout.visible_top;

          for (ix, lines) in line_numbers.iter().enumerate() {
            let row = prepaint.last_layout.visible_lines[ix].row;
            let row_bg_origin = point(input_bounds.origin.x, origin.y + offset_y);
            let is_active = prepaint.current_row == Some(row);

            let height = line_height * lines.len() as f32;
            if is_active && let Some(bg_color) = active_line_color {
              window.paint_quad(fill(
                Bounds::new(row_bg_origin, size(gutter_width, height)),
                bg_color,
              ));
            }

            for line in lines {
              let line_x = (input_bounds.origin.x + prepaint.last_layout.line_number_width
                - LINE_NUMBER_TEXT_GAP
                - LINE_NUMBER_GUTTER_RIGHT_PADDING
                - line.width)
                .max(input_bounds.origin.x + LINE_NUMBER_LEFT_MARGIN);
              let p = point(line_x, origin.y + offset_y);
              _ = line.paint(p, line_height, window, cx);
              offset_y += line_height;
            }
          }
        }

        if focused
          && show_cursor
          && let Some(cursor_bounds) = prepaint.cursor_bounds
        {
          paint_caret(window, cursor_bounds, cx.theme().primary, caret_opacity);
        }
      },
    );

    self.state.update(cx, |state, _| {
      state.last_layout = Some(prepaint.last_layout.clone());
      state.last_bounds = Some(bounds);
      state.last_cursor = Some(state.cursor());
      state.input_bounds = input_bounds;
      state.last_selected_range = Some(selected_range);
      state.top_row = viewport::clamp_top_row(
        state.top_row,
        state.display_row_count(),
        viewport::viewport_rows(input_bounds.size.height, prepaint.last_layout.line_height),
      );
    });

    if let Some(hitbox) = prepaint.hover_definition_hitbox.as_ref() {
      window.set_cursor_style(gpui::CursorStyle::PointingHand, hitbox);
    }

    self.paint_mouse_listeners(window, cx);
  }
}

pub(super) fn runs_for_range(
  runs: &[TextRun], line_offset: usize, range: &Range<usize>,
) -> Vec<TextRun> {
  let mut result = vec![];
  let range = (line_offset + range.start)..(line_offset + range.end);
  let mut cursor = 0;

  for run in runs {
    let run_start = cursor;
    let run_end = cursor + run.len;

    if run_end <= range.start {
      cursor = run_end;
      continue;
    }

    if run_start >= range.end {
      break;
    }

    let start = range.start.max(run_start) - run_start;
    let end = range.end.min(run_end) - run_start;
    let len = end - start;

    if len > 0 {
      result.push(TextRun { len, ..run.clone() });
    }

    cursor = run_end;
  }

  result
}

fn split_runs_by_bg_segments(
  start_offset: usize, runs: &[TextRun], bg_segments: &[(Range<usize>, Hsla)],
) -> Vec<TextRun> {
  let mut result = vec![];
  let mut cursor = start_offset;
  for run in runs {
    let mut run_start = cursor;
    let run_end = cursor + run.len;

    for (bg_range, bg_color) in bg_segments {
      if run_end <= bg_range.start || run_start >= bg_range.end {
        continue;
      }

      if run_start < bg_range.start {
        result.push(TextRun {
          len: bg_range.start - run_start,
          ..run.clone()
        });
      }

      let overlap_start = run_start.max(bg_range.start);
      let overlap_end = run_end.min(bg_range.end);
      let text_color = if bg_color.l >= 0.5 {
        gpui::black()
      } else {
        gpui::white()
      };

      let run_len = overlap_end.saturating_sub(overlap_start);
      if run_len > 0 {
        result.push(TextRun {
          len: run_len,
          color: text_color,
          ..run.clone()
        });
        cursor = bg_range.end;
        run_start = cursor;
      }
    }

    if run_end > cursor {
      result.push(TextRun {
        len: run_end - cursor,
        ..run.clone()
      });
    }

    cursor = run_end;
  }

  result
}
