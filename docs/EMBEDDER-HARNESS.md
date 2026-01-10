# HiWave Embedder Harness

The embedder harness (`hiwave-smoke`) is a minimal test application that reproduces HiWave's WebView embedding pattern to validate browser engine implementations.

## Purpose

1. **Reproduce WinCairo issues**: Reliably trigger resize, multi-view, and callback bugs
2. **Benchmark new engine**: Validate Rust engine implementation against same tests
3. **Stress testing**: Automated stability and performance validation
4. **Visual regression**: Screenshot capture for pixel-level comparison

## Quick Start

```powershell
# Build the harness
cargo build -p hiwave-smoke --release

# Run basic smoke test (4 seconds)
cargo run -p hiwave-smoke --release

# Run extended stress test (60 seconds)
cargo run -p hiwave-smoke --release -- --duration-ms 60000

# Run via AI orchestrator
python tools/ai-orchestrator/aiorch.py canary run --duration-ms 10000
```

## Architecture

The harness mimics HiWave's three-WebView layout:

```
┌─────────────────────────────────────────────────┐
│                    Chrome Bar                    │ ← WebView 1 (72px)
├──────────┬─────────────────────────┬────────────┤
│          │                         │            │
│  Left    │        Content          │   Right    │ ← WebView 2 (main)
│ Sidebar  │                         │  Sidebar   │
│          │                         │            │
│          │                         │            │
├──────────┴─────────────────────────┴────────────┤
│                      Shelf                       │ ← WebView 3 (0-120px)
└─────────────────────────────────────────────────┘
```

## Test Modes

### 1. Sidebar Drag Simulation

Simulates user dragging sidebar edges:

```rust
// Phase 1: sidebar drag simulation
for i in 0..30 {
    let left = (i as f64) * 8.0; // 0..240
    let right_open = i % 10 >= 5;
    let shelf = if i % 2 == 0 { 0.0 } else { 120.0 };
    apply_layout(&window, &chrome, &content, &shelf, left, right_open, shelf);
    sleep(Duration::from_millis(30));
}
```

### 2. Rapid Resize Stress

Tests compositor response to rapid bounds changes:

```rust
// Phase 2: rapid resize (10,000 iterations)
for i in 0..10_000 {
    let width = 800 + (i % 500);
    let height = 600 + (i % 300);
    window.set_inner_size(LogicalSize::new(width, height));
    // No sleep - stress the compositor
}
```

### 3. Multi-View Lifecycle

Tests view creation and destruction:

```rust
// Phase 3: create/destroy views
for _ in 0..100 {
    let view = WebViewBuilder::new()
        .with_html("<body>test</body>")
        .build_as_child(&window)?;
    // Use view...
    drop(view);
}
```

## Output Format

The harness outputs JSON to stdout for machine parsing:

### Success

```json
{
  "status": "pass",
  "elapsed_ms": 4523,
  "final_layout": {
    "left_sidebar_width": 232.0,
    "right_sidebar_open": true,
    "shelf_height": 120.0
  }
}
```

### Failure

```json
{
  "status": "fail",
  "reason": "user_closed_window",
  "elapsed_ms": 1234
}
```

### Crash

Exit code non-zero, no JSON output.

## CLI Options

| Option | Default | Description |
|--------|---------|-------------|
| `--duration-ms` | 4000 | Total test duration in milliseconds |
| `--stress-resize` | false | Enable rapid resize stress mode |
| `--stress-views` | false | Enable multi-view stress mode |
| `--capture` | false | Capture screenshots at intervals |
| `--output-dir` | `.ai/artifacts/` | Directory for screenshots/logs |

## Integration with AI Orchestrator

### Canary Runner

```powershell
# Run canary via orchestrator
python tools/ai-orchestrator/aiorch.py canary run --profile release --duration-ms 10000
```

The canary runner:
1. Builds `hiwave-smoke` in release mode
2. Runs with specified duration
3. Captures exit code and stdout
4. Generates `CanaryReport` artifact

### CI Integration

```powershell
# CI gate for smoke tests
python tools/ai-orchestrator/aiorch.py ci run --work-order hiwave-embedder-harness
```

## Metrics Captured

| Metric | Description |
|--------|-------------|
| `elapsed_ms` | Total test duration |
| `layout_changes` | Number of layout updates |
| `resize_events` | Number of resize operations |
| `view_creates` | Number of view creations |
| `view_destroys` | Number of view destructions |
| `js_evals` | Number of JS evaluations |
| `memory_peak_mb` | Peak memory usage |

## Interpreting Results

### Pass Criteria

- ✅ Exit code 0
- ✅ `status: "pass"` in JSON
- ✅ All layout changes complete without hang
- ✅ Memory returns to baseline after stress

### Failure Modes

| Symptom | Likely Cause |
|---------|--------------|
| Hang during resize | Compositor deadlock |
| Blank views | Multi-view resource conflict |
| Memory growth | Resource leak |
| Crash on view destroy | Use-after-free |
| Wrong final layout | Resize not applied |

## Extending the Harness

### Adding New Tests

```rust
// In hiwave-smoke/src/tests/mod.rs
pub fn test_new_feature(window: &Window, content: &WebView) -> TestResult {
    // Setup
    content.load_html("<body>test</body>");
    wait_for_load(content)?;
    
    // Action
    // ...
    
    // Verify
    let screenshot = capture_screenshot(content)?;
    assert!(!screenshot.is_blank());
    
    TestResult::Pass
}
```

### Screenshot Comparison

```rust
// Compare against baseline
let current = capture_screenshot(content)?;
let baseline = load_baseline("test_resize_basic")?;
let diff = compare_images(&current, &baseline);

if diff.mismatch_percent > 0.1 {
    save_diff_image(&diff, "test_resize_basic_diff.png")?;
    return TestResult::Fail("Visual regression detected");
}
```

## Troubleshooting

### Harness Won't Start

```powershell
# Check WebView2 runtime
reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"

# Install if missing
Invoke-WebRequest -Uri "https://go.microsoft.com/fwlink/p/?LinkId=2124703" -OutFile "MicrosoftEdgeWebview2Setup.exe"
.\MicrosoftEdgeWebview2Setup.exe /silent /install
```

### High Memory Usage

```powershell
# Run with memory profiling
$env:HIWAVE_DEBUG = "1"
cargo run -p hiwave-smoke --release -- --duration-ms 60000

# Check logs for GC timing
```

### Flaky Tests

- Increase `--duration-ms` for more settling time
- Add explicit waits between layout changes
- Check for race conditions in event handling

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Order: hiwave-embedder-harness*

