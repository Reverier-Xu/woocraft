# woocraft-terminal

Cross-platform PTY terminal session core for the [Woocraft](https://github.com/Reverier-Xu/woocraft) design system, headless-friendly.

`woocraft-terminal` provides everything needed to run and control a terminal outside of a GUI:

- Cross-platform PTY sessions (openpty on Linux/macOS, ConPTY on Windows)
- Terminal emulation based on `alacritty_terminal`
- A headless external-control API (spawn a program, feed input, wait for output, capture the screen)
- Runtime-agnostic event delivery through `async-channel`; no tokio/smol dependency

The GPUI view components built on top of this crate live in the [`woocraft`](https://crates.io/crates/woocraft) crate (`woocraft::widgets::terminal`).

## Requirements

- Rust `1.99` or newer
- Linux, macOS, or Windows

## Installation

```toml
[dependencies]
woocraft-terminal = "0.7"
```

## Quick Start

Typical external-control usage, no GUI required:

```rust
use std::time::Duration;

use woocraft_terminal::control::{spawn_with_events, wait_for_text_blocking};
use woocraft_terminal::{SpawnOptions, TerminalBounds};

let (session, events) = spawn_with_events(
  SpawnOptions::with_shell(("sh".into(), vec![])),
  TerminalBounds::new(20.0, 8.0, 80, 24),
)?;
session.input_str("echo hello-woocraft\r");
let text = wait_for_text_blocking(&events, &session, "hello-woocraft", Duration::from_secs(5))?;
assert!(text.contains("hello-woocraft"));
session.kill();
```

Events are delivered through an `async_channel` receiver, so they can be awaited from any executor (for example the one built into GPUI) or consumed synchronously via `recv_blocking`.

## Run Examples

Run from the repository root:

```bash
cargo run -p woocraft --example terminal
cargo run -p woocraft --example terminal_headless
```

Example sources are located in `crates/woocraft/examples/`.

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

## License

Copyright 2026 Reverier-Xu.

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at [http://www.apache.org/licenses/LICENSE-2.0](http://www.apache.org/licenses/LICENSE-2.0) .
Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
