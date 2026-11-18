//! Markdown rendering component.
//!
//! [`Markdown`] renders CommonMark and GitHub flavored markdown as native gpui
//! elements: headings, paragraphs with styled inline runs, bulleted and
//! numbered lists (including nested lists and task list markers), tables,
//! block quotes, horizontal rules, footnotes and code blocks. Code blocks are
//! syntax highlighted through the same tree-sitter highlighter the editor
//! uses, and can copy their content from a header button.
//!
//! Everything is drawn with the active [`Theme`](crate::Theme): text colors,
//! link colors, code backgrounds, borders, list markers and syntax colors all
//! follow the light/dark switch. Links are clickable: the default behavior
//! opens the URL through `cx.open_url`, and [`Markdown::on_link_click`]
//! replaces it when an application needs its own routing.
//!
//! # Features
//! - **Rich inline text**: strong, emphasis, strikethrough, inline code, links
//!   and soft/hard line breaks
//! - **Block structure**: headings `H1`–`H6`, paragraphs, nested lists, task
//!   lists (markers reuse the design system's `Checkbox`), tables with measured
//!   columns, block quotes (including `> [!NOTE]` kinds), rules and footnotes
//! - **Highlighted code blocks**: fenced code is highlighted per language with
//!   the editor's tree-sitter highlighter and can be copied to the clipboard
//! - **Clickable links**: open URLs or dispatch through a custom handler
//! - **Text selection**: drag to select, double click for a word, triple click
//!   for a line, with copy / select-all in the context menu
//! - **Theme aware**: all colors, fonts and radii derive from the active theme
//! - **Cacheable parse**: [`ParsedMarkdown`] can be parsed once and reused
//!   across frames with [`Markdown::from_shared`]
//!
//! # Limitations
//! - Remote images are not fetched. Inline images render as links; a paragraph
//!   that only contains an image renders a placeholder, and an application can
//!   install [`Markdown::image_handler`] to draw real images. Images whose URL
//!   the application's asset source can serve are drawn for real.
//! - Source-position mapping is not implemented; selection works on rendered
//!   text, not on the original source offsets.
//! - Inline HTML is dropped (except `<br>`); block-level HTML renders as plain
//!   text.
//!
//! # Example
//! ```rust,ignore
//! use woocraft::{Markdown, ParsedMarkdown};
//!
//! let parsed = ParsedMarkdown::parse("# Hello\n\nSome **bold** text.");
//! let markdown = Markdown::from_parsed(parsed.clone())
//!   .on_link_click(|url, _window, cx| {
//!     cx.open_url(url);
//!   });
//! ```

use std::{
  collections::HashMap,
  hash::{DefaultHasher, Hash, Hasher},
  ops::Range,
  rc::Rc,
  sync::{Arc, LazyLock, Mutex},
};

use gpui::{
  AnyElement, App, ClipboardItem, DispatchPhase, Div, ElementId, HighlightStyle,
  InteractiveElement as _, InteractiveText, IntoElement, MouseButton, MouseMoveEvent, MouseUpEvent,
  ParentElement, Pixels, Rems, RenderOnce, SharedString, StatefulInteractiveElement as _,
  StyleRefinement, Styled, StyledText, Window, div, prelude::FluentBuilder as _, px, relative,
  rems,
};
use ropey::Rope;

use super::{
  palette::{CodeBoxes, InlineRender, MarkdownPalette},
  parser::{
    MarkdownBlock, MarkdownCodeBlock, MarkdownHeadingLevel, MarkdownInline, MarkdownList,
    ParsedMarkdown,
  },
  selection::{MarkdownSelection, next_run_start, selectable_text, with_selection_highlight},
  table::render_table,
};
use crate::{
  ActiveTheme, Button, ButtonVariants as _, Checkbox, ContextMenuExt as _, Disableable as _,
  Divider, HighlightTheme, PopupMenu, PopupMenuItem, Sizable as _, StyledExt as _,
  SyntaxHighlighter, h_flex, translate_woocraft, v_flex,
};

/// Callback type used by [`Markdown::on_link_click`].
type LinkClickHandler = Rc<dyn Fn(&str, &mut Window, &mut App) + 'static>;

/// Callback type used by [`Markdown::on_selection_change`].
pub(super) type SelectionChangeHandler = Rc<dyn Fn(Option<String>)>;

/// Extends the markdown context menu with application-specific items.
///
/// Called when the menu opens, after the built-in items, so an application can
/// append its own actions without replacing copy/select-all. `selected_text`
/// is the current selection, if any.
pub type MarkdownContextMenuBuilder =
  Rc<dyn Fn(PopupMenu, Option<String>, &mut Window, &mut App) -> PopupMenu + 'static>;

/// Builds a real element for a markdown image, replacing the default
/// placeholder.
///
/// The handler receives the image URL and the plain-text alt text, and can
/// return any element: an `img` element backed by an application asset
/// source, a cached bitmap, a skeleton while loading, and so on.
///
/// ```rust,ignore
/// Markdown::new(source).image_handler(|url, alt, _window, _cx| {
///   div().child(alt.to_string()).into_any_element()
/// });
/// ```
pub type MarkdownImageHandler =
  Rc<dyn Fn(&str, &str, &mut Window, &mut App) -> AnyElement + 'static>;

#[derive(Clone)]
pub(super) struct RenderContext<'a> {
  pub(super) palette: &'a MarkdownPalette,
  pub(super) on_link_click: Option<&'a LinkClickHandler>,
  pub(super) image_handler: Option<&'a MarkdownImageHandler>,
  pub(super) copy_code_button: bool,
  /// Whether blocks emit their own trailing margin. Disabled inside a block
  /// quote, where the quote's padding and `gap` provide the spacing instead.
  pub(super) block_spacing: bool,
  /// Namespace for element ids, unique per parsed document.
  pub(super) id_prefix: SharedString,
  /// Shared text selection for this document, if selection is enabled.
  pub(super) selection: Option<MarkdownSelection>,
}

impl RenderContext<'_> {
  /// Trailing margin of a block: `mb_2` at the top level, nothing inside a
  /// quote (whose padding and `gap` own the spacing).
  pub(super) fn block_margin(&self) -> Rems {
    if self.block_spacing {
      rems(0.5)
    } else {
      rems(0.)
    }
  }
}

pub(super) fn render_inlines(
  inlines: &[MarkdownInline], ctx: &RenderContext<'_>, ids: &mut usize, window: &mut Window,
) -> gpui::Stateful<Div> {
  let built = InlineRender::build(inlines, ctx.palette);

  if built.text.is_empty() {
    let empty_id = *ids;
    *ids += 1;
    return div()
      .id(ElementId::NamedInteger(
        ctx.id_prefix.clone(),
        empty_id as u64,
      ))
      .w_full();
  }

  let built_text = built.text.clone();
  // Selection is a highlight run, not a hand-painted overlay: the text engine
  // already knows where every glyph is, so it draws the background for us
  // (wrapping, alignment and font fallback included). This replaces the canvas
  // overlay that had to recompute those positions by hand.
  let highlights = with_selection_highlight(
    built.highlights.clone(),
    next_run_start(ctx),
    built.text.len(),
    ctx,
  );

  let mut styled = StyledText::new(built.text);
  let styled_layout = styled.layout().clone();
  if !highlights.is_empty() {
    styled = styled.with_highlights(highlights);
  }
  if !built.code_ranges.is_empty() {
    styled = styled.with_font_family_overrides(
      built
        .code_ranges
        .iter()
        .cloned()
        .map(|range| (range, ctx.palette.mono_family.clone())),
    );
  }

  // The code boxes are painted behind the text from the layout the text
  // element produces, so wrapping, alignment and font fallback all stay with
  // gpui.
  let code_boxes = (!built.code_ranges.is_empty()).then(|| CodeBoxes {
    layout: styled.layout().clone(),
    ranges: built.code_ranges.clone(),
    fill: ctx.palette.inline_code_background,
    border: ctx.palette.border,
    radius: ctx.palette.radius,
    line_height: body_line_height(window),
  });

  let background = code_boxes.map(|boxes| {
    gpui::canvas(
      move |_, _, _| (),
      move |_, _, window, _| boxes.paint(window),
    )
    .absolute()
    .size_full()
  });

  let content: AnyElement = if built.links.is_empty() {
    styled.into_any_element()
  } else {
    let id = *ids;
    *ids += 1;

    let links = Rc::new(built.links);
    let ranges: Vec<Range<usize>> = links.iter().map(|(range, _)| range.clone()).collect();
    let handler = ctx.on_link_click.cloned();
    let clickable = links.clone();

    // Track which link the pointer is over so the context menu can offer
    // "copy link" only when it applies.
    let hover_selection = ctx.selection.clone();
    let hover_links = clickable.clone();

    InteractiveText::new(
      ElementId::NamedInteger(ctx.id_prefix.clone(), id as u64),
      styled,
    )
    .on_hover(move |index, _, _, cx| {
      let Some(selection) = hover_selection.as_ref() else {
        return;
      };
      let url = index.and_then(|index| {
        hover_links
          .iter()
          .find(|(range, _)| range.contains(&index))
          .map(|(_, url)| url.clone())
      });
      selection.set_hovered_link(url);
      cx.refresh_windows();
    })
    .on_click(ranges, move |index, window, cx| {
      let Some((_, url)) = clickable.get(index) else {
        return;
      };

      match handler.as_ref() {
        Some(handler) => handler(url, window, cx),
        None => cx.open_url(url),
      }
    })
    .into_any_element()
  };

  // Selection, cursor and the highlight behind the run are shared with code
  // blocks, so a drag can span a paragraph and a fence.
  let selection_id = *ids;
  *ids += 1;
  selectable_text(
    content,
    styled_layout,
    &built_text,
    ElementId::NamedInteger(ctx.id_prefix.clone(), selection_id as u64),
    ctx,
    window,
  )
  .children(background)
}

/// Wraps a `StyledText` in a selectable, pointer-aware container.
///
/// Shared by inline prose and code blocks: both register their text with the
/// document selection and map pointer positions back to offsets, so a drag can
/// start in a paragraph and end inside a code block.
///
/// `text` is the exact string the `StyledText` was built from; it is what gets
/// registered with the selection (the field on `StyledText` is private).
/// Merges the current selection into `highlights`, rebased to `run_start`.
fn render_blocks(
  blocks: &[MarkdownBlock], ctx: &RenderContext<'_>, ids: &mut usize, window: &mut Window,
  cx: &mut App,
) -> Vec<AnyElement> {
  let last = blocks.len().saturating_sub(1);

  blocks
    .iter()
    .enumerate()
    .map(|(index, block)| {
      if index == last {
        let no_trailing_margin = RenderContext {
          block_spacing: false,
          ..ctx.clone()
        };
        render_block(block, &no_trailing_margin, ids, window, cx)
      } else {
        render_block(block, ctx, ids, window, cx)
      }
    })
    .collect()
}

fn render_block(
  block: &MarkdownBlock, ctx: &RenderContext<'_>, ids: &mut usize, window: &mut Window,
  cx: &mut App,
) -> AnyElement {
  match block {
    MarkdownBlock::Paragraph(inlines) => {
      // A paragraph holding a single image is treated as a block image
      // instead of an inline text run.
      if let [MarkdownInline::Image { url, alt, .. }] = inlines.as_slice() {
        return render_image(url, alt, ctx, window, cx);
      }

      render_inlines(inlines, ctx, ids, window)
        .mb(ctx.block_margin())
        .into_any_element()
    }
    MarkdownBlock::Heading { level, inlines } => render_heading(*level, inlines, ctx, ids, window),
    MarkdownBlock::CodeBlock(code) => render_code_block(code, ctx, ids, window),
    MarkdownBlock::BlockQuote { kind, blocks } => {
      let accent = ctx.palette.quote_accent(*kind);
      // `render_blocks` drops the trailing margin of the last inner block, so
      // the symmetric padding below is the only bottom spacing and the text is
      // vertically centered. Inter-block spacing inside the quote is kept.
      let inner = render_blocks(blocks, ctx, ids, window, cx);

      v_flex()
        .w_full()
        .mb_3()
        .px_3()
        .py_3()
        .rounded(ctx.palette.radius)
        .border_l_3()
        .border_color(accent)
        .bg(ctx.palette.quote_background)
        .children(inner)
        .into_any_element()
    }
    MarkdownBlock::List(list) => {
      // Each list gets its own discriminator: without it, item 0 of two sibling
      // lists would share a hover group and an element id.
      let list_id = *ids;
      *ids += 1;
      render_list(list, list_id, ctx, ids, window, cx)
    }
    MarkdownBlock::Table(table) => render_table(table, ctx, ids, window, cx),
    MarkdownBlock::Rule => Divider::horizontal().my_4().into_any_element(),
    // Raw block HTML cannot be rendered without an HTML layout engine, so it
    // is shown as its source text, muted so it reads as "unrendered" rather
    // than as body copy.
    MarkdownBlock::Html(html) => div()
      .w_full()
      .mb(ctx.block_margin())
      .text_color(ctx.palette.muted_foreground)
      .child(html.clone())
      .into_any_element(),
    MarkdownBlock::FootnoteDefinition { label, blocks } => v_flex()
      .w_full()
      .mb(ctx.block_margin())
      .gap_1()
      .child(
        div()
          .text_color(ctx.palette.muted_foreground)
          .child(format!("[^{label}]")),
      )
      .child(
        v_flex()
          .w_full()
          .pl_3()
          .children(render_blocks(blocks, ctx, ids, window, cx)),
      )
      .into_any_element(),
  }
}

pub(super) fn render_heading(
  level: MarkdownHeadingLevel, inlines: &[MarkdownInline], ctx: &RenderContext<'_>,
  ids: &mut usize, window: &mut Window,
) -> AnyElement {
  let text = render_inlines(inlines, ctx, ids, window).line_height(relative(1.25));

  match level {
    MarkdownHeadingLevel::H1 => text.text_3xl().font_bold(),
    MarkdownHeadingLevel::H2 => text.text_2xl().font_bold(),
    MarkdownHeadingLevel::H3 => text.text_xl().font_semibold(),
    MarkdownHeadingLevel::H4 => text.text_lg().font_semibold(),
    MarkdownHeadingLevel::H5 => text.text_base().font_semibold(),
    MarkdownHeadingLevel::H6 => text.text_sm().font_semibold(),
  }
  .mt_4()
  .mb(ctx.block_margin())
  .into_any_element()
}

pub(super) fn render_code_block(
  block: &MarkdownCodeBlock, ctx: &RenderContext<'_>, ids: &mut usize, window: &mut Window,
) -> AnyElement {
  // Each code block consumes two ids: the horizontally scrollable body and
  // the copy button, which are siblings.
  let id = *ids;
  *ids += 2;

  let language = block.language.as_deref().unwrap_or("");
  let runs = highlighted_runs(language, &block.code, &ctx.palette.syntax);

  let highlights = with_selection_highlight(
    runs.as_ref().clone(),
    next_run_start(ctx),
    block.code.len(),
    ctx,
  );

  let mut styled = StyledText::new(block.code.clone());
  if !highlights.is_empty() {
    styled = styled.with_highlights(highlights);
  }
  let code_layout = styled.layout().clone();
  if !block.code.is_empty() {
    styled =
      styled.with_font_family_overrides([(0..block.code.len(), ctx.palette.mono_family.clone())]);
  }

  let copy_button = ctx.copy_code_button.then(|| {
    let code = block.code.clone();
    Button::new(ElementId::NamedInteger(
      ctx.id_prefix.clone(),
      id as u64 + 1,
    ))
    .label(translate_woocraft("markdown.copy_code"))
    .small()
    .flat()
    .on_click(move |_, _, cx| {
      cx.write_to_clipboard(ClipboardItem::new_string(code.to_string()));
    })
  });

  let show_header = !language.is_empty() || copy_button.is_some();

  // The surface mirrors `Editor` in code mode: `editor_background`, the theme
  // radius, an `input` border and the editor's 1.5 line height.
  v_flex()
    .w_full()
    .mb(ctx.block_margin())
    .rounded(ctx.palette.radius)
    .border_1()
    .border_color(ctx.palette.input_border)
    .bg(ctx.palette.code_block_background)
    .text_color(ctx.palette.foreground)
    .line_height(ctx.palette.code_line_height)
    .overflow_hidden()
    .when(show_header, |this| {
      this.child(
        h_flex()
          .w_full()
          .items_center()
          .justify_between()
          .px_3()
          .py_1()
          .border_b_1()
          .border_color(ctx.palette.input_border)
          .when(!language.is_empty(), |row| {
            row.child(
              div()
                .text_xs()
                .text_color(ctx.palette.muted_foreground)
                .child(language.to_string()),
            )
          })
          .when_some(copy_button, |row, button| row.child(button)),
      )
    })
    .child(
      div()
        .w_full()
        .id(ElementId::NamedInteger(ctx.id_prefix.clone(), id as u64))
        .overflow_x_scroll()
        .child(div().whitespace_nowrap().min_w_full().px_3().py_2().child(
          // Code is selectable through the same path as prose.
          selectable_text(
            styled.into_any_element(),
            code_layout,
            &block.code,
            ElementId::NamedInteger(ctx.id_prefix.clone(), id as u64 + 2),
            ctx,
            window,
          ),
        )),
    )
    .into_any_element()
}

pub(super) fn render_list(
  list: &MarkdownList, list_id: usize, ctx: &RenderContext<'_>, ids: &mut usize,
  window: &mut Window, cx: &mut App,
) -> AnyElement {
  // One text line, resolved from the window's text style. Markers are centered
  // inside a box of this height so they line up with the text's optical center
  // (gpui centers glyphs within the line box).
  let line_height = body_line_height(window);
  // The gutter is one rem by default, which already fits a bullet and a number
  // of a few digits. Only very long numbering needs more, so the widest marker
  // in *this* list is measured and the gutter grows to fit it. Every item
  // shares the resulting gutter, so their text starts at the same x.
  let gutter = if list.ordered {
    list
      .items
      .iter()
      .enumerate()
      .filter(|(_, item)| item.task.is_none())
      .map(|(index, _)| marker_width(&format!("{}.", list.start + index as u64), window))
      .fold(marker_gutter().0, f32::max)
  } else {
    marker_gutter().0
  };
  let gutter = rems(gutter);
  let mut items = Vec::with_capacity(list.items.len());

  for (index, item) in list.items.iter().enumerate() {
    let content = v_flex().flex_1().min_w_0().gap_1().children(render_blocks(
      &item.blocks,
      ctx,
      ids,
      window,
      cx,
    ));

    // Each item is its own hover group, so only the marker under the pointer
    // brightens to full opacity instead of the whole list. The list id keeps
    // sibling lists from sharing groups.
    let item_key = (list_id as u64) << 16 | index as u64;
    let group = SharedString::from(format!("md-li-{}", item_key));

    let marker: AnyElement = match item.task {
      Some(checked) => {
        let id = *ids;
        *ids += 1;
        render_task_marker(
          ElementId::NamedInteger(ctx.id_prefix.clone(), id as u64),
          checked,
          window,
        )
      }
      // Both marker kinds are plain colored glyphs: a number followed by a
      // period, or an asterisk. They share one gutter and one color so the two
      // list kinds read as the same component.
      None if list.ordered => marker_line(
        line_height,
        div()
          .min_w(gutter)
          .flex()
          .justify_end()
          .text_color(ctx.palette.list_marker)
          .group_hover(group.clone(), |this| {
            this.text_color(ctx.palette.list_marker_hovered)
          })
          .child(format!("{}.", list.start + index as u64)),
      ),
      // The bullet is right-aligned in the same gutter as the numbers, so both
      // list kinds end their marker at the content edge and their text starts
      // at the same x. Left-aligning the bullet instead (its natural width is
      // one glyph) leaves it visually to the left of a number's last digit.
      None => marker_line(
        line_height,
        div()
          .min_w(gutter)
          .flex()
          .justify_end()
          .text_color(ctx.palette.list_marker)
          .group_hover(group.clone(), |this| {
            this.text_color(ctx.palette.list_marker_hovered)
          })
          .child("*"),
      ),
    };

    // `items_start` aligns the marker with the top of the item's first line, so
    // bullets, numbers and task boxes all sit on the text without any
    // hand-tuned height.
    // `group` only registers a hitbox (and therefore only makes `group_hover`
    // fire) for elements that also have an id, so the row carries one.
    items.push(
      h_flex()
        .id(ElementId::NamedInteger(
          ctx.id_prefix.clone(),
          ROW_ID_BASE + item_key,
        ))
        .w_full()
        .items_start()
        .gap_2()
        .group(group)
        .child(marker)
        .child(content),
    );
  }

  v_flex()
    .w_full()
    .mb(ctx.block_margin())
    .children(items)
    .into_any_element()
}

/// Line height of the document body, as a multiple of the inherited font size.
/// The document root applies it via `line_height(relative(..))`; render code
/// that needs the value before the root style is applied (marker and code-box
/// centering) must compute it from this constant instead of reading
/// `window.text_style()`, which still reports the parent's style at that point.
const BODY_LINE_HEIGHT: f32 = 1.5;

/// Resolves the body line height in pixels from the current text style.
///
/// `RenderOnce::render` runs before the root `v_flex`'s style is pushed, so
/// `window.text_style()` here is the *parent's* style. Multiplying this
/// document's own ratio by the inherited font size gives the height the text
/// will actually be laid out at.
pub(super) fn body_line_height(window: &Window) -> Pixels {
  let rem_size = window.rem_size();
  let font_size = window.text_style().font_size.to_pixels(rem_size);
  font_size * BODY_LINE_HEIGHT
}

/// Offset that keeps list row ids from colliding with the ids allocated for
/// links, code blocks and task markers in the same document.
const ROW_ID_BASE: u64 = 1 << 40;

/// Default gutter reserved for a list marker: one rem, which fits a bullet and
/// a one- or two-digit number. Longer numbering widens the gutter per list (see
/// `render_list`).
fn marker_gutter() -> Rems {
  rems(1.0)
}

/// Width of one marker string, so a list can size its gutter to its widest
/// number instead of guessing.
fn marker_width(text: &str, window: &Window) -> f32 {
  let text_style = window.text_style();
  let rem_size = window.rem_size();
  let font_size = text_style.font_size.to_pixels(rem_size);
  let runs = [text_style.to_run(text.len())];
  let width = window
    .text_system()
    .shape_line(text.to_string().into(), font_size, &runs, None)
    .width;

  // `Pixels` has no public accessor, so convert through a ratio instead.
  let one_rem = rems(1.).to_pixels(rem_size);
  width / one_rem
}

/// Wraps a list marker in a one-line-tall box with the marker centered, so
/// bullets and numbers sit on the text's optical center instead of being
/// top-aligned against the line box.
fn marker_line(line_height: Pixels, marker: Div) -> AnyElement {
  h_flex()
    .flex_shrink_0()
    .items_center()
    .h(line_height)
    .child(marker)
    .into_any_element()
}

/// Renders the read-only checkbox of a task list item.
///
/// This is the design system's [`Checkbox`] itself, disabled so it neither
/// takes focus nor toggles. A task marker has to match every other checkbox in
/// the application — box size, corner radius, fill and border colors, check
/// glyph — so it is reused rather than approximated with a hand-drawn box.
fn render_task_marker(id: ElementId, checked: bool, window: &Window) -> AnyElement {
  // `medium` keeps the box at the body text size (`small` would shrink it
  // relative to the surrounding 1em copy).
  //
  // Vertical alignment: gpui centers a line's glyphs inside the line box
  // (`padding_top = (line_height - ascent - descent) / 2`), so the text's
  // optical center sits at half the line height. The checkbox box is only
  // `component_height / 2` tall (1em), so aligning it by its own top edge
  // leaves it above the text. Giving the marker a one-line-tall box and
  // centering the checkbox inside it puts the box on the text's center line.
  //
  // One text line at this document's line height. `window.text_style()` cannot
  // be used directly: `render` runs before the root style is applied, so it
  // still reports the parent's (default) line height rather than this
  // document's 1.5 ratio.
  let line_height = body_line_height(window);

  h_flex()
    .flex_shrink_0()
    .items_center()
    .h(line_height)
    .child(Checkbox::new(id).checked(checked).disabled(true).medium())
    .into_any_element()
}

/// Horizontal padding around an inline code span, in rems, before the border
/// is accounted for.
///
/// The box draws a 1px border on each side, and padding is measured inside the
/// border, so using this value directly would give the text `0.25rem + 1px` of
/// visual breathing room. `inline_code_pad_x` subtracts that pixel back, the
/// same way `Size::component_py` compensates its border (negative padding is
/// not representable in taffy). There is no vertical padding: the box is sized
/// by the text's own line box, so it stays flush with the surrounding prose.
const INLINE_CODE_PAD_X: f32 = 0.25;

/// Border width of an inline code box, in pixels.
const INLINE_CODE_BORDER: f32 = 1.0;

/// Horizontal padding actually applied, border excluded.
///
/// Clamped at zero: at very small rem sizes `0.25rem` can be narrower than the
/// 1px border, and a negative padding would produce an inverted rectangle.
pub(super) fn inline_code_pad_x(rem_size: Pixels) -> Pixels {
  let pad = rems(INLINE_CODE_PAD_X).to_pixels(rem_size) - px(INLINE_CODE_BORDER);
  pad.max(px(0.))
}

/// Horizontal padding of one table cell, matching `Size::container_px`.
pub(super) const TABLE_CELL_PX: f32 = 0.75;
/// Narrowest a measured column may become, so a one-character column still
/// reads as a column.
pub(super) const TABLE_MIN_COLUMN_WIDTH: f32 = 4.0;
/// Widest a measured column may become before its content wraps.
pub(super) const TABLE_MAX_COLUMN_WIDTH: f32 = 32.0;

/// Measures the natural width of every column by shaping its widest cell.
///
/// Columns size to their content instead of splitting the row evenly: a table
/// of `| a | b |` no longer gives both columns half the pane. Returns pixel
/// widths clamped to [`TABLE_MIN_COLUMN_WIDTH`]..[`TABLE_MAX_COLUMN_WIDTH`].
fn render_image(
  url: &str, alt: &[MarkdownInline], ctx: &RenderContext<'_>, window: &mut Window, cx: &mut App,
) -> AnyElement {
  let alt_text = InlineRender::build(alt, ctx.palette).text;

  if let Some(handler) = ctx.image_handler {
    return handler(url, &alt_text, window, cx);
  }

  let title = if alt_text.is_empty() {
    url
  } else {
    alt_text.as_str()
  };

  // An asset-backed URL draws the image; a remote one cannot be loaded here.
  if is_remote_url(url) || !asset_exists(url, cx) {
    return render_image_placeholder(url, title, ctx);
  }

  v_flex()
    .w_full()
    .mb(ctx.block_margin())
    .gap_1()
    .items_center()
    .child(
      gpui::img(url.to_string())
        .max_w_full()
        .rounded(ctx.palette.radius),
    )
    .when(!alt_text.is_empty(), |this| {
      this.child(
        div()
          .text_color(ctx.palette.muted_foreground)
          .child(alt_text.clone()),
      )
    })
    .into_any_element()
}

/// Whether an image URL points outside the asset source.
fn is_remote_url(url: &str) -> bool {
  let lower = url.trim_start().to_ascii_lowercase();
  lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("data:")
}

/// Whether the application's asset source can load `path`.
fn asset_exists(path: &str, cx: &App) -> bool {
  matches!(cx.asset_source().load(path), Ok(Some(_)))
}

/// Dashed placeholder used for images that cannot be loaded, so the document
/// still shows that an image belongs there.
fn render_image_placeholder(url: &str, title: &str, ctx: &RenderContext<'_>) -> AnyElement {
  v_flex()
    .w_full()
    .mb(ctx.block_margin())
    .gap_1()
    .p_3()
    .rounded(ctx.palette.radius)
    .border_1()
    .border_dashed()
    .border_color(ctx.palette.border)
    .bg(ctx.palette.placeholder_background)
    .child(
      h_flex()
        .gap_2()
        .items_center()
        .child(
          div()
            .px_1()
            .rounded_sm()
            .bg(ctx.palette.border)
            .text_color(ctx.palette.muted_foreground)
            .child(translate_woocraft("markdown.image")),
        )
        .child(div().font_semibold().child(title.to_string())),
    )
    .child(
      div()
        .text_color(ctx.palette.muted_foreground)
        .child(url.to_string()),
    )
    .into_any_element()
}

/// Returns the canonical highlighter language for a fenced code block's info
/// string, or `None` when the language is unknown or highlighting is disabled.
fn canonical_language(language: &str) -> Option<SharedString> {
  #[cfg(not(feature = "tree-sitter-languages"))]
  {
    let _ = language;
    None
  }

  #[cfg(feature = "tree-sitter-languages")]
  {
    let language = crate::Language::from_name(&language.to_ascii_lowercase());
    (language != crate::Language::Plain).then(|| SharedString::from(language.name()))
  }
}

/// Maximum number of syntax highlight results kept in the process-wide cache.
const HIGHLIGHT_CACHE_CAPACITY: usize = 128;

/// Highlighted style runs for a code block.
type HighlightRuns = Arc<Vec<(Range<usize>, HighlightStyle)>>;

/// Process-wide cache of highlighted code blocks, keyed by language, code
/// content and syntax theme.
static HIGHLIGHT_CACHE: LazyLock<Mutex<HashMap<u64, HighlightRuns>>> =
  LazyLock::new(|| Mutex::new(HashMap::new()));

/// Returns highlighted style runs for a code block, using a process-wide
/// cache so re-rendering the same document does not reparse the syntax tree.
pub(super) fn highlighted_runs(
  language: &str, code: &str, syntax: &HighlightTheme,
) -> HighlightRuns {
  if language.is_empty() || code.is_empty() {
    return Arc::new(Vec::new());
  }

  let Some(language) = canonical_language(language) else {
    return Arc::new(Vec::new());
  };

  let mut hasher = DefaultHasher::new();
  language.hash(&mut hasher);
  code.hash(&mut hasher);
  syntax.hash(&mut hasher);
  let key = hasher.finish();

  if let Some(runs) = HIGHLIGHT_CACHE
    .lock()
    .expect("markdown highlight cache lock poisoned")
    .get(&key)
    .cloned()
  {
    return runs;
  }

  let mut highlighter = SyntaxHighlighter::new(language.as_ref());
  highlighter.update(None, &Rope::from(code));
  let runs = Arc::new(highlighter.styles(&(0..code.len()), syntax));

  let mut cache = HIGHLIGHT_CACHE
    .lock()
    .expect("markdown highlight cache lock poisoned");
  if cache.len() >= HIGHLIGHT_CACHE_CAPACITY {
    cache.clear();
  }
  cache.insert(key, runs.clone());

  runs
}

/// Markdown renderer.
///
/// Build the element with [`Markdown::new`], which parses the source, or with
/// [`Markdown::from_parsed`] when the parse result is cached. The element
/// renders its blocks into a vertical stack, filling the available width;
/// wrap it in a scroll container to make long documents scrollable.
///
/// ```rust,ignore
/// use woocraft::Markdown;
///
/// div().p_6().overflow_y_scrollbar().child(
///   Markdown::new("# Title\n\nParagraph with a [link](https://example.com)."),
/// );
/// ```
/// Text selection inside one rendered markdown document.
///
/// Each rendered text run registers itself and its byte range, so a pointer
/// position can be mapped back to a document offset. Offsets are relative to
/// the concatenation of all runs in render order, which is what
/// [`Markdown::selected_text`] slices.
#[derive(Default, Clone, IntoElement)]
pub struct Markdown {
  style: StyleRefinement,
  parsed: Rc<ParsedMarkdown>,
  id_prefix: Option<SharedString>,
  on_link_click: Option<LinkClickHandler>,
  image_handler: Option<MarkdownImageHandler>,
  copy_code_button: bool,
  selectable: bool,
  on_selection_change: Option<Rc<dyn Fn(Option<String>)>>,
  context_menu_builder: Option<MarkdownContextMenuBuilder>,
  /// Populated on the first render; shared with the element's pointer handlers.
  selection: Option<MarkdownSelection>,
}

impl Markdown {
  /// Creates a markdown element by parsing the given source.
  ///
  /// Parsing happens eagerly; for documents that are rendered every frame,
  /// cache a [`ParsedMarkdown`] and use [`Markdown::from_parsed`] instead.
  pub fn new(source: impl Into<SharedString>) -> Self {
    let source = source.into();
    Self::from_parsed(ParsedMarkdown::parse(&source))
  }

  /// Creates a markdown element from an already parsed document.
  pub fn from_parsed(parsed: ParsedMarkdown) -> Self {
    Self::from_shared(Rc::new(parsed))
  }

  /// Creates a markdown element from a shared parse result.
  ///
  /// Use this when the parsed document is cached behind an `Rc` (for example
  /// in view state) so rebuilding the element every frame does not clone the
  /// block tree.
  pub fn from_shared(parsed: Rc<ParsedMarkdown>) -> Self {
    Self {
      style: StyleRefinement::default(),
      parsed,
      id_prefix: None,
      on_link_click: None,
      image_handler: None,
      copy_code_button: true,
      selectable: true,
      on_selection_change: None,
      context_menu_builder: None,
      selection: None,
    }
  }

  /// Replaces the source text, re-parsing it.
  pub fn source(mut self, source: impl Into<SharedString>) -> Self {
    let source = source.into();
    self.parsed = Rc::new(ParsedMarkdown::parse(&source));
    self
  }

  /// Overrides the default link behavior, which is `cx.open_url(url)`.
  ///
  /// Use this to route links inside an application (open a file, focus a
  /// panel, show a confirmation dialog, …) instead of handing them to the
  /// system browser.
  pub fn on_link_click(mut self, handler: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
    self.on_link_click = Some(Rc::new(handler));
    self
  }

  /// Installs a handler that draws real elements for markdown images.
  ///
  /// Without a handler, a paragraph that only contains an image renders a
  /// placeholder with the alt text and URL, and inline images render as
  /// links.
  pub fn image_handler(
    mut self, handler: impl Fn(&str, &str, &mut Window, &mut App) -> AnyElement + 'static,
  ) -> Self {
    self.image_handler = Some(Rc::new(handler));
    self
  }

  /// Overrides the namespace of the interactive element ids inside the
  /// document.
  ///
  /// The default prefix is derived from the source text, which keeps two
  /// different documents apart. Rendering the *same* document twice in one
  /// window (two panels showing one preview, a diff of itself, …) needs
  /// distinct prefixes, otherwise link and scroll state is shared:
  ///
  /// ```rust,ignore
  /// Markdown::from_shared(parsed.clone()).id_prefix("preview-left");
  /// Markdown::from_shared(parsed.clone()).id_prefix("preview-right");
  /// ```
  pub fn id_prefix(mut self, prefix: impl Into<SharedString>) -> Self {
    self.id_prefix = Some(prefix.into());
    self
  }

  /// Shows or hides the copy button in the header of code blocks.
  ///
  /// The button is shown by default.
  pub fn copy_code_button(mut self, copy_code_button: bool) -> Self {
    self.copy_code_button = copy_code_button;
    self
  }

  /// Registers a callback invoked whenever the text selection changes.
  ///
  /// The callback receives the selected text, or `None` when the selection is
  /// cleared. Useful for showing a selection count, syncing a toolbar, or
  /// mirroring the selection elsewhere.
  pub fn on_selection_change(mut self, handler: impl Fn(Option<String>) + 'static) -> Self {
    self.on_selection_change = Some(Rc::new(handler));
    self
  }

  /// Adds custom items to the markdown context menu.
  ///
  /// The builder receives the menu after the built-in items (copy, select all)
  /// and the current selection text, and returns the menu to show. This keeps
  /// the built-in actions intact while letting an application add its own.
  ///
  /// ```rust,ignore
  /// Markdown::new(source).context_menu(|menu, selected, _window, _cx| {
  ///   menu.separator().item(PopupMenuItem::new("Search selection"))
  /// });
  /// ```
  pub fn context_menu(
    mut self,
    builder: impl Fn(PopupMenu, Option<String>, &mut Window, &mut App) -> PopupMenu + 'static,
  ) -> Self {
    self.context_menu_builder = Some(Rc::new(builder));
    self
  }

  /// Enables or disables pointer text selection.
  ///
  /// Selection is on by default: dragging over the document highlights text,
  /// `cmd/ctrl-c` copies it and the context menu offers the same. Turn it off
  /// for read-only chrome where selecting the text would be noise.
  pub fn selectable(mut self, selectable: bool) -> Self {
    self.selectable = selectable;
    self
  }

  /// Returns the currently selected text, if any.
  pub fn selected_text(&self) -> Option<String> {
    self.selection.as_ref()?.selected_text()
  }

  /// Returns the parsed document backing this element.
  pub fn parsed(&self) -> &ParsedMarkdown {
    &self.parsed
  }
}

impl_styled!(Markdown);

impl RenderOnce for Markdown {
  fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let palette = MarkdownPalette::new(cx.theme());
    let mut ids = 0usize;
    let id_prefix = self
      .id_prefix
      .clone()
      .unwrap_or_else(|| SharedString::from(format!("md-{}", self.parsed.seed())));
    // The selection must survive across frames: pointer handlers run later than
    // render, so a fresh `default()` per frame would discard every drag. It is
    // kept in window keyed state, keyed by the document's id prefix.
    let selection = self.selectable.then(|| {
      window
        .use_keyed_state(
          ElementId::NamedInteger(SharedString::from("md-selection"), self.parsed.seed()),
          cx,
          |_, _| MarkdownSelection::default(),
        )
        .read(cx)
        .clone()
    });
    // The document text and run registry are rebuilt every frame: they mirror
    // the current render, so they must start empty. The selected range is kept
    // as-is.
    if let Some(selection) = selection.as_ref() {
      selection.runs.borrow_mut().clear();
      if let Some(handler) = self.on_selection_change.clone() {
        *selection.on_change.borrow_mut() = Some(handler);
      }
    }

    let ctx = RenderContext {
      palette: &palette,
      on_link_click: self.on_link_click.as_ref(),
      image_handler: self.image_handler.as_ref(),
      copy_code_button: self.copy_code_button,
      block_spacing: true,
      id_prefix,
      selection: selection.clone(),
    };

    // The document inherits the surrounding font size (1em) instead of forcing
    // one, and sets a single line height so every block shares the same
    // vertical rhythm. `1.5` matches `Editor`'s line height.
    let actions_selection = selection.clone();
    let select_all_selection = selection.clone();
    let menu_selection = selection.clone();
    let builder = self.context_menu_builder.clone();

    // A focus handle makes the document a keyboard target: without it the
    // `MARKDOWN_CONTEXT` actions never dispatch, so copy and select-all would
    // silently do nothing.
    let focus_handle = window
      .use_keyed_state(
        ElementId::NamedInteger(SharedString::from("md-focus"), self.parsed.seed()),
        cx,
        |_, cx| cx.focus_handle(),
      )
      .read(cx)
      .clone();

    // One window-level drag listener for the whole document: every run
    // registers its layout with the selection, and this resolves the pointer
    // against all of them. Per-run listeners would each clamp the offset to
    // their own bounds and fight over the shared anchor.
    let drag_selection = selection.clone();
    let drag_root = gpui::canvas(
      |_, _, _| (),
      move |_, _, window, _| {
        let Some(selection) = drag_selection.clone() else {
          return;
        };
        let drag = selection.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
          if phase != DispatchPhase::Bubble || !event.dragging() {
            return;
          }
          let Some(anchor) = drag.anchor() else {
            return;
          };
          if let Some(offset) = drag.offset_at(event.position) {
            // Mark the gesture as a drag so mouse-up keeps the dragged range
            // instead of resetting it.
            drag.mark_dragging();
            drag.set(anchor, offset);
            cx.refresh_windows();
          }
        });

        // Only a real drag finalizes on mouse up. A plain click, double click
        // or triple click sets its range in `on_click`; overwriting it here
        // would reset every click to "press point → release point".
        let release = selection;
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, cx| {
          if phase != DispatchPhase::Bubble || event.button != MouseButton::Left {
            return;
          }
          // Not a drag: a click, double click or triple click owns its range,
          // so the finalizer must leave it alone.
          if !release.is_dragging() {
            return;
          }
          let Some(anchor) = release.anchor() else {
            return;
          };
          if let Some(offset) = release.offset_at(event.position) {
            release.set(anchor, offset);
            cx.refresh_windows();
          }
          release.end_drag();
        });
      },
    )
    .absolute()
    .size_0();

    v_flex()
      .id(ElementId::NamedInteger(
        SharedString::from(format!("md-root-{}", self.parsed.seed())),
        0,
      ))
      .relative()
      .child(drag_root)
      .w_full()
      .line_height(relative(BODY_LINE_HEIGHT))
      .text_color(palette.foreground)
      .track_focus(&focus_handle)
      .key_context(crate::actions::MARKDOWN_CONTEXT)
      .when(selection.is_some(), |this| {
        this
          .on_action(move |_: &crate::actions::MarkdownCopy, _window, cx| {
            let Some(text) = actions_selection
              .as_ref()
              .and_then(MarkdownSelection::selected_text)
            else {
              return;
            };
            cx.write_to_clipboard(ClipboardItem::new_string(text));
          })
          .on_action(move |_: &crate::actions::MarkdownSelectAll, _, cx| {
            if let Some(selection) = select_all_selection.as_ref() {
              selection.select_all();
              cx.refresh_windows();
            }
          })
      })
      .refine_style(&self.style)
      .children(render_blocks(
        self.parsed.blocks(),
        &ctx,
        &mut ids,
        window,
        cx,
      ))
      .map(|this| match menu_selection {
        Some(selection) => this
          .context_menu(move |menu, window, cx| {
            let has_selection = !selection.is_empty();

            let copy = {
              let selection = selection.clone();
              PopupMenuItem::new(translate_woocraft("markdown.copy"))
                .disabled(!has_selection)
                .on_click(move |_, _, cx| {
                  if let Some(text) = selection.selected_text() {
                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                  }
                })
            };

            // A link under the pointer gets its own copy action, matching what
            // readers expect from a right-click on a hyperlink.
            let copy_link = selection.hovered_link().map(|url| {
              PopupMenuItem::new(translate_woocraft("markdown.copy_link")).on_click(
                move |_, _, cx| {
                  cx.write_to_clipboard(ClipboardItem::new_string(url.to_string()));
                },
              )
            });

            let select_all = {
              let selection = selection.clone();
              PopupMenuItem::new(translate_woocraft("markdown.select_all")).on_click(
                move |_, _, cx| {
                  selection.select_all();
                  cx.refresh_windows();
                },
              )
            };

            let menu = menu.item(copy);
            let menu = match copy_link {
              Some(item) => menu.item(item),
              None => menu,
            };
            let menu = menu.item(select_all);

            // Application-provided items come last so the built-ins stay put.
            let selected = selection.selected_text();
            match builder.as_ref() {
              Some(builder) => builder(menu, selected, window, cx),
              None => menu,
            }
          })
          .into_any_element(),
        None => this.into_any_element(),
      })
  }
}

#[cfg(test)]
mod tests {
  use std::{cell::RefCell, rc::Rc};

  use gpui::{Context, Render, TestAppContext};

  use super::*;

  /// Renders a document covering every supported block kind, so a regression
  /// in any renderer surfaces as a panic or a failed layout in CI.
  #[gpui::test]
  fn renders_every_block_kind(cx: &mut TestAppContext) {
    cx.update(crate::init);

    let source = r#"
# Heading one

Paragraph with **bold**, *italic*, ~~strike~~, `code`, a [link](https://example.com)
and an inline image ![alt](./logo.png).

## Heading two

> [!NOTE]
> A note quote.

> A plain quote.

- bullet
  - nested
    1. ordered inside
- bullet with task markers covered by `parses_nested_lists_and_task_markers`

| a | b |
| :- | -: |
| 1 | 2 |

```rust
fn main() {}
```

---

A footnote reference[^1] and an ordered list:

3. third
4. fourth

[^1]: The footnote body.
"#;

    let parsed = Rc::new(ParsedMarkdown::parse(source));

    struct Probe {
      parsed: Rc<ParsedMarkdown>,
    }

    impl Render for Probe {
      fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        Markdown::from_shared(self.parsed.clone())
      }
    }

    let (_root, visual) = cx.add_window_view(|_, _| Probe { parsed });
    visual.run_until_parked();
  }

  #[gpui::test]
  fn id_prefix_defaults_to_the_document_seed(cx: &mut TestAppContext) {
    cx.update(crate::init);

    let first = Rc::new(ParsedMarkdown::parse("same [link](https://example.com)"));
    let second = Rc::new(ParsedMarkdown::parse("same [link](https://example.com)"));
    let other = Rc::new(ParsedMarkdown::parse(
      "different [link](https://example.com)",
    ));

    // Equal sources produce equal seeds, so a document keeps its element ids
    // across frames; different sources do not collide.
    assert_eq!(first.seed(), second.seed());
    assert_ne!(first.seed(), other.seed());
  }

  /// Dragging over rendered text must report a selection through the
  /// `on_selection_change` callback, which is the user-visible behaviour.
  #[gpui::test]
  fn dragging_selects_text(cx: &mut TestAppContext) {
    use gpui::{Modifiers, MouseButton, point};

    struct Probe {
      parsed: Rc<ParsedMarkdown>,
      seen: Rc<RefCell<Option<String>>>,
    }

    impl Render for Probe {
      fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let seen = self.seen.clone();
        div().w(px(600.)).child(
          Markdown::from_shared(self.parsed.clone())
            .on_selection_change(move |text| *seen.borrow_mut() = text),
        )
      }
    }

    cx.update(crate::init);
    let parsed = Rc::new(ParsedMarkdown::parse(
      "hello world this is selectable text\n",
    ));
    let seen = Rc::new(RefCell::new(None));
    let seen_for_view = seen.clone();

    let (_root, visual) = cx.add_window_view(move |_, _| Probe {
      parsed,
      seen: seen_for_view,
    });
    visual.run_until_parked();

    visual.simulate_mouse_down(
      point(px(20.), px(10.)),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.simulate_mouse_move(
      point(px(200.), px(10.)),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.simulate_mouse_up(
      point(px(200.), px(10.)),
      MouseButton::Left,
      Modifiers::default(),
    );
    visual.run_until_parked();

    let selected = seen.borrow().clone();
    assert!(
      selected.as_deref().is_some_and(|text| !text.is_empty()),
      "dragging over the text should select a range, got {selected:?}"
    );
  }

  /// A single click must not select anything: `set(offset, offset)` leaves an
  /// empty range, and only a drag (or a multi-click) widens it.
  /// A multi-click selection must survive the mouse-up finalizer.
  ///
  /// This is the regression that took several rounds to find: an element-level
  /// `on_mouse_up` unconditionally reset the range to "press point → release
  /// point", silently shrinking every double/triple click. The finalizer is now
  /// gated on `dragging`, so a click-only gesture leaves the range alone.
  /// Alignment is applied by the cell container, not by `text_center()` /
  /// `text_right()` on the text.
  ///
  /// gpui resolves text alignment only while painting, so an aligned text
  /// element still reports left-aligned geometry from its layout; mapping a
  /// pointer through it lands on the wrong character. Asserted on the rendered
  /// position of a right-aligned cell: it must sit to the right of a
  /// left-aligned one in the same width.
  #[gpui::test]
  fn right_aligned_cells_place_their_text_at_the_right(cx: &mut TestAppContext) {
    struct Probe;

    impl Render for Probe {
      fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let cell = |align_right: bool, selector: &'static str| {
          div()
            .w(px(300.))
            .flex()
            .when(align_right, |this| this.justify_end())
            .child(
              div()
                .flex_shrink_0()
                .debug_selector(move || selector.to_string())
                .child("abc"),
            )
        };

        div()
          .flex_col()
          .child(cell(false, "md-cell-left"))
          .child(cell(true, "md-cell-right"))
      }
    }

    cx.update(crate::init);
    let (_root, visual) = cx.add_window_view(|_, _| Probe);
    visual.run_until_parked();

    let left = visual.debug_bounds("md-cell-left").expect("left cell");
    let right = visual.debug_bounds("md-cell-right").expect("right cell");

    assert!(
      right.origin.x > left.origin.x,
      "a right-aligned cell's content must sit further right: left={left:?} right={right:?}"
    );
  }

  #[test]
  fn a_document_without_blocks_renders_empty() {
    let parsed = ParsedMarkdown::parse("");
    assert!(parsed.blocks().is_empty());
  }
}
