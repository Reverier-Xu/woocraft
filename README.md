# Woocraft

Woocraft is a Rust component library built on top of [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui).

This repository is organized as a Cargo workspace, with the main maintained crate at `crates/woocraft`.

> [!WARNING]
>
> this crate is in early work-in-progress state.
>
> most of components come from [longbridge/gpui-component](https://github.com/longbridge/gpui-component) and [components in zed editor](https://github.com/zed-industries/zed), with some visual fixes / improvements.
>
> it may lacks of feature.

## Features

- Composable UI components powered by GPUI
- Built-in theme system (Light / Dark)
- Optional embedded icon and font assets
- Built-in internationalization (i18n)
- Multiple runnable examples

## Component Overview

`woocraft` currently exports the following components/modules:

- Core primitives: Theme, Icon, style extensions, IndexPath, Anchor
- Common widgets: Button, Input, Checkbox, Switch, Slider, Spinner, Tag
- Navigation/info: Breadcrumb, Pagination, Tooltip, Popover, Notification
- Layout/structure: Divider, TitleBar, WindowBorder, WidgetGroup
- Advanced widgets: List, VirtualList, Menu, Progress

> For exact exports, see `crates/woocraft/src/widgets/mod.rs` and `crates/woocraft/src/base/mod.rs`.

## Requirements

- Rust `1.93.0` or newer

## Installation

on crates.io:

```toml
[dependencies]
woocraft = "0.1"
```

To use directly from this repository:

```toml
[dependencies]
woocraft = { git = "https://github.com/Reverier-Xu/woocraft", package = "woocraft" }
```

## Quick Start

```rust
use gpui::{App, Application};

fn main() {
  // With the default `resources` feature enabled, built-in assets (icons/fonts) can be injected.
  let app = Application::new().with_assets(woocraft::Assets);

  app.run(|cx: &mut App| {
    woocraft::init(cx);
    // Create and open your windows/components here.
  });
}
```

`woocraft::init(cx)` will:

- Initialize i18n
- Register global actions
- Initialize base theme/style state
- Initialize component-level features (such as input/list/menu)

## Run Examples

Run from the repository root:

```bash
cargo run -p woocraft --example button
cargo run -p woocraft --example controls
cargo run -p woocraft --example menu
cargo run -p woocraft --example pagination
```

Example sources are located in `crates/woocraft/examples/`.

## Internationalization (i18n)

Built-in locales:

- `zh-hans`
- `zh-hant`
- `en-us`
- `ja-jp`

By default, locale is detected from `LC_ALL` / `LANG` and normalized to one of the supported values above.

You can also switch locale at runtime:

```rust
woocraft::set_locale("zh-CN");
let current = woocraft::locale();
```

## Feature Flags

- `resources` (enabled by default):
  - Enables `rust-embed`
  - Exposes `woocraft::Assets`
  - Automatically registers embedded fonts (inside `woocraft::init`)

Disable default features:

```toml
[dependencies]
woocraft = { version = "0.1.0", default-features = false }
```

## Repository Layout

```text
.
├─ crates/
│  └─ woocraft/        # Main crate
├─ packages/
│  └─ woocraft/        # npm package metadata (version alignment)
└─ deprecated/         # Legacy/migrating code
  ├─ ui/
  ├─ story/
  ├─ assets/
  ├─ webview/
  ├─ reqwest_client/
  └─ macros/
```

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
You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0 .
Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.