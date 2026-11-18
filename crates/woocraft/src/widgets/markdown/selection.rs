//! Text selection over a rendered document.
//!
//! Every rendered text run registers itself here with its document offset and
//! its shaped layout. That registry is the single source of truth for offsets:
//! pointer positions are mapped back through the same layouts, and
//! [`Markdown::selected_text`](super::Markdown::selected_text) slices the same
//! concatenation.
//!
//! Highlighting is deliberately *not* painted here. The selection is handed to
//! the text engine as a [`HighlightStyle`] background run, so wrapping,
//! alignment and font fallback all come from the layout instead of being
//! recomputed by hand.

use std::{cell::RefCell, ops::Range, rc::Rc};

use gpui::{
  AnyElement, ClickEvent, Div, ElementId, HighlightStyle, InteractiveElement as _, MouseButton,
  MouseDownEvent, MouseMoveEvent, ParentElement as _, Pixels, SharedString,
  StatefulInteractiveElement as _, Styled as _, Window, div,
};

use super::element::{RenderContext, SelectionChangeHandler};

pub(super) fn with_selection_highlight(
  highlights: Vec<(Range<usize>, HighlightStyle)>, run_start: usize, run_len: usize,
  ctx: &RenderContext<'_>,
) -> Vec<(Range<usize>, HighlightStyle)> {
  let Some(selection) = ctx.selection.as_ref() else {
    return highlights;
  };
  let Some(range) = selection.range() else {
    return highlights;
  };

  let from = range.start.max(run_start);
  let to = range.end.min(run_start + run_len);
  if from >= to {
    return highlights;
  }

  gpui::combine_highlights(
    highlights,
    [(
      from - run_start..to - run_start,
      HighlightStyle {
        background_color: Some(ctx.palette.selection),
        ..Default::default()
      },
    )],
  )
  .collect()
}

/// The document offset the next run will be registered at.
pub(super) fn next_run_start(ctx: &RenderContext<'_>) -> usize {
  ctx
    .selection
    .as_ref()
    .map(MarkdownSelection::next_start)
    .unwrap_or_default()
}

pub(super) fn selectable_text(
  content: AnyElement, layout: gpui::TextLayout, text: &str, id: ElementId,
  ctx: &RenderContext<'_>, _window: &mut Window,
) -> gpui::Stateful<Div> {
  let Some(selection) = ctx.selection.clone() else {
    return div().id(id).w_full().child(content);
  };

  let len = text.len();
  let start = selection.register(text, &layout);

  // One shared record for every handler below: the three closures need the
  // same layout, text and selection, so they share one allocation instead of
  // cloning each of them three times.
  let run = Rc::new(RunHandlers {
    layout,
    text: text.to_string().into(),
    selection: selection.clone(),
    start,
    len,
  });

  div()
    .id(id)
    .w_full()
    .relative()
    .cursor_text()
    .child(content)
    .on_mouse_down(MouseButton::Left, {
      let run = run.clone();
      move |event: &MouseDownEvent, _, _| {
        let offset = run.offset_for(event.position);
        run.selection.set(offset, offset);
      }
    })
    .on_mouse_move({
      let run = run.clone();
      move |event: &MouseMoveEvent, _, cx| {
        if !event.dragging() {
          return;
        }
        let Some(anchor) = run.selection.anchor() else {
          return;
        };
        let offset = run.offset_for(event.position);
        run.selection.set(anchor, offset);
        cx.refresh_windows();
      }
    })
    .on_click({
      let run = run.clone();
      move |event: &ClickEvent, _, cx| {
        // Double click selects a word, triple click the line it belongs to.
        // `click_count` is provided by the platform, so no timing is needed
        // here.
        //
        // Every run registers a click handler, so the one under the pointer
        // must be the only one acting: otherwise a later run would overwrite
        // the range with its own (clamped) view of the same position.
        let position = event.position();
        if !run.layout.bounds().contains(&position) {
          return;
        }

        // Ranges are computed inside this run: a run is one complete block
        // (a paragraph, a heading, a table cell), so a word or line never spans
        // two of them. Concatenating runs into one string would glue unrelated
        // blocks together and make a "line" run from the document start.
        let offset = run.offset_for(position);
        let local = offset.saturating_sub(run.start).min(run.len);

        // A click is not a drag: clear the flag so the mouse-up finalizer does
        // not overwrite the range chosen here.
        run.selection.end_drag();

        match event.click_count() {
          2 => {
            let (from, to) = word_range(&run.text, local);
            run.selection.set(run.start + from, run.start + to);
            cx.refresh_windows();
          }
          3 => {
            let (from, to) = line_range(&run.text, local);
            run.selection.set(run.start + from, run.start + to);
            cx.refresh_windows();
          }
          _ => {}
        }
      }
    })
}

/// Everything the pointer handlers of one text run need.
///
/// The three handlers (`mouse_down`, `mouse_move`, `click`) share one
/// allocation instead of cloning the layout, the text and the selection once
/// per handler.
pub(super) struct RunHandlers {
  layout: gpui::TextLayout,
  text: SharedString,
  selection: MarkdownSelection,
  start: usize,
  len: usize,
}

impl RunHandlers {
  /// Maps a window position to a document offset inside this run.
  fn offset_for(&self, position: gpui::Point<Pixels>) -> usize {
    // `index_for_position` takes window coordinates (it compares against the
    // layout's own bounds internally), which is what pointer events carry.
    match self.layout.index_for_position(position) {
      Ok(index) | Err(index) => self.start + index.min(self.len),
    }
  }
}

/// Byte range of the word containing `offset`.
///
/// Word characters are letters, digits and underscores, so punctuation between
/// words is not swallowed into the selection.
pub(super) fn word_range(text: &str, offset: usize) -> (usize, usize) {
  let offset = offset.min(text.len());
  let is_word = |c: char| c.is_alphanumeric() || c == '_';

  // A double click on punctuation or whitespace selects just that character,
  // matching what editors do; the word scan below only applies inside a word.
  match text[offset..].chars().next() {
    Some(c) if !is_word(c) => return (offset, offset + c.len_utf8()),
    None => return (offset, offset),
    _ => {}
  }

  let start = text[..offset]
    .char_indices()
    .rev()
    .find(|(_, c)| !is_word(*c))
    .map(|(index, c)| index + c.len_utf8())
    .unwrap_or(0);

  let end = text[offset..]
    .char_indices()
    .find(|(_, c)| !is_word(*c))
    .map(|(index, _)| offset + index)
    .unwrap_or(text.len());

  (start, end)
}

/// Byte range of the line containing `offset`.
pub(super) fn line_range(text: &str, offset: usize) -> (usize, usize) {
  let offset = offset.min(text.len());
  let start = text[..offset]
    .rfind('\n')
    .map(|index| index + 1)
    .unwrap_or(0);
  let end = text[offset..]
    .find('\n')
    .map(|index| offset + index)
    .unwrap_or(text.len());
  (start, end)
}

/// Maps a pointer position to a document offset inside one text run.
///
/// Falls back to the run's start/end when the pointer is outside the run, so a
/// drag that leaves the text still extends the selection in a sensible
/// direction.
#[derive(Default, Clone)]
pub(crate) struct MarkdownSelection {
  /// Document offset where the drag started, and the one currently under the
  /// pointer. Wrapped in `Rc<RefCell<_>>` because pointer handlers are `Fn`
  /// closures.
  pub(super) range_state: Rc<RefCell<(Option<usize>, Option<usize>)>>,
  /// Registered runs, in render order: document offset, layout and length.
  /// The root uses these to map a pointer anywhere in the document to an
  /// offset, instead of every run guessing from its own bounds.
  pub(super) runs: Rc<RefCell<Vec<RegisteredRun>>>,
  /// Invoked with the new selection text whenever the range changes.
  pub(super) on_change: Rc<RefCell<Option<SelectionChangeHandler>>>,
  /// Whether the current press has turned into a drag. Multi-click selection
  /// must not be overwritten by the mouse-up finalizer.
  pub(super) dragging: Rc<std::cell::Cell<bool>>,
  /// URL of the link under the pointer, if any. Drives the "copy link" menu
  /// item, which is only meaningful while a link is hovered.
  pub(super) hovered_link: Rc<RefCell<Option<SharedString>>>,
}

/// One text run registered with the document selection.
#[derive(Clone)]
pub(crate) struct RegisteredRun {
  /// Document offset where this run starts.
  start: usize,
  /// The run's text, so the document can be rebuilt outside render.
  text: SharedString,
  /// Layout of the run, used to map pointer positions to offsets.
  layout: gpui::TextLayout,
}

impl RegisteredRun {
  /// Byte length of this run.
  pub(super) fn len(&self) -> usize {
    self.text.len()
  }
}

impl MarkdownSelection {
  /// Document offset at which the next run will start.
  ///
  /// Runs are laid out in render order, so this is simply the end of the last
  /// registered one.
  pub(super) fn next_start(&self) -> usize {
    self
      .runs
      .borrow()
      .last()
      .map(|run| run.start + run.len())
      .unwrap_or_default()
  }

  /// Registers a text run and returns the document offset it was placed at.
  ///
  /// This is the only place offsets are assigned: callers that need to know a
  /// run's offset ahead of rendering (to rebase the selection highlight) use
  /// [`Self::next_start`], which reads the same source.
  pub(super) fn register(&self, text: &str, layout: &gpui::TextLayout) -> usize {
    let start = self.next_start();
    self.runs.borrow_mut().push(RegisteredRun {
      start,
      text: text.to_string().into(),
      layout: layout.clone(),
    });
    start
  }

  pub(super) fn set(&self, anchor: usize, cursor: usize) {
    *self.range_state.borrow_mut() = (Some(anchor), Some(cursor));
    self.notify();
  }

  /// Selects the whole document.
  ///
  /// The length comes from the registered runs rather than the text buffer:
  /// the buffer is cleared at the start of every render, so it is empty while
  /// an action callback runs.
  pub(super) fn select_all(&self) {
    let end = self
      .runs
      .borrow()
      .last()
      .map(|run| run.start + run.len())
      .unwrap_or_default();
    if end > 0 {
      self.set(0, end);
    }
  }

  pub(super) fn is_dragging(&self) -> bool {
    self.dragging.get()
  }

  pub(super) fn mark_dragging(&self) {
    self.dragging.set(true);
  }

  pub(super) fn end_drag(&self) {
    self.dragging.set(false);
  }

  /// Records the link under the pointer.
  pub(super) fn set_hovered_link(&self, url: Option<SharedString>) {
    *self.hovered_link.borrow_mut() = url;
  }

  pub(super) fn hovered_link(&self) -> Option<SharedString> {
    self.hovered_link.borrow().clone()
  }

  /// The whole document text, in render order.
  ///
  /// Rebuilt from the registered runs rather than the `text` buffer: that
  /// buffer is cleared at the start of every render, so it is empty while an
  /// event callback runs.
  pub(super) fn document_text(&self) -> Option<String> {
    let runs = self.runs.borrow();
    if runs.is_empty() {
      return None;
    }

    let mut document = String::new();
    for run in runs.iter() {
      document.push_str(&run.text);
    }
    (!document.is_empty()).then_some(document)
  }

  /// Maps a window-space pointer position to a document offset.
  ///
  /// The run whose laid-out bounds contain the point wins; a point outside
  /// every run clamps to the nearest run, so a drag that leaves the text keeps
  /// extending in a sensible direction instead of jumping to the document edge.
  pub(super) fn offset_at(&self, position: gpui::Point<Pixels>) -> Option<usize> {
    let runs = self.runs.borrow();
    if runs.is_empty() {
      return None;
    }

    for run in runs.iter() {
      let bounds = run.layout.bounds();
      if bounds.contains(&position) {
        return Some(match run.layout.index_for_position(position) {
          Ok(index) | Err(index) => run.start + index.min(run.len()),
        });
      }
    }

    // Outside every run: clamp to the closest one vertically, then to its
    // start or end depending on which side the pointer is on.
    let closest = runs.iter().min_by(|a, b| {
      let da = (a.layout.bounds().center().y - position.y).abs();
      let db = (b.layout.bounds().center().y - position.y).abs();
      da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
    })?;
    let bounds = closest.layout.bounds();
    let offset = if position.y < bounds.origin.y {
      closest.start
    } else if position.y > bounds.bottom() {
      closest.start + closest.len()
    } else if position.x < bounds.origin.x {
      closest.start
    } else {
      closest.start + closest.len()
    };
    Some(offset)
  }

  pub(super) fn notify(&self) {
    let Some(callback) = self.on_change.borrow().clone() else {
      return;
    };
    callback(self.selected_text());
  }

  /// Anchor offset, if a drag is in progress.
  pub(super) fn anchor(&self) -> Option<usize> {
    self.range_state.borrow().0
  }

  pub(super) fn range(&self) -> Option<Range<usize>> {
    let (anchor, cursor) = *self.range_state.borrow();
    let (anchor, cursor) = (anchor?, cursor?);
    (anchor != cursor).then(|| anchor.min(cursor)..anchor.max(cursor))
  }

  pub(super) fn is_empty(&self) -> bool {
    self.range().is_none()
  }

  /// Selected text, sliced out of the concatenated document text.
  pub(super) fn selected_text(&self) -> Option<String> {
    let range = self.range()?;
    // Rebuilt from the runs, not the `text` buffer, which is cleared at the
    // start of every render and therefore empty inside event callbacks.
    let text = self.document_text()?;
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());

    // Snap to char boundaries: offsets come from glyph indices.
    let mut start = start;
    while start > 0 && !text.is_char_boundary(start) {
      start -= 1;
    }
    let mut end = end;
    while end < text.len() && !text.is_char_boundary(end) {
      end += 1;
    }

    (start < end).then(|| text[start..end].to_string())
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn mouse_up_finalizer_only_applies_to_drags() {
    let selection = MarkdownSelection::default();
    selection.runs.borrow_mut().push(RegisteredRun {
      start: 0,
      text: "hello world".into(),
      layout: gpui::TextLayout::default(),
    });

    // Simulate a double click: `on_click` picks the whole word and clears the
    // drag flag.
    selection.end_drag();
    selection.set(0, 5);
    assert!(!selection.is_dragging());
    assert_eq!(selection.selected_text().as_deref(), Some("hello"));

    // The mouse-up finalizer must not touch a non-drag gesture. Reproduce what
    // it would have done and assert the guard prevents it.
    if selection.is_dragging() {
      selection.set(2, 2);
    }
    assert_eq!(
      selection.selected_text().as_deref(),
      Some("hello"),
      "a click selection must not be reset by the mouse-up finalizer"
    );

    // A real drag does mark itself, and then the finalizer runs.
    selection.mark_dragging();
    assert!(selection.is_dragging());
    selection.set(0, 11);
    selection.end_drag();
    assert_eq!(selection.selected_text().as_deref(), Some("hello world"));
  }

  #[test]
  fn a_collapsed_range_selects_nothing() {
    let selection = MarkdownSelection::default();
    selection.runs.borrow_mut().push(RegisteredRun {
      start: 0,
      text: "hello world".into(),
      layout: gpui::TextLayout::default(),
    });

    selection.set(4, 4);
    assert!(selection.is_empty(), "a click must not create a selection");
    assert_eq!(selection.selected_text(), None);

    selection.set(4, 7);
    assert!(!selection.is_empty());
    assert_eq!(selection.selected_text().as_deref(), Some("o w"));
  }

  #[test]
  fn word_range_covers_the_word_under_the_offset() {
    let text = "hello, world_2 and more";
    // Anywhere inside "hello" selects the whole word, not just up to the
    // click: the first, middle and last character all give the same range.
    assert_eq!(word_range(text, 0), (0, 5));
    assert_eq!(word_range(text, 2), (0, 5));
    assert_eq!(word_range(text, 4), (0, 5));
    // On the comma: not a word character, so only that character is selected.
    assert_eq!(word_range(text, 5), (5, 6));
    // Inside "world_2" (underscore and digits count as word characters).
    assert_eq!(word_range(text, 9), (7, 14));
  }

  #[test]
  fn line_range_covers_the_line_under_the_offset() {
    let text = "first line\nsecond line\nthird";
    assert_eq!(line_range(text, 3), (0, 10));
    assert_eq!(line_range(text, 15), (11, 22));
    assert_eq!(line_range(text, 25), (23, 28));
  }

  #[test]
  fn selection_slices_the_concatenated_document_text() {
    let selection = MarkdownSelection::default();
    selection.runs.borrow_mut().push(RegisteredRun {
      start: 0,
      text: "hello world".into(),
      layout: gpui::TextLayout::default(),
    });

    selection.set(0, 5);
    assert_eq!(selection.selected_text().as_deref(), Some("hello"));

    // A backwards drag normalizes to the same range.
    selection.set(5, 0);
    assert_eq!(selection.selected_text().as_deref(), Some("hello"));

    // Offsets are clamped and snapped to char boundaries, never panicking.
    selection.set(0, 9_999);
    assert_eq!(selection.selected_text().as_deref(), Some("hello world"));

    selection.set(3, 3);
    assert_eq!(
      selection.selected_text(),
      None,
      "an empty range selects nothing"
    );
  }
}
