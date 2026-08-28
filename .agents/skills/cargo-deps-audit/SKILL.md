---
name: cargo-deps-audit
description: Rust 依赖升级审计流水线 — cargo upgrade -i 升级不兼容版本、分析并修复 breaking changes、cargo update 同步锁定文件、代码质量修复与测试、最后提交并推送。适用于 Rust workspace（尤其是本仓库 woocraft）的例行依赖升级任务，用户要求"升级依赖"、"依赖审计"、"升级并修复 breaking changes"时使用。
---

# Cargo 依赖升级审计

对本 Rust workspace 执行一次完整的依赖升级审计，按以下阶段顺序执行，**不得跳过阶段**。每个阶段完成后简要向用户汇报发现。

## 阶段 0：前置检查

- `git status --short` 确认工作树干净；有未提交改动时先询问用户。
- 确认 `cargo-upgrade`（cargo-edit）可用：`cargo upgrade --version`。
  - 注意：旧版 cargo-edit（< 0.14）不支持 `--interactive`，`-i` 是 `--incompatible` 的缩写。
- 记录当前分支与 `git log --oneline -3`，用于后续提交说明。

## 阶段 1：升级依赖（含不兼容版本）

```fish
cargo upgrade --incompatible
```

- 默认升级所有依赖（含 incompatible/major 版本）；`--pinned allow` 可一并升级 `=` 锁定的版本。
- 如需保守策略，可先跑 `cargo upgrade --dry-run` 向用户展示计划再执行。
- 升级后先 `cargo build` 一次，收集编译错误清单（这是 breaking changes 的第一手来源）。

## 阶段 2：分析并修复 breaking changes

对每个 major 版本变更的依赖：

1. 用 web 搜索 / fetch 该 crate 的 CHANGELOG 或 release notes，列出 breaking changes 条目。
2. 在代码中定位受影响的 API：`rg "<旧API名>" crates -n`。
3. 逐一修复，小步修改，优先保持对外接口不变。
4. 常见坑：
   - workspace 多 crate 时公共接口变更会级联，先 `cargo build` 全 workspace 捕获回归。
   - `assets/arch` 下架构相关配置若被相关依赖的解析格式变更影响，需同步更新测试样例。
5. 修复后 `cargo build` 必须零错误；`cargo fix --allow-dirty` 可自动处理简单迁移。

## 阶段 3：同步锁定文件

```fish
cargo update
```

- 更新 `Cargo.lock` 中所有传递依赖到最新兼容版本。
- 检查 `git diff Cargo.lock` 是否有异常的 major 跳跃或移除项；有则回查是否引入了行为变化。

## 阶段 4：代码质量与测试

```fish
cargo +nightly fmt
cargo clippy --all-targets -- -D warnings
cargo test
```

- clippy 报错必须清零（新依赖版本常带来新 lint）。
- fmt 使用 `cargo +nightly fmt`（遵循仓库 `rustfmt.toml`）；若格式化波及无关文件，保持现状不必回退。
- 测试失败时回到阶段 2 排查是否为升级引起，而非掩盖测试。

## 阶段 5：提交并推送

- 提交信息遵循仓库现有风格（emoji 或 Conventional Commits，如 `:arrow_up: update dependencies`）。
- 小步提交：依赖版本升级与代码适配修复可分成两个 commit（`Cargo.toml`/`Cargo.lock` 一个，`crates/` 适配一个）。
- 提交前 `git status` 复核，不提交 `target/` 与大数据文件。
- 最后 `git push`，若远端拒绝先 pull --rebase。

## 汇报格式

结束时向用户输出：升级的依赖列表（旧 → 新）、breaking changes 及修复摘要、测试结果、提交与推送的 commit hash。
