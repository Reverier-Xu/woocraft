//! Markdown parser for the [`Markdown`](crate::Markdown) component.
//!
//! [`ParsedMarkdown::parse`] turns markdown source text into a small,
//! renderer-agnostic tree of [`MarkdownBlock`]s and [`MarkdownInline`]s. The
//! tree is detached from the source string (all text is owned), so callers can
//! parse once, cache the result and rebuild the gpui element every frame
//! without paying for parsing again.
//!
//! Supported syntax is the CommonMark set plus the GitHub flavored extensions:
//! tables, task lists, strikethrough, footnotes, smart punctuation and block
//! quote kinds (`> [!NOTE]`). Raw inline HTML is dropped (except `<br>`),
//! block-level HTML is kept as plain text. Math and definition lists are left
//! disabled.

use std::hash::{DefaultHasher, Hash, Hasher};

use gpui::SharedString;
use pulldown_cmark::{
  Alignment, BlockQuoteKind, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

/// Parser options enabled for the component.
///
/// Deliberately excludes `ENABLE_DEFINITION_LIST`, `ENABLE_MATH`,
/// `ENABLE_WIKILINKS`, `ENABLE_SUPERSCRIPT` and `ENABLE_SUBSCRIPT`: the
/// component does not render them.
const PARSE_OPTIONS: Options = Options::ENABLE_TABLES
  .union(Options::ENABLE_FOOTNOTES)
  .union(Options::ENABLE_STRIKETHROUGH)
  .union(Options::ENABLE_TASKLISTS)
  .union(Options::ENABLE_SMART_PUNCTUATION)
  .union(Options::ENABLE_HEADING_ATTRIBUTES)
  .union(Options::ENABLE_GFM);

/// Heading level, from `H1` (largest) to `H6` (smallest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MarkdownHeadingLevel {
  H1,
  H2,
  H3,
  H4,
  H5,
  H6,
}

impl MarkdownHeadingLevel {
  /// Returns the numeric level (`1` to `6`).
  pub fn level(self) -> u8 {
    match self {
      Self::H1 => 1,
      Self::H2 => 2,
      Self::H3 => 3,
      Self::H4 => 4,
      Self::H5 => 5,
      Self::H6 => 6,
    }
  }
}

impl From<HeadingLevel> for MarkdownHeadingLevel {
  fn from(level: HeadingLevel) -> Self {
    match level {
      HeadingLevel::H1 => Self::H1,
      HeadingLevel::H2 => Self::H2,
      HeadingLevel::H3 => Self::H3,
      HeadingLevel::H4 => Self::H4,
      HeadingLevel::H5 => Self::H5,
      HeadingLevel::H6 => Self::H6,
    }
  }
}

/// Horizontal alignment of a table column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MarkdownAlignment {
  /// No explicit alignment; falls back to the start of the line.
  #[default]
  None,
  Left,
  Center,
  Right,
}

impl From<Alignment> for MarkdownAlignment {
  fn from(alignment: Alignment) -> Self {
    match alignment {
      Alignment::None => Self::None,
      Alignment::Left => Self::Left,
      Alignment::Center => Self::Center,
      Alignment::Right => Self::Right,
    }
  }
}

/// GitHub flavored block quote kind (`> [!NOTE]` and friends).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MarkdownBlockQuoteKind {
  Note,
  Tip,
  Important,
  Warning,
  Caution,
}

impl From<BlockQuoteKind> for MarkdownBlockQuoteKind {
  fn from(kind: BlockQuoteKind) -> Self {
    match kind {
      BlockQuoteKind::Note => Self::Note,
      BlockQuoteKind::Tip => Self::Tip,
      BlockQuoteKind::Important => Self::Important,
      BlockQuoteKind::Warning => Self::Warning,
      BlockQuoteKind::Caution => Self::Caution,
    }
  }
}

/// A fenced or indented code block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownCodeBlock {
  /// Info string of the opening fence, lowercased and with trailing metadata
  /// removed. `None` for indented code blocks and bare fences.
  pub language: Option<SharedString>,
  /// Verbatim code content, without the trailing line break of the fence.
  pub code: SharedString,
}

/// A list item; `task` is set for GitHub task list items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownListItem {
  /// Task list state (`- [ ]` / `- [x]`), if this item is a task.
  pub task: Option<bool>,
  /// Block content of the item.
  pub blocks: Vec<MarkdownBlock>,
}

/// An ordered or unordered list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarkdownList {
  /// Whether items are numbered (`true`) or bulleted (`false`).
  pub ordered: bool,
  /// Number of the first item for ordered lists; `1` otherwise.
  pub start: u64,
  pub items: Vec<MarkdownListItem>,
}

/// One row of a table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkdownTableRow {
  /// Inline content of each cell, in column order.
  pub cells: Vec<Vec<MarkdownInline>>,
}

/// A GitHub flavored table.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MarkdownTable {
  /// Alignment of each column, taken from the header delimiter row.
  pub alignments: Vec<MarkdownAlignment>,
  pub header: MarkdownTableRow,
  pub rows: Vec<MarkdownTableRow>,
}

/// A block-level markdown element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
  /// A paragraph of inline content.
  Paragraph(Vec<MarkdownInline>),
  /// A heading with its level and inline content.
  Heading {
    level: MarkdownHeadingLevel,
    inlines: Vec<MarkdownInline>,
  },
  /// A fenced or indented code block.
  CodeBlock(MarkdownCodeBlock),
  /// A block quote, optionally of a GitHub flavored kind.
  BlockQuote {
    kind: Option<MarkdownBlockQuoteKind>,
    blocks: Vec<MarkdownBlock>,
  },
  /// An ordered or unordered list.
  List(MarkdownList),
  /// A table.
  Table(MarkdownTable),
  /// A horizontal rule.
  Rule,
  /// A raw HTML block, rendered as plain text.
  Html(SharedString),
  /// A footnote definition (`[^label]: ...`).
  FootnoteDefinition {
    label: SharedString,
    blocks: Vec<MarkdownBlock>,
  },
}

/// An inline markdown element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownInline {
  /// Literal text, including substituted HTML entities and smart punctuation.
  Text(SharedString),
  /// Inline code span.
  Code(SharedString),
  Emphasis(Vec<MarkdownInline>),
  Strong(Vec<MarkdownInline>),
  Strikethrough(Vec<MarkdownInline>),
  /// A link, with its destination URL and inline label.
  Link {
    url: SharedString,
    title: Option<SharedString>,
    inlines: Vec<MarkdownInline>,
  },
  /// An image. The component cannot fetch remote images, so it renders the
  /// alt text (or the URL) unless an image handler is installed.
  Image {
    url: SharedString,
    title: Option<SharedString>,
    alt: Vec<MarkdownInline>,
  },
  /// A soft line break inside a paragraph, rendered as a space.
  SoftBreak,
  /// A hard line break (`\` at the end of a line or two trailing spaces).
  HardBreak,
  /// A reference to a footnote definition, rendered as `[^label]`.
  FootnoteReference(SharedString),
  /// Inline raw HTML, kept for parity with the parser but dropped on render.
  Html(SharedString),
}

/// The result of parsing a markdown document.
///
/// Cloning is cheap relative to parsing: the tree is a plain owned structure
/// with `SharedString` leaves, so caching one instance and building
/// [`Markdown::from_parsed`](crate::Markdown::from_parsed) per frame is the
/// intended usage for long documents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ParsedMarkdown {
  blocks: Vec<MarkdownBlock>,
  plain_text: SharedString,
  /// Hash of the source text, used to derive element ids that stay unique
  /// between different documents rendered in the same window.
  seed: u64,
}

impl ParsedMarkdown {
  /// Parses markdown source text into the block tree.
  pub fn parse(source: &str) -> Self {
    let mut stack = vec![Builder::Root { blocks: Vec::new() }];

    for event in Parser::new_ext(source, PARSE_OPTIONS) {
      match event {
        Event::Start(tag) => start_tag(&mut stack, tag),
        Event::End(tag) => end_tag(&mut stack, tag),
        Event::Text(text) => push_text(&mut stack, text.as_ref()),
        Event::Code(code) => {
          push_inline(&mut stack, MarkdownInline::Code(code.as_ref().into()));
        }
        Event::InlineMath(math) | Event::DisplayMath(math) => {
          push_inline(&mut stack, MarkdownInline::Code(math.as_ref().into()));
        }
        Event::Html(html) => push_html_text(&mut stack, html.as_ref()),
        Event::InlineHtml(html) => push_inline_html(&mut stack, html.as_ref()),
        Event::FootnoteReference(label) => push_inline(
          &mut stack,
          MarkdownInline::FootnoteReference(label.as_ref().into()),
        ),
        Event::SoftBreak => push_break(&mut stack, false),
        Event::HardBreak => push_break(&mut stack, true),
        Event::Rule => push_block(&mut stack, MarkdownBlock::Rule),
        Event::TaskListMarker(checked) => set_task_marker(&mut stack, checked),
      }
    }

    // The parser guarantees balanced start/end events. Close leftovers anyway
    // so a future option cannot turn malformed input into dropped content.
    while stack.len() > 1 {
      let builder = stack.pop().expect("builder stack is not empty");
      close_builder(&mut stack, builder);
    }

    let Some(Builder::Root { blocks }) = stack.pop() else {
      unreachable!("the root builder is always on the stack")
    };

    let mut plain_text = String::new();
    collect_plain_text(&blocks, &mut plain_text);

    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);

    Self {
      blocks,
      plain_text: plain_text.trim().to_string().into(),
      seed: hasher.finish(),
    }
  }

  /// Returns the top-level blocks of the document.
  pub fn blocks(&self) -> &[MarkdownBlock] {
    &self.blocks
  }

  /// Returns the document text with markdown syntax removed, suitable for
  /// search, labels or copying.
  pub fn plain_text(&self) -> &str {
    &self.plain_text
  }

  /// Returns `true` when the document has no block content.
  pub fn is_empty(&self) -> bool {
    self.blocks.is_empty()
  }

  /// Hash of the source text, used as a namespace for interactive element
  /// ids so documents rendered side by side do not share element state.
  pub(crate) fn seed(&self) -> u64 {
    self.seed
  }
}

/// A frame of the parser's nesting stack.
enum Builder {
  Root {
    blocks: Vec<MarkdownBlock>,
  },
  Paragraph {
    inlines: Vec<MarkdownInline>,
  },
  Heading {
    level: MarkdownHeadingLevel,
    inlines: Vec<MarkdownInline>,
  },
  Emphasis {
    inlines: Vec<MarkdownInline>,
  },
  Strong {
    inlines: Vec<MarkdownInline>,
  },
  Strikethrough {
    inlines: Vec<MarkdownInline>,
  },
  /// Superscript/subscript or any other transparent inline container.
  Span {
    inlines: Vec<MarkdownInline>,
  },
  Link {
    url: SharedString,
    title: Option<SharedString>,
    inlines: Vec<MarkdownInline>,
  },
  Image {
    url: SharedString,
    title: Option<SharedString>,
    alt: Vec<MarkdownInline>,
  },
  CodeBlock {
    language: Option<SharedString>,
    code: String,
  },
  BlockQuote {
    kind: Option<MarkdownBlockQuoteKind>,
    blocks: Vec<MarkdownBlock>,
  },
  Footnote {
    label: SharedString,
    blocks: Vec<MarkdownBlock>,
  },
  List {
    ordered: bool,
    start: u64,
    items: Vec<MarkdownListItem>,
  },
  Item {
    task: Option<bool>,
    /// Inline content emitted directly by tight list items (pulldown-cmark
    /// omits `Paragraph` tags there).
    inlines: Vec<MarkdownInline>,
    blocks: Vec<MarkdownBlock>,
  },
  Table {
    alignments: Vec<MarkdownAlignment>,
    header: Vec<Vec<MarkdownInline>>,
    rows: Vec<Vec<Vec<MarkdownInline>>>,
    current_row: Vec<Vec<MarkdownInline>>,
  },
  Cell {
    inlines: Vec<MarkdownInline>,
  },
  Html {
    text: String,
  },
  /// Skipped metadata block content.
  Metadata {
    text: String,
  },
}

fn start_tag(stack: &mut Vec<Builder>, tag: Tag<'_>) {
  match tag {
    Tag::Paragraph => stack.push(Builder::Paragraph {
      inlines: Vec::new(),
    }),
    Tag::Heading { level, .. } => stack.push(Builder::Heading {
      level: level.into(),
      inlines: Vec::new(),
    }),
    Tag::BlockQuote(kind) => stack.push(Builder::BlockQuote {
      kind: kind.map(Into::into),
      blocks: Vec::new(),
    }),
    Tag::CodeBlock(kind) => stack.push(Builder::CodeBlock {
      language: fence_language(&kind),
      code: String::new(),
    }),
    Tag::HtmlBlock => stack.push(Builder::Html {
      text: String::new(),
    }),
    Tag::List(start) => stack.push(Builder::List {
      ordered: start.is_some(),
      start: start.unwrap_or(1),
      items: Vec::new(),
    }),
    Tag::Item => stack.push(Builder::Item {
      task: None,
      inlines: Vec::new(),
      blocks: Vec::new(),
    }),
    Tag::FootnoteDefinition(label) => stack.push(Builder::Footnote {
      label: label.as_ref().into(),
      blocks: Vec::new(),
    }),
    Tag::Table(alignments) => stack.push(Builder::Table {
      alignments: alignments.into_iter().map(Into::into).collect(),
      header: Vec::new(),
      rows: Vec::new(),
      current_row: Vec::new(),
    }),
    // `TableHead` and `TableRow` only group cells; the table frame collects
    // them on the end events, so they have no builder of their own.
    Tag::TableHead | Tag::TableRow => {}
    Tag::TableCell => stack.push(Builder::Cell {
      inlines: Vec::new(),
    }),
    Tag::Emphasis => stack.push(Builder::Emphasis {
      inlines: Vec::new(),
    }),
    Tag::Strong => stack.push(Builder::Strong {
      inlines: Vec::new(),
    }),
    Tag::Strikethrough => stack.push(Builder::Strikethrough {
      inlines: Vec::new(),
    }),
    Tag::Superscript | Tag::Subscript => stack.push(Builder::Span {
      inlines: Vec::new(),
    }),
    Tag::Link {
      dest_url, title, ..
    } => stack.push(Builder::Link {
      url: dest_url.as_ref().into(),
      title: non_empty(title.as_ref()),
      inlines: Vec::new(),
    }),
    Tag::Image {
      dest_url, title, ..
    } => stack.push(Builder::Image {
      url: dest_url.as_ref().into(),
      title: non_empty(title.as_ref()),
      alt: Vec::new(),
    }),
    Tag::MetadataBlock(_) => stack.push(Builder::Metadata {
      text: String::new(),
    }),
    // Definition lists are not enabled; ignore the tags defensively.
    Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
  }
}

fn end_tag(stack: &mut Vec<Builder>, tag: TagEnd) {
  match tag {
    TagEnd::TableHead => {
      if let Some(Builder::Table {
        header,
        current_row,
        ..
      }) = table_mut(stack)
      {
        *header = std::mem::take(current_row);
      }
    }
    TagEnd::TableRow => {
      if let Some(Builder::Table {
        rows, current_row, ..
      }) = table_mut(stack)
      {
        rows.push(std::mem::take(current_row));
      }
    }
    TagEnd::DefinitionList | TagEnd::DefinitionListTitle | TagEnd::DefinitionListDefinition => {}
    _ => {
      if stack.len() <= 1 {
        return;
      }

      let builder = stack.pop().expect("builder stack is not empty");
      close_builder(stack, builder);
    }
  }
}

fn close_builder(stack: &mut [Builder], builder: Builder) {
  match builder {
    Builder::Root { blocks } => {
      // Only reachable if a nested root is popped; merge into the parent.
      for block in blocks {
        push_block(stack, block);
      }
    }
    Builder::Paragraph { inlines } => {
      if !inlines.is_empty() {
        push_block(stack, MarkdownBlock::Paragraph(inlines));
      }
    }
    Builder::Heading { level, inlines } => {
      push_block(stack, MarkdownBlock::Heading { level, inlines });
    }
    Builder::Emphasis { inlines } => {
      push_inline(stack, MarkdownInline::Emphasis(inlines));
    }
    Builder::Strong { inlines } => {
      push_inline(stack, MarkdownInline::Strong(inlines));
    }
    Builder::Strikethrough { inlines } => {
      push_inline(stack, MarkdownInline::Strikethrough(inlines));
    }
    Builder::Span { inlines } => {
      for inline in inlines {
        push_inline(stack, inline);
      }
    }
    Builder::Link {
      url,
      title,
      inlines,
    } => {
      push_inline(
        stack,
        MarkdownInline::Link {
          url,
          title,
          inlines,
        },
      );
    }
    Builder::Image { url, title, alt } => {
      push_inline(stack, MarkdownInline::Image { url, title, alt });
    }
    Builder::CodeBlock { language, mut code } => {
      trim_trailing_line_break(&mut code);
      push_block(
        stack,
        MarkdownBlock::CodeBlock(MarkdownCodeBlock {
          language,
          code: code.into(),
        }),
      );
    }
    Builder::BlockQuote { kind, blocks } => {
      push_block(stack, MarkdownBlock::BlockQuote { kind, blocks });
    }
    Builder::Footnote { label, blocks } => {
      push_block(stack, MarkdownBlock::FootnoteDefinition { label, blocks });
    }
    Builder::List {
      ordered,
      start,
      items,
    } => {
      if !items.is_empty() {
        push_block(
          stack,
          MarkdownBlock::List(MarkdownList {
            ordered,
            start,
            items,
          }),
        );
      }
    }
    Builder::Item {
      task,
      mut inlines,
      mut blocks,
    } => {
      flush_item_inlines(&mut inlines, &mut blocks);
      push_item(stack, MarkdownListItem { task, blocks });
    }
    Builder::Table {
      alignments,
      header,
      mut rows,
      current_row,
    } => {
      if !current_row.is_empty() {
        rows.push(current_row);
      }
      push_block(
        stack,
        MarkdownBlock::Table(MarkdownTable {
          alignments,
          header: MarkdownTableRow { cells: header },
          rows: rows
            .into_iter()
            .map(|cells| MarkdownTableRow { cells })
            .collect(),
        }),
      );
    }
    Builder::Cell { inlines } => {
      if let Some(Builder::Table { current_row, .. }) = table_mut(stack) {
        current_row.push(inlines);
      }
    }
    Builder::Html { text } => {
      let text = text.trim();
      if !text.is_empty() {
        push_block(stack, MarkdownBlock::Html(text.into()));
      }
    }
    Builder::Metadata { .. } => {}
  }
}

fn push_block(stack: &mut [Builder], block: MarkdownBlock) {
  for builder in stack.iter_mut().rev() {
    match builder {
      Builder::Item {
        inlines, blocks, ..
      } => {
        // Tight list items emit inline content without `Paragraph` tags; a
        // block arriving after it must not reorder the item's content.
        flush_item_inlines(inlines, blocks);
        blocks.push(block);
        return;
      }
      Builder::Root { blocks }
      | Builder::BlockQuote { blocks, .. }
      | Builder::Footnote { blocks, .. } => {
        blocks.push(block);
        return;
      }
      _ => continue,
    }
  }
}

fn flush_item_inlines(inlines: &mut Vec<MarkdownInline>, blocks: &mut Vec<MarkdownBlock>) {
  if !inlines.is_empty() {
    let inlines = std::mem::take(inlines);
    blocks.push(MarkdownBlock::Paragraph(inlines));
  }
}

fn push_inline(stack: &mut [Builder], inline: MarkdownInline) {
  for builder in stack.iter_mut().rev() {
    match builder {
      Builder::Paragraph { inlines }
      | Builder::Heading { inlines, .. }
      | Builder::Emphasis { inlines }
      | Builder::Strong { inlines }
      | Builder::Strikethrough { inlines }
      | Builder::Span { inlines }
      | Builder::Link { inlines, .. }
      | Builder::Image { alt: inlines, .. }
      | Builder::Item { inlines, .. }
      | Builder::Cell { inlines } => {
        inlines.push(inline);
        return;
      }
      _ => continue,
    }
  }
}

fn push_item(stack: &mut [Builder], item: MarkdownListItem) {
  for builder in stack.iter_mut().rev() {
    if let Builder::List { items, .. } = builder {
      items.push(item);
      return;
    }
  }
}

fn table_mut(stack: &mut [Builder]) -> Option<&mut Builder> {
  stack
    .iter_mut()
    .rev()
    .find(|builder| matches!(builder, Builder::Table { .. }))
}

fn push_text(stack: &mut [Builder], text: &str) {
  if push_raw_text(stack, text) {
    return;
  }
  push_inline(stack, MarkdownInline::Text(text.into()));
}

/// Appends raw text to code and HTML blocks. Returns `false` for inline
/// contexts.
fn push_raw_text(stack: &mut [Builder], text: &str) -> bool {
  match stack.last_mut() {
    Some(Builder::CodeBlock { code, .. }) => {
      code.push_str(text);
      true
    }
    Some(Builder::Html { text: html }) | Some(Builder::Metadata { text: html }) => {
      html.push_str(text);
      true
    }
    _ => false,
  }
}

fn push_html_text(stack: &mut [Builder], html: &str) {
  // Stray block-level HTML outside an `HtmlBlock` frame is dropped.
  push_raw_text(stack, html);
}

fn push_inline_html(stack: &mut [Builder], html: &str) {
  let tag = html.trim();
  if tag.eq_ignore_ascii_case("<br>")
    || tag.eq_ignore_ascii_case("<br/>")
    || tag.eq_ignore_ascii_case("<br />")
  {
    push_inline(stack, MarkdownInline::HardBreak);
  } else {
    push_inline(stack, MarkdownInline::Html(html.into()));
  }
}

fn push_break(stack: &mut [Builder], hard: bool) {
  let in_raw_block = matches!(
    stack.last(),
    Some(Builder::CodeBlock { .. } | Builder::Html { .. } | Builder::Metadata { .. })
  );
  if in_raw_block {
    push_raw_text(stack, "\n");
    return;
  }

  push_inline(
    stack,
    if hard {
      MarkdownInline::HardBreak
    } else {
      MarkdownInline::SoftBreak
    },
  );
}

fn set_task_marker(stack: &mut [Builder], checked: bool) {
  for builder in stack.iter_mut().rev() {
    if let Builder::Item { task, .. } = builder {
      *task = Some(checked);
      return;
    }
  }
}

fn fence_language(kind: &CodeBlockKind<'_>) -> Option<SharedString> {
  let CodeBlockKind::Fenced(info) = kind else {
    return None;
  };

  let token = info
    .split(|character: char| character.is_whitespace() || character == ',')
    .find(|token| !token.is_empty())?;

  Some(token.to_ascii_lowercase().into())
}

fn non_empty(value: &str) -> Option<SharedString> {
  (!value.is_empty()).then(|| value.into())
}

fn trim_trailing_line_break(code: &mut String) {
  if code.ends_with('\n') {
    code.pop();
    if code.ends_with('\r') {
      code.pop();
    }
  }
}

fn collect_plain_text(blocks: &[MarkdownBlock], output: &mut String) {
  for block in blocks {
    match block {
      MarkdownBlock::Paragraph(inlines) => {
        collect_plain_inlines(inlines, output);
        output.push('\n');
      }
      MarkdownBlock::Heading { inlines, .. } => {
        collect_plain_inlines(inlines, output);
        output.push('\n');
      }
      MarkdownBlock::CodeBlock(code) => {
        output.push_str(&code.code);
        output.push('\n');
      }
      MarkdownBlock::BlockQuote { blocks, .. } => collect_plain_text(blocks, output),
      MarkdownBlock::List(list) => {
        for item in &list.items {
          if let Some(checked) = item.task {
            output.push_str(if checked { "[x] " } else { "[ ] " });
          }
          collect_plain_text(&item.blocks, output);
        }
      }
      MarkdownBlock::Table(table) => {
        collect_plain_cells(&table.header.cells, output);
        output.push('\n');
        for row in &table.rows {
          collect_plain_cells(&row.cells, output);
          output.push('\n');
        }
      }
      MarkdownBlock::Rule => output.push('\n'),
      MarkdownBlock::Html(html) => {
        output.push_str(html);
        output.push('\n');
      }
      MarkdownBlock::FootnoteDefinition { label, blocks } => {
        output.push_str(&format!("[^{label}]: "));
        collect_plain_text(blocks, output);
        output.push('\n');
      }
    }
  }
}

fn collect_plain_cells(cells: &[Vec<MarkdownInline>], output: &mut String) {
  for (index, cell) in cells.iter().enumerate() {
    if index > 0 {
      output.push(' ');
    }
    collect_plain_inlines(cell, output);
  }
}

fn collect_plain_inlines(inlines: &[MarkdownInline], output: &mut String) {
  for inline in inlines {
    match inline {
      MarkdownInline::Text(text) | MarkdownInline::Code(text) => output.push_str(text),
      MarkdownInline::Emphasis(inlines)
      | MarkdownInline::Strong(inlines)
      | MarkdownInline::Strikethrough(inlines) => collect_plain_inlines(inlines, output),
      MarkdownInline::Link { inlines, .. } => collect_plain_inlines(inlines, output),
      MarkdownInline::Image { url, alt, .. } => {
        if alt.is_empty() {
          output.push_str(url);
        } else {
          collect_plain_inlines(alt, output);
        }
      }
      MarkdownInline::SoftBreak => output.push(' '),
      MarkdownInline::HardBreak => output.push('\n'),
      MarkdownInline::FootnoteReference(label) => {
        output.push_str(&format!("[^{label}]"));
      }
      MarkdownInline::Html(_) => {}
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn text(value: &str) -> MarkdownInline {
    MarkdownInline::Text(value.into())
  }

  #[test]
  fn parses_headings_and_inline_styles() {
    let parsed =
      ParsedMarkdown::parse("# Title\n\nSome **bold**, *italic*, `code` and ~~gone~~ text.\n");

    assert_eq!(parsed.blocks().len(), 2);
    assert!(matches!(
      &parsed.blocks()[0],
      MarkdownBlock::Heading {
        level: MarkdownHeadingLevel::H1,
        inlines
      } if *inlines == [text("Title")]
    ));

    let MarkdownBlock::Paragraph(inlines) = &parsed.blocks()[1] else {
      panic!("expected a paragraph");
    };
    assert!(inlines.iter().any(|inline| matches!(
      inline,
      MarkdownInline::Strong(inner) if *inner == [text("bold")]
    )));
    assert!(inlines.iter().any(|inline| matches!(
      inline,
      MarkdownInline::Emphasis(inner) if *inner == [text("italic")]
    )));
    assert!(inlines.iter().any(|inline| matches!(
      inline,
      MarkdownInline::Code(code) if code == "code"
    )));
    assert!(inlines.iter().any(|inline| matches!(
      inline,
      MarkdownInline::Strikethrough(inner) if *inner == [text("gone")]
    )));
  }

  #[test]
  fn parses_links_and_images() {
    let parsed = ParsedMarkdown::parse("[docs](https://example.com) ![logo](logo.png)");

    let MarkdownBlock::Paragraph(inlines) = &parsed.blocks()[0] else {
      panic!("expected a paragraph");
    };
    assert!(inlines.iter().any(|inline| matches!(
      inline,
      MarkdownInline::Link { url, inlines, .. }
        if url == "https://example.com" && *inlines == [text("docs")]
    )));
    assert!(inlines.iter().any(|inline| matches!(
      inline,
      MarkdownInline::Image { url, alt, .. }
        if url == "logo.png" && *alt == [text("logo")]
    )));
  }

  #[test]
  fn parses_nested_lists_and_task_markers() {
    let parsed = ParsedMarkdown::parse("- [x] done\n- [ ] pending\n  - nested\n\n1. one\n2. two\n");

    let MarkdownBlock::List(list) = &parsed.blocks()[0] else {
      panic!("expected a list");
    };
    assert!(!list.ordered);
    assert_eq!(list.items.len(), 2);
    assert_eq!(list.items[0].task, Some(true));
    assert_eq!(list.items[1].task, Some(false));
    assert!(matches!(
      &list.items[1].blocks[1],
      MarkdownBlock::List(nested) if nested.items.len() == 1
    ));

    let MarkdownBlock::List(ordered) = &parsed.blocks()[1] else {
      panic!("expected a list");
    };
    assert!(ordered.ordered);
    assert_eq!(ordered.start, 1);
    assert_eq!(ordered.items.len(), 2);
  }

  #[test]
  fn parses_code_blocks_with_language() {
    let parsed = ParsedMarkdown::parse("```rust,ignore\nfn main() {}\n```\n");

    let MarkdownBlock::CodeBlock(code) = &parsed.blocks()[0] else {
      panic!("expected a code block");
    };
    assert_eq!(code.language.as_deref(), Some("rust"));
    assert_eq!(code.code, "fn main() {}");
  }

  #[test]
  fn parses_tables_and_alignments() {
    let parsed = ParsedMarkdown::parse("| a | b |\n| :- | :-: |\n| 1 | 2 |\n");

    let MarkdownBlock::Table(table) = &parsed.blocks()[0] else {
      panic!("expected a table");
    };
    assert_eq!(
      table.alignments,
      [MarkdownAlignment::Left, MarkdownAlignment::Center]
    );
    assert_eq!(table.header.cells.len(), 2);
    assert_eq!(table.rows.len(), 1);
    assert_eq!(table.header.cells[0], [text("a")]);
    assert_eq!(table.rows[0].cells[1], [text("2")]);
  }

  #[test]
  fn parses_block_quotes_and_rules() {
    let parsed = ParsedMarkdown::parse("> [!WARNING]\n> careful\n\n---\n");

    let MarkdownBlock::BlockQuote { kind, blocks } = &parsed.blocks()[0] else {
      panic!("expected a block quote");
    };
    assert_eq!(kind, &Some(MarkdownBlockQuoteKind::Warning));
    assert!(matches!(&blocks[0], MarkdownBlock::Paragraph(_)));
    assert!(matches!(&parsed.blocks()[1], MarkdownBlock::Rule));
  }

  #[test]
  fn parses_footnotes() {
    let parsed = ParsedMarkdown::parse("Body[^1]\n\n[^1]: note\n");

    let MarkdownBlock::Paragraph(inlines) = &parsed.blocks()[0] else {
      panic!("expected a paragraph");
    };
    assert!(matches!(
      &inlines[1],
      MarkdownInline::FootnoteReference(label) if label == "1"
    ));
    assert!(matches!(
      &parsed.blocks()[1],
      MarkdownBlock::FootnoteDefinition { label, .. } if label == "1"
    ));
  }

  #[test]
  fn hard_breaks_and_soft_breaks() {
    let parsed = ParsedMarkdown::parse("soft\nbreak and hard\\\nbreak\n");

    let MarkdownBlock::Paragraph(inlines) = &parsed.blocks()[0] else {
      panic!("expected a paragraph");
    };
    assert!(
      inlines
        .iter()
        .any(|i| matches!(i, MarkdownInline::SoftBreak))
    );
    assert!(
      inlines
        .iter()
        .any(|i| matches!(i, MarkdownInline::HardBreak))
    );
  }

  #[test]
  fn plain_text_strips_markup() {
    let parsed =
      ParsedMarkdown::parse("# Title\n\n**Bold** and [link](https://example.com)\n\n- item\n");

    assert_eq!(parsed.plain_text(), "Title\nBold and link\nitem");
  }

  #[test]
  fn empty_document_is_empty() {
    let parsed = ParsedMarkdown::parse("");
    assert!(parsed.is_empty());
    assert_eq!(parsed.plain_text(), "");
  }
}
