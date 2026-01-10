//! Native Linux Entry Point for HiWave
//!
//! This module provides a pure X11/Wayland entry point for HiWave that uses
//! RustKit for all rendering, completely bypassing wry/tao.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  X11/Wayland Main Window                │
//! ├─────────────────────────────────────────┤
//! │  RustKit Chrome View (tabs, toolbar)    │
//! ├─────────────────────────────────────────┤
//! │  RustKit Content View (web pages)       │
//! ├─────────────────────────────────────────┤
//! │  RustKit Shelf View (command palette)   │
//! └─────────────────────────────────────────┘
//! ```

use tracing::{error, info};

/// Entry point for native Linux mode.
///
/// This function is called from main.rs when native mode is enabled on Linux.
pub fn run_native() -> Result<(), String> {
    info!("Starting HiWave in native Linux mode");

    // TODO: Implement Linux native browser
    // This requires:
    // 1. rustkit_viewhost::linux::X11ViewHost
    // 2. rustkit_text Linux backend (Fontconfig+FreeType)
    // 3. rustkit_a11y Linux backend (AT-SPI)

    error!("Linux native mode not yet implemented");
    Err("Linux native mode not yet implemented - use hybrid mode".into())
}

