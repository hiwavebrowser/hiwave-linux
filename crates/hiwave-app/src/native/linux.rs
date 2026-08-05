//! Native Linux Entry Point for HiWave
//!
//! Pure X11 + RustKit. No wry, no tao, no WebKitGTK anywhere in this path —
//! the window is created by rustkit-viewhost, painted by rustkit-compositor
//! (wgpu: Vulkan or GL), and the page comes out of rustkit-engine.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │  X11 Main Window (rustkit-viewhost)     │
//! ├─────────────────────────────────────────┤
//! │  RustKit Content View (web pages)       │
//! └─────────────────────────────────────────┘
//! ```
//!
//! Chrome/tabs/shelf are NOT here yet — this is deliberately the smallest
//! shell that is honestly a RustKit browser window, mirroring the shape the
//! Windows tree proved out (native win32 rendered first, grew features after).
//! The previous version of this file returned
//! `Err("Linux native mode not yet implemented")`.

use std::time::{Duration, Instant};
use tracing::{error, info};

/// Smoke-mode receipt path. When `HIWAVE_NATIVE_SMOKE` is set, run headful for
/// ~2 seconds, capture what the engine painted, write it here, and exit 0.
/// This is the screenshot-receipt pattern: "it renders" is claimed with a file
/// someone can open, not with an exit code alone.
const SMOKE_SHOT: &str = ".ai/artifacts/screenshots/native_linux_about.png";

/// Minimal about page for the native shell.
///
/// Deliberately NOT `main.rs`'s ABOUT_HTML: that file is written against the
/// hybrid shell's IPC (`window.ipc.postMessage` handlers that only exist under
/// wry). Feeding the engine a page whose scripts assume a webview bridge would
/// test someone else's fixture. This one is plain HTML+CSS the engine owns end
/// to end.
const NATIVE_ABOUT_HTML: &str = r#"<!DOCTYPE html>
<html><head><style>
  body { margin: 0; background: #101820; font-family: sans-serif; }
  .card { width: 420px; margin: 80px auto; padding: 24px 32px;
          background: #00a3a3; border-radius: 12px;
          box-shadow: 6px 6px 0 #002233; }
  h1 { color: #ffffff; font-size: 32px; margin: 0 0 8px 0; }
  p  { color: #e0f7f7; font-size: 16px; margin: 0; }
</style></head>
<body>
  <div class="card">
    <h1>HiWave</h1>
    <p>Rendered natively by RustKit on X11. No WebKit. No wry.</p>
  </div>
</body></html>"#;

/// Entry point for native Linux mode.
pub fn run_native() -> Result<(), String> {
    info!("Starting HiWave in native Linux mode (pure RustKit/X11)");

    let mut engine = rustkit_engine::EngineBuilder::new()
        .user_agent("HiWave/1.0 RustKit/1.0")
        .javascript_enabled(true)
        .build()
        .map_err(|e| format!("engine: {e}"))?;

    let parent = engine
        .viewhost()
        .create_main_window(rustkit_viewhost::MainWindowConfig {
            title: "HiWave (RustKit native)".to_string(),
            width: 1024,
            height: 768,
            resizable: true,
            centered: true,
        })
        .map_err(|e| format!("main window: {e}"))?;

    let view = engine
        .create_view(
            parent,
            rustkit_viewhost::Bounds { x: 0, y: 0, width: 1024, height: 768 },
        )
        .map_err(|e| format!("create_view: {e}"))?;

    engine
        .load_html(view, NATIVE_ABOUT_HTML)
        .map_err(|e| format!("load_html: {e}"))?;

    engine
        .render_view(view)
        .map_err(|e| format!("render_view: {e}"))?;

    info!("RustKit native window up; entering event loop");

    let smoke = std::env::var("HIWAVE_NATIVE_SMOKE").is_ok();
    let started = Instant::now();

    loop {
        // Non-blocking X11 pump; false means the display is gone, so the loop
        // terminates instead of spinning on a dead connection.
        if !engine.viewhost().pump_messages() {
            info!("display gone; exiting native loop");
            break;
        }

        engine
            .render_view(view)
            .map_err(|e| format!("render_view (loop): {e}"))?;

        if smoke && started.elapsed() > Duration::from_secs(2) {
            let path = std::path::Path::new(SMOKE_SHOT);
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match engine.capture_view_screenshot(view, path) {
                Ok(meta) => {
                    info!(?meta, path = SMOKE_SHOT, "smoke screenshot captured");
                    println!("NATIVE_SMOKE_OK {SMOKE_SHOT}");
                }
                Err(e) => {
                    error!("smoke screenshot failed: {e}");
                    return Err(format!("smoke screenshot: {e}"));
                }
            }
            break;
        }

        std::thread::sleep(Duration::from_millis(16));
    }

    Ok(())
}
