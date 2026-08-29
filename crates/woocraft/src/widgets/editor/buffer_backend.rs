use gpui::SharedString;
use ropey::Rope;

use super::{
  EditorBackend, EditorBackendCapabilities, EditorBackendEditRequest, EditorBackendEditResult,
  EditorEditError, EditorHighlighter, EditorHighlighterProvider, EditorSnapshot, EditorTextChange,
  HighlightTheme, RopeEditorSnapshot, RopeExt as _, SyntaxHighlighter,
};

pub struct RopeBufferBackend {
  revision: u64,
  language: SharedString,
  text: Rope,
  read_only: bool,
}

impl RopeBufferBackend {
  pub fn new(text: impl AsRef<str>) -> Self {
    Self {
      revision: 0,
      language: SharedString::new_static("text"),
      text: Rope::from(text.as_ref()),
      read_only: false,
    }
  }

  pub fn with_language(mut self, language: impl Into<SharedString>) -> Self {
    self.language = language.into();
    self
  }

  pub fn with_read_only(mut self, read_only: bool) -> Self {
    self.read_only = read_only;
    self
  }

  pub fn set_language(&mut self, language: impl Into<SharedString>) {
    self.language = language.into();
    self.bump_revision();
  }

  pub fn set_read_only(&mut self, read_only: bool) {
    self.read_only = read_only;
  }

  pub fn text(&self) -> &Rope {
    &self.text
  }

  pub fn language(&self) -> &SharedString {
    &self.language
  }

  fn bump_revision(&mut self) {
    self.revision = self.revision.wrapping_add(1);
  }

  fn apply_edit_internal(
    &mut self, request: &EditorBackendEditRequest,
  ) -> Result<EditorBackendEditResult, EditorEditError> {
    if self.read_only {
      return Err(EditorEditError::Unsupported);
    }

    let start = (request.range.start as usize).min(self.text.len());
    let end = (request.range.end as usize).min(self.text.len());
    self.text.replace(start..end, &request.new_text);
    self.bump_revision();

    let cursor = (start + request.new_text.len()).min(self.text.len()) as u64;
    Ok(EditorBackendEditResult {
      accepted: true,
      selection: None,
      cursor: Some(cursor),
    })
  }
}

struct TreeSitterEditorHighlighter {
  inner: SyntaxHighlighter,
}

impl TreeSitterEditorHighlighter {
  fn new(language: &str) -> Self {
    Self {
      inner: SyntaxHighlighter::new(language),
    }
  }
}

impl EditorHighlighter for TreeSitterEditorHighlighter {
  fn sync(&mut self, snapshot: &dyn EditorSnapshot, change: Option<&EditorTextChange>) {
    let text = snapshot.rope();
    if let Some(change) = change {
      self.inner.update_text_change(
        change.range.start as usize..change.range.end as usize,
        change.new_text.as_ref(),
        &text,
      );
    } else {
      self.inner.update(None, &text);
    }
  }

  fn highlight_range(
    &self, _snapshot: &dyn EditorSnapshot, range: std::ops::Range<u64>, theme: &HighlightTheme,
  ) -> Vec<(std::ops::Range<u64>, gpui::HighlightStyle)> {
    self
      .inner
      .styles(&(range.start as usize..range.end as usize), theme)
      .into_iter()
      .map(|(range, style)| ((range.start as u64)..(range.end as u64), style))
      .collect()
  }
}

impl EditorHighlighterProvider for RopeBufferBackend {
  fn create_highlighter(&self) -> Option<Box<dyn EditorHighlighter>> {
    Some(Box::new(TreeSitterEditorHighlighter::new(
      self.language.as_ref(),
    )))
  }
}

impl super::EditorActionSink for RopeBufferBackend {}

impl super::EditorContextMenuProvider for RopeBufferBackend {}

impl EditorBackend for RopeBufferBackend {
  fn revision(&self) -> u64 {
    self.revision
  }

  fn capabilities(&self) -> EditorBackendCapabilities {
    EditorBackendCapabilities {
      editable: !self.read_only,
      custom_line_numbers: false,
      custom_highlighter: true,
    }
  }

  fn snapshot(&self) -> std::sync::Arc<dyn EditorSnapshot> {
    std::sync::Arc::new(RopeEditorSnapshot::new(self.revision, self.text.clone()))
  }

  fn apply_edit(
    &mut self, request: EditorBackendEditRequest,
  ) -> Result<EditorBackendEditResult, EditorEditError> {
    self.apply_edit_internal(&request)
  }
}

#[cfg(test)]
mod tests {
  use gpui::{AppContext as _, Entity, IntoElement, Render};

  use super::{super::state::InputState, *};

  /// Regression test for code editor IME composition: each preedit update
  /// must REPLACE the previous marked text (tracked by `ime_marked_range`),
  /// and the final commit must swap it for the composed text. The old
  /// behavior stored an empty marked range, so every keystroke appended
  /// another full copy of the pinyin and the final hanzi landed next to the
  /// never-removed pinyin fragments.
  #[gpui::test]
  fn ime_preedit_replaces_previous_marked_text(cx: &mut gpui::TestAppContext) {
    struct Host(Entity<InputState>);

    impl Render for Host {
      fn render(
        &mut self, _window: &mut gpui::Window, _cx: &mut gpui::Context<Self>,
      ) -> impl IntoElement {
        gpui::div()
      }
    }

    let window = cx.add_window(|window, cx| {
      let state = cx.new(|cx| {
        InputState::new(window, cx)
          .code_editor("rust")
          .backend(RopeBufferBackend::new(""))
      });
      Host(state)
    });

    window
      .update(cx, |host, window, cx| {
        use gpui::EntityInputHandler as _;

        host.0.update(cx, |state, cx| {
          // First composition keystroke inserts the pinyin fragment.
          state.replace_and_mark_text_in_range(None, "n", None, window, cx);
          assert_eq!(state.text.to_string(), "n");
          assert_eq!(
            state.ime_marked_range.map(|range| (range.start, range.end)),
            Some((0, 1))
          );

          // The next preedit update must replace the marked text, not append
          // another copy of it.
          state.replace_and_mark_text_in_range(None, "ni", None, window, cx);
          assert_eq!(state.text.to_string(), "ni");
          assert_eq!(
            state.ime_marked_range.map(|range| (range.start, range.end)),
            Some((0, 2))
          );

          // Committing swaps the whole marked range for the composed hanzi.
          state.replace_text_in_range(None, "你", window, cx);
          assert_eq!(state.text.to_string(), "你");
          assert!(state.ime_marked_range.is_none());
        });
      })
      .unwrap();
  }

  #[test]
  fn rope_buffer_backend_edits_and_updates_revision() {
    let mut backend = RopeBufferBackend::new("alpha\nbeta").with_language("rust");

    assert_eq!(EditorBackend::revision(&backend), 0);
    let result = EditorBackend::apply_edit(
      &mut backend,
      EditorBackendEditRequest {
        range: 0..5,
        new_text: "omega".to_string(),
        marked: false,
      },
    )
    .unwrap();

    assert!(result.accepted);
    assert_eq!(EditorBackend::revision(&backend), 1);
    assert_eq!(backend.text().to_string(), "omega\nbeta");
    assert!(backend.create_highlighter().is_some());
  }

  #[test]
  fn rope_buffer_backend_can_be_read_only() {
    let mut backend = RopeBufferBackend::new("alpha").with_read_only(true);

    let result = EditorBackend::apply_edit(
      &mut backend,
      EditorBackendEditRequest {
        range: 0..5,
        new_text: "beta".to_string(),
        marked: false,
      },
    );

    assert!(matches!(result, Err(EditorEditError::Unsupported)));
    assert_eq!(backend.text().to_string(), "alpha");
    assert!(!backend.capabilities().editable);
  }
}
