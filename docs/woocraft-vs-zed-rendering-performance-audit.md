# Woocraft vs. Zed Rendering Performance Audit

## Context

This document compares `woocraft` with `../zed` from the perspective of rendering pipeline design, hot-path invalidation, and interaction smoothness.

The immediate constraint is important:

- `cached()` is currently unsafe for Woocraft dock/tab-panel content because of the known GPUI cached-view + input crash class.
- Relevant references:
  - `https://github.com/Reverier-Xu/regressor-rs/issues/7`
  - `https://github.com/longbridge/gpui-component/issues/2035`
  - `https://github.com/zed-industries/zed/issues/50456`

The important conclusion is that Zed is not magically immune to the upstream GPUI bug. In fact, `zed` itself tracks the same crash class in issue `#50456`. Zed still feels fast because it does **not** depend on `cached()` as the primary performance mechanism for its hottest widgets.

## Short Version

- Zed uses `cached()` only at coarse, stable boundaries such as panes and dock panels, while its hottest widgets already minimize work internally.
- Woocraft's editor is already better than a typical tree-of-divs widget because it uses a custom `ViewportElement`, but it still performs too much synchronization and shaping work during `render`/`prepaint`, especially during width changes.
- Woocraft's weakest area is the dock/tab/drag pipeline: drag state, split-preview state, active-tab state, and some resize state all live in the same views that also host heavy child content, so `cx.notify()` frequently rebuilds more than necessary.
- Woocraft tables virtualize well vertically, but horizontal virtualization is repeated per visible row, and width changes are committed on every drag tick.
- If `cached()` cannot be used yet, the right strategy is not to wait for the upstream fix. The right strategy is to reduce invalidation scope, move transient interaction state out of heavy panel views, and separate preview state from committed layout state.

## Implementation Progress

### Completed work

- Phase 1 invalidation cleanup is now implemented.
- Phase 2 interaction-isolation work is now partially implemented for dock split previews and tiles.
- Phase 3 editor hot-path cleanup is now partially implemented.
- Phase 4 table and list interaction work is now partially implemented.
- `TabPanel::set_active_ix` no longer emits `LayoutChanged` for pure tab activation.
- `TabPanel::on_panel_drag_move` now notifies only when the derived split placement actually changes.
- `TabPanel::on_drop` no longer emits a redundant trailing `LayoutChanged` after add/insert/split helpers already emitted their structural event.
- `DockArea` now clears and rebuilds its panel subscriptions on `load(...)`, and deduplicates panel and tile drop subscriptions to avoid repeated listeners.
- `DockItem::split_with_sizes` no longer walks the same items twice during split construction.
- Split-preview rendering now lives at the `DockArea` overlay level instead of inside the active tab content subtree.
- `TabPanel` now updates split-preview state through `DockArea`, which keeps drag-hover redraws focused on the overlay layer.
- `Tiles` now swap the actively dragged or resized tile body for a lightweight placeholder frame instead of rerendering live panel content on every pointer move.
- The editor no longer carries the old `_pending_update` render-time refresh path for code-editor state.
- Hover handling no longer triggers an unconditional repaint on every mouse move; it now repaints only when hover definition or hover popover state actually changes.
- Line-number width and whitespace indicator shaping now use persistent caches instead of rebuilding those glyph metrics on every editor prepaint.
- Table column resizing now uses preview widths during drag and only commits the real widths when the resize interaction completes.
- The custom `VirtualList` now derives visible ranges with binary-search-style origin lookups instead of repeated linear scans through every item size.
- Dock resize interactions now keep a preview size during drag and only commit the real dock size when the resize gesture ends.
- Dock resize handles now follow the preview size at the `DockArea` overlay level, which avoids live layout churn in sibling content while the pointer is moving.
- Tiles now keep panel z-order normalized at mutation time, removing the old per-frame clone-and-sort render path.
- Search highlight overlay layout now filters directly to the visible match slice instead of scanning every match on every prepaint.

### Future opportunities

- Reduce resize-time editor work further, especially by replacing full-document wrap updates with more viewport-oriented behavior.
- Continue separating editor synchronization from paint-heavy paths, especially backend and wrap work tied to width changes.
- Revisit wide-table horizontal work to reduce repeated per-row horizontal virtualization overhead.
- Consider adding a more incremental wrap model so width changes do not require full-document rewraps even outside dock-preview mode.

### Verification

- `cargo +nightly fmt` passes after the refactor series.
- `cargo clippy` passes after the refactor series.

## What Zed Does Differently

### 1. Cache boundaries are coarse, not everywhere

Zed uses `AnyView::cached(...)` at stable outer boundaries:

- `crates/workspace/src/pane_group.rs` in `Member::render`
- `crates/workspace/src/dock.rs` in `Dock::render`
- `crates/gpui/src/view.rs` in `AnyView::cached`

This matters because the cached boundary is usually a pane or an active dock panel, not a small fragile widget. Zed does not try to solve every hot path by sprinkling caches into inner components.

### 2. Hot widgets are already custom-rendered

Zed's editor is a custom `Element`:

- `crates/editor/src/editor.rs` in `impl Render for Editor`
- `crates/editor/src/element.rs` in `impl Element for EditorElement`

The editor computes visible rows and paints them directly. It also respects parent clipping via `window.content_mask().bounds`, which is especially important for auto-height editors embedded in lists.

Relevant paths:

- `crates/editor/src/element.rs` in `EditorElement::prepaint`
- `crates/editor/src/element.rs` in `prepaint_lines`
- `crates/editor/src/element.rs` in `paint_lines`

This means a pane can rerender without the editor behaving like a normal deep element tree.

### 3. Interaction state is kept local whenever possible

Zed relies heavily on element-local state and stateful interactive elements:

- `crates/gpui/src/window.rs` in `with_element_state` and `with_optional_element_state`
- `crates/ui/src/components/tab.rs` where `Tab` implements `StatefulInteractiveElement`
- `crates/workspace/src/pane_group.rs` where split-handle drag state is stored with `with_element_state`

This is one of the biggest differences from Woocraft. Hover/drag/pressed state often does not live in the owning pane model, so those micro-interactions do not require broad view invalidation.

### 4. Drag and resize paths avoid redundant updates

Zed avoids notifying on every identical mouse-move result:

- `crates/workspace/src/pane.rs` in `handle_drag_move` updates split direction only when it actually changes
- `crates/workspace/src/workspace.rs` in `on_drag_move::<DraggedDock>` ignores duplicate pointer coordinates

Zed also uses deferred/overlay rendering for some interaction layers:

- `crates/workspace/src/pane.rs` in `render_menu_overlay`
- `crates/workspace/src/dock.rs` where resize handles are rendered as deferred overlays

This keeps layout and content trees more stable during interaction.

### 5. Zed's "input field" story is actually editor-based

Zed's reusable input field wraps a single-line editor rather than a separate generic input implementation:

- `crates/ui_input/src/input_field.rs`
- `crates/editor/src/editor.rs` where `ERASED_EDITOR_FACTORY` is set to `Editor::single_line(...)`

Operationally, this reduces the amount of custom input-stack behavior hiding under cached panel boundaries.

That does **not** mean the upstream GPUI cached+input issue is fixed. It means Zed's normal architecture leans less on the exact problematic combination.

## What Woocraft Already Gets Right

This audit should not ignore the strengths that already exist.

### Editor strengths

Woocraft's editor is already using a custom element and viewport culling rather than building one view per line:

- `crates/woocraft/src/widgets/editor/state.rs`
- `crates/woocraft/src/widgets/editor/viewport_element.rs`

That is the correct direction.

### Table strengths

Woocraft tables already virtualize rows and non-fixed columns:

- vertical virtualization via `uniform_list` in `crates/woocraft/src/widgets/table/state.rs`
- horizontal virtualization via `h_virtual_list` in `crates/woocraft/src/widgets/table/state.rs`

This is better than a naive full-table render.

### Dock strengths

Woocraft already renders dock resize handles at the `DockArea` level rather than burying them inside each dock subtree:

- `crates/woocraft/src/widgets/dock/mod.rs`

That same principle should be extended to tab split overlays and tile drag/resize overlays.

## Why Woocraft Still Feels Slower

### 1. Dock/tab invalidation scope is too large

This is the single biggest architectural weakness.

### Active-tab changes are treated like layout changes

`TabPanel::set_active_ix` emits `PanelEvent::LayoutChanged` even when the user only switches tabs:

- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `set_active_ix`

That event fans out into dock-area bookkeeping:

- `crates/woocraft/src/widgets/dock/mod.rs` in `subscribe_panel`

So a simple tab switch can trigger outer layout-related work even though the structure did not change.

### Drag preview state lives in the heavy view

`TabPanel::on_panel_drag_move` updates `will_split_placement` and always calls `cx.notify()`:

- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `on_panel_drag_move`

Unlike Zed, it does not first check whether the derived placement actually changed.

That means every drag move can invalidate the same `TabPanel` that also owns:

- the title bar
- the toolbar
- the active panel host
- the split/drop overlay

Because `.cached()` is currently unsafe here, the cost of these invalidations is paid directly.

### Add/insert paths notify more than once

`add_panel_with_active` and `insert_panel_at` call `set_active_ix`, which already emits and notifies, and then emit/notify again:

- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `add_panel_with_active`
- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `insert_panel_at`

This is unnecessary event churn during already-expensive structure changes.

### Split/stack model duplication increases work

Woocraft stores structure in both:

- `DockItem::{Split, Tabs, Tiles}` in `crates/woocraft/src/widgets/dock/mod.rs`
- live `TabPanel` state in `crates/woocraft/src/widgets/dock/tab_panel.rs`
- live `StackPanel` state in `crates/woocraft/src/widgets/dock/stack_panel.rs`

That duplication makes it easier to generate broad "layout changed" events and harder to keep invalidation tightly scoped.

### Subscription lifetime likely grows over time

`DockArea` stores subscriptions in `_subscriptions` and only pushes more:

- `crates/woocraft/src/widgets/dock/mod.rs` in `subscribe_panel`
- `crates/woocraft/src/widgets/dock/mod.rs` in `subscribe_tiles_item_drop`

`load(...)` replaces dock trees, but there is no visible clearing/deduplication step for old panel subscriptions before new ones are added.

At minimum, this deserves an audit. In long-lived sessions with many panel moves/splits/reloads, stale subscriptions can become both a correctness risk and a performance risk.

### There is also an obvious redundant split-construction loop

`DockItem::split_with_sizes` iterates the items twice and tries to add them both times:

- `crates/woocraft/src/widgets/dock/mod.rs` in `split_with_sizes`

`StackPanel::insert_panel` prevents duplicate children, so this is not catastrophic, but it is still wasted construction-time work and a sign that this path needs cleanup.

### 2. Editor work is still too "active" during render and resize

Woocraft's editor has the right broad shape, but too much heavy work still happens on the render path.

### Render still performs synchronization work

`InputState::render` does all of the following:

- installs the backend if needed
- syncs text from backend
- syncs wrap metrics
- refreshes the highlighter
- sometimes runs LSP update work

Relevant file:

- `crates/woocraft/src/widgets/editor/state.rs` in `impl Render for InputState`

This is a major difference from Zed, where the editor render path is much more focused on layout/paint over a prepared snapshot.

### Width changes can trigger full-document rewraps

This is one of the most serious practical problems in docked editors.

`ViewportElement::prepaint` calls `state.sync_wrap_metrics_for_view(...)`:

- `crates/woocraft/src/widgets/editor/viewport_element.rs` in `prepaint`

That eventually goes through `TextWrapper::sync(...)`, which calls `update_all(...)` whenever wrap width changes:

- `crates/woocraft/src/widgets/editor/text_wrapper.rs` in `sync`
- `crates/woocraft/src/widgets/editor/text_wrapper.rs` in `update_all`

So during dock resize or any panel-width drag, the editor can rewrap the whole document repeatedly.

This is exactly the kind of user-visible jank that makes dock resizing feel much worse than in Zed.

### Paint still writes state back into the model

`ViewportElement::paint` writes the following back into `InputState`:

- `last_layout`
- `last_bounds`
- `last_cursor`
- `input_bounds`
- `top_row`

Relevant file:

- `crates/woocraft/src/widgets/editor/viewport_element.rs` in `paint`

This keeps geometry-dependent features working, but it also means render-time geometry and model state are still tightly coupled.

### Visible lines are still reshaped every prepaint

`ViewportElement::layout_lines` shapes visible wrapped sub-lines every prepaint:

- `crates/woocraft/src/widgets/editor/viewport_element.rs` in `layout_lines`

That is acceptable for a small viewport, but it becomes much more expensive when combined with:

- repeated width changes
- repeated highlighter recomputation
- repeated search/selection/indent overlay layout

### Search is particularly expensive on large files

Woocraft search currently has two expensive stages:

1. `SearchMatcher::update_matches()` converts the full rope to a `String` and finds all matches.
2. `ViewportElement::layout_search_matches()` iterates all stored ranges again and computes visible paths.

Relevant files:

- `crates/woocraft/src/widgets/editor/search.rs`
- `crates/woocraft/src/widgets/editor/viewport_element.rs` in `layout_search_matches`

This is correct but not scalable.

### Mouse-move invalidation is too broad

The hover pipeline is noisy:

- `crates/woocraft/src/widgets/editor/state.rs` in `on_mouse_move`
- `crates/woocraft/src/widgets/editor/lsp/mod.rs` in `handle_mouse_move`
- `crates/woocraft/src/widgets/editor/lsp/hover.rs`
- `crates/woocraft/src/widgets/editor/lsp/definitions.rs`

`handle_mouse_move` always ends with `cx.notify()`, and hover-definition / hover-popover work can be retriggered frequently during pointer motion.

Zed is much more careful about keeping pointer-derived state local and only changing persistent state when the semantic result actually changes.

### 3. Table virtualization is good, but shared-scroll work is repeated too often

Woocraft tables are not naive, but they still do more work than necessary on horizontal interaction.

### Horizontal virtualization is computed separately for every visible row

Each visible row renders its own `h_virtual_list(...)`:

- `crates/woocraft/src/widgets/table/state.rs` in `render_table_row`

This means horizontal scroll does not compute a single shared visible-column range. Instead, every visible row recomputes its own visible subrange.

This is a defensible tradeoff for extremely wide tables, but it becomes expensive when:

- row count is large
- visible rows are numerous
- `render_td` is non-trivial
- column resize/scroll happens continuously

### The custom virtual list still finds visible ranges with linear scans

Woocraft's custom `VirtualList` finds first/last visible elements by walking cumulative sizes linearly:

- `crates/woocraft/src/widgets/virtual_list.rs`

That is fine for small lists, but it is weaker than a prefix-sum + binary-search design.

### Column resize commits real widths on every drag tick

Woocraft writes real widths during drag:

- `crates/woocraft/src/widgets/table/state.rs` in `render_resize_handle`
- `crates/woocraft/src/widgets/table/state.rs` in the resize path

Zed keeps preview widths separate from committed widths in its data-table interaction path:

- `crates/ui/src/components/data_table.rs` in `TableColumnWidths`
- `crates/ui/src/components/data_table.rs` in `render_resize_handles`

That allows the interaction layer to stay cheap while the final model is only committed on drop.

### Fixed-left columns are always live

Woocraft does not horizontally virtualize fixed-left columns, which is correct functionally, but it means their cost scales with:

- visible rows x fixed columns

If fixed columns host heavy cells, this becomes noticeable.

### 4. Tiles/floating panels are especially expensive during drag and resize

`Tiles` is one of the clearest hot spots in Woocraft.

### Sorting and cloning happen every render

- `crates/woocraft/src/widgets/dock/tiles.rs` in `sorted_panels`

The entire tile list is cloned and sorted on every render even though z-order only changes on specific actions.

### Dragging/resizing invalidates the whole tiles view

- `crates/woocraft/src/widgets/dock/tiles.rs` in `update_position`
- `crates/woocraft/src/widgets/dock/tiles.rs` in `resize`

Both paths call `cx.notify()` directly while moving live panels.

### Live panel content is rerendered while the frame moves

`render_panel(...)` renders the actual child panel view inside the moving/resizing frame:

- `crates/woocraft/src/widgets/dock/tiles.rs` in `render_panel`

So a drag or resize can repeatedly rerender not only the frame chrome, but also the heavy content inside the tile.

This is the floating-panel equivalent of the dock/tab problem.

## The Most Important Insight

Zed does not win because it found a secret version of `cached()`.

Zed wins because it combines several architectural choices:

- cache only stable outer boundaries
- custom-render the hottest widgets
- keep transient interaction state local
- avoid semantic no-op invalidations
- use overlays/deferred elements for drag/resize affordances
- separate preview state from committed model state

Woocraft currently loses mostly because it does the opposite in several places, especially around dock/tab/drag/resize.

## Recommended Plan Without Relying on `cached()` Yet

### Phase 0: Measure the right things first

Before changing behavior, add targeted counters and traces for these paths:

- `TabPanel::set_active_ix`
- `TabPanel::on_panel_drag_move`
- `Dock::resize`
- `Tiles::update_position`
- `Tiles::resize`
- `TextWrapper::update_all`
- `ViewportElement::layout_lines`
- `ViewportElement::layout_search_matches`
- `TableState::render_table_row`
- `VirtualList` visible-range calculation

The goal is to count:

- calls per second during drag/resize
- number of shaped lines per frame
- number of full-document rewraps during one dock resize
- number of `LayoutChanged` events per tab switch / drag operation

### Phase 1: Shrink invalidation domains immediately

These are high-ROI changes and do not require upstream GPUI fixes.

### A. Stop treating active-tab switches as layout changes

Replace broad `LayoutChanged` emissions with a cheaper event for purely visual state changes.

At minimum:

- tab activation should not trigger the same path as structural split/close/move
- dock toggle-button bookkeeping should run only for true structural changes

Primary targets:

- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `set_active_ix`
- `crates/woocraft/src/widgets/dock/mod.rs` in `subscribe_panel`

### B. Notify only when derived drag state actually changes

Copy the Zed pattern for split-direction changes.

In Woocraft, `on_panel_drag_move` should update and notify only when:

- `will_split_placement` changed from old value to new value

Primary target:

- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `on_panel_drag_move`

### C. Deduplicate add/insert notifications

Avoid emitting/notifying after a helper already emitted/notified for the same logical state transition.

Primary targets:

- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `add_panel_with_active`
- `crates/woocraft/src/widgets/dock/tab_panel.rs` in `insert_panel_at`

### D. Audit and clean dock subscription lifetime

Ensure subscriptions are:

- cleared on full reload
- removed on permanent panel detach
- deduplicated when a panel is reattached

Primary target:

- `crates/woocraft/src/widgets/dock/mod.rs`

### Phase 2: Move drag/resize preview state into lightweight overlays

This is the most important structural improvement if `cached()` remains unavailable.

### A. Tab split preview should not live inside the active panel subtree

Today the split overlay is rendered from `TabPanel::render_active_panel(...)`.

Instead:

- store split-preview target state at `DockArea` level or in a dedicated overlay entity
- render the highlight overlay as a sibling overlay layer, not as a child of the active content host

Benefits:

- drag-hover invalidation hits only the overlay layer
- the active editor/table/list subtree stays stable while dragging

Primary targets:

- `crates/woocraft/src/widgets/dock/tab_panel.rs`
- `crates/woocraft/src/widgets/dock/mod.rs`

### B. Dock resize should use preview-vs-commit behavior

Right now dock size changes immediately on every drag tick.

For Woocraft this is especially painful because width changes can trigger full editor rewraps.

Recommended behavior:

- keep handle feedback fully live at 60 Hz
- update cheap preview geometry every move
- commit heavy content width/layout at a throttled cadence or on mouse-up

Two workable variants:

1. commit only when width change crosses a text-cell threshold
2. commit at a fixed throttled cadence such as 16-33 ms, with final exact commit on drop

Primary targets:

- `crates/woocraft/src/widgets/dock/dock.rs`
- `crates/woocraft/src/widgets/editor/text_wrapper.rs`

### C. Tiles should drag a lightweight frame, not a fully live content subtree

During tile move/resize, prefer one of these:

- a lightweight drag proxy / ghost frame
- a stable content host plus separate moving chrome/overlay layer
- throttled content relayout with immediate chrome feedback

Primary target:

- `crates/woocraft/src/widgets/dock/tiles.rs`

### Phase 3: Make the editor cheap under dock resize

This is the biggest user-visible win after dock invalidation cleanup.

### A. Move backend/highlighter/LSP synchronization out of `Render`

Render should consume prepared state, not perform synchronization.

Target:

- `crates/woocraft/src/widgets/editor/state.rs`

Recommended split:

- edit operations update backend/text wrapper/highlighter explicitly
- async LSP/document-color tasks update dedicated caches
- render only reads current snapshot/caches

### B. Stop full-document wrap recomputation on every width tick

The current `TextWrapper` model is the root problem during dock resize.

Recommended options, in order:

1. best: move toward true viewport-oriented wrapping so only visible rows are wrapped
2. acceptable: cache wrapped results per `(revision, wrap_width_bucket)` and only recompute changed lines
3. minimum viable: throttle width-driven `update_all(...)` and coalesce repeated resize events

Primary targets:

- `crates/woocraft/src/widgets/editor/text_wrapper.rs`
- `crates/woocraft/src/widgets/editor/viewport_element.rs`

### C. Cache repeated metrics that are currently recomputed every prepaint

Candidates:

- line-number width
- shaped line numbers
- whitespace indicator glyphs
- indent width for current font/tab size
- shaped visible lines keyed by `(revision, row, wrap range, style, width)`

Primary target:

- `crates/woocraft/src/widgets/editor/viewport_element.rs`

### D. Reduce pointer-driven editor invalidation

Recommended rules:

- only recompute hover state when hovered symbol/offset changed
- debounce LSP hover requests by semantic target, not just time
- avoid unconditional `cx.notify()` in `handle_mouse_move`

Primary targets:

- `crates/woocraft/src/widgets/editor/lsp/mod.rs`
- `crates/woocraft/src/widgets/editor/lsp/hover.rs`
- `crates/woocraft/src/widgets/editor/lsp/definitions.rs`

### E. Make search viewport-aware

Recommended changes:

- avoid `self.text.to_string()` for every matcher refresh on large documents
- keep an index or interval structure for match ranges
- only compute path geometry for visible matches
- avoid iterating every match during every prepaint

Primary targets:

- `crates/woocraft/src/widgets/editor/search.rs`
- `crates/woocraft/src/widgets/editor/viewport_element.rs`

### Phase 4: Simplify table interaction costs

### A. Separate preview widths from committed widths

Copy the Zed table interaction approach.

During drag:

- update `preview_widths`
- paint using preview widths
- commit final widths on drop

This keeps model churn and downstream relayout cheaper.

Primary target:

- `crates/woocraft/src/widgets/table/state.rs`

### B. Share horizontal visible range across rows

If wide tables remain a major use case, avoid computing a separate horizontal visible range inside every visible row.

Prefer one of these:

- compute one shared horizontal visible-column range at the table body level
- or move large-table rendering toward a paint-based row/cell renderer

Primary target:

- `crates/woocraft/src/widgets/table/state.rs`

### C. Upgrade the custom virtual list

Replace the current linear visible-range walk with prefix sums plus binary search.

Primary target:

- `crates/woocraft/src/widgets/virtual_list.rs`

### Phase 5: Make the dock tree simpler and more stable

### A. Use one source of truth for panel structure

The dock layout should have one live model. Serialization can be derived from that model, but runtime updates should not bounce between parallel structures.

Primary targets:

- `crates/woocraft/src/widgets/dock/mod.rs`
- `crates/woocraft/src/widgets/dock/stack_panel.rs`
- `crates/woocraft/src/widgets/dock/tab_panel.rs`

### B. Extend the root-overlay pattern already used for dock resize handles

Woocraft already uses root-level overlays for dock resize handles. Extend that pattern to:

- tab split previews
- tile drag outlines
- tile resize handles
- possibly active drop indicators for table column moves

This is the most practical way to recover Zed-like interaction smoothness before `cached()` becomes reliable again.

## What Should Wait For the Upstream GPUI Fix

Once the cached-view + input crash class is fixed upstream, Woocraft should absolutely add cache boundaries back at stable hosts.

But it should do so **after** the invalidation cleanup above, not instead of it.

Safe future candidates:

- active panel host inside tab panels
- floating tile content hosts
- stable dock panel bodies

Unsafe strategy:

- blindly enabling `.cached()` on the current dock/tab pipeline and hoping it hides architectural invalidation problems

That would make the design fragile even if the upstream crash is fixed.

## Priority Order

If the goal is maximum real-world improvement with the current GPUI limitations, the recommended order is:

1. stop broad non-structural `LayoutChanged` emissions in dock/tab paths
2. notify only when drag/resize derived state actually changes
3. move split-preview / drag-preview rendering into root overlays
4. throttle or stage dock resize so editors do not full-rewrap every pointer tick
5. move editor sync/highlighter/LSP work out of `Render`
6. introduce preview-vs-commit widths for table resizing
7. reduce tiles live-content rerenders during drag/resize
8. clean subscription lifetime and collapse dock structure to one source of truth
9. only then re-evaluate `cached()` after the upstream fix lands

## Final Judgment

Woocraft's biggest performance problem is not that it currently cannot use `.cached()` on dock and tab-panel content.

The biggest problem is that its dock/tab/drag architecture currently **needs** `.cached()` more than Zed does.

Zed feels fast because its hot components are already designed to survive frequent interaction without broad rerender cost. Woocraft should follow that model first, then layer cache boundaries back in once the GPUI crash is fixed.
