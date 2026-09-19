# woocraft 迁移计划

> 工作清单，随迁移进度滚动更新。规范细则见 `AGENTS.md`；质量门禁一律跑
> `./scripts/gate`，以其退出码为唯一绿灯证据。

## 当前进度

- [x] 工作区规范（AGENTS.md / rustfmt / taplo / `scripts/gate`）
- [x] 基础设施：error（thiserror）/ logging（tracing）/ theme（oklch 体系）/
      i18n（rust-i18n，craft 域）/ assets（1635 图标 + Maple Mono 字体，zstd）
- [x] 依赖基座：gpui-pre 家族经 `woocraft::gpui` / `woocraft::platform` /
      `woocraft::base` 再导出，`application()` 一键引导
- [x] P1 基建：根级词汇再导出（traits / h_flex / v_flex）+ `base::init` 串联
- [x] Button：8 变体 + outline + loading（motion 驱动）+ disabled/selected 语义态
- [x] Icon 系统：build.rs 生成 IconName + Icon widget + 自定义注册
- [x] Window 家族：TitleBar（三平台窗口控制 + CSD 拖拽）+ WindowBorder
      （阴影/缩放边）+ window_paddings
- [x] Tray：SNI/dbusmenu（Linux）+ NSStatusItem（macOS）+ Shell_NotifyIcon
      （Windows），`tray` feature opt-in
- [x] P2a 展示件：Label / Divider / Badge / Tag / Kbd / Spinner / IconLabel
- [x] 示例：palette（色盘）/ button（变体 + rem 缩放）/ primitives（展示件）/
      window_tray（窗口 + 托盘）

## 依赖关键路径

```
styled/traits/geometry ──┬─→ 开关家族(checkbox/switch/toggle/radio)
                         ├─→ 展示件 ✓
positioner ──────────────┼─→ popover → tooltip/hover_card/popup → dialog/sheet/alert → toast
scrollbar ───────────────┼─→ list/virtual_list/table/tree → dock
input(状态机) ───────────┴─→ number_input/otp_input → select/combobox → color_picker
calendar → date_picker
```

## 阶段计划

### P2b 表单与开关件（下一轮）

- checkbox(705) / switch(729) / toggle(457) / toggle_group(67) —— base
  `state_style` 语义态钩子已在 Button 上验证过
- radio(414) / radio_group(67)
- avatar(187) / progress(182) / link(466) / collapsible(112)
- 示例：forms 画廊（沿用明暗切换 + rem 缩放控制）

### P3 弹层家族

- positioner(736) 已可用（base）→ popover(473) → tooltip(347) / hover_card(406)
  / popup(271)
- dialog(825) / sheet(270) / alert_dialog(314)（focus_trap 已可用）
- toast(1012)
- 回接项：TitleBar 的 `title_menu` 与语言切换按钮（menu 全功能组件就绪后接入
  `app_menu_bar` 插槽）

### P4 输入家族

- input 状态机 → number_input(489) / otp_input(237)
- select(507) / combobox(246)（依赖 P3 弹层）
- calendar(1078) → date_picker(112)
- color_picker(995)

### P5 容器与导航

- tabs(428) / pagination(281) / list + virtual_list(905) / nav_stack(833) /
  accordion(507) / table(424) / tree(654) / resizable(1485)
- scrollbar 样式化封装（base 行为已可用）

### P6 域组件（需单项决策）

- **editor**：先 spike 验证 base `text`(15k) + `text_selection`(4.4k) 能否替代
  旧 tree-sitter/ropey 链，避免整段照搬
- **chart**：旧 `base/plot` 子系统（axis/grid/scale/shape）随迁
- **terminal**：alacritty 依赖，单独 feature
- **dock**(8325)：最后，依赖 P3 + P5 全部就绪

## 设计规范速记（全文见 AGENTS.md §3.4 / §8）

- 组件唯一 default 尺寸；全部尺寸 rem 基准，1rem 默认 16px
- 文字继承 base 1rem；`text_xs/sm/lg` 原语保留语义仅供应用层
- border/outline 1px 是唯一 px 例外，宽度走 `theme.border_width`，内缩补偿
- Badge 中心锚定 45° 对角线（零尺寸 anchor + flex 居中模式可复用）
- 例外词汇预留：`dot-number`

## 待办与决策

- [ ] `feat/gpui-base-1.0` 旧实验分支清理（等确认删除）
- [ ] origin/master 落后 24 个提交，择机推送
- [ ] editor spike（P6 前完成）
- [ ] 旧库 menu / list / form / notification / breadcrumb / title_bar 与
      P3–P5 阶段的逐项映射
- [ ] `rust-i18n` 语言补全（kbd 示例等新增文案尚未进 locale 表）
