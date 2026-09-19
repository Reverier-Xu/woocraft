//! woocraft — gpu-accelerated graphical components for the woocraft design
//! system.
//!
//! the foundation layer is [`gpui-base`]: style-free behavior, interaction,
//! and infrastructure primitives for gpui applications, which internally
//! re-exposes the `gpui-pre` snapshot of zed's gpui runtime. woocraft grows
//! its component surface on top of that base; versioning restarts at `0.6.0`.
