//! Native platform entry points for HiWave.
//!
//! This module provides the Linux-native browser implementation that does not
//! depend on wry, tao, or any WebView abstraction layers.
//!
//! - **Linux**: X11/Wayland + RustKit (via `linux.rs`)

mod linux;

pub use linux::run_native;
