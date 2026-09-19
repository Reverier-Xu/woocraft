# woocraft

A GPU-accelerated graphical component library for Rust, built on
[GPUI](https://github.com/zed-industries/zed).

> this repository is a full rewrite of the previously published `woocraft`
> crates. the version line restarts at `0.6.0`.

## status

infrastructure is in place: thiserror-backed error type, tracing bootstrap,
the oklch theme system, domain-scoped i18n (zh-hans / zh-hant / en-us /
ja-jp), and embedded icon + font resources. the workspace builds on
`gpui-base` 0.6 (style-free foundations from the gpui-kit family, with the
`gpui-pre` gpui snapshot exposed internally). manifest requirements pin to
the latest minor series; `cargo.lock` fixes patches. the component surface
grows back under `crates/woocraft` module by module.

## license

`GPL-2.0-only` — see [LICENSE](LICENSE).
