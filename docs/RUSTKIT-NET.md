# RustKit Net

HTTP networking, request interception, and download management for the RustKit browser engine.

## Overview

RustKit Net provides:
- **Async HTTP**: Non-blocking network requests via `rustkit-http` (our own HTTP client)
- **Request interception**: Filter, modify, or block requests
- **Download management**: Progress tracking, pause, resume, cancel
- **fetch() API**: JavaScript-compatible interface

### Dependencies

| Crate | Purpose |
|-------|---------|
| `rustkit-http` | HTTP/1.1 client with native-tls (replaced `reqwest`) |
| `cookie_store` | Cookie management |
| `tokio` | Async runtime |

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     ResourceLoader                           │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  rustkit_http::Client                                │    │
│  │  - HTTP/1.1 with native-tls                          │    │
│  │  - Connection pooling                                │    │
│  │  - Cookie storage                                    │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  RequestInterceptor                                  │    │
│  │  - URL pattern matching                              │    │
│  │  - Block/Allow/Redirect rules                        │    │
│  │  - Custom handlers                                   │    │
│  └─────────────────────────────────────────────────────┘    │
│  ┌─────────────────────────────────────────────────────┐    │
│  │  DownloadManager                                     │    │
│  │  - Progress tracking                                 │    │
│  │  - Pause/Resume/Cancel                               │    │
│  │  - Event notifications                               │    │
│  └─────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## Usage

### Basic Fetching

```rust
use rustkit_net::{ResourceLoader, LoaderConfig, Request};
use url::Url;

// Create loader
let config = LoaderConfig::default();
let loader = ResourceLoader::new(config)?;

// Fetch a URL
let url = Url::parse("https://example.com")?;
let response = loader.fetch(Request::get(url)).await?;

// Read response
if response.ok() {
    let text = response.text().await?;
    println!("Content: {}", text);
}
```

### Request Building

```rust
use rustkit_net::Request;
use http::{HeaderName, HeaderValue, Method};
use bytes::Bytes;

// GET request
let request = Request::get(url)
    .header(HeaderName::from_static("accept"), HeaderValue::from_static("application/json"))
    .timeout(Duration::from_secs(10));

// POST request
let body = Bytes::from(r#"{"key": "value"}"#);
let request = Request::post(url, body)
    .header(HeaderName::from_static("content-type"), HeaderValue::from_static("application/json"));
```

### Response Handling

```rust
let response = loader.fetch(request).await?;

// Status check
if response.ok() {
    // Read as text
    let text = response.text().await?;
    
    // Or as JSON
    let data: MyStruct = response.json().await?;
    
    // Or as bytes
    let bytes = response.bytes().await?;
}

// Access headers
let content_type = response.content_type;
let content_length = response.content_length;

// Get suggested filename
let filename = response.suggested_filename();
```

## Request Interception

### URL Pattern Matching

```rust
use rustkit_net::intercept::{UrlPattern, RequestInterceptor};

// Exact match
let pattern = UrlPattern::exact("https://example.com/");

// Prefix match
let pattern = UrlPattern::prefix("https://ads.");

// Suffix match (domain)
let pattern = UrlPattern::suffix(".tracking.com/");

// Contains substring
let pattern = UrlPattern::contains("/analytics/");
```

### Blocking Requests

```rust
use rustkit_net::intercept::{RequestInterceptor, UrlPattern};

let mut interceptor = RequestInterceptor::new();

// Block ad domains
interceptor.block(UrlPattern::prefix("https://ads."));
interceptor.block(UrlPattern::contains("/tracking/"));

// Allow specific URL (higher priority)
interceptor.allow(UrlPattern::exact("https://ads.example.com/allowed"));

// Apply to loader
loader.set_interceptor(interceptor);
```

### Redirecting Requests

```rust
use rustkit_net::intercept::{RequestInterceptor, UrlPattern};

let mut interceptor = RequestInterceptor::new();

// Redirect old URLs to new
interceptor.redirect(
    UrlPattern::prefix("https://old.example.com/"),
    "https://new.example.com/",
);
```

### Custom Handlers

```rust
use rustkit_net::intercept::{InterceptHandler, InterceptAction};
use rustkit_net::Request;

struct MyHandler;

impl InterceptHandler for MyHandler {
    fn intercept(&self, request: &Request) -> InterceptAction {
        if request.url.host_str() == Some("blocked.com") {
            InterceptAction::Block
        } else {
            InterceptAction::Allow
        }
    }
}

let mut interceptor = RequestInterceptor::new();
interceptor.add_handler(Arc::new(MyHandler));
```

## Download Management

### Starting Downloads

```rust
use rustkit_net::ResourceLoader;
use std::path::PathBuf;

let loader = ResourceLoader::new(LoaderConfig::default())?;
let manager = loader.download_manager();

// Start download
let url = Url::parse("https://example.com/file.zip")?;
let destination = PathBuf::from("downloads/file.zip");
let download_id = loader.start_download(url, destination).await?;
```

### Tracking Progress

```rust
use rustkit_net::download::{DownloadEvent, DownloadManager};
use tokio::sync::mpsc;

let (tx, mut rx) = mpsc::unbounded_channel();
manager.set_event_sender(tx).await;

// Handle events
while let Some(event) = rx.recv().await {
    match event {
        DownloadEvent::Started { id, filename, .. } => {
            println!("Started: {}", filename);
        }
        DownloadEvent::Progress { id, progress } => {
            if let Some(pct) = progress.percentage() {
                println!("Progress: {:.1}%", pct * 100.0);
            }
            println!("Speed: {:.1} KB/s", progress.speed_bps / 1024.0);
        }
        DownloadEvent::Completed { id, path } => {
            println!("Completed: {:?}", path);
        }
        DownloadEvent::Failed { id, error } => {
            eprintln!("Failed: {}", error);
        }
        DownloadEvent::Cancelled { id } => {
            println!("Cancelled");
        }
        _ => {}
    }
}
```

### Controlling Downloads

```rust
// Cancel a download
manager.cancel(download_id).await?;

// Get state
let state = manager.get_state(download_id).await;

// Get progress
let progress = manager.get_progress(download_id).await;

// List all downloads
let downloads = manager.list().await;

// Cleanup completed downloads
manager.cleanup().await;
```

## Fetch API (JavaScript)

```rust
use rustkit_net::{FetchApi, FetchOptions, ResourceLoader};

let loader = Arc::new(ResourceLoader::new(LoaderConfig::default())?);
let fetch = FetchApi::new(loader);

// Simple GET
let response = fetch.fetch("https://api.example.com/data", FetchOptions::default()).await?;

// POST with JSON
let options = FetchOptions {
    method: Some("POST".into()),
    headers: [("Content-Type".into(), "application/json".into())].into(),
    body: Some(Bytes::from(r#"{"key": "value"}"#)),
    credentials: Some("include".into()),
    ..Default::default()
};
let response = fetch.fetch("https://api.example.com/submit", options).await?;
```

## Configuration

```rust
use rustkit_net::LoaderConfig;
use std::time::Duration;

let config = LoaderConfig {
    user_agent: "RustKit/1.0 HiWave/1.0".into(),
    accept_language: "en-US,en;q=0.9".into(),
    default_timeout: Duration::from_secs(30),
    max_redirects: 10,
    cookies_enabled: true,
};

let loader = ResourceLoader::new(config)?;
```

## Error Handling

```rust
use rustkit_net::NetError;

match loader.fetch(request).await {
    Ok(response) => { /* success */ }
    Err(NetError::Timeout(duration)) => {
        eprintln!("Request timed out after {:?}", duration);
    }
    Err(NetError::Blocked) => {
        eprintln!("Request blocked by interceptor");
    }
    Err(NetError::Cancelled) => {
        eprintln!("Request was cancelled");
    }
    Err(e) => {
        eprintln!("Request failed: {}", e);
    }
}
```

## Testing

```bash
# Run networking tests
cargo test -p rustkit-net

# With logging
RUST_LOG=rustkit_net=debug cargo test -p rustkit-net
```

---

*Document Version: 1.0*
*Last Updated: 2026-01-02*
*Work Orders: networking-http, request-interception, download-manager*

