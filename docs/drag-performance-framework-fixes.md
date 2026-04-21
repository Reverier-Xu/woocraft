# Drag Performance: Framework-Level Fixes (gpuim / woocraft)

本文档记录了导致桌面端拖动性能问题的框架级根因及修复方案。需要在上游 `gpuim` 和 `woocraft` 仓库中实施。

## 问题概述

| 问题 | 表现 | 根因层级 |
|------|------|----------|
| Dock tab 拖放 | 分割预览时数十秒卡顿 | gpuim + woocraft |
| Dock resize | 低但可感知的延迟 | 应用层 (另见 `resize-performance.md`) |
| Hex viewer 拖拽选择 | ~500ms 延迟 | 应用层 (另见 `hex-viewer-drag-performance.md`) |
| Table 横向滚动 | 未实现 | 应用层 (另见 `hex-viewer-drag-performance.md`) |

---

## Fix 1: gpuim — 拖动期间刷新节流

**文件:** `crates/gpuim/src/window.rs`  
**位置:** `dispatch_mouse_event` 方法末尾 (约 4441-4452 行)

**问题代码:**
```rust
if cx.has_active_drag() {
    if event.is::<MouseMoveEvent>() {
        self.refresh();        // 每个 MouseMoveEvent 都触发全量刷新
    } else if event.is::<MouseUpEvent>() {
        cx.active_drag = None;
        self.refresh();
    }
}
```

**问题分析:** 鼠标在拖动期间以 60-120Hz 的频率产生 `MouseMoveEvent`。每次事件都调用 `self.refresh()` 设置 dirty 标志，但框架的 `on_request_frame` 回调受限于显示器刷新率（通常 60Hz）。这意味着在两次实际绘制之间可能累积多次 refresh 调用。虽然 `WindowInvalidator` 的 dirty 标志会合并，但 `dispatch_mouse_event` 中的 hit-test 和所有 mouse listener 回调仍然会在每个事件上执行，造成不必要的 CPU 消耗。

**修复方案:**

```rust
if cx.has_active_drag() {
    if event.is::<MouseMoveEvent>() {
        // 仅在 dirty 标志已被清除（上一帧已绘制完成）时才请求刷新
        // 避免在同一个绘制周期内重复设置 dirty
        if !self.invalidator.is_dirty() {
            self.refresh();
        }
    } else if event.is::<MouseUpEvent>() {
        cx.active_drag = None;
        self.refresh();
    }
}
```

---

## Fix 2: gpuim — 缓存拖动元素布局

**文件:** `crates/gpuim/src/window.rs`  
**位置:** `draw_roots` 方法 (约 2567-2572 行)

**问题代码:**
```rust
} else if let Some(active_drag) = cx.active_drag.take() {
    let mut element = active_drag.view.clone().into_any();
    let offset = self.mouse_position() - active_drag.cursor_offset;
    element.prepaint_as_root(offset, AvailableSpace::min_size(), self, cx);
    active_drag_element = Some(element);
    cx.active_drag = Some(active_drag);
}
```

**问题分析:** 每一帧都执行 `prepaint_as_root`，其中包括 `layout_as_root`（完整的 taffy flexbox 布局计算）。拖动预览元素的布局通常是固定的（取决于其内容，而非位置），但位置参数 `offset` 每帧都变化。layout 的结果不依赖于 offset（offset 只在后续的 `with_absolute_element_offset` 中使用），因此不需要每帧重新计算布局。

**修复方案:**

在 `Window` 结构体中添加缓存字段：
```rust
// 新增字段
cached_drag_layout: Option<(TypeId, AnyElement)>,
cached_drag_value_type: Option<TypeId>,
```

修改 `draw_roots` 中的拖动元素处理：
```rust
} else if let Some(active_drag) = cx.active_drag.take() {
    let drag_type_id = active_drag.value.as_ref().type_id();
    let offset = self.mouse_position() - active_drag.cursor_offset;

    let mut element = if self.cached_drag_value_type == Some(drag_type_id) {
        // 复用缓存的布局
        self.cached_drag_layout.take().unwrap()
    } else {
        // 首次或类型变更：执行完整布局
        let mut element = active_drag.view.clone().into_any();
        element.layout_as_root(AvailableSpace::min_size().into(), self, cx);
        element.prepaint(self, cx);
        self.cached_drag_layout = Some(element.clone());
        self.cached_drag_value_type = Some(drag_type_id);
        element
    };

    // 仅更新绘制位置，不重新布局
    element.set_origin(offset);
    element.paint(self, cx);
    active_drag_element = Some(element);
    cx.active_drag = Some(active_drag);
}
```

当 `drag_value` 发生变化时（`on_drag` 回调产生新值），清除缓存。

---

## Fix 3: gpuim — 批量化 `drag_over` hover 检测

**文件:** `crates/gpuim/src/elements/div.rs`  
**位置:** `compute_style_internal` (约 2838-2865 行) 及 mouse listener 注册 (约 2307-2332 行)

**问题代码 (style computation):**
```rust
if let Some(hitbox) = hitbox
    && let Some(drag) = cx.active_drag.take()
{
    // ...
    for (state_type, build_drag_over_style) in &self.drag_over_styles {
        if *state_type == drag.value.as_ref().type_id()
            && hitbox.is_hovered(window)    // 每个有 drag_over 的元素都调用
        {
            style.refine(&build_drag_over_style(drag.value.as_ref(), window, cx));
        }
    }
    // ...
}
```

**问题代码 (mouse listener 注册):**
```rust
if self.hover_style.is_some()
    || self.base_style.mouse_cursor.is_some()
    || cx.active_drag.is_some() && !self.drag_over_styles.is_empty()
{
    window.on_mouse_event(move |_: &MouseMoveEvent, phase, window, cx| {
        let hovered = hitbox.is_hovered(window);  // 每个有 drag_over 的元素每帧调用
        // ...
        cx.notify(current_view);
    });
}
```

**问题分析:** 在 DockArea 场景中，每个 TabPanel 的每个 tab 都注册了 `drag_over` 样式和对应的 mouse listener。对于一个 3 面板 × 4 tab 的布局，这意味着一帧内有约 15 个元素执行 `hitbox.is_hovered(window)` 检测。`is_hovered` 方法需要在 `window.mouse_hit_test.ids` (FxHashSet) 中查找，虽然是 O(1) 但乘以元素数量后仍不可忽视。此外，mouse listener 在每次 mouse move 时都会触发，即使 hover 状态没有变化也会调用 `cx.notify()`。

**修复方案:**

在 `Window` 或 `App` 中维护一个全局的拖动悬停跟踪器：
```rust
// Window 新增字段
drag_hover_tracker: DragHoverTracker,

struct DragHoverTracker {
    last_hovered_hitboxes: FxHashSet<HitboxId>,
    current_hovered_hitboxes: FxHashSet<HitboxId>,
}
```

1. **在 `draw_roots` 的 hit-test 阶段**，一次性计算所有被悬停的 hitbox ID，存入 `drag_hover_tracker.current_hovered_hitboxes`。

2. **在 style computation 中**，用 `drag_hover_tracker.is_hovered(hitbox)` 替代 `hitbox.is_hovered(window)`，改为直接查询预计算的集合。

3. **移除 per-element 的 drag-over mouse listener**。改为在 `dispatch_mouse_event` 中统一处理：mouse move 后比较 `last_hovered_hitboxes` 和 `current_hovered_hitboxes`，仅对状态变化的元素调用 `cx.notify()`。

4. **样式变更触发**：当 `drag_over` 样式需要变化时（hover 状态改变），通过 `drag_hover_tracker` 发出通知，由框架统一调度受影响的元素重绘。

---

## Fix 4: woocraft — 容器级 `drag_over` 替代逐 tab 注册

**文件:** `crates/woocraft/src/widgets/dock/tab_panel.rs`  
**位置:** `render_title_bar` (约 1058-1123 行) 和 `render_vertical_tab_bar` (约 1205-1277 行)

**问题代码 (水平多 tab 模式):**
```rust
for (ix, panel) in self.panels.iter().enumerate().filter_map(...) {
    // ...
    tab.child(
        div()
            // ...
            .when(droppable, |this| {
                this.drag_over::<DragPanel>(|style, _, _, _| style.bg(...))
                    .on_drop(cx.listener(move |view, drag, window, cx| {
                        view.on_drop(drag, Some(ix), false, window, cx);
                    }))
            })
    )
}
// 空余空间
let last_empty_space = div().when(droppable, |this| {
    this.drag_over::<DragPanel>(...)
        .on_drop(...)
});
```

**问题分析:** 每个 tab 和空余空间都独立注册了 `drag_over::<DragPanel>` 和 `on_drop`。这导致：
- N+1 个 `drag_over_styles` 条目需要检查
- N+1 个 mouse move listener 注册
- N+1 次 `is_hovered` 检测每帧
- N+1 个 `on_drop` 回调闭包分配

**修复方案:**

将 `drag_over` 和 `on_drop` 从单个 tab 提升到 tab bar 容器级别，在容器上使用 `on_drag_move` 计算插入位置：

```rust
// 容器级 drag_over + on_drop
let tab_bar = h_flex()
    // ...
    .when(droppable, |this| {
        this.on_drag_move(cx.listener(Self::on_tab_bar_drag_move))
            .drag_over::<DragPanel>(|style, _, _, _| style.bg(...))
            .on_drop(cx.listener(move |view, drag, window, cx| {
                // 根据 on_tab_bar_drag_move 中计算的插入位置
                let ix = view.pending_drop_index.take();
                view.on_drop(drag, ix, false, window, cx);
            }))
    })
    .children(
        self.panels.iter().enumerate().filter_map(|(ix, panel)| {
            // 移除每个 tab 上的 drag_over/on_drop
            Some(self.render_tab(ix, panel, &tab_state, cx))
        })
    );

fn on_tab_bar_drag_move(&mut self, drag: &DragMoveEvent<DragPanel>, _window: &mut Window, cx: &mut Context<Self>) {
    let local_x = drag.event.position.x - drag.bounds.left();
    let mut cumulative_x = 0.0;

    for (ix, tab_width) in self.tab_widths.iter().enumerate() {
        cumulative_x += tab_width;
        if local_x < cumulative_x {
            self.pending_drop_index = Some(ix);
            cx.notify();
            return;
        }
    }
    self.pending_drop_index = Some(self.panels.len()); // 末尾
    cx.notify();
}
```

同样的模式应用于垂直 tab bar (`render_vertical_tab_bar`)。

**收益:** 将 O(N) 个 `drag_over` 元素减少为 O(1)，大幅减少每帧的 hover 检测和 listener 回调次数。

---

## Fix 5: woocraft — 消除 TabPanel 每帧不必要的 Vec 分配

**文件:** `crates/woocraft/src/widgets/dock/tab_panel.rs`  
**位置:** `render_title_bar` 单标题模式 (约 910 行)

**问题代码:**
```rust
let panels: Vec<_> = self.visible_panels(cx).collect();
```

**问题分析:** 在单标题模式（`show_single_title == true`）下，`visible_panels` 遍历并收集所有可见面板到一个新 Vec 中，但这个 Vec 实际上只用于判断面板数量是否为 1。

**修复方案:**

```rust
let panel_count = self.visible_panels(cx).count();
// 或更直接地：
let panel_count = self.panels.iter().filter(|p| p.visible(cx)).count();
```

仅在需要遍历面板时才使用迭代器，避免不必要的堆分配。

---

## Fix 6: woocraft — `split_panel` 中的冗余 `LayoutChanged` 事件

**文件:** `crates/woocraft/src/widgets/dock/mod.rs`  
**位置:** `PanelEvent::LayoutChanged` 处理 (约 1083-1103 行)

**问题代码:**
```rust
cx.subscribe(&mut self.panel, move |view, _emitter, event, window, cx| {
    if matches!(event, PanelEvent::LayoutChanged) {
        cx.spawn_in(window, async move |view, window| {
            _ = view.update_in(window, |view, window, cx| {
                view.update_toggle_button_tab_panels(window, cx);
            });
        }).detach();
        cx.emit(DockEvent::LayoutChanged);
    }
}).detach();
```

**问题分析:** `split_panel` 操作可能触发多次 `LayoutChanged`（源 TabPanel、新 TabPanel、StackPanel 各一次）。每次 `LayoutChanged` 都会异步执行 `update_toggle_button_tab_panels` 并发出 `DockEvent::LayoutChanged`。这导致一次分割操作产生多次冗余扫描和事件发出。

**修复方案:**

添加一个延迟合并机制：
```rust
// DockArea 新增字段
pending_layout_change: bool,

cx.subscribe(&mut self.panel, move |view, _emitter, event, window, cx| {
    if matches!(event, PanelEvent::LayoutChanged) {
        if !view.read(cx).pending_layout_change {
            view.update(cx, |view, cx| {
                view.pending_layout_change = true;
            });
            cx.spawn_in(window, async move |view, window| {
                window.request_animation_frame(); // 等待下一帧
                _ = view.update_in(window, |view, window, cx| {
                    view.pending_layout_change = false;
                    view.update_toggle_button_tab_panels(window, cx);
                });
            }).detach();
            cx.emit(DockEvent::LayoutChanged);
        }
    }
}).detach();
```

这确保无论多少次 `LayoutChanged` 在同一帧内触发，`update_toggle_button_tab_panels` 只执行一次。

---

## 实现优先级

| 优先级 | Fix | 预期收益 | 复杂度 |
|--------|-----|---------|--------|
| **P0** | Fix 1: 拖动刷新节流 | 消除冗余的全量刷新调用 | 低 |
| **P0** | Fix 4: 容器级 drag_over | 将 O(N) drag_over 元素减少为 O(1) | 中 |
| **P1** | Fix 3: 批量 hover 检测 | 减少 hover 检测和 listener 开销 | 中 |
| **P2** | Fix 2: 缓存拖动元素布局 | 减少 drag ghost 的布局计算 | 中 |
| **P2** | Fix 6: LayoutChanged 合并 | 减少分割操作的事件风暴 | 低 |
| **P3** | Fix 5: 消除 Vec 分配 | 减少每帧堆分配 | 低 |

建议按 P0 → P1 → P2 → P3 顺序实施，每个 fix 完成后在 regressor 项目中验证效果。
