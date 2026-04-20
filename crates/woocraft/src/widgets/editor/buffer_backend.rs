use gpuim::SharedString;
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
  ) -> Vec<(std::ops::Range<u64>, gpuim::HighlightStyle)> {
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
  use super::*;

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
