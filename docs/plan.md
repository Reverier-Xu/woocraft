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
- [x] P2b 表单与开关件：Checkbox（含 indeterminate）/ Switch（motion 滑块）/
      Toggle + ToggleGroup / Radio + RadioGroup / Avatar（首字母回退）/
      Progress（indeterminate 滑动）/ Link（默认 open_url 策略）/
      Collapsible（动画展开）
- [x] 示例：palette（色盘）/ button（变体 + rem 缩放）/ primitives（展示件）/
      window_tray（窗口 + 托盘）/ forms（表单开关件全画廊）

## 依赖关键路径

```
styled/traits/geometry ──┬─→ 开关家族 ✓
                         ├─→ 展示件 ✓
positioner ──────────────┼─→ popover → tooltip → dialog/sheet/alert → toast
scrollbar ───────────────┼─→ list/virtual_list/table/tree → dock
input(状态机) ───────────┴─→ number_input/otp_input → select/combobox → color_picker
calendar → date_picker
```

## 阶段计划

### P2b 表单与开关件 ✓（已完成）

- checkbox / switch / toggle(+group) / radio(+group)：base 行为件 + 主题封装，
  焦点指示统一走 keyed focus handle + `is_focused`，动画统一走 `gpui_base::motion`
  （自动尊重系统 reduce-motion）
- avatar / progress / link / collapsible：同上；共享 `theme::with_alpha` 助手，
  新增 `REVEAL` / `INDETERMINATE` 时长 token
- 示例：forms 画廊（受控状态 + 明暗切换 + rem 缩放）

### P3 弹层家族 ✓（已完成；review 后返工一轮，对齐官方 component 写法）

- 对齐 gpui-kit 官方 styled 层（longbridge/gpui-kit crates/component）后的最终形态：
  - popover：官方式薄组合——行为全部由 base Popover 承担，styled 层只贡
    献主题 content 面（popover 色、radius_lg、hairline、阴影、p 1rem）+
    children 槽；无自创状态镜像、无入场动画（官方注释明确：面板随状态
    翻转同帧卸载，淡入需跨关闭续挂，留给 select/combobox/date_picker 的
    dropdown 通道）
  - tooltip：**走 gpui 原生 `.tooltip()`**（用户决策，2026-09-21）；woocraft
    只出主题卡片 `Tooltip`（文字/自定义元素/kbd 提示，`build → AnyView`），
    不建托管 overlay/注册表。base 的 TooltipOverlay 留给未来需要跨触发
    宽限期动画时再接（官方经 Root 挂载）
  - dialog / sheet / alert_dialog：默认遮罩（scrim 共享一个深度台阶）+
    默认卡片面，标题/描述/关闭部件主题化；alert 收敛 alert 角色 + 禁背板
    关闭 + danger 确认部件
  - toast：ToastManager/Options/Motion 原样再导出；主题卡片带意图色/图标/
    标题描述/关闭钮；Toaster 以 rem 重定 sonner 的 peek/gap。**宿主必须挂
    在滚动容器外**（滚动祖先会裁剪 absolute 栈）
- 示例：overlays 画廊（弹层全家族 + toast 生命周期 tick）
- 基建顺带：button 补齐 InteractiveElement/StatefulInteractiveElement 转发，
  原生 `.tooltip()` 等交互扩展可直接挂按钮
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
