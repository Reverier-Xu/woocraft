# Editor Backend Migration

## Current Architecture

The editor now uses a backend-driven document pipeline.

- `EditorBackend` owns document access, edit application, capabilities, and
  optional highlighter provisioning.
- `EditorSnapshot` is the immutable read surface consumed by the editor for
  line access, byte ranges, and position/offset conversion.
- `EditorHighlighter` is provided by the backend and queried by the viewport
  renderer.
- `RopeBufferBackend` is the built-in buffered implementation for code editor
  mode. It owns the rope document and vends the tree-sitter highlighter.

## Behavioral Changes

- `EditorState::code_editor(...)` now installs a built-in `RopeBufferBackend`
  lazily when the editor first renders.
- Syntax highlighting is no longer stored in `InputMode`; it comes from the
  backend/provider layer.
- The editor no longer exposes the legacy custom data backend bridge.
- No-wrap compatibility shims are gone; the editor always uses wrapped layout.

## Removed APIs

- `EditorDataBackend`
- `LegacyEditorDataBackendAdapter`
- `EditorState::data_backend(...)`
- `EditorState::set_data_backend(...)`
- `EditorState::clear_data_backend(...)`
- `EditorState::has_custom_data_backend()`
- `EditorState::set_highlighter(...)`
- `EditorState::soft_wrap(...)`
- `EditorState::set_soft_wrap(...)`

## Follow-up Refactors

The editor still mirrors backend snapshots into local rope state for geometry,
selection, search, and LSP integration. Future work should continue shrinking
`InputState` by moving more logic onto backend snapshots and dedicated overlay
services.
