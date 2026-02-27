# 注意事项

本文件为协助在本仓库中执行自动化 agent（或人工协同）任务的快速指南，包含本地可用工具、常见工作流、重要路径与安全/构建注意事项。请在执行写入、构建或大规模修改前阅读本文件。

## 本地常用工具

请在需要使用工具时优先使用以下工具，不要使用 grep、find、cat。

- **ripgrep（rg）**：快速全文搜索源码与文本。
- **fd**：快速查找文件（比 find 更友好）。
- **bat**：替代 `cat`，带语法高亮与分页，阅读完整文件时建议使用 `bat -Pp`。
- **eza**：替代 `ls`。
- **fish**：默认 shell 为 fish，运行脚本或演示命令时请使用 fish 语法（示例见下）。

## 仓库概览（要点）

- 本仓库为一个 Rust workspace，顶层 `Cargo.toml` 管理多个 crate（见 `crates/` 目录）。
- 主要子项目：
  - `crates/desktop`：桌面 GUI / 集成部分（lib）。
  - `crates/instr`：指令分析、反汇编与内存/符号模块（核心分析逻辑）。
  - `crates/shell`：命令行交互与命令集实现（CLI）。
  - `crates/training`：训练/模型相关代码（训练器、解析器）。
- 常用数据目录：`data/` 包含示例二进制、数据库和测试文件。
- 构建输出位于 `target/`，CI 或本地 release 构建会产出二进制与依赖缓存。

## 典型工作流

- 本地构建（debug）：

 fish 示例：

 cargo build

- 本地构建（release）：

 cargo build --release

- 运行某个 crate 的二进制，例如 shell:

 cargo run -p shell --release

- 运行测试：

 cargo test

注意：当在 fish 中运行多条命令或需要激活环境变量，参考 fish 语法 `set -x VAR value`。

## 编辑与格式化

- 使用 `cargo +nightly fmt` 来格式化 Rust 代码（遵循仓库 `rustfmt.toml`），如若影响到无关文件，无需在意，保持格式化状态。
- 使用 `cargo clippy` 做静态检查。

## 调试与快速检查

- 在大型改动前，可用 `rg` 与 `fd` 快速定位相关模块或函数。
- 若需查找实现或使用位置，可使用 ripgrep，例如：

 rg "fn train|struct Trainer" crates -n

## 注意事项与陷阱

- Shell 注意：本机默认 shell 为 fish —— 不要在自动化脚本中假设 bash，除非显式调用 `bash -c`。
- 并发构建：当并发修改多个 crate 或修改公共接口时，先运行 `cargo build` 捕获类型/接口回归。
- 大数据/模型文件请不要提交到仓库：使用 data/ 仅作示例，真实数据请使用外部存储并记录引用。
- 目标平台与架构：`assets/arch` 下包含架构相关配置（arm/arm64/mips/riscv/x86_64/x86）。修改架构相关逻辑时请同步对应表述与测试样例。

## CI / 发布提示

- CI 会在 clean 环境执行构建与测试；在本地调试过失败的测试后再提交。
- 确认 `Cargo.lock`（如果存在）在需要时更新并通过 CI 校验。

## 当你作为 agent 执行更改时（操作清单）

1. 在本地分支进行改动并运行 `cargo build` + `cargo test`。
2. 小步提交（small, focused commits），方便 CI 回滚与代码审查。
3. 对影响面大的改动，先在 issue/PR 中描述计划与回滚路径。

## 联系与贡献

- 提交 PR 时请在描述中包含复现步骤、已运行的本地命令与测试结果。
- 如对构建脚本或 CI 有疑问，请在 issue 中标明你的开发环境（OS、Rust 版本、fish 版本等）。

---

如果你需要，我可以：

- 运行一次 `cargo build` 并报告错误；
- 为常见任务添加脚本（Makefile / cargo aliases）；
- 或根据你的工作流补充更多注意事项。
