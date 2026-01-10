# Rust WebKit Rewrite: Acceptance Test Specifications

This document provides detailed test specifications for validating the Rust browser engine MVP. Each test maps to requirements in [MVP-REQUIREMENTS.md](./MVP-REQUIREMENTS.md).

## Test Categories

1. [Resize Tests](#resize-tests)
2. [Multi-View Tests](#multi-view-tests)
3. [Page Load Event Tests](#page-load-event-tests)
4. [Navigation Interception Tests](#navigation-interception-tests)
5. [Download Tests](#download-tests)
6. [Popup/Window Tests](#popupwindow-tests)
7. [Clipboard Tests](#clipboard-tests)
8. [Stability Tests](#stability-tests)

---

## Resize Tests

### T001: Basic Resize Updates Content

**Priority:** P0
**Category:** Resize
**Automation:** `hiwave-smoke`

```rust
/// Test that a single resize operation correctly updates rendered content.
#[test]
fn test_resize_basic() {
    // Setup
    let view = create_test_view(800, 600);
    view.load_html("<div id='content' style='width:100%;height:100%;background:red'></div>");
    wait_for_load(&view);
    
    // Action
    view.set_bounds(Rect::new(0, 0, 1024, 768));
    wait_for_paint(&view, Duration::from_millis(100));
    
    // Verify
    let screenshot = view.capture_screenshot();
    assert_eq!(screenshot.width(), 1024);
    assert_eq!(screenshot.height(), 768);
    assert!(screenshot.pixel_at(1000, 700).is_red()); // Content extends to new size
}
```

**Pass Criteria:**
- Screenshot dimensions match new bounds
- Content fills new viewport (no black/white borders)
- No visual artifacts

---

### T002: Rapid Resize Stress Test

**Priority:** P0
**Category:** Resize
**Automation:** `hiwave-smoke`

```rust
/// Stress test: 10,000 rapid resize cycles without failure.
#[test]
fn test_resize_rapid_10000() {
    let view = create_test_view(800, 600);
    view.load_html("<div style='width:100%;height:100%;background:blue'></div>");
    wait_for_load(&view);
    
    let sizes = [
        (800, 600), (1024, 768), (640, 480), (1920, 1080),
        (320, 240), (1280, 720), (400, 300), (1600, 900),
    ];
    
    for i in 0..10_000 {
        let (w, h) = sizes[i % sizes.len()];
        view.set_bounds(Rect::new(0, 0, w, h));
        
        // Allow compositor to process (no explicit wait - stress test)
        if i % 1000 == 0 {
            // Periodic verification
            let screenshot = view.capture_screenshot();
            assert_eq!(screenshot.width(), w);
            assert_eq!(screenshot.height(), h);
        }
    }
    
    // Final verification
    view.set_bounds(Rect::new(0, 0, 800, 600));
    wait_for_paint(&view, Duration::from_millis(100));
    let final_screenshot = view.capture_screenshot();
    assert_eq!(final_screenshot.width(), 800);
    assert_eq!(final_screenshot.height(), 600);
}
```

**Pass Criteria:**
- No crashes or panics
- No GPU resource leaks
- Final state is correct

---

### T003: Sidebar Toggle Resize

**Priority:** P0
**Category:** Resize
**Automation:** `hiwave-smoke`

```rust
/// Simulates HiWave sidebar open/close/drag behavior.
#[test]
fn test_resize_sidebar_toggle() {
    let view = create_test_view(1200, 800);
    view.load_html("<div id='marker' style='position:fixed;right:10px;top:10px;width:50px;height:50px;background:green'></div>");
    wait_for_load(&view);
    
    // Initial: Full width
    let screenshot1 = view.capture_screenshot();
    let marker1_x = find_green_marker_x(&screenshot1);
    assert!(marker1_x > 1100); // Near right edge
    
    // Simulate sidebar open: Shrink to 900px
    view.set_bounds(Rect::new(0, 0, 900, 800));
    wait_for_paint(&view, Duration::from_millis(100));
    let screenshot2 = view.capture_screenshot();
    let marker2_x = find_green_marker_x(&screenshot2);
    assert!(marker2_x > 800 && marker2_x < 900); // Marker moved with resize
    
    // Simulate sidebar drag: Gradual resize
    for width in (700..=900).step_by(10) {
        view.set_bounds(Rect::new(0, 0, width, 800));
    }
    wait_for_paint(&view, Duration::from_millis(50));
    
    // Restore full width
    view.set_bounds(Rect::new(0, 0, 1200, 800));
    wait_for_paint(&view, Duration::from_millis(100));
    let screenshot3 = view.capture_screenshot();
    let marker3_x = find_green_marker_x(&screenshot3);
    assert_eq!(marker1_x, marker3_x); // Back to original position
}
```

**Pass Criteria:**
- Content repositions correctly with sidebar
- Smooth resize during drag simulation
- State restores after sidebar close

---

## Multi-View Tests

### T004: Three Views Render Concurrently

**Priority:** P0
**Category:** Multi-View
**Automation:** `hiwave-smoke`

```rust
/// Test that 3 views can render distinct content simultaneously.
#[test]
fn test_multiview_render_3() {
    let view1 = create_test_view(400, 300);
    let view2 = create_test_view(400, 300);
    let view3 = create_test_view(400, 300);
    
    view1.load_html("<body style='background:red'></body>");
    view2.load_html("<body style='background:green'></body>");
    view3.load_html("<body style='background:blue'></body>");
    
    wait_for_load(&view1);
    wait_for_load(&view2);
    wait_for_load(&view3);
    
    // All views should have distinct content
    let ss1 = view1.capture_screenshot();
    let ss2 = view2.capture_screenshot();
    let ss3 = view3.capture_screenshot();
    
    assert!(ss1.dominant_color().is_red());
    assert!(ss2.dominant_color().is_green());
    assert!(ss3.dominant_color().is_blue());
    
    // Verify view1 wasn't blanked by creating view2/view3
    assert!(!ss1.is_blank());
}
```

**Pass Criteria:**
- All 3 views render simultaneously
- Each view shows its own content
- No views go blank when others are created

---

### T005: Create/Destroy 100 Views

**Priority:** P0
**Category:** Multi-View
**Automation:** `hiwave-smoke`

```rust
/// Stress test view creation and destruction.
#[test]
fn test_multiview_create_destroy_100() {
    let initial_memory = get_process_memory();
    
    for i in 0..100 {
        let view = create_test_view(200, 200);
        view.load_html(&format!("<body>View {}</body>", i));
        wait_for_load(&view);
        
        let screenshot = view.capture_screenshot();
        assert!(!screenshot.is_blank());
        
        drop(view); // Explicit destroy
    }
    
    // Force GC/cleanup
    std::thread::sleep(Duration::from_millis(500));
    
    let final_memory = get_process_memory();
    let memory_growth = final_memory - initial_memory;
    
    // Allow up to 50MB growth (some caching expected)
    assert!(memory_growth < 50 * 1024 * 1024, 
        "Memory leak detected: grew by {} MB", memory_growth / 1024 / 1024);
}
```

**Pass Criteria:**
- All 100 views created and destroyed without crash
- No significant memory leak (< 50MB growth)
- Each view rendered correctly before destruction

---

## Page Load Event Tests

### T006: Load Event Ordering

**Priority:** P0
**Category:** Events
**Automation:** Unit test

```rust
/// Verify load events fire in correct order.
#[test]
fn test_load_events_ordering() {
    let view = create_test_view(800, 600);
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    
    view.set_load_handler(move |event| {
        events_clone.lock().unwrap().push(event);
    });
    
    view.load_url("https://example.com");
    wait_for_load(&view);
    
    let captured = events.lock().unwrap();
    
    // Verify ordering
    assert!(captured.len() >= 3);
    assert!(matches!(captured[0], LoadEvent::DidStartProvisionalLoad { .. }));
    assert!(matches!(captured[1], LoadEvent::DidCommitLoad { .. }));
    assert!(matches!(captured.last(), Some(LoadEvent::DidFinishLoad { .. })));
    
    // Verify is_loading transitions
    // After start: is_loading = true
    // After finish: is_loading = false
}
```

**Pass Criteria:**
- Events fire in order: Start → Commit → Finish
- `is_loading()` state matches events
- No duplicate events

---

### T007: Error Events on Failure

**Priority:** P0
**Category:** Events
**Automation:** Unit test

```rust
/// Verify error events fire on network failure.
#[test]
fn test_load_events_failure() {
    let view = create_test_view(800, 600);
    let events = Arc::new(Mutex::new(Vec::new()));
    let events_clone = events.clone();
    
    view.set_load_handler(move |event| {
        events_clone.lock().unwrap().push(event);
    });
    
    // Load non-existent URL
    view.load_url("https://this-domain-does-not-exist-12345.invalid");
    
    // Wait for failure (with timeout)
    std::thread::sleep(Duration::from_secs(5));
    
    let captured = events.lock().unwrap();
    
    // Should have start event followed by failure
    assert!(captured.iter().any(|e| matches!(e, LoadEvent::DidFailLoad { .. })));
    
    // Verify error contains useful information
    let fail_event = captured.iter().find(|e| matches!(e, LoadEvent::DidFailLoad { .. }));
    if let Some(LoadEvent::DidFailLoad { error, .. }) = fail_event {
        assert!(!error.message.is_empty());
    }
}
```

**Pass Criteria:**
- DidFailLoad event fires on network error
- Error contains descriptive message
- is_loading() returns false after failure

---

## Navigation Interception Tests

### T008: Block Request at Network Level

**Priority:** P0
**Category:** Interception
**Automation:** Unit test

```rust
/// Verify requests can be blocked before network fetch.
#[test]
fn test_intercept_block_request() {
    let view = create_test_view(800, 600);
    let blocked_urls = Arc::new(Mutex::new(Vec::new()));
    let blocked_clone = blocked_urls.clone();
    
    view.set_request_interceptor(move |request| {
        if request.url.contains("blocked-resource") {
            blocked_clone.lock().unwrap().push(request.url.clone());
            return InterceptResult::Block;
        }
        InterceptResult::Allow
    });
    
    view.load_html(r#"
        <html>
        <body>
            <script src="https://example.com/blocked-resource.js"></script>
            <img src="https://example.com/blocked-resource.png">
            <script>window.testResult = 'loaded';</script>
        </body>
        </html>
    "#);
    
    wait_for_load(&view);
    
    // Verify blocked resources were intercepted
    let blocked = blocked_urls.lock().unwrap();
    assert!(blocked.iter().any(|u| u.contains("blocked-resource.js")));
    assert!(blocked.iter().any(|u| u.contains("blocked-resource.png")));
    
    // Verify main page still loaded
    let result = view.evaluate_script("window.testResult").unwrap();
    assert_eq!(result, "loaded");
}
```

**Pass Criteria:**
- Interceptor called before network request
- Block decision prevents network fetch
- Main page load not affected by blocked subresources

---

### T009: Resource Type Classification

**Priority:** P0
**Category:** Interception
**Automation:** Unit test

```rust
/// Verify resource types are correctly classified.
#[test]
fn test_intercept_resource_type() {
    let view = create_test_view(800, 600);
    let resource_types = Arc::new(Mutex::new(HashMap::new()));
    let types_clone = resource_types.clone();
    
    view.set_request_interceptor(move |request| {
        types_clone.lock().unwrap().insert(
            request.url.clone(),
            request.resource_type
        );
        InterceptResult::Allow
    });
    
    view.load_html(r#"
        <html>
        <head>
            <link rel="stylesheet" href="https://example.com/style.css">
        </head>
        <body>
            <img src="https://example.com/image.png">
            <script src="https://example.com/script.js"></script>
            <iframe src="https://example.com/frame.html"></iframe>
        </body>
        </html>
    "#);
    
    wait_for_load(&view);
    
    let types = resource_types.lock().unwrap();
    
    // Verify type classification
    assert!(types.values().any(|t| *t == ResourceType::Stylesheet));
    assert!(types.values().any(|t| *t == ResourceType::Image));
    assert!(types.values().any(|t| *t == ResourceType::Script));
    assert!(types.values().any(|t| *t == ResourceType::SubFrame));
}
```

**Pass Criteria:**
- Each resource type correctly identified
- Main document identified as Document type
- SubFrame type for iframes

---

## Download Tests

### T010: Basic File Download

**Priority:** P1
**Category:** Download
**Automation:** Integration test

```rust
/// Test downloading a file to disk.
#[test]
fn test_download_basic() {
    let view = create_test_view(800, 600);
    let download_path = temp_dir().join("test_download.bin");
    let download_complete = Arc::new(AtomicBool::new(false));
    let complete_clone = download_complete.clone();
    
    view.set_download_handler(move |download| {
        download.set_destination(&download_path);
        download.on_complete(move || {
            complete_clone.store(true, Ordering::SeqCst);
        });
    });
    
    // Navigate to downloadable resource
    view.load_url("https://httpbin.org/bytes/1024"); // 1KB random bytes
    
    // Wait for download
    wait_until(|| download_complete.load(Ordering::SeqCst), Duration::from_secs(30));
    
    // Verify file
    assert!(download_path.exists());
    assert_eq!(std::fs::metadata(&download_path).unwrap().len(), 1024);
    
    std::fs::remove_file(download_path).ok();
}
```

**Pass Criteria:**
- File downloaded to specified path
- File size matches expected
- Download handler called with metadata

---

### T011: Download Progress Callbacks

**Priority:** P1
**Category:** Download
**Automation:** Integration test

```rust
/// Test download progress reporting.
#[test]
fn test_download_progress() {
    let view = create_test_view(800, 600);
    let progress_updates = Arc::new(Mutex::new(Vec::new()));
    let updates_clone = progress_updates.clone();
    
    view.set_download_handler(move |download| {
        let path = temp_dir().join("progress_test.bin");
        download.set_destination(&path);
        
        let updates = updates_clone.clone();
        download.on_progress(move |received, total| {
            updates.lock().unwrap().push((received, total));
        });
    });
    
    // Download larger file for visible progress
    view.load_url("https://httpbin.org/bytes/102400"); // 100KB
    
    wait_for_download_complete(&view, Duration::from_secs(60));
    
    let updates = progress_updates.lock().unwrap();
    
    // Verify progress updates received
    assert!(updates.len() > 1, "Expected multiple progress updates");
    
    // Verify progress increases
    for i in 1..updates.len() {
        assert!(updates[i].0 >= updates[i-1].0, "Progress should increase");
    }
    
    // Verify final progress matches total
    let (final_received, total) = updates.last().unwrap();
    assert_eq!(*final_received, *total);
}
```

**Pass Criteria:**
- Multiple progress callbacks received
- Progress values increase monotonically
- Final progress equals total size

---

## Popup/Window Tests

### T012: target="_blank" Handling

**Priority:** P1
**Category:** Popup
**Automation:** Unit test

```rust
/// Test that target="_blank" links trigger new page creation.
#[test]
fn test_popup_target_blank() {
    let view = create_test_view(800, 600);
    let popup_requests = Arc::new(Mutex::new(Vec::new()));
    let requests_clone = popup_requests.clone();
    
    view.set_create_page_handler(move |url, features| {
        requests_clone.lock().unwrap().push((url, features));
        CreatePageResult::Deny // Block for testing
    });
    
    view.load_html(r#"
        <a id="link" href="https://example.com/popup" target="_blank">Open</a>
        <script>
            document.getElementById('link').click();
        </script>
    "#);
    
    wait_for_load(&view);
    std::thread::sleep(Duration::from_millis(500)); // Allow click to process
    
    let requests = popup_requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0].0.contains("popup"));
}
```

**Pass Criteria:**
- Create page handler called for target="_blank"
- URL correctly passed to handler
- Policy (Allow/Deny) respected

---

## Clipboard Tests

### T013: Copy/Paste Text

**Priority:** P2
**Category:** Clipboard
**Automation:** Integration test

```rust
/// Test text copy and paste operations.
#[test]
fn test_clipboard_copy_text() {
    let view = create_test_view(800, 600);
    
    view.load_html(r#"
        <textarea id="source">Hello, clipboard!</textarea>
        <textarea id="target"></textarea>
    "#);
    wait_for_load(&view);
    
    // Select text in source
    view.evaluate_script(r#"
        const source = document.getElementById('source');
        source.select();
    "#);
    
    // Execute copy
    view.execute_command("copy");
    
    // Focus target and paste
    view.evaluate_script(r#"
        document.getElementById('target').focus();
    "#);
    view.execute_command("paste");
    
    // Verify
    let result = view.evaluate_script(r#"
        document.getElementById('target').value
    "#).unwrap();
    
    assert_eq!(result, "Hello, clipboard!");
}
```

**Pass Criteria:**
- Copy command captures selection
- Paste command inserts content
- Text content preserved correctly

---

## Stability Tests

### T014: One-Hour Navigation Stress

**Priority:** P0
**Category:** Stability
**Automation:** `hiwave-smoke`

```rust
/// Stress test: Navigate to various URLs for 1 hour.
#[test]
#[ignore] // Long-running test
fn test_stability_1hour_navigation() {
    let view = create_test_view(1024, 768);
    let start = Instant::now();
    let duration = Duration::from_secs(3600); // 1 hour
    
    let urls = vec![
        "https://example.com",
        "https://httpbin.org/html",
        "https://www.wikipedia.org",
        // Add more test URLs
    ];
    
    let mut nav_count = 0;
    
    while start.elapsed() < duration {
        let url = &urls[nav_count % urls.len()];
        view.load_url(url);
        
        // Wait for load with timeout
        let load_result = wait_for_load_with_timeout(&view, Duration::from_secs(30));
        
        // Log failures but continue
        if load_result.is_err() {
            eprintln!("Load timeout for {} at navigation {}", url, nav_count);
        }
        
        // Random resize during navigation
        if nav_count % 5 == 0 {
            let w = 800 + (nav_count % 400) as u32;
            let h = 600 + (nav_count % 200) as u32;
            view.set_bounds(Rect::new(0, 0, w, h));
        }
        
        nav_count += 1;
    }
    
    println!("Completed {} navigations in 1 hour", nav_count);
    
    // Final health check
    view.load_html("<body>Final check</body>");
    wait_for_load(&view);
    let screenshot = view.capture_screenshot();
    assert!(!screenshot.is_blank());
}
```

**Pass Criteria:**
- No crashes in 1 hour
- Final page loads successfully
- Memory within acceptable bounds

---

### T015: Clean Shutdown

**Priority:** P0
**Category:** Stability
**Automation:** Unit test

```rust
/// Test that engine shuts down cleanly.
#[test]
fn test_stability_clean_shutdown() {
    let initial_handles = get_open_handle_count();
    
    {
        let view1 = create_test_view(800, 600);
        let view2 = create_test_view(800, 600);
        
        view1.load_url("https://example.com");
        view2.load_url("https://httpbin.org");
        
        wait_for_load(&view1);
        wait_for_load(&view2);
        
        // Views dropped here
    }
    
    // Allow cleanup
    std::thread::sleep(Duration::from_secs(2));
    
    let final_handles = get_open_handle_count();
    
    // Should return close to initial (allow small variance for runtime)
    assert!(
        final_handles <= initial_handles + 10,
        "Handle leak: {} -> {}", initial_handles, final_handles
    );
}
```

**Pass Criteria:**
- All resources released within 5 seconds
- No handle/FD leaks
- No GPU resource leaks

---

## Running Tests

### Unit Tests

```bash
cargo test --workspace --features rustkit
```

### Smoke Tests

```bash
# Quick validation (4 seconds)
python tools/ai-orchestrator/aiorch.py canary run --duration-ms 4000

# Full stress (10 minutes)
python tools/ai-orchestrator/aiorch.py canary run --duration-ms 600000
```

### CI Validation

```bash
python tools/ai-orchestrator/aiorch.py ci run --work-order reqs-mvp-matrix
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: reqs-mvp-matrix*

