//! Table measurement and rendering.
//!
//! Columns are measured by shaping each cell's text with the real font, so
//! wide tables get a real horizontal extent to scroll instead of clipped text.
//! Alignment is applied by the cell container (flex), never by `text_center()`
//! on the text: gpui resolves text alignment only at paint time, so an aligned
//! text element still reports left-aligned geometry from its layout and pointer
//! mapping through it would land on the wrong character.

use gpui::{
  AnyElement, App, ElementId, FontWeight, InteractiveElement as _, IntoElement, ParentElement as _,
  Pixels, StatefulInteractiveElement as _, Styled as _, Window, div, prelude::FluentBuilder as _,
  px, rems,
};

use super::{
  element::{
    RenderContext, TABLE_CELL_PX, TABLE_MAX_COLUMN_WIDTH, TABLE_MIN_COLUMN_WIDTH, render_inlines,
  },
  palette::{InlineRender, MarkdownPalette},
  parser::{MarkdownAlignment, MarkdownTable, MarkdownTableRow},
};
use crate::{ActiveTheme as _, StyledExt as _, h_flex, v_flex};

pub(super) fn measure_table_columns(
  table: &MarkdownTable, alignments: &[MarkdownAlignment], window: &Window, cx: &App,
) -> Vec<Pixels> {
  let rem_size = window.rem_size();
  let text_style = window.text_style();
  let font_size = text_style.font_size.to_pixels(rem_size);
  let padding = rems(TABLE_CELL_PX * 2.).to_pixels(rem_size);
  let min_width = rems(TABLE_MIN_COLUMN_WIDTH).to_pixels(rem_size);
  let max_width = rems(TABLE_MAX_COLUMN_WIDTH).to_pixels(rem_size);
  let palette = MarkdownPalette::new(cx.theme());

  let mut widths = vec![px(0.); alignments.len()];
  let measure = |row: &MarkdownTableRow, bold: bool, widths: &mut Vec<Pixels>| {
    for (index, inlines) in row.cells.iter().enumerate() {
      let Some(width) = widths.get_mut(index) else {
        continue;
      };
      // `shape_line` asserts its input has no newlines, and a hard break
      // (`<br>` or a trailing backslash) inside a cell produces one. Measure
      // the longest visual line instead of the raw text.
      let text = InlineRender::build(inlines, &palette)
        .text
        .replace('\n', " ");
      if text.is_empty() {
        continue;
      }

      let mut style = text_style.clone();
      if bold {
        style.font_weight = FontWeight::BOLD;
      }
      let runs = [style.to_run(text.len())];
      let shaped = window.text_system().shape_line(
        text.clone().into(),
        font_size,
        &runs,
        Some(max_width - padding),
      );
      *width = (*width).max(shaped.width + padding);
    }
  };

  measure(&table.header, true, &mut widths);
  for row in &table.rows {
    measure(row, false, &mut widths);
  }

  widths
    .into_iter()
    .map(|width| width.max(min_width).min(max_width))
    .collect()
}

pub(super) fn render_table(
  table: &MarkdownTable, ctx: &RenderContext<'_>, ids: &mut usize, window: &mut Window,
  cx: &mut App,
) -> AnyElement {
  let column_count = table.alignments.len().max(table.header.cells.len()).max(
    table
      .rows
      .iter()
      .map(|row| row.cells.len())
      .max()
      .unwrap_or(0),
  );

  let alignments: Vec<MarkdownAlignment> = (0..column_count)
    .map(|index| {
      table
        .alignments
        .get(index)
        .copied()
        .unwrap_or(MarkdownAlignment::None)
    })
    .collect();

  let widths = measure_table_columns(table, &alignments, window, cx);
  let total: Pixels = widths.iter().copied().sum();

  let layout = TableLayout {
    alignments: &alignments,
    widths: &widths,
    total,
  };

  let mut rendered_rows = Vec::with_capacity(table.rows.len() + 1);
  rendered_rows.push(render_table_row(
    &table.header,
    &layout,
    RowStyle {
      header: true,
      border_bottom: !table.rows.is_empty(),
      striped: false,
    },
    ctx,
    ids,
    window,
  ));

  for (index, row) in table.rows.iter().enumerate() {
    rendered_rows.push(render_table_row(
      row,
      &layout,
      RowStyle {
        header: false,
        border_bottom: index + 1 < table.rows.len(),
        // Alternating rows use the same stripe color as the table component.
        striped: index.is_multiple_of(2),
      },
      ctx,
      ids,
      window,
    ));
  }

  let id = *ids;
  *ids += 1;

  // Wide tables scroll horizontally instead of clipping: `flex_shrink_0`
  // cells keep their measured width, so the row's intrinsic width is the sum
  // of the columns and the scroll container gets a real extent to scroll.
  div()
    .w_full()
    .mb(ctx.block_margin())
    .id(ElementId::NamedInteger(ctx.id_prefix.clone(), id as u64))
    .overflow_x_scroll()
    .child(
      v_flex()
        .min_w_full()
        .rounded(ctx.palette.radius)
        .border_1()
        .border_color(ctx.palette.border)
        .overflow_hidden()
        .children(rendered_rows),
    )
    .into_any_element()
}

#[allow(clippy::too_many_arguments)]
/// Column geometry shared by every row of one table.
struct TableLayout<'a> {
  alignments: &'a [MarkdownAlignment],
  widths: &'a [Pixels],
  /// Sum of all column widths, used to share extra space proportionally.
  total: Pixels,
}

/// How one table row differs from its siblings.
#[derive(Clone, Copy)]
struct RowStyle {
  /// Header rows are semibold and never striped.
  header: bool,
  /// Whether a separator is drawn below this row.
  border_bottom: bool,
  /// Whether the row takes the alternating stripe color.
  striped: bool,
}

fn render_table_row(
  row: &MarkdownTableRow, layout: &TableLayout<'_>, style: RowStyle, ctx: &RenderContext<'_>,
  ids: &mut usize, window: &mut Window,
) -> AnyElement {
  let mut cells = Vec::with_capacity(layout.alignments.len());

  for (index, alignment) in layout.alignments.iter().enumerate() {
    let width = layout.widths.get(index).copied().unwrap_or_default();
    // Grow in proportion to the measured width so extra space is shared by
    // content, not split evenly; `min_w` keeps the column readable.
    let grow = if layout.total > px(0.) {
      width / layout.total
    } else {
      1.
    };
    let content = row
      .cells
      .get(index)
      .map(|inlines| render_inlines(inlines, ctx, ids, window))
      .unwrap_or_else(|| {
        let empty_id = *ids;
        *ids += 1;
        div()
          .id(ElementId::NamedInteger(
            ctx.id_prefix.clone(),
            empty_id as u64,
          ))
          .w_full()
      });

    // Alignment is applied by the *container* (flex), not by `text_center()` /
    // `text_right()` on the text. gpui computes text alignment only at paint
    // time (`aligned_origin_x`), so a centered or right-aligned text element
    // still reports left-aligned geometry from its layout — which made
    // `index_for_position` resolve pointer positions to the wrong character in
    // every non-left column. Sizing the content box to the text and letting
    // flex place it keeps the layout's own origin equal to where the text is
    // actually drawn.
    cells.push(
      div()
        .flex_shrink_0()
        .flex_grow(grow)
        .min_w(width)
        .px(rems(TABLE_CELL_PX))
        .py_2()
        .flex()
        .when(matches!(alignment, MarkdownAlignment::Center), |this| {
          this.justify_center()
        })
        .when(matches!(alignment, MarkdownAlignment::Right), |this| {
          this.justify_end()
        })
        .when(
          matches!(alignment, MarkdownAlignment::Left | MarkdownAlignment::None),
          |this| this.justify_start(),
        )
        .when(style.header, |this| this.font_semibold())
        .child(div().flex_shrink_0().child(content)),
    );
  }

  h_flex()
    .w_full()
    .items_start()
    .when(style.header, |this| {
      this
        .bg(ctx.palette.table_header_background)
        .text_color(ctx.palette.table_head_foreground)
    })
    .when(style.striped, |this| this.bg(ctx.palette.table_stripe))
    .when(style.border_bottom, |this| {
      this.border_b_1().border_color(ctx.palette.border)
    })
    .children(cells)
    .into_any_element()
}

#[cfg(test)]
mod tests {
  use gpui::{Context, IntoElement, Render, TestAppContext, Window, div};

  use super::*;
  use crate::widgets::markdown::parser::{MarkdownBlock, ParsedMarkdown};

  /// Measures a table inside a real window, so column sizing is asserted on
  /// pixels produced by the text system rather than on the code path that
  /// produced them.
  fn measure(cx: &mut TestAppContext, source: &str) -> Vec<Pixels> {
    let parsed = ParsedMarkdown::parse(source);
    let MarkdownBlock::Table(table) = &parsed.blocks()[0] else {
      panic!("expected a table");
    };
    let table = table.clone();

    let alignments: Vec<MarkdownAlignment> = (0..table.header.cells.len())
      .map(|index| {
        table
          .alignments
          .get(index)
          .copied()
          .unwrap_or(MarkdownAlignment::None)
      })
      .collect();

    cx.update(crate::init);

    struct Probe;
    impl Render for Probe {
      fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
      }
    }

    let (_root, visual) = cx.add_window_view(|_, _| Probe);
    visual.run_until_parked();
    visual.update(|window, cx| measure_table_columns(&table, &alignments, window, cx))
  }

  #[gpui::test]
  fn cells_with_hard_breaks_do_not_panic(cx: &mut TestAppContext) {
    let widths = measure(cx, "| a<br>b | c |\n| - | - |\n| 1 | 2 |\n");
    assert_eq!(widths.len(), 2);
  }

  #[gpui::test]
  fn columns_size_to_content_not_evenly(cx: &mut TestAppContext) {
    let widths = measure(cx, "| short | a much longer cell |\n| - | - |\n| 1 | 2 |\n");
    assert!(
      widths[1] > widths[0],
      "a wider column should measure wider: {widths:?}"
    );
  }

  #[gpui::test]
  fn columns_respect_the_configured_maximum(cx: &mut TestAppContext) {
    let long = "x".repeat(500);
    let widths = measure(cx, &format!("| a | {long} |\n| - | - |\n| 1 | 2 |\n"));
    let max = rems(TABLE_MAX_COLUMN_WIDTH).to_pixels(gpui::px(16.));
    assert!(
      widths[1] <= max,
      "a very long cell should be clamped to the maximum: {widths:?}"
    );
  }
}
