# AGENTS.md

本文件为协助在本仓库中执行自动化 agent（或人工协同）任务的快速指南，包含本地可用工具、常见工作流、重要路径与安全/构建注意事项。请在执行写入、构建或大规模修改前阅读本文件。

## 本地常用工具

请在需要使用工具时优先使用以下工具，不要使用 grep、find、cat。

- **ripgrep（rg）**：快速全文搜索源码与文本。
- **fd**：快速查找文件（比 find 更友好）。
- **bat**：替代 `cat`，带语法高亮与分页，阅读完整文件时建议使用 `bat -Pp`。
- **eza**：替代 `ls`。
- **fish**：默认 shell 为 fish，运行脚本或演示命令时请使用 fish 语法（示例见下）。

## 仓库概览（要点）

- 本仓库是一个 Rust workspace，内容为 **woocraft 设计系统的 GPUI 组件库**（`woocraft` crate），并非应用程序。
- 子 crate：
  - `crates/woocraft`：组件库本体。`src/widgets/` 下按组件分模块（`button`、`editor`、`dock`、`terminal` 等），`src/base/` 为主题（`theme`）与样式工具（`style`、`layout` 等），`src/actions.rs` 定义跨组件 action 与键位；`examples/` 提供各组件的可运行示例。
  - `crates/terminal`（包名 `woocraft-terminal`）：终端核心层——跨平台 PTY 会话（Linux/macOS openpty、Windows ConPTY）、基于 alacritty_terminal 的终端模拟与 headless 外部控制 API，不依赖 gpui、不依赖任何 async 运行时；视图组件在 `crates/woocraft/src/widgets/terminal/`。
- 文档：
  - `docs/terminal-design.md`：terminal 组件的架构设计与阶段验收标准。
  - `docs/terminal.md`：terminal 组件的使用文档（视图 + 外部控制）。
- 构建输出位于 `target/`，不要提交。

## 关键技术约束

- **gpui 依赖来自 zed 的 git 仓库**（`gpui` / `gpui_platform` 等，见根 `Cargo.toml` 的 workspace dependencies）；`alacritty_terminal` 同样使用 zed 维护的 fork（固定 rev）。升级这些依赖时注意 API 变化。
- **禁止引入 tokio / smol 等外部 async 运行时**：
  - 视图层只用 gpui 原生原语（`cx.spawn` / `Task` / `BackgroundExecutor` 等）；
  - 核心层只用 `async-channel`（runtime-agnostic）+ 自管理线程。
- edition 2024、rust-version 1.95；代码注释与文档使用英文。

## Feature 机制（crates/woocraft）

- `resources`（默认）：rust-embed 嵌入字体/图标资源。
- `tree-sitter-languages`（默认）：启用 code editor 的各语言语法高亮。
- `terminal`（默认）：启用 `widgets/terminal` 组件并引入 `woocraft-terminal` 依赖。
- `tray`：系统托盘（可选平台依赖较重）。

新增可选组件时遵循同样模式：`#[cfg(feature = "...")] mod ...` + optional dependency + feature 入口。

## 典型工作流

- 本地构建（debug）：

  ```fish
  cargo build
  ```

- 本地构建（release）：

  ```fish
  cargo build --release
  ```

- 运行测试：

  ```fish
  cargo test
  ```

- 运行某个示例，例如 terminal：

  ```fish
  cargo run -p woocraft --example terminal
  cargo run -p woocraft --example terminal_headless
  ```

- 提交前检查（必须全绿）：

  ```fish
  cargo +nightly fmt
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

注意：当在 fish 中运行多条命令或需要激活环境变量，参考 fish 语法 `set -x VAR value`。

## 编辑与格式化

- 使用 `cargo +nightly fmt` 格式化（`rustfmt.toml` 启用了 unstable 选项：2 空格缩进、`PreferSameLine`、crate 粒度 import 分组等）。如若影响到无关文件，无需在意，保持格式化状态。
- 使用 `cargo clippy` 做静态检查，目标是无警告。

## 调试与快速检查

- 在大型改动前，可用 `rg` 与 `fd` 快速定位相关模块或函数。
- 若需查找实现或使用位置，可使用 ripgrep，例如：

  ```fish
  rg "impl Render for|fn init\(" crates -n
  ```

## 注意事项与陷阱

- Shell 注意：本机默认 shell 为 fish —— 不要在自动化脚本中假设 bash，除非显式调用 `bash -c`。
- 并发构建：当并发修改多个 crate 或修改公共接口时，先运行 `cargo build` 捕获类型/接口回归。
- 跨端校验：组件需支持 Windows / macOS / Linux。本机无法直接运行其它平台的测试时，用 `cargo check --target <t>`（配合 `rustup target add x86_64-pc-windows-msvc` / `x86_64-apple-darwin` / `aarch64-apple-darwin`）做交叉编译检查；平台相关代码务必分 `cfg(unix)` / `cfg(windows)` 处理。
- 持锁纪律（terminal 核心层）：`FairMutex<Term>` 只允许短临界区，禁止跨 `await` 或持锁回调。

## CI / 发布提示

- CI 会在 clean 环境执行构建与测试；在本地调试过失败的测试后再提交。
- 确认 `Cargo.lock` 在需要时更新并通过 CI 校验。
- 各 crate 继承 workspace 的 `publish = true` 与版本号，发版在根 `Cargo.toml` 统一调整。

## 当你作为 agent 执行更改时（操作清单）

1. 在本地分支进行改动并运行 `cargo build` + `cargo test`。
2. 小步提交（small, focused commits），conventional commits 风格（参考 `git log` 既有风格，如 `:sparkles: ...`、`:memo: ...`、`:arrow_up: ...`），方便 CI 回滚与代码审查。
3. 对影响面大的改动，先在 issue/PR 中描述计划与回滚路径。

## 联系与贡献

- 提交 PR 时请在描述中包含复现步骤、已运行的本地命令与测试结果。
- 如对构建脚本或 CI 有疑问，请在 issue 中标明你的开发环境（OS、Rust 版本、fish 版本等）。
