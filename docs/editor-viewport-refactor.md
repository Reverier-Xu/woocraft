# Editor Viewport Refactor

## Goal

Refactor `CodeEditor` to stop relying on GPUI's native scroll container and scrollbar path.
The new editor should:

- own its viewport state inside the editor itself;
- render only the visible logical rows plus a small overscan window;
- always use wrapped lines, removing horizontal scrolling and the no-wrap mode;
- manage mouse selection and wheel scrolling inside the editor viewport;
- draw its own vertical scrollbar;
- avoid paint-phase state write-back and smooth scrolling.

## Non-goals

- preserve the old no-wrap behavior;
- preserve pixel-perfect scroll semantics for very tall wrapped lines;
- keep the old `TextWrapper`-driven full-document layout pipeline.

## Current Problems

- The current editor writes layout and scroll state back during paint.
- The current editor depends on GPUI scroll state and native scrollbar widgets.
- Docked rendering amplifies the invalidation cost because the editor is wrapped by tab and dock containers.
- Full-document wrapping/layout work is performed even when only a small viewport region is visible.

## Target Model

The refactored editor keeps a small viewport cache:

- `top_row`: first logical row rendered in the viewport;
- `viewport_bounds`: measured viewport rectangle used for hit testing and scroll math;
- `viewport_rows`: visible logical row count derived from viewport height;
- `visible_rows`: `top_row..top_row + viewport_rows + overscan`;
- `visible_layout`: shaped lines and per-row geometry only for `visible_rows`;
- `cursor_bounds`: derived from the visible layout for caret/popover positioning.

Scrolling is line-based, not smooth. Long wrapped lines may occupy more than one visual row, but the viewport still advances in logical rows. The editor can over-render a few rows and let overflow clipping hide excess content.

## Refactor Steps

### Step 1 - Document the plan

- Add this document.
- Establish staged migration and verification rules.

### Step 2 - Add viewport primitives

- Introduce editor viewport state and vertical scrollbar math helpers.
- Keep behavior unchanged while preparing the new render path.
- Ensure the new primitives compile cleanly.

### Step 3 - Switch the editor to viewport-driven rendering

- Replace the current `TextElement` layout/paint pipeline with a viewport renderer.
- Remove the native `Scrollbar` integration from `CodeEditor`.
- Move wheel scrolling, mouse selection, and hit testing to the viewport model.
- Draw a custom vertical scrollbar inside the editor.

### Step 4 - Migrate geometry-dependent features

- Re-anchor caret, hover, diagnostic, completion, search, and IME bounds to the new viewport geometry.
- Replace old `last_layout` consumers with viewport-based geometry helpers.

### Step 5 - Remove obsolete no-wrap and full-layout code

- Drop no-wrap behavior and related horizontal scroll code.
- Remove or reduce `TextWrapper` and old scroll/layout state that is no longer used.
- Clean up dead code and warnings.

## Verification Rules

After each completed step:

1. format relevant Rust files;
2. run `cargo build` to validate compilation;
3. fix warnings introduced by the step;
4. create a focused gitmoji commit before starting the next step.

## Expected Outcome

After the refactor, the docked `CodeEditor` should behave like a self-contained viewport widget instead of a GPUI scroll container. That removes the most expensive invalidation paths while also reducing the amount of text work performed per frame.
