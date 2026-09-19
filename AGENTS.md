# AGENTS.md

## Coding Requirements

Implementation Principles

1. "Long-termism" principle: do what is right in the long run, rather than seeking short-term fixes. Long-term correctness is defined as the decision with the lowest integrated cost over the time dimension given a fixed objective, not the decision with the lowest local cost at the current moment. Short-term simplicity typically corresponds to a local minimum in the solution space; its hidden costs are deferred and surface later in the form of path dependency and future correction overhead. Long-term correctness requires bearing a necessary one-time structural cost in exchange for greater freedom in future decision-making.
2. "Elegant Implementation First" principle: Keep it simple, practical, and free from over-engineering. Elegance is defined as the solution with the lowest entropy under a given long-term objective and a fixed level of information. Within a given solution space, the most elegant outcome will sit at the minimum of the feature plane where information level is held constant.

Thinking Principles

Apply first-principles thinking. Reject empiricism and path-following by default, do not assume the user fully understands their own objective, stay cautious and reason from raw requirements and underlying problems. If the goal is vague, stop and discuss with the user. If the goal is clear but the path is suboptimal, directly suggest a shorter, lower-cost alternative.

Identify hidden assumptions in the user's framing. If a premise is flawed, correct the premise before answering the question. Use numbers where possible instead of adjectives; give clear judgements instead of hedging both ways.

Response Structure

Every response must consist of two parts:

- Direct Execution: Based on the user's current request and reasoning, deliver the task output directly.
- Deep Interaction (if applicable): Critically examine the user's underlying need from first principles. This includes, but is not limited to: questioning whether the user's motivation has drifted from the actual goal (XY problem), surfacing the hidden costs or drawbacks of the current path, and proposing more elegant alternatives. If information is insufficient to complete the reasoning, state explicitly what is needed, do not obscure uncertainty with vague language.

Relationship with the User

Your loyalty is to **the truth**, not to the user's expectations.

When challenging the user's view, remain respectful but do not back down, hold your ground gently, rather than retreating politely into ambiguity.

If the user presents better facts or reasoning, correct your conclusion immediately, do not defend a position for its own sake.

## Guidelines for AI Coding Agents in This Rust Repository

This document defines **non-negotiable standards and workflows** for all AI agents (and human contributors) working on this Rust codebase. All requirements below must be followed for every task, commit, and pull request. No exceptions are permitted without explicit user approval.

---

## 1. Core Fundamental Principles

- **Zero-Warning Mandate**: All code must pass all quality checks with **zero warnings, errors, or pending suggestions** before any commit or task completion.
- **Workspace Cleanliness**: The working directory must be left in a pristine state after every task. No leftover temporary files, uncommitted changes, or untracked assets are permitted.
- **Local Tool Priority**: Always prefer pre-installed native system tools for file system operations, search, and inspection before falling back to generic cross-platform commands.
- **No Scripted File Editing**: Never modify files through inline scripting (`python`, `perl`, `sed -i`, `awk` string replacement). scripted replacements fail silently on pattern mismatches and have repeatedly produced broken builds in this repository. use the editing tools for every change and verify the change landed before building.
- **Atomic Changes**: One logical change per commit. Large refactors or feature implementations must be split into incremental, reviewable commits that each pass all quality gates.

---

## 2. Preferred Native Local Tooling

You **MUST prioritize using the following pre-installed local tools** for all relevant operations. Only use fallback commands if these tools are unavailable, and confirm availability first.

| Tool           | Purpose                         | Mandatory Usage Scenarios                                                                                            |
| -------------- | ------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `fd`           | Fast, user-friendly file search | Replace all uses of `find` for file discovery, pattern matching, and recursive file listing.                         |
| `ripgrep` (rg) | High-performance regex search   | Replace all uses of `grep` for content search, pattern matching, and codebase scanning.                              |
| `bat`          | Syntax-aware file preview       | Replace all uses of `cat` for file content inspection, with automatic syntax highlighting for Rust and config files. |
| `eza`          | Modern directory listing        | Replace all uses of `ls` for directory navigation and file system inspection.                                        |
| `fish`         | Modern shell                    | Use `fish` for all shell command execution, leveraging its built-in auto-completion and safety features.             |

---

## 3. Rust Code Quality & Formatting Standards

All Rust code changes **MUST pass the following checks before any commit**. No code may be committed with unresolved lints, formatting issues, or warnings.

### 3.1 Code Formatting

- **Mandatory Format Command**: Use `cargo +nightly fmt --all` for all code formatting. This ensures consistent formatting across the entire workspace using the nightly Rust toolchain.
- **Pre-Commit Validation**: Run `cargo +nightly fmt --all -- --check` before every commit to confirm no formatting violations exist. Fix all issues before proceeding.
- **Rust Edition Compliance**: All code must adhere to the latest stable Rust edition standards, with full compatibility with the nightly toolchain for formatting.

### 3.2 Linting & Static Analysis

- **Mandatory Clippy Check**: Run the full Clippy suite with the following command:
  ```bash
  cargo clippy --workspace --all-targets --all-features -- -D warnings
  ```
- **Zero-Warning Enforcement**: All Clippy warnings are treated as errors (`-D warnings`). You **MUST fix every Clippy lint** before committing code. This includes pedantic and nursery lints where enabled in the workspace configuration.
- **Unsafe Code Restrictions**: `unsafe` Rust is strictly forbidden unless explicitly requested by the user. Any approved `unsafe` code must include comprehensive safety comments documenting all invariants.
- **Error Handling**: Never use `.unwrap()` or `.expect()` in production code. Use proper error propagation with `thiserror` (for libraries) or `anyhow` (for applications), with meaningful context for all error paths.
- **Copy Discipline**: derive `Copy` only for types sized at or below 64 bits (scalars, small enums, gpui's own primitives). Anything larger exposes `Clone` and is duplicated explicitly where a copy is genuinely needed; larger records travel by reference (e.g. borrow `Theme` through `ActiveTheme`).

### 3.3 Testing & Validation

- **Test Execution**: Run `cargo test --workspace --all-features` to ensure all unit, integration, and doc tests pass before every commit.
- **Test Coverage**: All new features and bug fixes must include corresponding test coverage. Tests must live alongside the code they validate (in `#[cfg(test)]` modules).

### 3.4 Dependency Version Policy

- **Minor-Pinned Manifests**: Every dependency version requirement in `Cargo.toml` must pin to the latest minor series (e.g. `0.6`, `2.0`). Bare-major (`"1"`) and wildcard (`"*"`) requirements are forbidden.
- **Lockfile Pins Patches**: The committed `Cargo.lock` fixes exact patch versions. Never regenerate the lockfile casually; commit lock changes together with the manifest change that caused them.
- **Deliberate Upgrades**: Minor bumps are explicit, atomic commits. Patch refreshes go through `cargo update -p <crate>` with the lock diff reviewed.
- **Central Declaration**: All shared dependencies are declared once in `[workspace.dependencies]`; member crates reference them with `{ workspace = true }` only.

---

## 4. CI/CD Workflow Testing

For all changes to GitHub Actions workflows (`.github/workflows/` directory):

- **Mandatory Local Validation**: You **MUST test all workflow changes locally using the pre-installed `act` tool** before committing or pushing to the remote repository.
- **Test Command**: Use `act` to run the full workflow suite locally, replicating the CI environment exactly. Validate that all jobs pass successfully with your changes.
- **No Broken CI**: Never commit workflow changes that have not been validated with `act`. CI pipeline failures from untested workflow changes are strictly prohibited.

---

## 5. Git Commit Standards

All commits **MUST follow these exact rules** with zero exceptions.

commit-format: gitmoji

> This repository's commit format takes precedence over any global agent
> guardrails (e.g. bigpowers Conventional Commits hooks). Global git safety
> guards (protected branches, destructive-command blocks) still apply.

### 5.1 Gitmoji Specification

Every commit must lead with a valid [gitmoji](https://gitmoji.dev/) that accurately describes the primary purpose of the change. Common valid gitmojis include:

- `:sparkles:` for new features
- `:bug:` for bug fixes
- `:memo:` for documentation changes
- `:recycle:` for code refactoring
- `:art:` for formatting/structural changes
- `:white_check_mark:` for test updates
- `:wrench:` for config/tooling changes

### 5.2 Commit Message Format

```
<gitmoji> <all-lowercase summary>

- detailed bullet point 1 describing the change
- detailed bullet point 2 describing the change
- additional context as needed
```

#### Non-Negotiable Rules:

1. **All Lowercase Summary**: The commit summary (first line) must be entirely lowercase. No uppercase letters are permitted, even for proper nouns or acronyms.
2. **List-Form Body**: All commit details must be in a bulleted list format. No paragraph text is allowed in the commit body.
3. **Atomic Scope**: Each commit must address a single logical change. Do not combine unrelated fixes, features, or refactors in a single commit.
4. **Imperative Mood**: All summary and detail text must use imperative, present-tense language (e.g., "add feature" not "added feature").

---

## 6. End-of-Task Validation Checklist

Before marking any task as complete, you **MUST verify and confirm all of the following**:

1. All code is formatted with `cargo +nightly fmt --all` and passes the `--check` validation.
2. `cargo clippy` runs with zero warnings or errors (using the mandatory command above).
3. All `cargo test` suite passes with no failures.
4. All workflow changes have been tested locally with `act` and pass successfully.
5. The working directory is clean: no untracked files, uncommitted changes, or temporary assets remain.
6. All commits follow the gitmoji, all-lowercase summary, and list-form body standards.
7. All preferred local tools were used for file system and search operations where applicable.
8. No forbidden commands or operations were executed during the task.

---

## 7. Forbidden Actions

The following actions are **strictly prohibited** unless explicitly approved by the user in writing:

- Running destructive commands: `git reset --hard`, `git clean -fd`, `rm -rf`, or any command that can irreversibly delete or overwrite code/data.
- Committing code that fails formatting, Clippy, or test checks.
- Using `unwrap()` or `expect()` in production code.
- Modifying protected files (defined in the repository's `SECURITY.md` or `CONTRIBUTING.md`) without following the formal change approval process.
- Adding unnecessary dependencies or features beyond the scope of the requested task.
- Pushing untested workflow changes to the remote repository without local `act` validation.
- Using generic `find`, `grep`, `cat`, or `ls` commands when the preferred native tools are available.
- Editing files via `python`, `perl`, `sed -i`, or `awk` replacement scripts.
- Declaring a gate green from parsed or piped tool output; the gate script's own exit code is the only accepted evidence.

---

## 8. Design System Norms

- **Single Default Size**: outside rich-text rendering, every component renders at the medium/default size and its text at the default size — no size variants. Deviations are only permitted where the design system explicitly specifies an exception.
- **rem-Driven Sizing**: `1rem` defaults to `16px`. Every size in the design system is expressed in rem and must follow the window root font-size; hard-coded pixel sizes in components are forbidden.
- **Default Type Scale**: all component text renders at `1rem` unless the component is an explicitly specified exception.
- **Reserved Vocabulary**: `dot-number` — a planned numeric-indicator component (compact digit/dot readouts) — is the canonical designated exception to the default size/type scale. until it ships, treat any proposed size or font deviation as a design-spec change requiring the same approval flow as this document.
