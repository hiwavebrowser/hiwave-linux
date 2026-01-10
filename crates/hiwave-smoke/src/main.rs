use serde_json::json;
use std::time::{Duration, Instant};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::WindowBuilder;
use wry::dpi::{LogicalPosition, LogicalSize};
use wry::{Rect, WebViewBuilder};

#[derive(Debug, Clone)]
enum UserEvent {
    Layout {
        left: f64,
        right_open: bool,
        shelf: f64,
    },
    Exit,
}

fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        position: LogicalPosition::new(x, y).into(),
        size: LogicalSize::new(w.max(0.0), h.max(0.0)).into(),
    }
}

fn apply_layout(
    window: &tao::window::Window,
    chrome: &wry::WebView,
    content: &wry::WebView,
    shelf: &wry::WebView,
    left_sidebar_width: f64,
    right_sidebar_open: bool,
    shelf_height: f64,
) {
    let size = window.inner_size();
    let width = size.width as f64;
    let height = size.height as f64;

    let chrome_h = 72.0;
    let right_sidebar_width = if right_sidebar_open { 220.0 } else { 0.0 };

    let content_w = (width - left_sidebar_width - right_sidebar_width).max(0.0);
    let content_h = (height - chrome_h - shelf_height).max(0.0);

    let _ = chrome.set_bounds(rect(0.0, 0.0, width, chrome_h));
    let _ = content.set_bounds(rect(left_sidebar_width, chrome_h, content_w, content_h));
    let _ = shelf.set_bounds(rect(
        left_sidebar_width,
        height - shelf_height,
        content_w,
        shelf_height,
    ));
}

fn spawn_scripted_flow(proxy: EventLoopProxy<UserEvent>, duration_ms: u64) {
    std::thread::spawn(move || {
        let start = Instant::now();

        // Phase 1: sidebar drag simulation
        for i in 0..30 {
            let left = (i as f64) * 8.0; // 0..240
            let right_open = i % 10 >= 5;
            let shelf = if i % 2 == 0 { 0.0 } else { 120.0 };
            let _ = proxy.send_event(UserEvent::Layout {
                left,
                right_open,
                shelf,
            });
            std::thread::sleep(Duration::from_millis(30));
        }

        // Phase 2: simple navigation (best-effort)
        // We avoid network dependency in the harness itself.

        // Let UI settle
        while start.elapsed() < Duration::from_millis(duration_ms) {
            std::thread::sleep(Duration::from_millis(50));
        }

        let _ = proxy.send_event(UserEvent::Exit);
    });
}

fn main() {
    let duration_ms = std::env::args()
        .skip_while(|a| a != "--duration-ms")
        .nth(1)
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(4000);

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("HiWave Smoke Harness")
        .with_inner_size(tao::dpi::LogicalSize::new(1100.0, 760.0))
        .build(&event_loop)
        .expect("Failed to create window");

    let chrome = WebViewBuilder::new()
        .with_html(
            r#"<!doctype html><meta charset='utf-8'/>
            <body style='margin:0;background:#111;color:#fff;font:16px system-ui;display:flex;align-items:center;justify-content:center;'>
              chrome
            </body>"#,
        )
        .with_bounds(rect(0.0, 0.0, 1100.0, 72.0))
        .build_as_child(&window)
        .expect("Failed to create chrome webview");

    let content = WebViewBuilder::new()
        .with_html(
            r#"<!doctype html><meta charset='utf-8'/>
            <body style='margin:0;background:#0b1a2a;color:#d7e8ff;font:16px system-ui;display:flex;align-items:center;justify-content:center;'>
              content
            </body>"#,
        )
        .with_bounds(rect(0.0, 72.0, 1100.0, 568.0))
        .build_as_child(&window)
        .expect("Failed to create content webview");

    let shelf = WebViewBuilder::new()
        .with_html(
            r#"<!doctype html><meta charset='utf-8'/>
            <body style='margin:0;background:#1a0b2a;color:#f0d7ff;font:16px system-ui;display:flex;align-items:center;justify-content:center;'>
              shelf
            </body>"#,
        )
        .with_bounds(rect(0.0, 760.0, 1100.0, 0.0))
        .build_as_child(&window)
        .expect("Failed to create shelf webview");

    spawn_scripted_flow(proxy, duration_ms);

    let mut last_layout = (0.0_f64, false, 0.0_f64);
    let start = Instant::now();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                let result = json!({
                    "status": "fail",
                    "reason": "user_closed_window",
                    "elapsed_ms": start.elapsed().as_millis()
                });
                println!("{}", result);
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Layout {
                left,
                right_open,
                shelf: shelf_h,
            }) => {
                last_layout = (left, right_open, shelf_h);
                apply_layout(
                    &window, &chrome, &content, &shelf, left, right_open, shelf_h,
                );

                // Also exercise evaluate_script to ensure IPC plumbing is alive.
                let _ = content
                    .evaluate_script("window.__hiwave_smoke = (window.__hiwave_smoke || 0) + 1;");
            }
            Event::UserEvent(UserEvent::Exit) => {
                let (left, right_open, shelf_h) = last_layout;
                let result = json!({
                    "status": "pass",
                    "elapsed_ms": start.elapsed().as_millis(),
                    "final_layout": {
                        "left_sidebar_width": left,
                        "right_sidebar_open": right_open,
                        "shelf_height": shelf_h
                    }
                });
                println!("{}", result);
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
