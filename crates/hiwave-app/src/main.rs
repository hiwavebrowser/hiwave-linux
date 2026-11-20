// SPDX-License-Identifier: MPL-2.0
// Contains code contributed under MPL-2.0 by an outside contributor; see NOTICE.
//! HiWave - Main Application Entry Point (Linux)
//!
//! This is the primary entry point for the HiWave browser application on Linux.
//!
//! Two architectures supported:
//! 1. Hybrid mode: GTK WebKit for chrome UI + RustKit for content
//! 2. Native Linux mode: 100% RustKit for everything (--features native-linux)

// Allow Arc with non-Send/Sync types - wry::WebView is single-threaded by design
#![allow(clippy::arc_with_non_send_sync)]

#[cfg(not(feature = "native-linux"))]
mod platform;

// Conditional imports for hybrid/WebView mode
#[cfg(not(feature = "native-linux"))]
use muda::Menu;
#[cfg(not(feature = "native-linux"))]
use platform::get_platform_manager;
#[cfg(not(feature = "native-linux"))]
use std::rc::Rc;
#[cfg(not(feature = "native-linux"))]
use std::sync::{atomic::AtomicBool, Arc, Mutex};
#[cfg(not(feature = "native-linux"))]
use tao::{
    dpi::{LogicalPosition, LogicalSize},
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder},
    window::{Icon, WindowBuilder},
};
#[cfg(not(feature = "native-linux"))]
use webview::{engine_name, HiWaveWebView, IWebView};
#[cfg(not(feature = "native-linux"))]
use wry::{Rect, WebViewBuilder, WebViewBuilderExtUnix};

// Always need logging
#[cfg(feature = "native-linux")]
use tracing::{error, info};
#[cfg(not(feature = "native-linux"))]
use tracing::{error, info, warn};
use tracing_subscriber::FmtSubscriber;

// Constants used by hybrid mode
#[cfg(not(feature = "native-linux"))]
const CHROME_HEIGHT_DEFAULT: u32 = 104;
#[cfg(not(feature = "native-linux"))]
const CHROME_HEIGHT_SMALL: u32 = 148;
#[cfg(not(feature = "native-linux"))]
const CHROME_HEIGHT_EXPANDED: u32 = 460;
#[cfg(not(feature = "native-linux"))]
const SHELF_HEIGHT_DEFAULT: u32 = 0;
#[cfg(not(feature = "native-linux"))]
const SHELF_HEIGHT_EXPANDED: u32 = 280;
#[cfg(not(feature = "native-linux"))]
const SIDEBAR_WIDTH: f64 = 220.0;

// Core modules (needed by both modes)
mod import;
mod state;

// Native platform module (no wry/tao/WebView)
#[cfg(feature = "native-linux")]
mod native;

// Hybrid mode specific modules (wry/tao)
#[cfg(not(feature = "native-linux"))]
mod ipc;
#[cfg(not(feature = "native-linux"))]
mod webview;

// RustKit integration (not yet implemented for Linux - placeholder for future X11/Wayland support)
// RustKit content path. Compiled in whenever the rustkit feature is on, alongside WebKit: the RustKit page view
// is chosen at runtime by HIWAVE_RUSTKIT_CONTENT=1 on an X11 window (see rustkit_content.rs), WebKit otherwise.
#[cfg(all(target_os = "linux", feature = "rustkit"))]
mod webview_rustkit;
#[cfg(all(target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
mod rustkit_content;
#[cfg(all(target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
mod rustkit_chrome;
#[cfg(all(target_os = "linux", feature = "rustkit"))]
use hiwave_shield_intercept as shield_adapter;
#[cfg(not(feature = "native-linux"))]
mod content_webview_trait;
#[cfg(not(feature = "native-linux"))]
mod content_webview_enum;
#[cfg(not(feature = "native-linux"))]
mod chrome_ui;
#[cfg(not(feature = "native-linux"))]
mod inspector_panel;
#[cfg(all(target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
mod devtools_panel;
// M1d (macOS #1012): tab-switching shortcuts. `state.rs` and `ipc/commands.rs` name `crate::tab_shortcuts`
// unconditionally, so the module is declared here; the Cmd/Ctrl chord handling that calls it stays in
// the macOS main.rs.
mod tab_shortcuts;
#[cfg(all(test, target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
mod inspector_page_tests;

/// Engine-backed unit tests (each builds an `Engine`: a GPU instance and an X11 connection, some on a content thread) take turns:
/// run concurrently they make this binary SIGSEGV intermittently (measured 3 of 12 parallel runs with only the Inspector tests serialised).
#[cfg(all(test, target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
pub(crate) fn engine_test_turn() -> std::sync::MutexGuard<'static, ()> {
    static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());
    TURN.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}
#[cfg(not(feature = "native-linux"))]
#[allow(unused_imports)]
use content_webview_enum::ContentWebView as UnifiedContentWebView;
// mod shield_adapter;

#[cfg(not(feature = "native-linux"))]
use hiwave_shield::ResourceType;
#[cfg(not(feature = "native-linux"))]
use ipc::{IpcMessage, JS_BRIDGE};
#[cfg(not(feature = "native-linux"))]
use state::AppState;

// HTML constants (used by hybrid mode)
#[cfg(not(feature = "native-linux"))]
const CHROME_HTML: &str = include_str!("ui/chrome.html");
#[cfg(not(feature = "native-linux"))]
const SHELF_HTML: &str = include_str!("ui/shelf.html");
#[cfg(not(feature = "native-linux"))]
const NEW_TAB_URL: &str = "hiwave://newtab";
#[cfg(not(feature = "native-linux"))]
const ABOUT_HTML: &str = include_str!("ui/about.html");
#[cfg(not(feature = "native-linux"))]
const ABOUT_URL: &str = "hiwave://about";
#[cfg(not(feature = "native-linux"))]
const SETTINGS_HTML: &str = include_str!("ui/settings.html");
/// The HTML content for the inspector panel (restored canonical template from macOS; wired in shell work)
#[allow(dead_code)]
pub const INSPECTOR_HTML: &str = include_str!("ui/inspector.html");
/// The script injected into a WebKit page for the Inspector (DOM tree, styles, picker, console capture).
const INSPECTOR_INJECT: &str = include_str!("ui/inspector_inject.js");
#[cfg(not(feature = "native-linux"))]
const REPORT_HTML: &str = include_str!("ui/report.html");
#[cfg(not(feature = "native-linux"))]
const REPORT_URL: &str = "hiwave://report";
#[cfg(not(feature = "native-linux"))]
const FIND_IN_PAGE_HELPER: &str = include_str!("ui/find_in_page.js");
#[cfg(not(feature = "native-linux"))]
const CONTEXT_MENU_HELPER: &str = include_str!("ui/context_menu.js");
#[cfg(not(feature = "native-linux"))]
const AUDIO_DETECTOR: &str = include_str!("ui/audio_detector.js");
#[cfg(not(feature = "native-linux"))]
const AUTOFILL_HELPER: &str = include_str!("ui/autofill.js");
#[cfg(not(feature = "native-linux"))]
const CHART_JS: &str = include_str!("ui/chart.umd.min.js");

#[cfg(not(feature = "native-linux"))]
fn create_window_icon() -> Option<Icon> {
    const SIZE: u32 = 32;
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    
    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32;
            let fy = y as f32;
            let center_y = SIZE as f32 / 2.0;
            
            // Create wave pattern - two sine waves with different frequencies
            let wave1 = (fx * 0.35).sin() * 4.0;
            let wave2 = (fx * 0.2 + 1.5).sin() * 3.0;
            let wave_y = center_y + wave1 + wave2;
            
            // Distance from wave line (for anti-aliasing and glow)
            let dist_from_wave = (fy - wave_y).abs();
            
            // Ocean blue gradient background
            let depth = fy / SIZE as f32;
            let bg_r = 15.0 + 20.0 * (1.0 - depth);
            let bg_g = 60.0 + 80.0 * (1.0 - depth);
            let bg_b = 140.0 + 80.0 * (1.0 - depth);
            
            // Wave colors - bright cyan/blue
            let wave_r = 80.0;
            let wave_g = 200.0;
            let wave_b = 255.0;
            
            // Blend based on distance from wave
            let wave_width = 3.5;
            let glow_width = 6.0;
            
            let (r, g, b) = if dist_from_wave < wave_width {
                // Core wave line
                (wave_r, wave_g, wave_b)
            } else if dist_from_wave < glow_width {
                // Glow around wave
                let t = (dist_from_wave - wave_width) / (glow_width - wave_width);
                let blend = 1.0 - t;
                (
                    bg_r + (wave_r - bg_r) * blend * 0.5,
                    bg_g + (wave_g - bg_g) * blend * 0.5,
                    bg_b + (wave_b - bg_b) * blend * 0.5,
                )
            } else {
                (bg_r, bg_g, bg_b)
            };
            
            data.push(r as u8);
            data.push(g as u8);
            data.push(b as u8);
            data.push(255);
        }
    }
    Icon::from_rgba(data, SIZE, SIZE).ok()
}

#[cfg(not(feature = "native-linux"))]
#[derive(Debug, Clone, Copy)]
enum ShelfScope {
    Workspace,
    All,
}

#[cfg(not(feature = "native-linux"))]
impl ShelfScope {
    fn as_str(self) -> &'static str {
        match self {
            ShelfScope::Workspace => "workspace",
            ShelfScope::All => "all",
        }
    }
}

#[cfg(not(feature = "native-linux"))]
impl ShelfScope {
    fn from_option(scope: Option<&str>) -> Self {
        match scope {
            Some("all") => ShelfScope::All,
            _ => ShelfScope::Workspace,
        }
    }
}

/// User events for cross-WebView communication
#[cfg(not(feature = "native-linux"))]
#[derive(Debug, Clone)]
#[allow(dead_code)]
enum UserEvent {
    Navigate(String),
    GoBack,
    GoForward,
    Reload,
    Stop,
    NewTab,
    UpdateTitle(String),
    /// The layout container moved inside the window (title bar, menu bar): place the views again.
    Relayout,
    UpdateUrl(String),
    UpdateActiveTabUrl(String),
    NavigationStateChanged {
        can_go_back: bool,
        can_go_forward: bool,
    },
    SetLoading(bool),
    RecordVisit {
        url: String,
    },
    SyncTabs,
    SyncWorkspaces,
    SyncDownloads,
    SyncShieldStats,
    FindInPage {
        query: String,
        case_sensitive: bool,
        direction: String,
    },
    FindInPageResult(serde_json::Value),
    SyncShelf(ShelfScope),
    SyncBlocklist,
    SyncHistory,
    ShowCommands(String),
    OpenSettings,
    CloseSettings,
    // Dynamic resize events
    ExpandChrome,
    ExpandChromeSmall,
    CollapseChrome,
    ExpandShelf,
    CollapseShelf,
    SetSidebarOpen(bool),
    SetSidebarWidth(f64),
    SetRightPanelOpen(bool),
    // Import events
    GetBrowserProfiles(String),
    ImportBrowserProfilesResult(String),
    ImportBookmarks {
        browser: String,
        profile_path: String,
    },
    ImportBookmarksResult(String),
    // Settings events
    GetSettings,
    SettingsResult(String),
    SaveSettings(serde_json::Value),
    SettingsSaved,
    // Export/Import events
    ExportData {
        include_settings: bool,
        include_workspaces: bool,
    },
    ExportDataResult(String),
    SaveExportToFile {
        data: String,
        suggested_name: String,
    },
    ImportStateData {
        data: String,
        replace: bool,
    },
    ImportStateDataResult(String),
    PickImportFile,
    // Clear data event
    ClearBrowsingData {
        history: bool,
        downloads: bool,
        shelf: bool,
    },
    ClearBrowsingDataResult(String),
    // Cellar events
    GetCellar,
    RestoreFromCellar {
        id: String,
    },
    ClearCellar,
    // Decay ticker
    DecayTick,
    // Focus mode events
    EnterFocusMode,
    ExitFocusMode,
    ToggleFocusMode,
    SyncFocusModeStatus,
    ShowFocusPeek,
    HideFocusPeek,
    FocusAutoTriggerCheck,
    // Focus mode settings events (from settings webview)
    GetFocusModeConfigForSettings,
    SaveFocusModeConfigFromSettings(serde_json::Value),
    AddToFocusBlocklistFromSettings(String),
    RemoveFromFocusBlocklistFromSettings(String),
    // Script evaluation (for IPC responses that need to send data back to chrome)
    EvaluateScript(String),
    // Script evaluation on content webview
    EvaluateContentScript(String),
    // Internal page loading
    LoadAboutPage,
    LoadReportPage,
    /// A page's navigation to a browser page was refused: say so in the page's console, as Chrome does.
    ReportRefusedNavigation(String),
    // Print
    PrintPage,
    // Zoom controls
    ZoomIn,
    ZoomOut,
    ResetZoom,
    SyncZoomLevel,
    // Tab audio state
    TabAudioStateChanged {
        playing: bool,
    },
    ToggleTabMute {
        id: String,
    },
    // Autofill
    TriggerAutofill,
    SendAutofillCredentials {
        credentials_json: String,
    },
    // Vault settings events (from settings webview)
    GetVaultStatus,
    UnlockVault(String),
    LockVault,
    GetAllCredentials,
    SaveCredential {
        url: String,
        username: String,
        password: String,
    },
    DeleteCredential(i64),
    // Analytics settings events (from settings webview)
    GetAnalyticsSettings,
    UpdateAnalyticsSettings {
        enabled: bool,
        retention_days: i32,
        weekly_report: bool,
        report_day: String,
    },
    ClearAnalyticsData,
    ExportAnalyticsData {
        format: String,
    },
    // UI shortcuts (forwarded from content WebView on Windows)
    CloseActiveTab,
    FocusAddressBar,
    OpenFind,
    ToggleSidebar,
    OpenCommandPalette,
    ActivateTabByIndex {
        index: usize,
    },
    // Inspector panel (I1a)
    OpenInspector,
    CloseInspector,
    ToggleInspector,
    // Inspector data (I1b)
    InspectorSendDomTree(String),
    InspectorSendElementStyles(String),
    InspectorElementPicked { path: String, styles: serde_json::Value },
    InspectorPickerStopped,
    InspectorHighlightElement { path: String },
    InspectorClearHighlight,
    InspectorStartPicker,
    InspectorStopPicker,
    InspectorScrollToElement { path: String },
    InspectElement { path: String },
    InspectorRefreshTree,
    InspectorSelect { path: String },
    InspectorExpand { path: String },
    InspectorConsoleMessage(String),
    InspectorConsoleClear,
    InspectorConsoleEval { code: String },
}

/// Where the shared container sits inside the window, in window pixels. On GNOME Wayland GTK draws its own
/// title bar and puts the container BELOW it (observed: y = 47). The WebKit layer's `set_bounds` re-places a
/// view with `size_allocate`, whose coordinates are measured from the top of the WINDOW, not from the
/// container, so without this offset every view moved up by the title bar's height (the tab strip hid behind
/// the title bar and the address row was half covered). Before this the same bug showed as a 27 px shift from
/// the "Edit" menu bar. 0 on a bare X11 window with no decorations.
#[cfg(not(feature = "native-linux"))]
static LAYOUT_ORIGIN_X: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
#[cfg(not(feature = "native-linux"))]
static LAYOUT_ORIGIN_Y: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

#[cfg(not(feature = "native-linux"))]
fn layout_origin() -> (f64, f64) {
    use std::sync::atomic::Ordering::Relaxed;
    (LAYOUT_ORIGIN_X.load(Relaxed) as f64, LAYOUT_ORIGIN_Y.load(Relaxed) as f64)
}

/// The Inspector's native developer-tools state for a RustKit page (`devtools_panel`); nothing for a WebKit page.
#[cfg(all(target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
type DevtoolsHandle = Rc<std::cell::RefCell<devtools_panel::Devtools>>;
#[cfg(not(all(target_os = "linux", feature = "rustkit")))]
type DevtoolsHandle = ();

/// Run `$body` with the page's `Devtools` (`$d`) and RustKit view (`$v`) when the page is a RustKit view; true when
/// it was (the caller then skips the WebKit script path), false for a WebKit page or a build without RustKit.
macro_rules! with_devtools {
    ($content:expr, $dev:expr, $insp:expr, |$d:ident, $v:ident| $body:expr) => {{
        #[cfg(all(target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
        let handled = if let UnifiedContentWebView::RustKit(ref $v) = $content {
            let mut $d = $dev.borrow_mut();
            let _ = $body;
            true
        } else {
            false
        };
        #[cfg(not(all(target_os = "linux", feature = "rustkit", not(feature = "native-linux"))))]
        let handled = {
            let _ = (&$content, &$dev, &$insp);
            false
        };
        handled
    }};
}

/// The Inspector panel's `window.ipc` messages. A function, not the handler closure, so the RustKit panel's pump
/// can feed it the same messages.
#[cfg(not(feature = "native-linux"))]
fn handle_inspector_ipc(body: &str, inspector_proxy: &tao::event_loop::EventLoopProxy<UserEvent>) {
    let send = |event: UserEvent| {
        let _ = inspector_proxy.send_event(event);
    };
    match serde_json::from_str::<IpcMessage>(body) {
        Ok(msg) => match msg {
            IpcMessage::CloseInspector | IpcMessage::InspectorClose => send(UserEvent::CloseInspector),
            IpcMessage::InspectorGetDomTree | IpcMessage::InspectorRefresh => send(UserEvent::InspectorRefreshTree),
            IpcMessage::InspectorGetElementStyles { path } | IpcMessage::InspectorSelectElement { path } => {
                send(UserEvent::InspectorSelect { path })
            }
            IpcMessage::InspectorExpandNode { path } => send(UserEvent::InspectorExpand { path }),
            IpcMessage::InspectorHighlightElement { path } => send(UserEvent::InspectorHighlightElement { path }),
            IpcMessage::InspectorClearHighlight => send(UserEvent::InspectorClearHighlight),
            IpcMessage::InspectorStartPicker => send(UserEvent::InspectorStartPicker),
            IpcMessage::InspectorStopPicker => send(UserEvent::InspectorStopPicker),
            IpcMessage::InspectorScrollToElement { path } => send(UserEvent::InspectorScrollToElement { path }),
            IpcMessage::InspectorConsoleEval { code } => send(UserEvent::InspectorConsoleEval { code }),
            _ => {}
        },
        Err(e) => error!("Failed to parse Inspector IPC: {}", e),
    }
}

/// Build the Inspector panel's view at `rect` (called once, on the panel's first open): a RustKit view when the
/// chrome is RustKit, else a WebKit view in the app's container.
#[cfg(not(feature = "native-linux"))]
fn build_inspector_view(
    window: &tao::window::Window,
    gtk_fixed: &gtk::Fixed,
    ipc: &Rc<dyn Fn(&str)>,
    rustkit_chrome_on: bool,
    rect: Rect,
) -> Option<chrome_ui::ChromeView> {
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    if rustkit_chrome_on {
        let at = rect.position.to_logical::<f64>(1.0);
        let size = rect.size.to_logical::<f64>(1.0);
        let bounds = rustkit_viewhost::Bounds::new(at.x as i32, at.y as i32, size.width as u32, size.height as u32);
        let document = rustkit_chrome::html_with_bridge(INSPECTOR_HTML, JS_BRIDGE, "");
        match rustkit_chrome::create(window, bounds, &document) {
            Ok(view) => {
                rustkit_chrome::install_pump(Arc::clone(&view), Rc::clone(ipc));
                return Some(chrome_ui::ChromeView::RustKit(view));
            }
            Err(why) => error!("RustKit Inspector could not be created, using the WebKit panel: {why}"),
        }
    }
    let _ = (window, rustkit_chrome_on);
    let handler = Rc::clone(ipc);
    match WebViewBuilder::new()
        .with_html(INSPECTOR_HTML)
        .with_devtools(cfg!(debug_assertions))
        .with_clipboard(true)
        .with_initialization_script(JS_BRIDGE)
        .with_bounds(rect)
        .with_ipc_handler(move |message| handler(message.body()))
        .build_gtk(gtk_fixed)
    {
        Ok(view) => Some(chrome_ui::ChromeView::WebKit(Arc::new(view))),
        Err(e) => {
            error!("Inspector WebView could not be created: {e}");
            None
        }
    }
}

#[cfg(not(feature = "native-linux"))]
#[allow(clippy::too_many_arguments)]
fn apply_layout(
    window: &tao::window::Window,
    chrome: &chrome_ui::ChromeView,
    content: &impl IWebView,
    shelf: Option<&impl IWebView>,
    inspector: &inspector_panel::InspectorPanel,
    chrome_height: f64,
    shelf_height: f64,
    inspector_height: f64,
    sidebar_width: f64,
    right_sidebar_open: bool,
) {
    let scale = window.scale_factor();
    let window_size = window.inner_size();
    let width = window_size.width as f64 / scale;
    // The usable height is the window's minus what sits above the container (the title bar).
    let (origin_x, origin_y) = layout_origin();
    let height = (window_size.height as f64 / scale - origin_y).max(0.0);

    // Clamp sidebar width to valid range (0 = closed, 48-400 when open)
    let sidebar_width = sidebar_width.min(width).max(0.0);
    let right_sidebar_width = if right_sidebar_open {
        SIDEBAR_WIDTH.min(width - sidebar_width)
    } else {
        0.0
    };
    let content_width = (width - sidebar_width - right_sidebar_width).max(0.0);
    let column = inspector_panel::column(height, chrome_height, shelf_height, inspector_height);
    let content_height = column.content_height;

    let chrome_height_rect = height;
    let chrome_rect = Rect {
        position: LogicalPosition::new(origin_x as i32, origin_y as i32).into(),
        size: LogicalSize::new(width, chrome_height_rect).into(),
    };

    let content_rect = Rect {
        position: LogicalPosition::new((sidebar_width + origin_x) as i32, (chrome_height + origin_y) as i32).into(),
        size: LogicalSize::new(content_width, content_height).into(),
    };

    chrome.set_bounds(chrome_rect);
    // The RustKit chrome shows the window less the page column (the hole); the WebKit chrome ignores it.
    // The page column (page, Inspector, shelf) is the hole: those views sit below the chrome.
    chrome_ui::ChromeUi::compose(chrome, width, chrome_height_rect, (sidebar_width, chrome_height, content_width, (height - chrome_height).max(0.0)));
    content.set_bounds(content_rect);
    // The Inspector sits between the page and the shelf; built on its first open.
    let inspector_rect = Rect {
        position: LogicalPosition::new((sidebar_width + origin_x) as i32, (column.inspector_y + origin_y) as i32).into(),
        size: LogicalSize::new(content_width, inspector_height).into(),
    };
    inspector.lay_out(window, inspector_rect);

    if let Some(shelf) = shelf {
        let shelf_rect = Rect {
            position: LogicalPosition::new((sidebar_width + origin_x) as i32, (height - shelf_height + origin_y) as i32).into(),
            size: LogicalSize::new(content_width, shelf_height).into(),
        };
        shelf.set_bounds(shelf_rect);
    }
    // The chrome stays frontmost, over panels built after it.
    chrome_ui::ChromeUi::keep_on_top(chrome);
}

#[cfg(not(feature = "native-linux"))]
fn sync_tabs_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView) {
    if let Ok(s) = state.lock() {
        let active_tab_id = s.shell.get_active_tab().map(|t| t.id);
        let workspace_id = s.shell.get_active_workspace().map(|w| w.id);
        let decay_days = s.user_settings.tab_decay_days;
        let tabs: Vec<serde_json::Value> = s
            .shell
            .list_tabs(workspace_id)
            .iter()
            .map(|tab| {
                let decay_level =
                    hiwave_shell::BrowserShell::calculate_decay_level(tab.last_visited, decay_days);
                serde_json::json!({
                    "id": tab.id.0,
                    "title": tab.title.clone().unwrap_or_else(|| "New Tab".to_string()),
                    "url": tab.url.to_string(),
                    "is_active": Some(tab.id) == active_tab_id,
                    "locked": tab.locked,
                    "last_visited": tab.last_visited,
                    "decay_level": decay_level
                })
            })
            .collect();

        let tabs_json = serde_json::to_string(&tabs).unwrap_or_else(|_| "[]".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.renderTabs({}); }}",
            tabs_json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn sync_workspaces_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView) {
    if let Ok(s) = state.lock() {
        let active_ws_id = s.shell.get_active_workspace().map(|w| w.id);
        let active_tab_id = s.shell.get_active_tab().map(|t| t.id);
        let decay_days = s.user_settings.tab_decay_days;
        let workspaces: Vec<serde_json::Value> = s
            .shell
            .list_workspaces()
            .iter()
            .map(|ws| {
                let tabs: Vec<serde_json::Value> = ws
                    .tabs
                    .iter()
                    .filter_map(|tab_id| {
                        s.shell.get_tab(*tab_id).map(|tab| {
                            let decay_level = hiwave_shell::BrowserShell::calculate_decay_level(
                                tab.last_visited,
                                decay_days,
                            );
                            serde_json::json!({
                                "id": tab.id.0.to_string(),
                                "title": tab.title.clone().unwrap_or_else(|| "New Tab".to_string()),
                                "url": tab.url.to_string(),
                                "is_active": active_tab_id == Some(tab.id),
                                "locked": tab.locked,
                                "decay_level": decay_level,
                            })
                        })
                    })
                    .collect();
                serde_json::json!({
                    "id": ws.id.0.to_string(),
                    "name": ws.name,
                    "tab_count": ws.tabs.len(),
                    "tabs": tabs,
                    "is_active": Some(ws.id) == active_ws_id,
                    "is_suspended": ws.suspended,
                    "locked_count": s.workspace_locked_count(ws.id),
                })
            })
            .collect();

        let ws_json = serde_json::to_string(&workspaces).unwrap_or_else(|_| "[]".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.renderWorkspaces({}); }}",
            ws_json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn sync_shield_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView) {
    if let Ok(s) = state.lock() {
        let stats = s.shield.get_stats();
        let payload = serde_json::json!({
            "enabled": s.shield.is_enabled(),
            "requests_blocked": stats.requests_blocked,
            "trackers_blocked": stats.trackers_blocked
        });
        let json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.updateShieldStats({}); }}",
            json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn sync_shelf_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView, scope: ShelfScope) {
    if let Ok(s) = state.lock() {
        let (items, workspace_name) = match scope {
            ShelfScope::All => (s.shelf_items_all(), None),
            ShelfScope::Workspace => (
                s.shelf_items_for_active_workspace(),
                s.shell.get_active_workspace().map(|ws| ws.name.clone()),
            ),
        };

        let payload = serde_json::json!({
            "scope": scope.as_str(),
            "workspace": workspace_name,
            "items": items,
            "global_count": s.shelf_count_all(),
        });
        let json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.renderShelf({}); }}",
            json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn sync_blocklist_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView) {
    if let Ok(s) = state.lock() {
        let domains: Vec<&String> = s.user_blocklist.domains.iter().collect();
        let json = serde_json::to_string(&domains).unwrap_or_else(|_| "[]".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.updateBlocklist({}); }}",
            json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn sync_downloads_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView) {
    if let Ok(s) = state.lock() {
        let payload = serde_json::json!({
            "downloads": s.downloads_snapshot(),
            "active_count": s.active_download_count(),
        });
        let json = serde_json::to_string(&payload).unwrap_or_else(|_| "null".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.updateDownloads({}); }}",
            json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn sync_history_to_chrome(state: &Arc<Mutex<AppState>>, chrome: &impl IWebView) {
    if let Ok(s) = state.lock() {
        let history = s.visit_history_snapshot();
        let json = serde_json::to_string(&history).unwrap_or_else(|_| "[]".to_string());
        let script = format!(
            "if(window.hiwaveChrome) {{ hiwaveChrome.updateHistory({}); }}",
            json
        );
        chrome.evaluate_script(&script);
    }
}

#[cfg(not(feature = "native-linux"))]
fn load_new_tab(content: &impl IWebView) {
    mark_app_load();
    content.load_html(ABOUT_HTML);
}

#[cfg(not(feature = "native-linux"))]
fn load_about_page(content: &impl IWebView) {
    mark_app_load();
    content.load_html(ABOUT_HTML);
}

#[cfg(not(feature = "native-linux"))]
fn load_report_page(content: &impl IWebView) {
    // Embed Chart.js library inline in the report HTML
    let report_with_chartjs = REPORT_HTML.replace(
        "<!-- Chart.js is injected by Rust before page load -->",
        &format!("<script>{}</script>", CHART_JS),
    );
    mark_app_load();
    content.load_html(&report_with_chartjs);
}

/// Whether the main window shows its pages: not while it is minimised or hidden (upstream's). A
/// window fully covered by others still counts as shown.
#[cfg(all(not(feature = "native-linux"), any(test, feature = "rustkit")))]
fn window_shows_pages(window_visible: bool, minimized: bool) -> bool {
    window_visible && !minimized
}

/// Tell the RustKit pages of the main window (a RustKit chrome, a RustKit content page) whether the
/// window shows them (`PageCmd::SetVisible`); each view sends only when its answer changes. A WebKit
/// view tracks this itself.
#[cfg(all(feature = "rustkit", not(feature = "native-linux")))]
fn sync_page_visibility(
    window: &tao::window::Window,
    chrome: &chrome_ui::ChromeView,
    content: &UnifiedContentWebView,
) {
    let shown = window_shows_pages(window.is_visible(), window.is_minimized());
    let mut changed = false;
    if let chrome_ui::ChromeView::RustKit(chrome) = chrome {
        changed |= chrome.set_page_visible(shown);
    }
    if let UnifiedContentWebView::RustKit(content) = content {
        changed |= content.set_page_visible(shown);
    }
    if changed {
        info!(shown, "Main window pages visibility");
    }
}

/// `RUST_LOG` when it is set to a directive this process can parse. Unset, blank, and unparseable
/// values stay at info, which is what the app logged before `RUST_LOG` was read (upstream's).
fn rust_log_filter(rust_log: Option<&str>) -> tracing_subscriber::EnvFilter {
    use tracing_subscriber::EnvFilter;
    match rust_log.map(str::trim).filter(|value| !value.is_empty()) {
        Some(value) => EnvFilter::try_new(value).unwrap_or_else(|_| EnvFilter::new("info")),
        None => EnvFilter::new("info"),
    }
}

/// A shortcut the app takes at window level, before the focused view (a page included) sees the
/// key: toggling focus mode and autofill are the app's, not requests a page can make over IPC.
#[cfg(not(feature = "native-linux"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowShortcut {
    ToggleFocusMode,
    TriggerAutofill,
}

/// `key` is the GDK key name (`F11`, `f`, `F`, `L`, ...). F11 or Ctrl+Shift+F toggles focus mode,
/// Ctrl+Shift+L fills the page's login (the keys context_menu.js used to forward); Alt never.
#[cfg(not(feature = "native-linux"))]
fn window_shortcut(key: &str, ctrl: bool, shift: bool, alt: bool) -> Option<WindowShortcut> {
    if alt {
        return None;
    }
    match key {
        "F11" if !ctrl => Some(WindowShortcut::ToggleFocusMode),
        "f" | "F" if ctrl && shift => Some(WindowShortcut::ToggleFocusMode),
        "l" | "L" if ctrl && shift => Some(WindowShortcut::TriggerAutofill),
        _ => None,
    }
}

/// A new tab a content page asks for: none (Ctrl+T opens the new-tab page) or an http(s) URL.
/// Every other scheme (`javascript:`, `file:`, `data:`, `hiwave:`, `about:`) is refused.
#[cfg(not(feature = "native-linux"))]
fn content_new_tab_url_allowed(url: Option<&str>) -> bool {
    match url {
        None => true,
        Some(u) => {
            let lower = u.to_ascii_lowercase();
            lower.starts_with("https://") || lower.starts_with("http://")
        }
    }
}

/// May a page navigate the content view to `url`? The browser's own pages (`hiwave:`) open only from
/// the browser's UI: a page that navigates to one is refused, as Chrome refuses a page's
/// navigation to chrome:// ("Not allowed to load local resource").
pub(crate) fn page_navigation_allowed(url: &str) -> bool {
    // As the URL parser reads it: leading C0 controls and spaces dropped, then every tab and newline
    // anywhere; and a scheme is case-insensitive.
    let url: String = url
        .trim_start_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .take(7)
        .collect();
    !url.eq_ignore_ascii_case("hiwave:")
}

/// What the address bar and the active tab's stored URL take when the content view's MAIN frame
/// commits `url` (`current_tab_is_internal`: the tab shows About or the new-tab page).
#[cfg(not(feature = "native-linux"))]
#[derive(Debug, PartialEq, Eq)]
enum TabUrlUpdate {
    /// The new-tab page (or the app's own page, loaded as about:blank): an empty address bar; the
    /// tab's URL becomes hiwave://newtab only when `store`.
    NewTab { store: bool },
    /// A web page: the address bar and the tab's URL take it.
    Page(String),
}

#[cfg(not(feature = "native-linux"))]
/// How many of the app's own page loads (load_html of the new-tab, About or Report page, or the
/// start page) have not committed yet. They commit in the main frame as about:blank, which is also
/// what a page's own navigation to about:blank commits as; this tells the two apart.
#[cfg(not(feature = "native-linux"))]
static APP_LOADS_PENDING: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Called just before the app loads one of its own pages into the content view.
#[cfg(not(feature = "native-linux"))]
fn mark_app_load() {
    APP_LOADS_PENDING.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}

/// For the main frame's committed `url`: is it one of the app's own loads? An about:blank commit
/// takes one pending app load if there is one. Any other commit is a page's, which supersedes the
/// app loads still pending (a load WebKit replaced never commits), so they are dropped.
#[cfg(not(feature = "native-linux"))]
fn take_app_load(url: &str) -> bool {
    use std::sync::atomic::Ordering;
    if url == "about:blank" {
        APP_LOADS_PENDING
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
    } else {
        APP_LOADS_PENDING.store(0, Ordering::SeqCst);
        false
    }
}

#[cfg(not(feature = "native-linux"))]
fn main_frame_url_update(url: &str, current_tab_is_internal: bool, app_load: bool) -> TabUrlUpdate {
    if url == "about:blank" && !app_load {
        // A page's own navigation to about:blank: that page, as in Chrome.
        return TabUrlUpdate::Page(url.to_string());
    }
    if is_new_tab_url(url) {
        // The app's About page also commits as about:blank: a tab showing it keeps hiwave://about.
        TabUrlUpdate::NewTab { store: !current_tab_is_internal || url == NEW_TAB_URL }
    } else {
        TabUrlUpdate::Page(url.to_string())
    }
}

/// What the content view opens at startup for the restored active tab's URL.
#[cfg(not(feature = "native-linux"))]
#[derive(Debug, PartialEq, Eq)]
enum StartPage {
    /// The start page the view was built with (the new-tab page, or a browser page it has no loader for).
    Keep,
    About,
    Report,
    /// A web page, loaded by URL.
    Url,
}

#[cfg(not(feature = "native-linux"))]
fn start_page(url: &str) -> StartPage {
    if is_about_url(url) {
        StartPage::About
    } else if is_report_url(url) {
        StartPage::Report
    } else if is_new_tab_url(url) || !page_navigation_allowed(url) {
        StartPage::Keep
    } else {
        StartPage::Url
    }
}

#[cfg(not(feature = "native-linux"))]
fn is_new_tab_url(url: &str) -> bool {
    url == "about:blank" || url == NEW_TAB_URL || url.starts_with("data:text/html")
}

#[cfg(not(feature = "native-linux"))]
fn is_about_url(url: &str) -> bool {
    url == ABOUT_URL
}

#[cfg(not(feature = "native-linux"))]
fn is_report_url(url: &str) -> bool {
    url == REPORT_URL
}

#[cfg(not(feature = "native-linux"))]
fn should_record_history_url(url: &str) -> bool {
    if is_new_tab_url(url) || is_about_url(url) || is_report_url(url) {
        return false;
    }
    let lower = url.to_lowercase();
    if lower.starts_with("about:")
        || lower.starts_with("data:")
        || lower.starts_with("javascript:")
        || lower.starts_with("hiwave:")
    {
        return false;
    }
    true
}

// Popup filtering is now handled by platform::is_definitely_popup()
// which provides comprehensive cross-platform URL blocking

#[cfg(target_os = "linux")]
fn ensure_main_thread_stack_16mib() {
    const TARGET: libc::rlim_t = 16 * 1024 * 1024;
    unsafe {
        let mut rlim = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        if libc::getrlimit(libc::RLIMIT_STACK, &mut rlim) == 0 {
            if rlim.rlim_cur < TARGET && std::env::var_os("HIWAVE_STACK_REEXEC").is_none() {
                let mut new_rlim = rlim;
                new_rlim.rlim_cur = TARGET;
                if new_rlim.rlim_max < TARGET && new_rlim.rlim_max != libc::RLIM_INFINITY {
                    new_rlim.rlim_max = TARGET;
                }
                if libc::setrlimit(libc::RLIMIT_STACK, &new_rlim) == 0 {
                    std::env::set_var("HIWAVE_STACK_REEXEC", "1");
                    let args: Vec<std::ffi::CString> = std::env::args_os()
                        .map(|a| {
                            use std::os::unix::ffi::OsStrExt;
                            std::ffi::CString::new(a.as_bytes()).unwrap_or_default()
                        })
                        .collect();
                    let mut argv: Vec<*const libc::c_char> = args.iter().map(|s| s.as_ptr()).collect();
                    argv.push(std::ptr::null());
                    let exe = std::ffi::CString::new("/proc/self/exe").unwrap();
                    libc::execv(exe.as_ptr(), argv.as_ptr());
                } else {
                    // If setrlimit fails (e.g. EPERM when hard limit is constrained),
                    // execution continues with the current stack limit.
                    eprintln!("WARN: unable to raise RLIMIT_STACK to 16 MiB: errno {}", *libc::__errno_location());
                }
            } else if std::env::var_os("HIWAVE_STACK_REEXEC").is_some() {
                // Do not leak re-exec guard variable to child processes
                std::env::remove_var("HIWAVE_STACK_REEXEC");
            }
        }
    }
}

/// Stack size of the calling thread, as the OS reports it.
#[cfg(target_os = "linux")]
pub fn main_thread_stack_size() -> usize {
    extern "C" {
        fn pthread_getattr_np(thread: libc::pthread_t, attr: *mut libc::pthread_attr_t) -> libc::c_int;
        fn pthread_attr_getstack(
            attr: *const libc::pthread_attr_t,
            stackaddr: *mut *mut libc::c_void,
            stacksize: *mut libc::size_t,
        ) -> libc::c_int;
        fn pthread_attr_destroy(attr: *mut libc::pthread_attr_t) -> libc::c_int;
    }
    unsafe {
        let mut attr: libc::pthread_attr_t = std::mem::zeroed();
        if pthread_getattr_np(libc::pthread_self(), &mut attr) == 0 {
            let mut stackaddr: *mut libc::c_void = std::ptr::null_mut();
            let mut stacksize: libc::size_t = 0;
            pthread_attr_getstack(&attr, &mut stackaddr, &mut stacksize);
            pthread_attr_destroy(&mut attr);
            stacksize as usize
        } else {
            0
        }
    }
}

/// Recursively touches 1 MiB stack frames to depth `depth` to verify
/// runtime stack expansion past the 8 MiB OS default without SIGSEGV.
#[cfg(target_os = "linux")]
#[inline(never)]
fn probe_10mib_stack_recursion(depth: usize) -> u8 {
    let mut frame = [0u8; 1024 * 1024];
    frame[0] = depth as u8;
    frame[1024 * 1024 - 1] = depth as u8;
    std::hint::black_box(&mut frame);
    let val = frame[0] ^ frame[1024 * 1024 - 1];
    if depth <= 1 {
        val
    } else {
        val.wrapping_add(probe_10mib_stack_recursion(depth - 1))
    }
}

// ---- Logging that never writes a secret (ported from hiwave-macos CH3.A6/#946, S3 sync 2026-11-06) ----
// A URL in a log line is scheme, host and path only; an IPC message is its `cmd` and size unless
// HIWAVE_DEBUG=1 / HIWAVE_TEST_HOOKS=1. Linux logged both whole before this.
/// The `cmd` of an IPC message, for the log (#907 R1 nit 1): `?` when it
/// has none, or when it is not a command name (letters, digits, `_`; a page
/// can post anything to the content view's handler).
fn ipc_cmd(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|msg| msg.get("cmd").and_then(|c| c.as_str()).map(str::to_string))
        .filter(|cmd| !cmd.is_empty() && cmd.len() <= 64 && cmd.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .unwrap_or_else(|| "?".to_string())
}

/// Whether IPC messages are logged whole: only with `HIWAVE_DEBUG=1` (the
/// app's verbose switch) or `HIWAVE_TEST_HOOKS=1` (the real-window checks,
/// which read them). By default a line says the message's `cmd` and size:
/// a body carries URLs with their queries and find-in-page text. The
/// RustKit panels' messages are never logged whole (`panel_ipc_log_line`).
fn ipc_bodies_logged() -> bool {
    static LOGGED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *LOGGED.get_or_init(|| {
        ["HIWAVE_DEBUG", "HIWAVE_TEST_HOOKS"]
            .iter()
            .any(|name| std::env::var(name).map(|v| v == "1").unwrap_or(false))
    })
}

/// What the log says about a chrome IPC command and its answer: the command name, only when it
/// looks like one (a short snake_case word; anything else is "unknown"), and the answer's kind and
/// size. Never the answer itself: it can carry what the vault or the history holds.
#[cfg(not(feature = "native-linux"))]
fn ipc_log_shape(body: &str, response: &ipc::IpcResponse) -> String {
    let name = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("cmd").and_then(|c| c.as_str().map(str::to_owned)))
        .filter(|c| !c.is_empty() && c.len() <= 40 && c.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_'))
        .unwrap_or_else(|| "unknown".to_string());
    match response {
        ipc::IpcResponse::Success { data } => {
            let bytes = serde_json::to_vec(data).map(|v| v.len()).unwrap_or(0);
            format!("{name}: success, {bytes} bytes")
        }
        ipc::IpcResponse::Error { .. } => format!("{name}: error"),
    }
}

/// One IPC message in the log, as `ipc_bodies_logged` allows.
fn log_ipc(what: &str, body: &str) {
    if ipc_bodies_logged() {
        info!(%body, "{}", what);
    } else {
        info!(cmd = %ipc_cmd(body), len = body.len(), "{}", what);
    }
}

/// A URL as it may be logged (hiwave-windows #162; upstream CH3.A6): a `data:` URL (how the app
/// loads its own pages) by size only; anything else as scheme, host, port and path as the URL parser
/// reads them, without its query, fragment or userinfo, cut to a sane length. A URL that does not
/// parse is logged by size only.
fn url_for_log(url: &str) -> std::borrow::Cow<'_, str> {
    use std::borrow::Cow;
    let url = url.trim_start_matches(|c: char| c.is_ascii_whitespace() || c.is_ascii_control());
    // `get`, not a slice: byte 5 may sit inside a multi-byte character.
    if url.get(..5).is_some_and(|scheme| scheme.eq_ignore_ascii_case("data:")) {
        return Cow::Owned(format!("data: URL ({} bytes)", url.len()));
    }
    let shown = match url::Url::parse(url) {
        Ok(parsed) => match parsed.host_str() {
            Some(host) => {
                let port = parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
                format!("{}://{}{}{}", parsed.scheme(), host, port, parsed.path())
            }
            None => format!("{}:{}", parsed.scheme(), parsed.path()),
        },
        Err(_) => return Cow::Owned(format!("unparsed URL ({} bytes)", url.len())),
    };
    if shown.len() > 200 {
        let mut cut = 200;
        while !shown.is_char_boundary(cut) {
            cut -= 1;
        }
        return Cow::Owned(format!("{}... ({} bytes)", &shown[..cut], shown.len()));
    }
    Cow::Owned(shown)
}

/// A page URL for an info line (CH3.A6): whole only when IPC bodies are
/// logged whole (`HIWAVE_DEBUG=1`, or `HIWAVE_TEST_HOOKS=1` for the
/// real-window checks, which read queries); `url_for_log` otherwise.
fn page_url_for_log(url: &str) -> std::borrow::Cow<'_, str> {
    page_url_for_log_as(url, ipc_bodies_logged())
}

fn page_url_for_log_as(url: &str, whole: bool) -> std::borrow::Cow<'_, str> {
    if whole {
        std::borrow::Cow::Borrowed(url)
    } else {
        url_for_log(url)
    }
}

/// WebKitGTK 2.4x+ draws through a DMA-BUF renderer. On NVIDIA's proprietary driver (especially under
/// Wayland) that renderer commonly fails and the window opens but stays empty. The documented switch is
/// `WEBKIT_DISABLE_DMABUF_RENDERER=1`. Set it for the user when that driver is present, unless they have
/// set the variable themselves (so `WEBKIT_DISABLE_DMABUF_RENDERER=0` still wins). Must run before GTK
/// or WebKit initialise. Returns whether the renderer is disabled by this rule, so `main` can log it once logging is up.
#[cfg(target_os = "linux")]
fn apply_gpu_workarounds() -> bool {
    // Either marker: /proc/driver/nvidia/version is absent on some driver builds (this machine has the
    // directory but no `version` file), while the control node and the directory are always there.
    let nvidia_proprietary = ["/proc/driver/nvidia", "/dev/nvidiactl"]
        .iter()
        .any(|p| std::path::Path::new(p).exists());
    if nvidia_proprietary && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // Report the STATE, not "did I just set it": the app re-executes itself once for its 16 MiB stack and
    // the second process inherits the variable, so "did I set it" would be false there and log nothing.
    nvidia_proprietary && std::env::var("WEBKIT_DISABLE_DMABUF_RENDERER").map(|v| v == "1").unwrap_or(false)
}


/// A JavaScript string literal for `text` (JSON's escaping is a valid JS literal).
fn js_string(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
}

/// The chrome's script calling `hiwaveChrome.<method>` with `text` (a page's title or URL).
fn chrome_update_script(method: &str, text: &str) -> String {
    format!("if(window.hiwaveChrome) {{ hiwaveChrome.{}({}); }}", method, js_string(text))
}

/// The Inspector panel's script for an element picked in the content
/// page: select it by `path` and show `styles_json`.
fn inspector_picked_script(path: &str, styles_json: &str) -> String {
    format!(
        "if (window.hiwaveInspector) {{ window.hiwaveInspector.selectElementByPath({}); window.hiwaveInspector.setElementStyles({}); window.hiwaveInspector.setPickerActive(false); }}",
        js_string(path),
        styles_json
    )
}

fn main() {
    #[cfg(target_os = "linux")]
    let dmabuf_renderer_disabled = apply_gpu_workarounds();
    #[cfg(target_os = "linux")]
    ensure_main_thread_stack_16mib();

    #[cfg(target_os = "linux")]
    if std::env::args().any(|a| a == "--check-stack") {
        let _ = probe_10mib_stack_recursion(10);
        println!("{}", main_thread_stack_size());
        return;
    }

    // Initialize logging with log compatibility. `RUST_LOG` selects the
    // level when it is set; otherwise info, as before.
    tracing_log::LogTracer::init().expect("Failed to set log tracer");
    let subscriber = FmtSubscriber::builder()
        .with_env_filter(rust_log_filter(std::env::var("RUST_LOG").ok().as_deref()))
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to set subscriber");

    info!("Starting HiWave...");
    #[cfg(target_os = "linux")]
    if dmabuf_renderer_disabled {
        info!("NVIDIA proprietary driver detected: WebKit DMA-BUF renderer disabled (set WEBKIT_DISABLE_DMABUF_RENDERER=0 to override)");
    }
    #[cfg(target_os = "linux")]
    info!(
        "Main thread stack: {} MiB",
        main_thread_stack_size() / (1024 * 1024)
    );

    // Native platform mode: Use RustKit for everything (no wry/tao)
    #[cfg(feature = "native-linux")]
    {
        info!("WebView engine: RustKit (native)");
        if let Err(e) = native::run_native() {
            error!("Native browser failed: {}", e);
            std::process::exit(1);
        }
        #[allow(clippy::needless_return)]
        return; // Early return - hybrid mode code is below
    }

    // Everything below is hybrid mode (wry/tao)
    #[cfg(not(feature = "native-linux"))]
    {
    info!("WebView engine: {}", engine_name());

    // Initialize application state
    let state = match AppState::with_defaults() {
        Ok(s) => Arc::new(Mutex::new(s)),
        Err(e) => {
            error!("Failed to initialize app state: {}", e);
            panic!("Failed to initialize app state: {}", e);
        }
    };

    info!("Application state initialized");

    // Track session start
    {
        let s = state.lock().unwrap();
        if let Err(e) = s.analytics.track_session_start() {
            error!("Failed to track session start: {}", e);
        }
    }

    // Create initial tab in the default workspace
    {
        let mut s = state.lock().unwrap();
        let has_tabs = !s.shell.list_tabs(None).is_empty();
        if !has_tabs {
            if let Some(workspace) = s.shell.get_active_workspace() {
                let workspace_id = workspace.id;
                let tab = hiwave_core::types::TabInfo {
                    id: hiwave_core::types::TabId::new(),
                    url: url::Url::parse(NEW_TAB_URL).unwrap(),
                    title: Some("New Tab".to_string()),
                    favicon: None,
                    workspace_id,
                    suspended: false,
                    loading: false,
                    locked: false,
                    last_visited: None,
                };
                if let Err(e) = s.shell.create_tab(tab) {
                    error!("Failed to create initial tab: {}", e);
                } else {
                    info!("Initial tab created");
                }
                if let Err(e) = s.save_workspace_state() {
                    error!("Failed to persist workspace state: {}", e);
                }
            }
        }
    }

    // Create the event loop with custom user events
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    // Create the main window
    let mut window_builder = WindowBuilder::new()
        .with_title("HiWave")
        .with_inner_size(LogicalSize::new(1280.0, 800.0));
    if let Some(icon) = create_window_icon() {
        window_builder = window_builder.with_window_icon(Some(icon));
    }
    let window = window_builder
        .build(&event_loop)
        .expect("Failed to create window");

    let main_window_id = window.id();

    // Initialize platform manager and menu
    let platform = get_platform_manager();
    info!("Platform: {}", platform.platform_name());

    // Create and initialize native menu (required for clipboard on macOS)
    let menu_bar = Menu::new();
    if let Err(e) = platform.initialize_menu(&window, &menu_bar) {
        error!("Failed to initialize menu: {}", e);
        // Continue without menu - clipboard may not work on macOS
    }

    info!("Window created successfully");

    // Window-level shortcuts (focus mode, autofill): GTK runs a toplevel's key-press handler before
    // the focused child, so these keys reach the app whichever view (a page included) has focus, and
    // stop there; the page is not asked to forward them over IPC.
    #[cfg(not(feature = "native-linux"))]
    {
        use gtk::gdk::ModifierType;
        use gtk::prelude::*;
        use tao::platform::unix::WindowExtUnix;
        let key_proxy = proxy.clone();
        let key_state = Arc::clone(&state);
        window.gtk_window().connect_key_press_event(move |_, event| {
            let mods = event.state();
            let name = event.keyval().name().map(|n| n.to_string()).unwrap_or_default();
            let shortcut = window_shortcut(
                &name,
                mods.contains(ModifierType::CONTROL_MASK),
                mods.contains(ModifierType::SHIFT_MASK),
                mods.contains(ModifierType::MOD1_MASK),
            );
            match shortcut {
                Some(WindowShortcut::ToggleFocusMode) => {
                    info!("Shortcut: toggle focus mode");
                    let _ = ipc::commands::handle_message(&key_state, IpcMessage::ToggleFocusMode);
                    let _ = key_proxy.send_event(UserEvent::ToggleFocusMode);
                    gtk::glib::Propagation::Stop
                }
                Some(WindowShortcut::TriggerAutofill) => {
                    info!("Shortcut: autofill");
                    let _ = key_proxy.send_event(UserEvent::TriggerAutofill);
                    gtk::glib::Propagation::Stop
                }
                None => gtk::glib::Propagation::Proceed,
            }
        });
    }

    // Linux: the three webviews (chrome/content/shelf) embed into a shared
    // gtk::Fixed. wry's build_gtk honors each webview's with_bounds — (x,y) via
    // Fixed::put and (w,h) via size_request — and set_bounds repositions within
    // the Fixed on resize. build_as_child (Windows HWND / macOS NSView child)
    // panics on GTK with UnsupportedWindowHandle, so we must use the Unix path.
    let gtk_fixed = {
        use gtk::prelude::*;
        use tao::platform::unix::WindowExtUnix;
        let fixed = gtk::Fixed::new();
        if let Some(vbox) = window.default_vbox() {
            vbox.pack_start(&fixed, true, true, 0);
        } else {
            window.gtk_window().add(&fixed);
        }
        fixed.show_all();
        // Learn where GTK actually put the container (below GNOME's title bar, below a menu bar...) and re-run
        // the layout when it changes. Also logs the geometry once, for diagnosing a layout offset from a log.
        {
            let origin_proxy = proxy.clone();
            fixed.connect_size_allocate(move |_, a| {
                use std::sync::atomic::Ordering::Relaxed;
                let old_x = LAYOUT_ORIGIN_X.swap(a.x(), Relaxed);
                let old_y = LAYOUT_ORIGIN_Y.swap(a.y(), Relaxed);
                let changed = old_x != a.x() || old_y != a.y();
                if changed {
                    tracing::debug!(x = a.x(), y = a.y(), w = a.width(), h = a.height(), "Layout container moved");
                    let _ = origin_proxy.send_event(UserEvent::Relayout);
                }
            });
        }
        fixed
    };

    // Get initial window size for WebView bounds
    let window_size = window.inner_size();
    let scale_factor = window.scale_factor();
    let width = window_size.width as f64 / scale_factor;
    let height = window_size.height as f64 / scale_factor;

    // Track current heights (will be modified dynamically)
    let chrome_height = Arc::new(Mutex::new(CHROME_HEIGHT_DEFAULT as f64));
    let shelf_height = Arc::new(Mutex::new(SHELF_HEIGHT_DEFAULT as f64));
    let inspector_height = Arc::new(Mutex::new(inspector_panel::INSPECTOR_HEIGHT_DEFAULT as f64));
    // Sidebar width: 0.0 = closed, >0 = open with that width
    let sidebar_width_state = Arc::new(Mutex::new(0.0_f64));
    let right_sidebar_open = Arc::new(Mutex::new(false));

    // Calculate initial bounds
    let sidebar_width = *sidebar_width_state.lock().unwrap();
    let content_width = (width - sidebar_width).max(0.0);
    let content_height =
        (height - CHROME_HEIGHT_DEFAULT as f64 - SHELF_HEIGHT_DEFAULT as f64).max(0.0);
    let chrome_bounds = Rect {
        position: LogicalPosition::new(0, 0).into(),
        size: LogicalSize::new(width, height).into(),
    };

    let content_bounds = Rect {
        position: LogicalPosition::new(sidebar_width as i32, CHROME_HEIGHT_DEFAULT as i32).into(),
        size: LogicalSize::new(content_width, content_height).into(),
    };

    let shelf_bounds = Rect {
        position: LogicalPosition::new(
            sidebar_width as i32,
            (height - SHELF_HEIGHT_DEFAULT as f64) as i32,
        )
        .into(),
        size: LogicalSize::new(content_width, SHELF_HEIGHT_DEFAULT as f64).into(),
    };

    // === CHROME WEBVIEW (created first, at top) ===
    let chrome_state = Arc::clone(&state);
    let chrome_proxy = proxy.clone();
    let settings_proxy_for_handler = proxy.clone();
    let chrome_ready_flag = Arc::new(AtomicBool::new(false));

    // The chrome's IPC handler: one closure, called by the WebKit chrome's handler or by the RustKit chrome's
    // pump (S2).
    let chrome_ipc: Rc<dyn Fn(&str)> = Rc::new(move |body: &str| {
        log_ipc("Chrome IPC", body);

            match serde_json::from_str::<IpcMessage>(body) {
                Ok(msg) => match &msg {
                    IpcMessage::Navigate { url } => {
                        let _ = chrome_proxy.send_event(UserEvent::Navigate(url.clone()));
                    }
                    IpcMessage::GoBack => {
                        let _ = chrome_proxy.send_event(UserEvent::GoBack);
                    }
                    IpcMessage::GoForward => {
                        let _ = chrome_proxy.send_event(UserEvent::GoForward);
                    }
                    IpcMessage::Reload => {
                        let _ = chrome_proxy.send_event(UserEvent::Reload);
                    }
                    IpcMessage::Stop => {
                        let _ = chrome_proxy.send_event(UserEvent::Stop);
                    }
                    IpcMessage::FindInPage {
                        ref query,
                        case_sensitive,
                        ref direction,
                    } => {
                        let _ = chrome_proxy.send_event(UserEvent::FindInPage {
                            query: query.clone(),
                            case_sensitive: *case_sensitive,
                            direction: direction.clone(),
                        });
                    }
                    IpcMessage::CreateTab { ref url } => {
                        let nav_url = url.clone();
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("IPC response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        if let Some(u) = nav_url {
                            let _ = chrome_proxy.send_event(UserEvent::Navigate(u));
                        } else {
                            let _ = chrome_proxy.send_event(UserEvent::NewTab);
                        }
                    }
                    IpcMessage::CloseTab { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Close tab response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        // Navigate to the new active tab after closing
                        if let Ok(s) = chrome_state.lock() {
                            if let Some(active_tab) = s.shell.get_active_tab() {
                                let url = active_tab.url.to_string();
                                info!("Navigating to new active tab after close: {}", crate::page_url_for_log(&url));
                                let _ = chrome_proxy.send_event(UserEvent::Navigate(url));
                            }
                        }
                    }
                    IpcMessage::ActivateTab { ref id } => {
                        let _tab_id = id.clone();
                        let already_active = chrome_state
                            .lock()
                            .ok()
                            .and_then(|s| s.shell.get_active_tab().map(|tab| tab.id.0))
                            .map(|active_id| id.parse::<u64>().ok() == Some(active_id))
                            .unwrap_or(false);
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Tab activation response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        if !already_active {
                            if let ipc::IpcResponse::Success { data } = response {
                                if let Some(url) = data.get("url").and_then(|v| v.as_str()) {
                                    info!("Navigating to tab URL: {}", crate::page_url_for_log(&url));
                                    let _ = chrome_proxy
                                        .send_event(UserEvent::Navigate(url.to_string()));
                                }
                            }
                            // Sync zoom level for the newly activated tab
                            let _ = chrome_proxy.send_event(UserEvent::SyncZoomLevel);
                        }
                    }
                    IpcMessage::ExecuteCommand { ref id } => {
                        let _cmd_id = id.clone();
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Command execution response: {}", ipc_log_shape(&body, &response));

                        if let ipc::IpcResponse::Success { data } = response {
                            if let Some(result) = data.get("result") {
                                if let Some(action) = result.get("action").and_then(|a| a.as_str())
                                {
                                    match action {
                                        "navigate" => {
                                            if let Some(url) =
                                                result.get("url").and_then(|u| u.as_str())
                                            {
                                                let _ = chrome_proxy.send_event(
                                                    UserEvent::Navigate(url.to_string()),
                                                );
                                            } else {
                                                let _ = chrome_proxy.send_event(UserEvent::NewTab);
                                            }
                                        }
                                        "reload" => {
                                            let _ = chrome_proxy.send_event(UserEvent::Reload);
                                        }
                                        "go_back" => {
                                            let _ = chrome_proxy.send_event(UserEvent::GoBack);
                                        }
                                        "go_forward" => {
                                            let _ = chrome_proxy.send_event(UserEvent::GoForward);
                                        }
                                        "open_vault" => {
                                            // Vault is now accessed via Settings
                                            let _ = chrome_proxy.send_event(UserEvent::OpenSettings);
                                        }
                                        _ => {
                                            info!("Unhandled action: {}", action);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    IpcMessage::SearchCommands { ref query } => {
                        let _q = query.clone();
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        if let ipc::IpcResponse::Success { data } = response {
                            let json =
                                serde_json::to_string(&data).unwrap_or_else(|_| "[]".to_string());
                            let _ = chrome_proxy.send_event(UserEvent::ShowCommands(json));
                        }
                    }
                    IpcMessage::GetWorkspaces => {
                        let _response = ipc::commands::handle_message(&chrome_state, msg);
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                    }
                    IpcMessage::CreateWorkspace { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Create workspace response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                    }
                    IpcMessage::ActivateWorkspace { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Activate workspace response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let active_url = chrome_state
                            .lock()
                            .ok()
                            .and_then(|s| s.shell.get_active_tab().map(|tab| tab.url.to_string()));
                        if let Some(url) = active_url {
                            let _ = chrome_proxy.send_event(UserEvent::Navigate(url));
                        }
                    }
                    IpcMessage::DeleteWorkspace { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Delete workspace response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                    }
                    IpcMessage::RenameWorkspace { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Rename workspace response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                    }
                    IpcMessage::SetMode { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Set mode response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::OpenSettings => {
                        let _ = chrome_proxy.send_event(UserEvent::OpenSettings);
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Open settings response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::CloseSettings => {
                        let _ = chrome_proxy.send_event(UserEvent::CloseSettings);
                        info!("Close settings requested");
                    }
                    IpcMessage::GetShelf { scope } => {
                        let scope = ShelfScope::from_option(scope.as_deref());
                        let _response = ipc::commands::handle_message(&chrome_state, msg);
                        let _ = chrome_proxy.send_event(UserEvent::SyncShelf(scope));
                    }
                    IpcMessage::AddToShelf { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Add to shelf response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                    }
                    IpcMessage::RestoreFromShelf { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Restore from shelf response: {}", ipc_log_shape(&body, &response));
                        let _ =
                            chrome_proxy.send_event(UserEvent::SyncShelf(ShelfScope::Workspace));
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        if let ipc::IpcResponse::Success { data } = response {
                            if let Some(restored) = data.get("restored") {
                                if let Some(url) = restored.get("url").and_then(|v| v.as_str()) {
                                    let _ = chrome_proxy
                                        .send_event(UserEvent::Navigate(url.to_string()));
                                }
                            }
                        }
                    }
                    IpcMessage::MoveTabToWorkspace { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Move tab response: {}", ipc_log_shape(&body, &response));
                        let _ =
                            chrome_proxy.send_event(UserEvent::SyncShelf(ShelfScope::Workspace));
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                    }
                    IpcMessage::LockPage { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Lock page response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                    }
                    IpcMessage::UnlockPage { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Unlock page response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let _ = chrome_proxy.send_event(UserEvent::SyncWorkspaces);
                    }
                    IpcMessage::GetStaleLocks { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Get stale locks response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::DeleteFromShelf { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Delete from shelf response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::UpdateActiveTabUrl { ref url } => {
                        let _ = chrome_proxy.send_event(UserEvent::UpdateActiveTabUrl(url.clone()));
                    }
                    IpcMessage::SetSidebarOpen { open } => {
                        let _ = chrome_proxy.send_event(UserEvent::SetSidebarOpen(*open));
                    }
                    IpcMessage::SetSidebarWidth { width } => {
                        let _ = chrome_proxy.send_event(UserEvent::SetSidebarWidth(*width));
                    }
                    IpcMessage::SetRightPanelOpen { open } => {
                        let _ = chrome_proxy.send_event(UserEvent::SetRightPanelOpen(*open));
                    }
                    IpcMessage::ExpandChrome => {
                        info!("Expanding chrome (large)");
                        let _ = chrome_proxy.send_event(UserEvent::ExpandChrome);
                    }
                    IpcMessage::ExpandChromeSmall => {
                        info!("Expanding chrome (small)");
                        let _ = chrome_proxy.send_event(UserEvent::ExpandChromeSmall);
                    }
                    IpcMessage::CollapseChrome => {
                        info!("Collapsing chrome");
                        let _ = chrome_proxy.send_event(UserEvent::CollapseChrome);
                    }
                    IpcMessage::ExpandShelf => {
                        info!("Expanding shelf for command palette");
                        let _ = chrome_proxy.send_event(UserEvent::ExpandShelf);
                    }
                    IpcMessage::CollapseShelf => {
                        info!("Collapsing shelf");
                        let _ = chrome_proxy.send_event(UserEvent::CollapseShelf);
                    }
                    IpcMessage::OpenInspector => {
                        let _ = chrome_proxy.send_event(UserEvent::OpenInspector);
                    }
                    IpcMessage::CloseInspector => {
                        let _ = chrome_proxy.send_event(UserEvent::CloseInspector);
                    }
                    IpcMessage::ToggleInspector => {
                        let _ = chrome_proxy.send_event(UserEvent::ToggleInspector);
                    }
                    // Focus mode IPC handlers
                    IpcMessage::EnterFocusMode => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Enter focus mode response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::EnterFocusMode);
                    }
                    IpcMessage::ExitFocusMode => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Exit focus mode response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::ExitFocusMode);
                    }
                    IpcMessage::ToggleFocusMode => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Toggle focus mode response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::ToggleFocusMode);
                    }
                    IpcMessage::GetFocusModeStatus => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Get focus mode status response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncFocusModeStatus);
                    }
                    IpcMessage::GetFocusModeConfig => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Get focus mode config response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::SaveFocusModeConfig { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Save focus mode config response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::FocusScrollProgress { .. } => {
                        let _ = ipc::commands::handle_message(&chrome_state, msg);
                        // No response needed, just update state
                    }
                    IpcMessage::FocusMediaPlaying { .. } => {
                        let _ = ipc::commands::handle_message(&chrome_state, msg);
                        // No response needed, just update state
                    }
                    IpcMessage::FocusPageLoaded { .. } => {
                        let _ = ipc::commands::handle_message(&chrome_state, msg);
                        // No response needed, just update state
                    }
                    IpcMessage::ShowFocusPeek => {
                        let _ = ipc::commands::handle_message(&chrome_state, msg);
                        let _ = chrome_proxy.send_event(UserEvent::ShowFocusPeek);
                    }
                    IpcMessage::HideFocusPeek => {
                        let _ = ipc::commands::handle_message(&chrome_state, msg);
                        let _ = chrome_proxy.send_event(UserEvent::HideFocusPeek);
                    }
                    IpcMessage::AddToFocusBlocklist { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Add to focus blocklist response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::RemoveFromFocusBlocklist { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Remove from focus blocklist response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::BlockAndClose { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Block and close response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncTabs);
                        let _ = chrome_proxy.send_event(UserEvent::SyncBlocklist);
                    }
                    IpcMessage::GetBlocklist => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Get blocklist response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncBlocklist);
                    }
                    IpcMessage::UnblockDomain { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Unblock domain response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncBlocklist);
                    }
                    IpcMessage::GetDownloads => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Get downloads response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::ClearDownloads => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Clear downloads response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncDownloads);
                    }
                    IpcMessage::GetVisitHistory => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Get visit history response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncHistory);
                    }
                    IpcMessage::ClearVisitHistory => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Clear visit history response: {}", ipc_log_shape(&body, &response));
                        let _ = chrome_proxy.send_event(UserEvent::SyncHistory);
                    }
                    IpcMessage::OpenDownload { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Open download response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::ShowDownloadInFolder { .. } => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("Show download response: {}", ipc_log_shape(&body, &response));
                    }
                    IpcMessage::GetSidebarWidth => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        if let ipc::IpcResponse::Success { data } = response {
                            if let Some(width) = data.get("width").and_then(|w| w.as_u64()) {
                                let script = format!(
                                    "if(window.hiwaveChrome && window.hiwaveChrome.setSidebarWidth) {{ window.hiwaveChrome.setSidebarWidth({}); }}",
                                    width
                                );
                                let _ = chrome_proxy.send_event(UserEvent::EvaluateScript(script));
                            }
                        }
                    }
                    IpcMessage::PrintPage => {
                        info!("Print page requested");
                        let _ = chrome_proxy.send_event(UserEvent::PrintPage);
                    }
                    IpcMessage::ZoomIn => {
                        info!("Zoom in requested");
                        let _ = chrome_proxy.send_event(UserEvent::ZoomIn);
                    }
                    IpcMessage::ZoomOut => {
                        info!("Zoom out requested");
                        let _ = chrome_proxy.send_event(UserEvent::ZoomOut);
                    }
                    IpcMessage::ResetZoom => {
                        info!("Reset zoom requested");
                        let _ = chrome_proxy.send_event(UserEvent::ResetZoom);
                    }
                    IpcMessage::TabAudioStateChanged { playing } => {
                        info!("Tab audio state changed: playing={}", playing);
                        let _ = chrome_proxy.send_event(UserEvent::TabAudioStateChanged { playing: *playing });
                    }
                    IpcMessage::ToggleTabMute { ref id } => {
                        info!("Toggle tab mute: {}", id);
                        let _ = chrome_proxy.send_event(UserEvent::ToggleTabMute { id: id.clone() });
                    }
                    IpcMessage::TriggerAutofill => {
                        info!("Trigger autofill requested");
                        let _ = chrome_proxy.send_event(UserEvent::TriggerAutofill);
                    }
                    _ => {
                        let response = ipc::commands::handle_message(&chrome_state, msg);
                        info!("IPC response: {}", ipc_log_shape(&body, &response));
                    }
                },
                Err(e) => {
                    error!("Failed to parse IPC: {}", e);
                }
            }
    });

    // HIWAVE_RUSTKIT_CHROME=1 on an X11 window: the chrome is a RustKit view, built after the page below so it
    // stacks above it. Otherwise (the default) the WebKit chrome below.
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    let rustkit_chrome_view: Option<Arc<webview_rustkit::RustKitView>> =
        if matches!(rustkit_chrome::choose(&window), rustkit_content::Engagement::On) {
            let at = chrome_bounds.position.to_logical::<f64>(1.0);
            let size = chrome_bounds.size.to_logical::<f64>(1.0);
            let bounds = rustkit_viewhost::Bounds::new(at.x as i32, at.y as i32, size.width as u32, size.height as u32);
            let document = rustkit_chrome::html_with_bridge(CHROME_HTML, JS_BRIDGE, rustkit_chrome::OUTSIDE_CLICK);
            // Built here, before the page: if it cannot be built the WebKit chrome below is, as if the flag were
            // off. Its window is raised above the page once the page exists (below).
            match rustkit_chrome::create(&window, bounds, &document) {
                Ok(view) => Some(view),
                Err(why) => {
                    error!("RustKit chrome could not be created, using the WebKit chrome: {why}");
                    None
                }
            }
        } else {
            None
        };
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    let rustkit_chrome_on = rustkit_chrome_view.is_some();
    #[cfg(not(all(target_os = "linux", feature = "rustkit")))]
    let rustkit_chrome_on = false;

    // WRY (WebView2) Chrome WebView creation
    let chrome_webview_wry: Option<wry::WebView> = if rustkit_chrome_on {
        None
    } else {
        Some(WebViewBuilder::new()
        .with_html(CHROME_HTML)
        .with_devtools(cfg!(debug_assertions))
        .with_clipboard(true)
        .with_focused(true)
        .with_initialization_script(JS_BRIDGE)
        .with_bounds(chrome_bounds)
        .with_ipc_handler({
            let handler = Rc::clone(&chrome_ipc);
            move |message| handler(message.body())
        })
        .build_gtk(&gtk_fixed)
        .expect("Failed to create Chrome WebView"))
    };

    // Chrome always uses WRY (WebView2)

    info!(
        "Chrome WebView created (full window, top bar height {}px)",
        CHROME_HEIGHT_DEFAULT
    );

    // === CONTENT WEBVIEW (created second, below chrome) ===
    #[allow(unused_variables)]
    let content_proxy = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy2 = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy3 = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy4 = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy5 = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy_new_window = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy_downloads = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy_downloads_complete = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy_history = proxy.clone();
    #[allow(unused_variables)]
    let content_proxy_ipc = proxy.clone();
    #[allow(unused_variables)]
    let content_state = Arc::clone(&state);
    #[allow(unused_variables)]
    let content_state2 = Arc::clone(&state);
    #[allow(unused_variables)]
    let content_state_page_load = Arc::clone(&state);
    #[allow(unused_variables)]
    let content_state_ipc = Arc::clone(&state);
    #[allow(unused_variables)]
    let content_state_download_start = Arc::clone(&state);
    #[allow(unused_variables)]
    let content_state_download_complete = Arc::clone(&state);
    let initial_url = {
        let s = state.lock().unwrap();
        s.shell
            .get_active_tab()
            .map(|tab| tab.url.to_string())
            .unwrap_or_else(|| NEW_TAB_URL.to_string())
    };
    // The content view's IPC handler is shared: the WebKit view calls it from wry's handler, the RustKit page
    // view (HIWAVE_RUSTKIT_CONTENT=1) from the pump that drains its messages.
    let content_ipc: std::rc::Rc<dyn Fn(&str)> = std::rc::Rc::new(move |body: &str| {
        log_ipc("Content IPC", &body);
            if let Ok(msg) = serde_json::from_str::<IpcMessage>(body) {
                match &msg {
                    IpcMessage::GoBack => {
                        let _ = content_proxy_ipc.send_event(UserEvent::GoBack);
                    }
                    IpcMessage::GoForward => {
                        let _ = content_proxy_ipc.send_event(UserEvent::GoForward);
                    }
                    IpcMessage::CreateTab { url } => {
                        // A page opens a new tab with an http(s) URL or none (Ctrl+T); nothing else.
                        if !content_new_tab_url_allowed(url.as_deref()) {
                            info!("New tab from content with a non-http(s) URL: not served");
                            return;
                        }
                        let _ = ipc::commands::handle_message(&content_state_ipc, msg.clone());
                        let _ = content_proxy_ipc.send_event(UserEvent::SyncTabs);
                        let _ = content_proxy_ipc.send_event(UserEvent::SyncWorkspaces);
                        if let Some(u) = url {
                            let _ = content_proxy_ipc.send_event(UserEvent::Navigate(u.clone()));
                        }
                    }
                    IpcMessage::AddToShelf { tab_id } => {
                        // The context menu shelves the active tab; a page names no other tab.
                        let actual_tab_id = if tab_id == "active" {
                            if let Ok(s) = content_state_ipc.lock() {
                                s.shell.get_active_tab().map(|t| t.id.0.to_string()).unwrap_or_default()
                            } else {
                                String::new()
                            }
                        } else {
                            info!("Add to shelf from content by id: not served");
                            String::new()
                        };

                        if !actual_tab_id.is_empty() {
                            let shelf_msg = IpcMessage::AddToShelf { tab_id: actual_tab_id };
                            let _ = ipc::commands::handle_message(&content_state_ipc, shelf_msg);
                            let _ = content_proxy_ipc.send_event(UserEvent::SyncShelf(crate::ShelfScope::Workspace));
                            let _ = content_proxy_ipc.send_event(UserEvent::SyncTabs);
                        }
                    }
                    IpcMessage::FindInPageResult { result } => {
                        let _ = content_proxy_ipc.send_event(UserEvent::FindInPageResult(result.clone()));
                    }
                    // Inspector data from the injected script in the page (I1b). The page can post
                    // these itself: each payload is read into the shape the injected scripts send,
                    // and one that does not fit is dropped (upstream SEC1b).
                    IpcMessage::InspectorDomTreeResult { tree } => {
                        let Some(tree) = ipc::inspector_payload::dom_tree(tree) else {
                            warn!("inspector: dropped a malformed DOM tree from the content page");
                            return;
                        };
                        let json = serde_json::to_string(&tree).unwrap_or_else(|_| "null".to_string());
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectorSendDomTree(json));
                    }
                    IpcMessage::InspectorElementStylesResult { styles } => {
                        let Some(styles) = ipc::inspector_payload::element_styles(styles) else {
                            warn!("inspector: dropped malformed element styles from the content page");
                            return;
                        };
                        let json = serde_json::to_string(&styles).unwrap_or_else(|_| "null".to_string());
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectorSendElementStyles(json));
                    }
                    IpcMessage::InspectorElementPicked { path, styles } => {
                        let Some(styles) = ipc::inspector_payload::element_styles(styles) else {
                            warn!("inspector: dropped a malformed picked element from the content page");
                            return;
                        };
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectorElementPicked { path: path.clone(), styles });
                    }
                    IpcMessage::InspectorPickerStopped => {
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectorPickerStopped);
                    }
                    IpcMessage::InspectElement { path } => {
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectElement { path: path.clone() });
                    }
                    IpcMessage::InspectorConsoleMessage { entry } => {
                        let Some(entry) = ipc::inspector_payload::console_entry(entry) else {
                            warn!("inspector: dropped a malformed console line from the content page");
                            return;
                        };
                        let json = serde_json::to_string(&entry).unwrap_or_else(|_| "null".to_string());
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectorConsoleMessage(json));
                    }
                    IpcMessage::InspectorConsoleClear => {
                        let _ = content_proxy_ipc.send_event(UserEvent::InspectorConsoleClear);
                    }
                    IpcMessage::TabAudioStateChanged { playing } => {
                        info!("Tab audio state changed from content: playing={}", playing);
                        let _ = content_proxy_ipc.send_event(UserEvent::TabAudioStateChanged { playing: *playing });
                    }
                    IpcMessage::GetCredentialsForAutofill { ref domain } => {
                        // Autofill hardening: credentials are delivered only for a user-requested fill on the
                        // page's own origin. A content page's request gets nothing.
                        let _ = ipc::commands::content_page_autofill(&content_state_ipc, domain);
                    }
                    // Escape forwarded from the content view (context_menu.js): leaving focus mode is the only
                    // focus-mode request a page is served; entering or toggling is a window-level shortcut.
                    IpcMessage::ExitFocusMode => {
                        info!("Exit focus mode from content WebView");
                        let _ = ipc::commands::handle_message(&content_state_ipc, msg.clone());
                        let _ = content_proxy_ipc.send_event(UserEvent::ExitFocusMode);
                    }
                    // UI shortcuts forwarded from content WebView
                    IpcMessage::CloseTab { id } => {
                        // The context menu closes the active tab; a page names no other tab.
                        if id == "active" {
                            info!("Close tab from content: active");
                            let _ = content_proxy_ipc.send_event(UserEvent::CloseActiveTab);
                        } else {
                            info!("Close tab from content by id: not served");
                        }
                    }
                    IpcMessage::Refresh => {
                        info!("Refresh from content");
                        let _ = content_proxy_ipc.send_event(UserEvent::Reload);
                    }
                    IpcMessage::FocusAddressBar => {
                        info!("Focus address bar from content");
                        let _ = content_proxy_ipc.send_event(UserEvent::FocusAddressBar);
                    }
                    IpcMessage::OpenFind => {
                        info!("Open find from content");
                        let _ = content_proxy_ipc.send_event(UserEvent::OpenFind);
                    }
                    IpcMessage::ToggleSidebar => {
                        info!("Toggle sidebar from content");
                        let _ = content_proxy_ipc.send_event(UserEvent::ToggleSidebar);
                    }
                    IpcMessage::OpenCommandPalette => {
                        info!("Open command palette from content");
                        let _ = content_proxy_ipc.send_event(UserEvent::OpenCommandPalette);
                    }
                    IpcMessage::ActivateTabByIndex { index } => {
                        info!("Activate tab by index from content: {}", index);
                        let _ = content_proxy_ipc.send_event(UserEvent::ActivateTabByIndex { index: *index });
                    }
                    // SEC3 content analytics arm: a content page gets no analytics, can reset none, and is sent
                    // nothing back.
                    IpcMessage::GetTodayStats
                    | IpcMessage::GetWeeklyReport
                    | IpcMessage::GetMonthlyReport
                    | IpcMessage::GetCustomReport { .. }
                    | IpcMessage::GetTopDomains { .. }
                    | IpcMessage::GetWorkspaceStats
                    | IpcMessage::ResetAnalyticsData
                    | IpcMessage::ExportAnalyticsData { .. } => {
                        let _ = ipc::commands::content_page_analytics(&content_state_ipc);
                    }
                    _ => {}
                }
            }
    });

    // Which engine draws the page: WebKit unless HIWAVE_RUSTKIT_CONTENT=1 engages the RustKit view on an X11
    // window. With the variable unset nothing RustKit is created.
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    let rustkit_content_view: Option<(Arc<webview_rustkit::RustKitView>, std::time::Instant)> =
        match rustkit_content::choose(&window) {
            rustkit_content::Engagement::On => {
                let b = rustkit_viewhost::Bounds {
                    x: content_bounds.position.to_logical::<f64>(1.0).x as i32,
                    y: content_bounds.position.to_logical::<f64>(1.0).y as i32,
                    width: content_bounds.size.to_logical::<f64>(1.0).width as u32,
                    height: content_bounds.size.to_logical::<f64>(1.0).height as u32,
                };
                match rustkit_content::create(&window, b, None) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        error!("RustKit content view failed ({e}); using the WebKit page view");
                        None
                    }
                }
            }
            _ => None,
        };
    #[cfg(not(all(target_os = "linux", feature = "rustkit")))]
    let rustkit_content_view: Option<()> = None;

    // WRY (WebView2) Content WebView creation. Skipped when the RustKit page view is engaged.
    let content_webview_wry: Option<wry::WebView> = if rustkit_content_view.is_some() {
        None
    } else {
        // The start page is the app's own load (it commits as about:blank).
        mark_app_load();
        Some(WebViewBuilder::new()
        .with_html(ABOUT_HTML)
        .with_devtools(cfg!(debug_assertions))
        .with_clipboard(true)
        .with_focused(true)
        .with_bounds(content_bounds)
        .with_navigation_handler(move |url| {
            info!("Content navigating to: {}", crate::page_url_for_log(&url));
            if !page_navigation_allowed(&url) {
                info!("Content navigation to a browser page: refused");
                let _ = content_proxy.send_event(UserEvent::ReportRefusedNavigation(url));
                return false;
            }

            // This handler is called for every frame's navigation, an iframe's too, with only its
            // URL: it decides whether the navigation may go ahead, nothing more. The address bar,
            // the tab's URL and the loading state follow the main frame's committed load (the
            // page-load handler below).
            if is_new_tab_url(&url) {
                let _ = content_proxy.send_event(UserEvent::SyncShieldStats);
                return true;
            }
            if let Ok(s) = content_state.lock() {
                if s.shield.is_enabled() {
                    if let Ok(parsed_url) = url::Url::parse(&url) {
                        if s.shield
                            .should_block(&parsed_url, &parsed_url, ResourceType::Document)
                        {
                            // Track blocked navigation as both tracker and ad
                            if let Some(domain) = parsed_url.host_str() {
                                let workspace_id = s.shell.get_active_workspace()
                                    .map(|w| w.id.0.to_string());
                                let _ = s.analytics.track_tracker_blocked(
                                    domain,
                                    workspace_id.as_deref()
                                );
                                let _ = s.analytics.track_ad_blocked(
                                    domain,
                                    workspace_id.as_deref()
                                );
                            }
                            let _ = content_proxy.send_event(UserEvent::SyncShieldStats);
                            return false;
                        }
                    }
                }
            }
            let _ = content_proxy.send_event(UserEvent::SyncShieldStats);
            true
        })
        .with_on_page_load_handler(move |event, url| match event {
            wry::PageLoadEvent::Started => {
                // The MAIN frame committed `url` (wry: load-changed Committed, webview.uri()).
                let _ = content_proxy2.send_event(UserEvent::SetLoading(true));
                let current_tab_is_internal = content_state_page_load
                    .lock()
                    .ok()
                    .and_then(|s| s.shell.get_active_tab().map(|tab| {
                        let url_str = tab.url.to_string();
                        is_about_url(&url_str) || url_str == NEW_TAB_URL
                    }))
                    .unwrap_or(false);
                match main_frame_url_update(&url, current_tab_is_internal, take_app_load(&url)) {
                    TabUrlUpdate::NewTab { store } => {
                        if store {
                            let _ = content_proxy2.send_event(UserEvent::UpdateActiveTabUrl(NEW_TAB_URL.to_string()));
                        }
                        let _ = content_proxy2.send_event(UserEvent::UpdateUrl(String::new()));
                    }
                    TabUrlUpdate::Page(page) => {
                        let _ = content_proxy2.send_event(UserEvent::UpdateUrl(page.clone()));
                        let _ = content_proxy2.send_event(UserEvent::UpdateActiveTabUrl(page));
                    }
                }
            }
            wry::PageLoadEvent::Finished => {
                let _ = content_proxy2.send_event(UserEvent::SetLoading(false));
                let _ = content_proxy2.send_event(UserEvent::SyncShieldStats);
                let _ = content_proxy_history.send_event(UserEvent::RecordVisit {
                    url: url.to_string(),
                });
            }
        })
        .with_document_title_changed_handler(move |title| {
            let _ = content_proxy3.send_event(UserEvent::UpdateTitle(title));
        })
        .with_download_started_handler(move |url, suggested_path| {
            if let Ok(mut s) = content_state_download_start.lock() {
                let target_path = s.prepare_download_path(&url);
                *suggested_path = target_path.clone();
                s.register_download(&url, &target_path);
                if let Err(err) = s.save_workspace_state() {
                    error!("Failed to persist download state: {}", err);
                }
            }
            let _ = content_proxy_downloads.send_event(UserEvent::SyncDownloads);
            true
        })
        .with_download_completed_handler(move |url, path, success| {
            if let Ok(mut s) = content_state_download_complete.lock() {
                s.finalize_download(&url, path, success);
                if let Err(err) = s.save_workspace_state() {
                    error!("Failed to persist download state: {}", err);
                }
            }
            let _ = content_proxy_downloads_complete.send_event(UserEvent::SyncDownloads);
        })
        .with_new_window_req_handler(move |url| {
            // Allow Cloudflare challenges to stay as iframes (required for verification)
            if url.contains("challenges.cloudflare.com") || url.contains("turnstile") {
                info!("Allowing Cloudflare challenge iframe: {}", crate::page_url_for_log(&url));
                return true; // Allow the iframe/popup to render in-place
            }

            // Allow Proton authentication popups/iframes
            if url.contains("proton.me") || url.contains("protonmail.com") {
                info!("Allowing Proton auth iframe: {}", crate::page_url_for_log(&url));
                return true; // Allow the iframe/popup to render in-place
            }

            // Allow about:srcdoc and about:blank - these are inline documents used by
            // verification widgets (like Cloudflare Turnstile) and don't navigate away
            if url == "about:srcdoc" || url == "about:blank" {
                return true; // Allow inline iframes
            }

            // Filter out unwanted popup/iframe requests using platform layer
            if platform::is_definitely_popup(&url) {
                info!("New window request blocked (filtered): {}", crate::page_url_for_log(&url));
                return false;
            }

            // Check Shield for ad/tracker blocking and user blocklist
            if let Ok(mut s) = content_state2.lock() {
                // Check user blocklist first (hot-editable)
                if s.user_blocklist.should_block(&url) {
                    info!("New window request blocked (user blocklist): {}", crate::page_url_for_log(&url));
                    // Increment Shield counters so all blocks show in UI
                    s.shield.increment_block_count();
                    // Track user blocklist blocks in analytics as popup, tracker, and ad
                    if let Ok(parsed_url) = url::Url::parse(&url) {
                        if let Some(domain) = parsed_url.host_str() {
                            let workspace_id = s.shell.get_active_workspace()
                                .map(|w| w.id.0.to_string());
                            let _ = s.analytics.track_popup_blocked(
                                domain,
                                workspace_id.as_deref()
                            );
                            let _ = s.analytics.track_tracker_blocked(
                                domain,
                                workspace_id.as_deref()
                            );
                            let _ = s.analytics.track_ad_blocked(
                                domain,
                                workspace_id.as_deref()
                            );
                        }
                    }
                    drop(s);
                    let _ = content_proxy_new_window.send_event(UserEvent::SyncShieldStats);
                    return false;
                }

                // Check popup flood protection (rate limiting + duplicate detection)
                if !s.should_allow_popup(&url) {
                    // Warning already logged by should_allow_popup
                    // Increment Shield counters so all blocks show in UI
                    s.shield.increment_block_count();
                    // Track flood protection blocks in analytics as popup, tracker, and ad
                    if let Ok(parsed_url) = url::Url::parse(&url) {
                        if let Some(domain) = parsed_url.host_str() {
                            let workspace_id = s.shell.get_active_workspace()
                                .map(|w| w.id.0.to_string());
                            let _ = s.analytics.track_popup_blocked(
                                domain,
                                workspace_id.as_deref()
                            );
                            let _ = s.analytics.track_tracker_blocked(
                                domain,
                                workspace_id.as_deref()
                            );
                            let _ = s.analytics.track_ad_blocked(
                                domain,
                                workspace_id.as_deref()
                            );
                        }
                    }
                    drop(s);
                    let _ = content_proxy_new_window.send_event(UserEvent::SyncShieldStats);
                    return false;
                }

                // Check Shield (EasyList)
                if s.shield.is_enabled() {
                    if let Ok(parsed_url) = url::Url::parse(&url) {
                        // Use SubDocument type for popups/iframes
                        if s.shield
                            .should_block(&parsed_url, &parsed_url, ResourceType::Other)
                        {
                            // Track blocked popup in analytics as popup, tracker, and ad
                            if let Some(domain) = parsed_url.host_str() {
                                let workspace_id = s.shell.get_active_workspace()
                                    .map(|w| w.id.0.to_string());
                                let _ = s.analytics.track_popup_blocked(
                                    domain,
                                    workspace_id.as_deref()
                                );
                                let _ = s.analytics.track_tracker_blocked(
                                    domain,
                                    workspace_id.as_deref()
                                );
                                let _ = s.analytics.track_ad_blocked(
                                    domain,
                                    workspace_id.as_deref()
                                );
                            }
                            info!("New window request blocked (Shield): {}", crate::page_url_for_log(&url));
                            drop(s);
                            let _ = content_proxy_new_window.send_event(UserEvent::SyncShieldStats);
                            return false;
                        }
                    }
                }
            }

            // A page opens a tab only at an http(s) URL, as with its CreateTab command; the
            // browser's own pages (hiwave:) and other schemes open only from the browser's UI.
            if !content_new_tab_url_allowed(Some(&url)) {
                info!("New window request with a non-http(s) URL: not served");
                return false;
            }

            info!("New window request -> new tab: {}", crate::page_url_for_log(&url));
            let response = ipc::commands::handle_message(
                &content_state2,
                IpcMessage::CreateTab {
                    url: Some(url.clone()),
                },
            );
            info!("New window redirected to tab: {}", if matches!(response, ipc::IpcResponse::Success { .. }) { "success" } else { "error" });
            let _ = content_proxy4.send_event(UserEvent::SyncTabs);
            let _ = content_proxy4.send_event(UserEvent::Navigate(url));
            false
        })
        .with_ipc_handler({
            let handler = std::rc::Rc::clone(&content_ipc);
            move |message| handler(message.body())
        })
        .build_gtk(&gtk_fixed)
        .expect("Failed to create Content WebView"))
    };

    // The content view, from whichever engine was chosen above.
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    let content_created_at = rustkit_content_view.as_ref().map(|(_, at)| *at);
    let content_webview: UnifiedContentWebView = match (content_webview_wry, rustkit_content_view) {
        (Some(w), _) => UnifiedContentWebView::Wry(Arc::new(w)),
        #[cfg(all(target_os = "linux", feature = "rustkit"))]
        (None, Some((v, _))) => UnifiedContentWebView::RustKit(v),
        _ => unreachable!("the content view is WebKit unless the RustKit view was built"),
    };

    // Initial page. The WebKit view starts on ABOUT_HTML (with_html above); the RustKit view has to be told.
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    if let UnifiedContentWebView::RustKit(_) = content_webview {
        let issued = std::time::Instant::now();
        content_webview.load_html(ABOUT_HTML);
        info!(
            since_view_created_ms = content_created_at.map(|t| t.elapsed().as_millis() as u64).unwrap_or(0),
            call_ms = issued.elapsed().as_millis() as u64,
            "RustKit content view: initial load_html issued"
        );
    }
    // A restored browser page opens through the browser's own loader: load_url would reach the
    // navigation handler, which refuses a hiwave: URL as a page's navigation.
    match start_page(&initial_url) {
        StartPage::Keep => {}
        StartPage::About => {
            let _ = proxy.send_event(UserEvent::UpdateUrl(ABOUT_URL.to_string()));
            let _ = proxy.send_event(UserEvent::LoadAboutPage);
        }
        StartPage::Report => {
            let _ = proxy.send_event(UserEvent::UpdateUrl(REPORT_URL.to_string()));
            let _ = proxy.send_event(UserEvent::LoadReportPage);
        }
        StartPage::Url => content_webview.load_url(&initial_url),
    }
    info!("Content WebView created (content area)");

    // === SHELF WEBVIEW (created third, at bottom) ===
    // Only create shelf webview if height > 0 (zero-height windows are invalid in X11)
    let shelf_webview: Option<wry::WebView> = if SHELF_HEIGHT_DEFAULT > 0 {
        let shelf_proxy = proxy.clone();
        let shelf_state = Arc::clone(&state);

        // WRY (WebView2) Shelf WebView creation
        let webview = WebViewBuilder::new()
            .with_html(SHELF_HTML)
            .with_devtools(cfg!(debug_assertions))
            .with_clipboard(true)
            .with_initialization_script(JS_BRIDGE)
            .with_bounds(shelf_bounds)
            .with_ipc_handler(move |message| {
                let body = message.body();
                log_ipc("Shelf IPC", &body);

                match serde_json::from_str::<IpcMessage>(body) {
                    Ok(msg) => {
                        match &msg {
                            IpcMessage::CollapseShelf => {
                                let _ = shelf_proxy.send_event(UserEvent::CollapseShelf);
                            }
                            IpcMessage::SearchCommands { ref query } => {
                                let _q = query.clone();
                                let response = ipc::commands::handle_message(&shelf_state, msg);
                                if let ipc::IpcResponse::Success { data } = response {
                                    let json = serde_json::to_string(&data)
                                        .unwrap_or_else(|_| "[]".to_string());
                                    let _ = shelf_proxy.send_event(UserEvent::ShowCommands(json));
                                }
                            }
                            IpcMessage::ExecuteCommand { ref id } => {
                                let _cmd_id = id.clone();
                                let response = ipc::commands::handle_message(&shelf_state, msg);
                                info!("Command execution response: {}", ipc_log_shape(&body, &response));
                                // Close shelf after executing command
                                let _ = shelf_proxy.send_event(UserEvent::CollapseShelf);

                                if let ipc::IpcResponse::Success { data } = response {
                                    if let Some(result) = data.get("result") {
                                        if let Some(action) =
                                            result.get("action").and_then(|a| a.as_str())
                                        {
                                            match action {
                                                "navigate" => {
                                                    if let Some(url) =
                                                        result.get("url").and_then(|u| u.as_str())
                                                    {
                                                        let _ = shelf_proxy.send_event(
                                                            UserEvent::Navigate(url.to_string()),
                                                        );
                                                    } else {
                                                        let _ =
                                                            shelf_proxy.send_event(UserEvent::NewTab);
                                                    }
                                                }
                                                "reload" => {
                                                    let _ = shelf_proxy.send_event(UserEvent::Reload);
                                                }
                                                "go_back" => {
                                                    let _ = shelf_proxy.send_event(UserEvent::GoBack);
                                                }
                                                "go_forward" => {
                                                    let _ =
                                                        shelf_proxy.send_event(UserEvent::GoForward);
                                                }
                                                "open_vault" => {
                                                    // Vault is now accessed via Settings
                                                    let _ =
                                                        shelf_proxy.send_event(UserEvent::OpenSettings);
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                            }
                            IpcMessage::Navigate { url } => {
                                let _ = shelf_proxy.send_event(UserEvent::Navigate(url.clone()));
                                let _ = shelf_proxy.send_event(UserEvent::CollapseShelf);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        error!("Failed to parse Shelf IPC: {}", e);
                    }
                }
            })
            .with_ipc_handler(move |message| {
                if let Ok(IpcMessage::FindInPageResult { result }) =
                    serde_json::from_str::<IpcMessage>(message.body())
                {
                    let _ = content_proxy5.send_event(UserEvent::FindInPageResult(result));
                }
            })
            .build_gtk(&gtk_fixed)
            .expect("Failed to create Shelf WebView");

        info!("Shelf WebView created (bottom, starts hidden)");
        Some(webview)
    } else {
        info!("Shelf WebView skipped (height is 0)");
        None
    };

    info!("WebView architecture initialized");

    // Check for debug mode via environment variable
    if std::env::var("HIWAVE_DEBUG")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        info!("Debug mode enabled via HIWAVE_DEBUG=1");
        if let Some(ref chrome) = chrome_webview_wry {
            let _ = chrome.evaluate_script("window.enableDebugMode && window.enableDebugMode();");
        }
    }

    // Store WebViews in Arcs for event loop access
    // The chrome and the shelf behind the ChromeUi seam (S1b): WebKit is the only backend, so every call goes
    // where it went before.
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    let chrome_webview: chrome_ui::ChromeView = match (chrome_webview_wry, rustkit_chrome_view) {
        (Some(w), _) => chrome_ui::ChromeView::WebKit(Arc::new(w)),
        (None, Some(v)) => chrome_ui::ChromeView::RustKit(v),
        _ => unreachable!("the chrome is WebKit unless the RustKit chrome was built"),
    };
    #[cfg(not(all(target_os = "linux", feature = "rustkit")))]
    let chrome_webview: chrome_ui::ChromeView = chrome_ui::ChromeView::WebKit(Arc::new(
        chrome_webview_wry.expect("the WebKit chrome is built unless the RustKit chrome was requested"),
    ));
    let shelf_webview: Option<chrome_ui::ChromeView> = shelf_webview.map(|v| chrome_ui::ChromeView::WebKit(Arc::new(v)));

    let chrome_for_events: chrome_ui::ChromeView = chrome_webview.clone();
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    if let chrome_ui::ChromeView::RustKit(ref view) = chrome_webview {
        rustkit_chrome::install_pump(Arc::clone(view), Rc::clone(&chrome_ipc));
        view.raise();
        // A WebKit page under the RustKit chrome gives the keyboard back to the window when it is clicked.
        if let UnifiedContentWebView::Wry(ref page) = content_webview {
            rustkit_chrome::return_focus_on_page_click(page, Arc::clone(view), &window);
        }
        // The first region, before the first layout turn: the window less the page column.
        chrome_ui::ChromeUi::compose(&chrome_webview, width, height, (sidebar_width, CHROME_HEIGHT_DEFAULT as f64, content_width, (height - CHROME_HEIGHT_DEFAULT as f64).max(0.0)));
    }
    let shelf_for_events: Option<chrome_ui::ChromeView> = shelf_webview.clone();

    // The Inspector panel (I1a): a view below the page, built on its first open. RustKit when the chrome is, so
    // the panel and the chrome stay one engine; WebKit otherwise.
    let inspector_height_for_events = Arc::clone(&inspector_height);
    let inspector_ipc: Rc<dyn Fn(&str)> = {
        let inspector_proxy = proxy.clone();
        Rc::new(move |body: &str| {
            log_ipc("Inspector IPC", body);
            handle_inspector_ipc(body, &inspector_proxy);
        })
    };
    let inspector_for_events = Rc::new(inspector_panel::InspectorPanel::new(INSPECTOR_HTML, {
        let gtk_fixed = gtk_fixed.clone();
        let ipc = Rc::clone(&inspector_ipc);
        move |window, rect| build_inspector_view(window, &gtk_fixed, &ipc, rustkit_chrome_on, rect)
    }));
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    let devtools: DevtoolsHandle = Rc::new(std::cell::RefCell::new(devtools_panel::Devtools::default()));
    #[cfg(not(all(target_os = "linux", feature = "rustkit")))]
    let devtools: DevtoolsHandle = ();

    let content_for_events: UnifiedContentWebView = content_webview.clone();
    // The RustKit page view is driven from the GTK main loop (its clock, ipc, navigation, title/url, a frame).
    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    if let UnifiedContentWebView::RustKit(ref view) = content_webview {
        // The Inspector's native path rides the page's pump: its answers each turn, and its element picker takes
        // the page's pointer input while it is on.
        let devtools_turn = Rc::clone(&devtools);
        let inspector_turn = Rc::clone(&inspector_for_events);
        let devtools_pick = Rc::clone(&devtools);
        let inspector_pick = Rc::clone(&inspector_for_events);
        rustkit_content::install_pump(
            Arc::clone(view),
            proxy.clone(),
            std::rc::Rc::clone(&content_ipc),
            content_created_at.unwrap_or_else(std::time::Instant::now),
            rustkit_content::PumpHooks {
                on_turn: Box::new(move |view| devtools_turn.borrow_mut().pump(view, &*inspector_turn)),
                input_filter: Box::new(move |view, delivery| {
                    use webview_rustkit::X11Delivery;
                    let mut dev = devtools_pick.borrow_mut();
                    if !dev.picking() {
                        return false;
                    }
                    match delivery {
                        X11Delivery::MouseMove { x, y } => dev.pick_hover(view, &*inspector_pick, Some((*x, *y))),
                        X11Delivery::MouseLeave => dev.pick_hover(view, &*inspector_pick, None),
                        X11Delivery::MouseUp { x, y } => dev.pick_at(view, &*inspector_pick, *x, *y),
                        X11Delivery::KeyDown { vk: 0x1B, .. } => dev.stop_picker(view, &*inspector_pick),
                        X11Delivery::MouseDown { .. } => {}
                        _ => return false,
                    }
                    true
                }),
            },
        );
    }
    let state_for_events = Arc::clone(&state);
    let chrome_height_for_events = Arc::clone(&chrome_height);
    let shelf_height_for_events = Arc::clone(&shelf_height);
    let sidebar_width_for_events = Arc::clone(&sidebar_width_state);
    let right_sidebar_open_for_events = Arc::clone(&right_sidebar_open);
    let _chrome_ready_flag_for_events = Arc::clone(&chrome_ready_flag);
    let focus_state_for_events = Arc::clone(&state);

    // Trigger initial state sync
    let init_proxy = proxy.clone();
    std::thread::spawn(move || {
        // Short delay to ensure event loop is running
        std::thread::sleep(std::time::Duration::from_millis(100));

        // Wait a bit for views to activate before syncing data
        {
            std::thread::sleep(std::time::Duration::from_millis(400));
            let _ = init_proxy.send_event(UserEvent::SyncTabs);
            let _ = init_proxy.send_event(UserEvent::SyncWorkspaces);
            let _ = init_proxy.send_event(UserEvent::SyncDownloads);
            let _ = init_proxy.send_event(UserEvent::SyncShelf(ShelfScope::Workspace));
            let _ = init_proxy.send_event(UserEvent::SyncHistory);
        }
    });

    // Track settings window (created on demand)
    let settings_window: Arc<Mutex<Option<(tao::window::Window, HiWaveWebView)>>> =
        Arc::new(Mutex::new(None));
    let settings_window_for_events = Arc::clone(&settings_window);

    // Spawn background decay ticker thread
    let decay_proxy = proxy.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
        let _ = decay_proxy.send_event(UserEvent::DecayTick);
    });
    info!("Started background decay ticker");

    // Spawn focus mode auto-trigger checker thread
    let focus_proxy = proxy.clone();
    std::thread::spawn(move || {
        loop {
            // Check every 5 seconds for auto-trigger conditions
            std::thread::sleep(std::time::Duration::from_secs(5));
            let _ = focus_proxy.send_event(UserEvent::FocusAutoTriggerCheck);
        }
    });
    info!("Started focus mode auto-trigger checker");

    // Run the event loop
    event_loop.run(move |event, event_loop_target, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                window_id,
                ..
            } => {
                if window_id == main_window_id {
                    // Main window close - save state and exit
                    info!("Main window close requested, shutting down...");
                    if let Ok(s) = state_for_events.lock() {
                        // Track session end
                        let session_duration = s.session_start_time.elapsed().as_secs() as i64;
                        if let Err(e) = s.analytics.track_session_end(session_duration) {
                            error!("Failed to track session end: {}", e);
                        }

                        if let Err(e) = s.save_workspace_state() {
                            error!("Failed to persist workspace state: {}", e);
                        }
                    }
                    *control_flow = ControlFlow::Exit;
                } else {
                    // Secondary window close (settings, vault) - just hide it
                    info!("Secondary window close requested, hiding...");

                    // Check if it's the settings window
                    if let Ok(settings_guard) = settings_window_for_events.lock() {
                        if let Some((ref settings_win, _)) = *settings_guard {
                            if settings_win.id() == window_id {
                                settings_win.set_visible(false);
                            }
                        }
                    }

                }
            }
            // Minimised, restored, hidden or shown again: the main window's pages hear it. A
            // minimise and a restore each move the focus, and a restore resizes.
            #[cfg(feature = "rustkit")]
            Event::WindowEvent {
                event: WindowEvent::Focused(_),
                window_id,
                ..
            } if window_id == main_window_id => {
                sync_page_visibility(&window, &chrome_for_events, &content_for_events);
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_new_size),
                ..
            } => {
                #[cfg(feature = "rustkit")]
                sync_page_visibility(&window, &chrome_for_events, &content_for_events);
                let ch = *chrome_height_for_events.lock().unwrap();
                let sh = *shelf_height_for_events.lock().unwrap();
                let sw = *sidebar_width_for_events.lock().unwrap();
                let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                apply_layout(
                    &window,
                    &chrome_for_events,
                    &content_for_events,
                    shelf_for_events.as_ref(),
                    &inspector_for_events,
                    ch,
                    sh,
                    *inspector_height_for_events.lock().unwrap(),
                    sw,
                    right_sidebar_open,
                );
            }
            Event::UserEvent(user_event) => {
                match user_event {
                    UserEvent::Relayout => {
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sw = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sw,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::Navigate(url) => {
                        info!("Navigating content to: {}", crate::page_url_for_log(&url));
                        if is_new_tab_url(&url) {
                            load_new_tab(&content_for_events);
                            chrome_for_events.evaluate_script(
                                "if(window.hiwaveChrome) { hiwaveChrome.updateUrl(''); }",
                            );
                            sync_shield_to_chrome(&state_for_events, &chrome_for_events);
                        } else if is_about_url(&url) {
                            // Update tab URL BEFORE loading the about page
                            // This prevents the data URL from overwriting the tab URL
                            if let Ok(mut s) = state_for_events.lock() {
                                if let Some(tab_id) = s.shell.get_active_tab().map(|tab| tab.id) {
                                    if let Ok(parsed_url) = url::Url::parse(ABOUT_URL) {
                                        let _ = s.shell.update_tab_url(tab_id, parsed_url);
                                    }
                                }
                            }
                            load_about_page(&content_for_events);
                            chrome_for_events.evaluate_script(
                                "if(window.hiwaveChrome) { hiwaveChrome.updateUrl('hiwave://about'); }",
                            );
                        } else if is_report_url(&url) {
                            // Update tab URL BEFORE loading the report page
                            if let Ok(mut s) = state_for_events.lock() {
                                if let Some(tab_id) = s.shell.get_active_tab().map(|tab| tab.id) {
                                    if let Ok(parsed_url) = url::Url::parse(REPORT_URL) {
                                        let _ = s.shell.update_tab_url(tab_id, parsed_url);
                                    }
                                }
                            }
                            load_report_page(&content_for_events);
                            chrome_for_events.evaluate_script(
                                "if(window.hiwaveChrome) { hiwaveChrome.updateUrl('hiwave://report'); }",
                            );
                        } else if !url.starts_with("about:") && !url.starts_with("hiwave://") {
                            // Skip about: URLs (about:blank, about:srcdoc, etc.)
                            // Skip hiwave:// protocol URLs (handled above if recognized, otherwise ignore)
                            let full_url = if url.starts_with("http://") || url.starts_with("https://") {
                                url
                            } else if url.contains('.') {
                                format!("https://{}", url)
                            } else {
                                format!("https://duckduckgo.com/?q={}", urlencoding::encode(&url))
                            };
                            content_for_events.load_url(&full_url);
                            let script = chrome_update_script("updateUrl", &full_url);
                            chrome_for_events.evaluate_script(&script);
                        }
                    }
                    UserEvent::GoBack => {
                        info!("Going back");
                        content_for_events.evaluate_script("history.back();");
                    }
                    UserEvent::GoForward => {
                        info!("Going forward");
                        content_for_events.evaluate_script("history.forward();");
                    }
                    UserEvent::Reload => {
                        info!("Reloading");
                        content_for_events.evaluate_script("location.reload();");
                    }
                    UserEvent::Stop => {
                        info!("Stopping load");
                        content_for_events.evaluate_script("window.stop();");
                    }
                    UserEvent::NewTab => {
                        info!("New tab - navigating to start page");
                        load_new_tab(&content_for_events);
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { hiwaveChrome.updateUrl(''); }",
                        );
                        sync_shield_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::UpdateTitle(title) => {
                        let trimmed = title.trim();
                        // Only update title if non-empty to prevent reverting to "New Tab"
                        // during navigation when the document title temporarily becomes empty
                        if !trimmed.is_empty() {
                            if let Ok(mut s) = state_for_events.lock() {
                                let active_tab_id = s.shell.get_active_tab().map(|tab| tab.id);
                                if let Some(tab_id) = active_tab_id {
                                    if let Some(tab) = s.shell.get_tab_mut(tab_id) {
                                        tab.title = Some(trimmed.to_string());
                                    }
                                }
                                if let Err(e) = s.save_workspace_state() {
                                    error!("Failed to persist workspace state: {}", e);
                                }
                            }
                            let script = chrome_update_script("updateTitle", &title);
                            chrome_for_events.evaluate_script(&script);
                            sync_tabs_to_chrome(&state_for_events, &chrome_for_events);
                            // Also sync workspaces so sidebar tab info updates
                            sync_workspaces_to_chrome(&state_for_events, &chrome_for_events);
                        }
                    }
                    UserEvent::UpdateUrl(url) => {
                        let script = chrome_update_script("updateUrl", &url);
                        chrome_for_events.evaluate_script(&script);
                        sync_shield_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::UpdateActiveTabUrl(url) => {
                        if let Ok(mut s) = state_for_events.lock() {
                            let active_tab_id = s.shell.get_active_tab().map(|tab| tab.id);
                            if let Some(tab_id) = active_tab_id {
                                if let Ok(parsed_url) = url::Url::parse(&url) {
                            if let Err(e) = s.shell.update_tab_url(tab_id, parsed_url) {
                                error!("Failed to update tab URL: {}", e);
                            }
                            if let Err(e) = s.touch_tab(tab_id) {
                                error!("Failed to record tab visit: {}", e);
                            }
                                }
                                if let Err(e) = s.save_workspace_state() {
                                    error!("Failed to persist workspace state: {}", e);
                                }
                            }
                        }
                        sync_tabs_to_chrome(&state_for_events, &chrome_for_events);
                        // Also sync workspaces so sidebar tab info updates
                        sync_workspaces_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::NavigationStateChanged { can_go_back, can_go_forward } => {
                        let script = format!(
                            "if(window.hiwaveChrome) {{ hiwaveChrome.updateNavState({}, {}); }}",
                            can_go_back, can_go_forward
                        );
                        chrome_for_events.evaluate_script(&script);
                    }
                    UserEvent::SetLoading(loading) => {
                        let script = format!(
                            "if(window.hiwaveChrome) {{ hiwaveChrome.setLoading({}); }}",
                            loading
                        );
                        chrome_for_events.evaluate_script(&script);
                        // Inject context menu handler and audio detector when page finishes loading
                        if !loading {
                            info!("Injecting context menu handler into content webview");
                            content_for_events.evaluate_script(CONTEXT_MENU_HELPER);
                            info!("Injecting audio detector into content webview");
                            content_for_events.evaluate_script(AUDIO_DETECTOR);
                            info!("Injecting autofill helper into content webview");
                            content_for_events.evaluate_script(AUTOFILL_HELPER);
                        }
                    }
                    UserEvent::RecordVisit { url } => {
                        if should_record_history_url(&url) {
                            if let Ok(mut s) = state_for_events.lock() {
                                let title = s
                                    .shell
                                    .get_active_tab()
                                    .and_then(|tab| tab.title.clone());
                                let workspace = s
                                    .shell
                                    .get_active_workspace()
                                    .map(|ws| ws.name.clone());
                                if let Err(err) = s.record_visit(&url, title, workspace.clone()) {
                                    error!("Failed to persist history entry: {}", err);
                                }

                                // Track page visit (domain only)
                                if let Ok(parsed_url) = url::Url::parse(&url) {
                                    if let Some(domain) = parsed_url.host_str() {
                                        let workspace_id = s.shell.get_active_workspace()
                                            .map(|w| w.id.0.to_string());
                                        if let Err(e) = s.analytics.track_page_visit(
                                            domain,
                                            workspace_id.as_deref()
                                        ) {
                                            error!("Failed to track page visit: {}", e);
                                        }
                                    }
                                }

                                // Update focus mode page loaded timestamp for auto-trigger
                                s.focus_mode.record_navigation(&url);
                            }
                        }
                        sync_history_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::FindInPageResult(payload) => {
                        if payload.is_null() {
                            return;
                        }
                        if let Ok(json) = serde_json::to_string(&payload) {
                            let update_script = format!(
                                "if(window.hiwaveChrome) {{ hiwaveChrome.updateFindState({}); }}",
                                json
                            );
                            chrome_for_events.evaluate_script(&update_script);
                        }
                    }
                    UserEvent::FindInPage { query, case_sensitive, direction } => {
                        let payload = serde_json::json!({
                            "query": query,
                            "case_sensitive": case_sensitive,
                            "direction": direction,
                        });
                        let script = format!(
                            "{}\n(() => {{\n    const result = window.__hiwaveFind && window.__hiwaveFind.run({});\n    if (window.ipc && result) {{\n        window.ipc.postMessage(JSON.stringify({{ cmd: 'find_in_page_result', result }}));\n    }}\n}})();",
                            FIND_IN_PAGE_HELPER,
                            payload
                        );
                        content_for_events.evaluate_script(&script);
                    }
                    UserEvent::SyncTabs => {
                        info!("Syncing tabs to Chrome UI");
                        sync_tabs_to_chrome(&state_for_events, &chrome_for_events);
                        sync_shield_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::SyncWorkspaces => {
                        info!("Syncing workspaces to Chrome UI");
                        sync_workspaces_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::SyncDownloads => {
                        sync_downloads_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::SyncShelf(scope) => {
                        sync_shelf_to_chrome(&state_for_events, &chrome_for_events, scope);
                    }
                    UserEvent::SyncBlocklist => {
                        sync_blocklist_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::SyncHistory => {
                        sync_history_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::SyncShieldStats => {
                        sync_shield_to_chrome(&state_for_events, &chrome_for_events);
                    }
                    UserEvent::ShowCommands(json) => {
                        let script = format!(
                            "if(window.hiwaveShelf) {{ hiwaveShelf.showCommands({}); }}",
                            json
                        );
                        if let Some(ref shelf) = shelf_for_events {
                            shelf.evaluate_script(&script);
                        }
                    }
                    UserEvent::ExpandChrome => {
                        // Update chrome height
                        *chrome_height_for_events.lock().unwrap() = CHROME_HEIGHT_EXPANDED as f64;
                        let ch = CHROME_HEIGHT_EXPANDED as f64;
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sidebar_open = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sidebar_open,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::ExpandChromeSmall => {
                        // Update chrome height (small expansion for tab actions panel)
                        *chrome_height_for_events.lock().unwrap() = CHROME_HEIGHT_SMALL as f64;
                        let ch = CHROME_HEIGHT_SMALL as f64;
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sidebar_open = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sidebar_open,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::CollapseChrome => {
                        // Update chrome height
                        *chrome_height_for_events.lock().unwrap() = CHROME_HEIGHT_DEFAULT as f64;
                        let ch = CHROME_HEIGHT_DEFAULT as f64;
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sidebar_open = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sidebar_open,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::OpenInspector => {
                        info!("Opening inspector panel");
                        // The shelf closes, as on macOS.
                        *shelf_height_for_events.lock().unwrap() = SHELF_HEIGHT_DEFAULT as f64;
                        *inspector_height_for_events.lock().unwrap() = inspector_panel::INSPECTOR_HEIGHT_EXPANDED as f64;
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sw = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            SHELF_HEIGHT_DEFAULT as f64,
                            inspector_panel::INSPECTOR_HEIGHT_EXPANDED as f64,
                            sw,
                            right_sidebar_open,
                        );
                        // Its console answers without a click first, as the shelf's input does.
                        inspector_for_events.take_keyboard();
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.open(v, &*inspector_for_events)) {
                            return;
                        }
                        // WebKit page: inject the Inspector's script (once) and ask for the DOM tree.
                        let inject_script = format!("if (!window.__hiwaveInspector) {{ {} }}", INSPECTOR_INJECT);
                        content_for_events.evaluate_script(&inject_script);
                        content_for_events.evaluate_script(r#"
                            setTimeout(() => {
                                if (window.__hiwaveInspector) {
                                    const tree = window.__hiwaveInspector.getDomTree(10);
                                    window.ipc.postMessage(JSON.stringify({ cmd: 'inspector_dom_tree_result', tree: tree }));
                                }
                            }, 100);
                        "#);
                    }
                    UserEvent::CloseInspector => {
                        info!("Closing inspector panel");
                        *inspector_height_for_events.lock().unwrap() = inspector_panel::INSPECTOR_HEIGHT_DEFAULT as f64;
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sw = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            inspector_panel::INSPECTOR_HEIGHT_DEFAULT as f64,
                            sw,
                            right_sidebar_open,
                        );
                        // The keyboard goes back to the page; its highlight goes.
                        content_for_events.focus();
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.close(v, &*inspector_for_events)) {
                            return;
                        }
                        content_for_events.evaluate_script("if (window.__hiwaveInspector) { window.__hiwaveInspector.clearHighlight(); }");
                    }
                    UserEvent::ToggleInspector => {
                        let open = *inspector_height_for_events.lock().unwrap() > 0.0;
                        let _ = proxy.send_event(if open { UserEvent::CloseInspector } else { UserEvent::OpenInspector });
                    }
                    UserEvent::InspectorSendDomTree(json) => {
                        // Send DOM tree to inspector panel
                        let script = format!(
                            "if (window.hiwaveInspector) {{ window.hiwaveInspector.setDomTree({}); }}",
                            json
                        );
                        let _ = inspector_for_events.eval(&script);
                    }
                    UserEvent::InspectorSendElementStyles(json) => {
                        // Send element styles to inspector panel
                        let script = format!(
                            "if (window.hiwaveInspector) {{ window.hiwaveInspector.setElementStyles({}); }}",
                            json
                        );
                        let _ = inspector_for_events.eval(&script);
                    }
                    UserEvent::InspectorElementPicked { path, styles } => {
                        // Element was picked in content webview, update inspector
                        let styles_json = serde_json::to_string(&styles).unwrap_or_else(|_| "null".to_string());
                        let script = inspector_picked_script(&path, &styles_json);
                        let _ = inspector_for_events.eval(&script);
                    }
                    UserEvent::InspectorPickerStopped => {
                        let _ = inspector_for_events.eval(
                            "if (window.hiwaveInspector) { window.hiwaveInspector.setPickerActive(false); }"
                        );
                    }
                    UserEvent::InspectorHighlightElement { path } => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.highlight(v, Some(&path))) {
                            return;
                        }
                        let script = format!(
                            "if (window.__hiwaveInspector) {{ window.__hiwaveInspector.highlightElement({}); }}",
                            js_string(&path)
                        );
                        let _ = content_for_events.evaluate_script(&script);
                    }
                    UserEvent::InspectorClearHighlight => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.highlight(v, None)) {
                            return;
                        }
                        let _ = content_for_events.evaluate_script(
                            "if (window.__hiwaveInspector) { window.__hiwaveInspector.clearHighlight(); }"
                        );
                    }
                    UserEvent::InspectorStartPicker => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, _v| d.start_picker(&*inspector_for_events)) {
                            return;
                        }
                        let _ = content_for_events.evaluate_script(
                            "if (window.__hiwaveInspector) { window.__hiwaveInspector.startPicker(); }"
                        );
                        let _ = inspector_for_events.eval(
                            "if (window.hiwaveInspector) { window.hiwaveInspector.setPickerActive(true); }"
                        );
                    }
                    UserEvent::InspectorStopPicker => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.stop_picker(v, &*inspector_for_events)) {
                            return;
                        }
                        let _ = content_for_events.evaluate_script(
                            "if (window.__hiwaveInspector) { window.__hiwaveInspector.stopPicker(); }"
                        );
                        let _ = inspector_for_events.eval(
                            "if (window.hiwaveInspector) { window.hiwaveInspector.setPickerActive(false); }"
                        );
                    }
                    UserEvent::InspectorScrollToElement { path } => {
                        let script = format!(
                            "if (window.__hiwaveInspector) {{ window.__hiwaveInspector.scrollToElement({}); }}",
                            js_string(&path)
                        );
                        let _ = content_for_events.evaluate_script(&script);
                    }
                    UserEvent::InspectorRefreshTree => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.send_tree(v)) {
                            return;
                        }
                        let _ = content_for_events.evaluate_script(r#"
                            (function() {
                                if (window.__hiwaveInspector) {
                                    const tree = window.__hiwaveInspector.getDomTree(10);
                                    window.ipc.postMessage(JSON.stringify({
                                        cmd: 'inspector_dom_tree_result',
                                        tree: tree
                                    }));
                                }
                            })();
                        "#);
                    }
                    UserEvent::InspectorSelect { path } => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.select(v, &path)) {
                            return;
                        }
                        let script = format!(r#"
                            (function() {{
                                if (window.__hiwaveInspector) {{
                                    const styles = window.__hiwaveInspector.getElementStyles({});
                                    window.ipc.postMessage(JSON.stringify({{
                                        cmd: 'inspector_element_styles_result',
                                        styles: styles
                                    }}));
                                }}
                            }})();
                        "#, js_string(&path));
                        let _ = content_for_events.evaluate_script(&script);
                    }
                    UserEvent::InspectorExpand { path } => {
                        // The injected-script tree is sent whole; only RustKit's is cut and expanded on demand.
                        with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.expand(v, &path));
                    }
                    UserEvent::InspectElement { path } => {
                        // Open inspector and select the element
                        let _ = proxy.send_event(UserEvent::OpenInspector);
                        // Delay the selection to allow inspector to initialize
                        let path_literal = js_string(&path);
                        let script = format!(r#"
                            setTimeout(() => {{
                                if (window.__hiwaveInspector) {{
                                    const styles = window.__hiwaveInspector.getElementStyles({});
                                    window.ipc.postMessage(JSON.stringify({{
                                        cmd: 'inspector_element_picked',
                                        path: {},
                                        styles: styles
                                    }}));
                                }}
                            }}, 200);
                        "#, path_literal, path_literal);
                        let _ = content_for_events.evaluate_script(&script);
                    }
                    // Console events
                    UserEvent::InspectorConsoleMessage(json) => {
                        // Forward console message to inspector panel
                        let script = format!(
                            "if (window.hiwaveInspector) {{ window.hiwaveInspector.addConsoleEntry({}); }}",
                            json
                        );
                        let _ = inspector_for_events.eval(&script);
                    }
                    UserEvent::InspectorConsoleClear => {
                        // Clear console in inspector panel
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, _v| d.clear_console(&*inspector_for_events)) {
                            return;
                        }
                        let _ = inspector_for_events.eval("window.hiwaveInspector && window.hiwaveInspector.clearConsole()");
                    }
                    UserEvent::InspectorConsoleEval { code } => {
                        if with_devtools!(content_for_events, devtools, inspector_for_events, |d, v| d.eval(v, &code)) {
                            return;
                        }
                        // Execute JavaScript in content webview
                        let code_literal = js_string(&code);
                        let script = format!(r#"
                            (function() {{
                                try {{
                                    const result = eval({});
                                    const serialized = typeof result === 'object' ? JSON.stringify(result, null, 2) : String(result);
                                    window.ipc.postMessage(JSON.stringify({{
                                        cmd: 'inspector_console_message',
                                        entry: {{
                                            id: Date.now(),
                                            level: 'result',
                                            message: serialized,
                                            timestamp: Date.now(),
                                            stack: []
                                        }}
                                    }}));
                                }} catch (e) {{
                                    window.ipc.postMessage(JSON.stringify({{
                                        cmd: 'inspector_console_message',
                                        entry: {{
                                            id: Date.now(),
                                            level: 'error',
                                            message: e.name + ': ' + e.message,
                                            timestamp: Date.now(),
                                            stack: []
                                        }}
                                    }}));
                                }}
                            }})();
                        "#, code_literal);
                        let _ = content_for_events.evaluate_script(&script);
                    }
                    UserEvent::ExpandShelf => {
                        let ch = *chrome_height_for_events.lock().unwrap();

                        // Update shelf height
                        *shelf_height_for_events.lock().unwrap() = SHELF_HEIGHT_EXPANDED as f64;
                        let sh = SHELF_HEIGHT_EXPANDED as f64;
                        let sidebar_open = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sidebar_open,
                            right_sidebar_open,
                        );

                        // Focus the command palette input and trigger initial search
                        if let Some(ref shelf) = shelf_for_events {
                            shelf.evaluate_script(r#"
                                if(window.hiwaveShelf) {
                                    hiwaveShelf.clear();
                                    hiwaveShelf.focus();
                                    // Trigger initial search after a brief delay to ensure IPC is ready
                                    setTimeout(() => {
                                        const input = document.getElementById('commandInput');
                                        if (input && window.ipc) {
                                            window.ipc.postMessage(JSON.stringify({ cmd: 'search_commands', query: '' }));
                                        }
                                    }, 50);
                                }
                            "#);
                        }
                    }
                    UserEvent::CollapseShelf => {
                        let ch = *chrome_height_for_events.lock().unwrap();

                        // Update shelf height
                        *shelf_height_for_events.lock().unwrap() = SHELF_HEIGHT_DEFAULT as f64;
                        let sh = SHELF_HEIGHT_DEFAULT as f64;
                        let sidebar_open = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sidebar_open,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::SetSidebarOpen(open) => {
                        // Get current width or use default when opening
                        let new_width = if open { SIDEBAR_WIDTH } else { 0.0 };
                        *sidebar_width_for_events.lock().unwrap() = new_width;
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            new_width,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::SetSidebarWidth(width) => {
                        // Update sidebar width during resize drag
                        *sidebar_width_for_events.lock().unwrap() = width;
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            width,
                            right_sidebar_open,
                        );
                    }
                    UserEvent::SetRightPanelOpen(open) => {
                        // Update right sidebar state and relayout
                        *right_sidebar_open_for_events.lock().unwrap() = open;
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sw = *sidebar_width_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sw,
                            open,
                        );
                        info!("Right panel: {}", if open { "open" } else { "closed" });
                    }
                    // Focus mode event handlers
                    UserEvent::EnterFocusMode => {
                        info!("Entering focus mode - hiding chrome UI");
                        // In focus mode, keep a small chrome area (10px) for the reveal zone
                        // This allows mouse hover at top to trigger peek mode
                        // Content takes most of the window
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            10.0, // Minimal chrome height for reveal zone hover detection
                            0.0,  // No shelf height
                            0.0,  // No Inspector
                            0.0,  // Sidebar hidden (width = 0)
                            false, // Right sidebar hidden
                        );
                        // Get focus mode config to pass to chrome
                        let config = {
                            let state = focus_state_for_events.lock().unwrap();
                            serde_json::json!({
                                "show_progress_bar": state.focus_mode_config.show_progress_bar,
                                "hide_cursor": state.focus_mode_config.hide_cursor,
                                "cursor_hide_delay_secs": state.focus_mode_config.cursor_hide_delay_secs,
                            })
                        };
                        // Notify chrome of focus mode state with config
                        let script = format!(
                            "if(window.hiwaveChrome) {{ hiwaveChrome.setFocusMode(true, {}); }}",
                            config
                        );
                        chrome_for_events.evaluate_script(&script);
                    }
                    UserEvent::ExitFocusMode => {
                        info!("Exiting focus mode - restoring chrome UI");
                        // Restore normal layout
                        let ch = *chrome_height_for_events.lock().unwrap();
                        let sh = *shelf_height_for_events.lock().unwrap();
                        let sidebar_open = *sidebar_width_for_events.lock().unwrap();
                        let right_sidebar_open = *right_sidebar_open_for_events.lock().unwrap();
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            ch,
                            sh,
                            *inspector_height_for_events.lock().unwrap(),
                            sidebar_open,
                            right_sidebar_open,
                        );
                        // Notify chrome of focus mode state - call setFocusMode immediately
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { \
                                console.log('[Focus] ExitFocusMode event - calling setFocusMode(false)'); \
                                hiwaveChrome.setFocusMode(false); \
                            } else { \
                                console.error('[Focus] hiwaveChrome not available!'); \
                            }"
                        );
                        info!("Called setFocusMode(false)");
                    }
                    UserEvent::ToggleFocusMode => {
                        // State was already toggled by IPC handler, so check the NEW state
                        // If active is now true, we should enter focus mode
                        // If active is now false, we should exit focus mode
                        let is_focus_mode = {
                            let state = focus_state_for_events.lock().unwrap();
                            state.focus_mode.active
                        };
                        if is_focus_mode {
                            let _ = proxy.send_event(UserEvent::EnterFocusMode);
                        } else {
                            let _ = proxy.send_event(UserEvent::ExitFocusMode);
                        }
                    }
                    UserEvent::SyncFocusModeStatus => {
                        // Sync focus mode status to chrome
                        if let Ok(state) = focus_state_for_events.lock() {
                            let status = serde_json::json!({
                                "active": state.focus_mode.active,
                                "peek_visible": state.focus_mode.peek_visible,
                                "scroll_progress": state.focus_mode.scroll_progress,
                            });
                            let script = format!(
                                "if(window.hiwaveChrome) {{ hiwaveChrome.updateFocusModeStatus({}); }}",
                                status
                            );
                            chrome_for_events.evaluate_script(&script);
                        }
                    }
                    UserEvent::ShowFocusPeek => {
                        info!("Showing focus mode peek UI");
                        // In peek mode, show minimal chrome (60px) at top
                        let peek_height = 60.0;
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            peek_height,
                            0.0,  // No shelf in peek mode
                            0.0,  // No Inspector
                            0.0,  // No sidebar in peek mode (width = 0)
                            false,  // No right sidebar in peek mode
                        );
                    }
                    UserEvent::HideFocusPeek => {
                        info!("Hiding focus mode peek UI");
                        // Return to full focus mode - no chrome visible
                        apply_layout(
                            &window,
                            &chrome_for_events,
                            &content_for_events,
                            shelf_for_events.as_ref(),
                            &inspector_for_events,
                            0.0,  // No chrome
                            0.0,  // No shelf
                            0.0,  // No Inspector
                            0.0,  // No sidebar (width = 0)
                            false,  // No right sidebar
                        );
                    }
                    UserEvent::FocusAutoTriggerCheck => {
                        // Check if auto-focus should trigger
                        let (should_enter, debug_reason) = {
                            let state = focus_state_for_events.lock().unwrap();
                            let result = state.focus_mode.should_auto_enter(&state.focus_mode_config);
                            let reason = if state.focus_mode.active {
                                "already active"
                            } else if state.focus_mode.auto_triggered_for_page {
                                "already triggered for this page"
                            } else if !state.focus_mode_config.auto_enabled {
                                "auto-trigger disabled"
                            } else if state.focus_mode.page_loaded_at.is_none() {
                                "no page loaded"
                            } else {
                                let elapsed = state.focus_mode.page_loaded_at.unwrap().elapsed().as_secs();
                                if elapsed < state.focus_mode_config.activation_delay_secs as u64 {
                                    "waiting for delay"
                                } else {
                                    "ready"
                                }
                            };
                            (result, reason)
                        };

                        // Log every 30 seconds (6 checks) when debug enabled
                        static CHECK_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
                        let count = CHECK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if count.is_multiple_of(6) && std::env::var("HIWAVE_DEBUG").is_ok() {
                            info!("Focus auto-trigger check: {} ({})", should_enter, debug_reason);
                        }

                        if should_enter {
                            info!("Auto-triggering focus mode");
                            // Update state and mark as auto-triggered for this page
                            {
                                let mut state = focus_state_for_events.lock().unwrap();
                                state.focus_mode.enter();
                                state.focus_mode.auto_triggered_for_page = true;
                            }
                            // Trigger the UI update
                            let _ = proxy.send_event(UserEvent::EnterFocusMode);
                        }
                    }
                    UserEvent::OpenSettings => {
                        info!("Opening settings window");
                        let mut settings_guard = settings_window_for_events.lock().unwrap();

                        if let Some((ref settings_win, _)) = *settings_guard {
                            settings_win.set_visible(true);
                            settings_win.set_focus();
                        } else {
                            let settings_win = WindowBuilder::new()
                                .with_title("HiWave Settings")
                                .with_inner_size(LogicalSize::new(720.0, 680.0))
                                .with_min_inner_size(LogicalSize::new(600.0, 500.0))
                                .with_resizable(true)
                                .build(event_loop_target)
                                .expect("Failed to create settings window");

                            let settings_proxy = settings_proxy_for_handler.clone();

                            // WRY (WebView2) Settings WebView creation
                            let settings_webview = WebViewBuilder::new()
                                .with_html(SETTINGS_HTML)
                                .with_devtools(cfg!(debug_assertions))
                                .with_clipboard(true)
                                .with_focused(true)
                                .with_initialization_script(JS_BRIDGE)
                                .with_ipc_handler(move |msg: wry::http::Request<String>| {
                                    let body = msg.body();
                                    if let Ok(ipc_msg) = serde_json::from_str::<IpcMessage>(body) {
                                        match ipc_msg {
                                            IpcMessage::CloseSettings => {
                                                let _ = settings_proxy.send_event(UserEvent::CloseSettings);
                                            }
                                            IpcMessage::GetBrowserProfiles { browser } => {
                                                let _ = settings_proxy.send_event(UserEvent::GetBrowserProfiles(browser));
                                            }
                                            IpcMessage::ImportBookmarks { browser, profile_path } => {
                                                let _ = settings_proxy.send_event(UserEvent::ImportBookmarks { browser, profile_path });
                                            }
                                            IpcMessage::GetSettings => {
                                                let _ = settings_proxy.send_event(UserEvent::GetSettings);
                                            }
                                            IpcMessage::SaveSettings { settings } => {
                                                let _ = settings_proxy.send_event(UserEvent::SaveSettings(settings));
                                            }
                                            IpcMessage::ExportData { include_settings, include_workspaces } => {
                                                let _ = settings_proxy.send_event(UserEvent::ExportData { include_settings, include_workspaces });
                                            }
                                            IpcMessage::SaveExportToFile { data, suggested_name } => {
                                                let _ = settings_proxy.send_event(UserEvent::SaveExportToFile { data, suggested_name });
                                            }
                                            IpcMessage::ImportData { data, replace } => {
                                                let _ = settings_proxy.send_event(UserEvent::ImportStateData { data, replace });
                                            }
                                            IpcMessage::PickImportFile => {
                                                let _ = settings_proxy.send_event(UserEvent::PickImportFile);
                                            }
                                            IpcMessage::ClearBrowsingData { history, downloads, shelf } => {
                                                let _ = settings_proxy.send_event(UserEvent::ClearBrowsingData { history, downloads, shelf });
                                            }
                                            IpcMessage::GetCellar => {
                                                let _ = settings_proxy.send_event(UserEvent::GetCellar);
                                            }
                                            IpcMessage::RestoreFromCellar { id } => {
                                                let _ = settings_proxy.send_event(UserEvent::RestoreFromCellar { id });
                                            }
                                            IpcMessage::ClearCellar => {
                                                let _ = settings_proxy.send_event(UserEvent::ClearCellar);
                                            }
                                            IpcMessage::GetFocusModeConfig => {
                                                let _ = settings_proxy.send_event(UserEvent::GetFocusModeConfigForSettings);
                                            }
                                            IpcMessage::SaveFocusModeConfig { config } => {
                                                let _ = settings_proxy.send_event(UserEvent::SaveFocusModeConfigFromSettings(config));
                                            }
                                            IpcMessage::AddToFocusBlocklist { domain } => {
                                                let _ = settings_proxy.send_event(UserEvent::AddToFocusBlocklistFromSettings(domain));
                                            }
                                            IpcMessage::RemoveFromFocusBlocklist { domain } => {
                                                let _ = settings_proxy.send_event(UserEvent::RemoveFromFocusBlocklistFromSettings(domain));
                                            }
                                            // Vault IPC commands
                                            IpcMessage::GetVaultStatus => {
                                                let _ = settings_proxy.send_event(UserEvent::GetVaultStatus);
                                            }
                                            IpcMessage::UnlockVault { password } => {
                                                let _ = settings_proxy.send_event(UserEvent::UnlockVault(password));
                                            }
                                            IpcMessage::LockVault => {
                                                let _ = settings_proxy.send_event(UserEvent::LockVault);
                                            }
                                            IpcMessage::GetAllCredentials => {
                                                let _ = settings_proxy.send_event(UserEvent::GetAllCredentials);
                                            }
                                            IpcMessage::SaveCredential { url, username, password } => {
                                                let _ = settings_proxy.send_event(UserEvent::SaveCredential { url, username, password });
                                            }
                                            IpcMessage::DeleteCredential { id } => {
                                                let _ = settings_proxy.send_event(UserEvent::DeleteCredential(id));
                                            }
                                            // Analytics IPC commands
                                            IpcMessage::GetAnalyticsSettings => {
                                                let _ = settings_proxy.send_event(UserEvent::GetAnalyticsSettings);
                                            }
                                            IpcMessage::UpdateAnalyticsSettings { enabled, retention_days, weekly_report, report_day } => {
                                                let _ = settings_proxy.send_event(UserEvent::UpdateAnalyticsSettings { enabled, retention_days, weekly_report, report_day });
                                            }
                                            IpcMessage::ClearAnalyticsData => {
                                                let _ = settings_proxy.send_event(UserEvent::ClearAnalyticsData);
                                            }
                                            IpcMessage::ExportAnalyticsData { format } => {
                                                let _ = settings_proxy.send_event(UserEvent::ExportAnalyticsData { format });
                                            }
                                            _ => {
                                                info!("Settings window received unhandled IPC: {}", ipc_cmd(&body));
                                            }
                                        }
                                    }
                                })
                                // Linux/Wayland: .build(&window) uses the X11-only window-handle
                                // path and renders a white screen on Wayland. Build into the
                                // settings window's gtk vbox instead (pack_start fills the window)
                                // — same Unix path as the main webviews.
                                .build_gtk({
                                    use tao::platform::unix::WindowExtUnix;
                                    settings_win
                                        .default_vbox()
                                        .expect("settings window has no gtk vbox")
                                })
                                .expect("Failed to create settings WebView");

                            *settings_guard = Some((settings_win, settings_webview));
                        }
                    }
                    UserEvent::CloseSettings => {
                        info!("Closing settings window");
                        let settings_guard = settings_window_for_events.lock().unwrap();
                        if let Some((ref settings_win, _)) = *settings_guard {
                            settings_win.set_visible(false);
                        }
                    }
                    UserEvent::GetBrowserProfiles(browser) => {
                        info!("Getting browser profiles for: {}", browser);
                        let browser_type = match browser.to_lowercase().as_str() {
                            "chrome" => import::Browser::Chrome,
                            "firefox" => import::Browser::Firefox,
                            "brave" => import::Browser::Brave,
                            _ => {
                                let error_json = serde_json::json!({
                                    "type": "profiles_result",
                                    "error": format!("Unknown browser: {}", browser)
                                }).to_string();
                                if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                                    let script = format!("window.postMessage({}, '*');", error_json);
                                    let _ = settings_wv.evaluate_script(&script);
                                }
                                return;
                            }
                        };

                        let profiles = import::get_browser_profiles(browser_type);
                        let profiles_json: Vec<serde_json::Value> = profiles
                            .iter()
                            .map(|p| serde_json::json!({
                                "name": p.name,
                                "path": p.path.to_string_lossy(),
                            }))
                            .collect();

                        let result_json = serde_json::json!({
                            "type": "profiles_result",
                            "profiles": profiles_json,
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ImportBrowserProfilesResult(json) => {
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ImportBookmarks { browser, profile_path } => {
                        info!("Importing bookmarks from {} at {}", browser, profile_path);
                        let browser_type = match browser.to_lowercase().as_str() {
                            "chrome" => import::Browser::Chrome,
                            "firefox" => import::Browser::Firefox,
                            "brave" => import::Browser::Brave,
                            _ => {
                                let error_json = serde_json::json!({
                                    "type": "import_result",
                                    "error": format!("Unknown browser: {}", browser)
                                }).to_string();
                                if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                                    let script = format!("window.postMessage({}, '*');", error_json);
                                    let _ = settings_wv.evaluate_script(&script);
                                }
                                return;
                            }
                        };

                        let profile = import::BrowserProfile {
                            name: "Import".to_string(),
                            path: std::path::PathBuf::from(&profile_path),
                            browser: browser_type,
                        };

                        // Parse bookmarks
                        let bookmarks = match import::import_bookmarks(&profile) {
                            Ok(b) => b,
                            Err(e) => {
                                let error_json = serde_json::json!({
                                    "type": "import_result",
                                    "error": format!("Failed to parse bookmarks: {}", e)
                                }).to_string();
                                if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                                    let script = format!("window.postMessage({}, '*');", error_json);
                                    let _ = settings_wv.evaluate_script(&script);
                                }
                                return;
                            }
                        };

                        if bookmarks.is_empty() {
                            let result_json = serde_json::json!({
                                "type": "import_result",
                                "stats": {
                                    "workspaces_created": 0,
                                    "tabs_created": 0,
                                },
                                "message": "No bookmarks found to import"
                            }).to_string();
                            if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                                let script = format!("window.postMessage({}, '*');", result_json);
                                let _ = settings_wv.evaluate_script(&script);
                            }
                            return;
                        }

                        // Convert to workspaces
                        let config = import::converter::ConversionConfig::for_browser(browser_type);
                        let result = import::converter::convert_to_workspaces(bookmarks, &config);

                        // Add workspaces and tabs
                        let mut state = state.lock().unwrap();
                        let mut workspaces_created = 0;
                        let mut tabs_created = 0;

                        for import_ws in &result.workspaces {
                            let ws_id = state.shell.create_workspace(import_ws.name.clone());
                            workspaces_created += 1;

                            for tab in &import_ws.tabs {
                                let parsed_url = url::Url::parse(&tab.url)
                                    .unwrap_or_else(|_| url::Url::parse("about:blank").unwrap());

                                let tab_info = hiwave_core::types::TabInfo {
                                    id: hiwave_core::types::TabId::new(),
                                    url: parsed_url,
                                    title: Some(tab.title.clone()),
                                    favicon: None,
                                    workspace_id: ws_id,
                                    suspended: false,
                                    loading: false,
                                    locked: false,
                                    last_visited: None,
                                };

                                if state.shell.create_tab(tab_info).is_ok() {
                                    tabs_created += 1;
                                }
                            }
                        }

                        // Save state
                        if let Err(e) = state.save_workspace_state() {
                            warn!("Failed to save state after import: {}", e);
                        }

                        // Update UI
                        let _ = proxy.send_event(UserEvent::SyncWorkspaces);

                        let result_json = serde_json::json!({
                            "type": "import_result",
                            "stats": {
                                "workspaces_created": workspaces_created,
                                "tabs_created": tabs_created,
                            }
                        }).to_string();

                        drop(state); // Release lock before accessing settings window

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }

                        info!("Import complete: {} workspaces, {} tabs", workspaces_created, tabs_created);
                    }
                    UserEvent::ImportBookmarksResult(json) => {
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::GetSettings => {
                        let state = state_for_events.lock().unwrap();
                        let settings = state.get_settings();
                        let result_json = serde_json::json!({
                            "type": "settings_result",
                            "data": settings
                        }).to_string();
                        drop(state);

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::SettingsResult(json) => {
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::SaveSettings(settings_value) => {
                        let settings: Result<crate::state::UserSettings, _> = serde_json::from_value(settings_value);
                        let result_json = match settings {
                            Ok(s) => {
                                let mut state = state_for_events.lock().unwrap();
                                match state.update_settings(s) {
                                    Ok(_) => serde_json::json!({ "type": "settings_saved" }),
                                    Err(e) => serde_json::json!({ "type": "settings_saved", "error": e.to_string() }),
                                }
                            }
                            Err(e) => serde_json::json!({ "type": "settings_saved", "error": e.to_string() }),
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::SettingsSaved => {
                        let result_json = serde_json::json!({ "type": "settings_saved" }).to_string();
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ExportData { include_settings, include_workspaces } => {
                        let state = state_for_events.lock().unwrap();
                        let export = state.export_data(include_settings, include_workspaces);
                        let result_json = match serde_json::to_string_pretty(&export) {
                            Ok(data) => serde_json::json!({
                                "type": "export_result",
                                "data": data,
                                "workspaces_count": export.workspaces.len()
                            }),
                            Err(e) => serde_json::json!({
                                "type": "export_result",
                                "error": e.to_string()
                            }),
                        }.to_string();
                        drop(state);

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ExportDataResult(json) => {
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::SaveExportToFile { data, suggested_name } => {
                        // Show native save dialog
                        let result = rfd::FileDialog::new()
                            .set_title("Export HiWave Data")
                            .set_file_name(&suggested_name)
                            .add_filter("JSON", &["json"])
                            .save_file();

                        let result_json = match result {
                            Some(path) => {
                                match std::fs::write(&path, &data) {
                                    Ok(_) => {
                                        info!("Exported data to: {:?}", path);
                                        serde_json::json!({
                                            "type": "export_file_result",
                                            "success": true,
                                            "path": path.to_string_lossy()
                                        })
                                    }
                                    Err(e) => {
                                        error!("Failed to write export file: {}", e);
                                        serde_json::json!({
                                            "type": "export_file_result",
                                            "error": format!("Failed to write file: {}", e)
                                        })
                                    }
                                }
                            }
                            None => {
                                // User cancelled
                                serde_json::json!({
                                    "type": "export_file_result",
                                    "cancelled": true
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::PickImportFile => {
                        // Show native open dialog
                        let result = rfd::FileDialog::new()
                            .set_title("Import HiWave Data")
                            .add_filter("JSON", &["json"])
                            .pick_file();

                        let result_json = match result {
                            Some(path) => {
                                match std::fs::read_to_string(&path) {
                                    Ok(contents) => {
                                        info!("Loaded import file from: {:?}", path);
                                        serde_json::json!({
                                            "type": "import_file_picked",
                                            "data": contents,
                                            "path": path.to_string_lossy()
                                        })
                                    }
                                    Err(e) => {
                                        error!("Failed to read import file: {}", e);
                                        serde_json::json!({
                                            "type": "import_file_picked",
                                            "error": format!("Failed to read file: {}", e)
                                        })
                                    }
                                }
                            }
                            None => {
                                // User cancelled
                                serde_json::json!({
                                    "type": "import_file_picked",
                                    "cancelled": true
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ImportStateData { data, replace } => {
                        let result_json = match serde_json::from_str::<crate::state::HiWaveExport>(&data) {
                            Ok(import_data) => {
                                let mut state = state_for_events.lock().unwrap();
                                match state.import_data(&import_data, replace) {
                                    Ok(result) => serde_json::json!({
                                        "type": "import_data_result",
                                        "success": result.success,
                                        "workspaces_created": result.workspaces_created,
                                        "tabs_created": result.tabs_created,
                                        "errors": result.errors
                                    }),
                                    Err(e) => serde_json::json!({
                                        "type": "import_data_result",
                                        "error": e.to_string()
                                    }),
                                }
                            }
                            Err(e) => serde_json::json!({
                                "type": "import_data_result",
                                "error": format!("Invalid import format: {}", e)
                            }),
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ImportStateDataResult(json) => {
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ClearBrowsingData { history, downloads, shelf } => {
                        let mut state = state_for_events.lock().unwrap();
                        let mut cleared = Vec::new();

                        if history
                            && state.clear_visit_history().is_ok() {
                                cleared.push("history");
                            }
                        if downloads {
                            state.clear_download_history();
                            cleared.push("downloads");
                        }
                        if shelf {
                            state.clear_shelf();
                            cleared.push("shelf");
                        }

                        let _ = state.save_workspace_state();
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "clear_data_result",
                            "cleared": cleared
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ClearBrowsingDataResult(json) => {
                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::GetCellar => {
                        let state = state_for_events.lock().unwrap();
                        let cellar = state.list_cellar();
                        let items: Vec<serde_json::Value> = cellar
                            .iter()
                            .map(|item| {
                                serde_json::json!({
                                    "id": item.id,
                                    "url": item.url,
                                    "title": item.title,
                                    "workspace": item.workspace,
                                    "originally_shelved": item.originally_shelved,
                                    "cellared_at": item.cellared_at,
                                })
                            })
                            .collect();
                        let result_json = serde_json::json!({
                            "type": "cellar_result",
                            "items": items,
                            "count": items.len()
                        }).to_string();
                        drop(state);

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::RestoreFromCellar { id } => {
                        let mut state = state_for_events.lock().unwrap();
                        let result_json = match state.restore_from_cellar(&id) {
                            Some(shelf_item) => {
                                let _ = state.save_workspace_state();
                                info!("Restored item from cellar: {}", id);
                                serde_json::json!({
                                    "type": "cellar_restore_result",
                                    "restored": true,
                                    "item": {
                                        "id": shelf_item.id,
                                        "url": shelf_item.url,
                                        "title": shelf_item.title,
                                        "workspace": shelf_item.workspace,
                                    }
                                })
                            }
                            None => {
                                serde_json::json!({
                                    "type": "cellar_restore_result",
                                    "error": format!("Cellar item not found: {}", id)
                                })
                            }
                        }.to_string();
                        drop(state);

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ClearCellar => {
                        let mut state = state_for_events.lock().unwrap();
                        state.clear_cellar();
                        let _ = state.save_workspace_state();
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "cellar_cleared"
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::GetFocusModeConfigForSettings => {
                        let state = state_for_events.lock().unwrap();
                        let config = &state.focus_mode_config;
                        let result_json = serde_json::json!({
                            "type": "focus_config_result",
                            "data": config
                        }).to_string();
                        drop(state);

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::SaveFocusModeConfigFromSettings(config_value) => {
                        let config: Result<crate::state::FocusModeConfig, _> = serde_json::from_value(config_value);
                        let result_json = match config {
                            Ok(c) => {
                                let mut state = state_for_events.lock().unwrap();
                                match state.update_focus_config(c) {
                                    Ok(_) => {
                                        info!("Focus mode config saved from settings");
                                        // Reset auto-trigger flag so new settings can take effect
                                        state.focus_mode.auto_triggered_for_page = false;
                                        serde_json::json!({ "type": "focus_config_saved" })
                                    }
                                    Err(e) => serde_json::json!({ "type": "focus_config_error", "error": e.to_string() })
                                }
                            }
                            Err(e) => serde_json::json!({ "type": "focus_config_error", "error": e.to_string() })
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::AddToFocusBlocklistFromSettings(domain) => {
                        let mut state = state_for_events.lock().unwrap();
                        let added = state.focus_mode_config.add_to_blocklist(domain.clone());
                        if added {
                            if let Err(e) = state.save_focus_config() {
                                warn!("Failed to save focus config: {}", e);
                            }
                        }
                        let blocklist = state.focus_mode_config.blocklist.clone();
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "focus_blocklist_updated",
                            "blocklist": blocklist,
                            "added": added,
                            "domain": domain
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::RemoveFromFocusBlocklistFromSettings(domain) => {
                        let mut state = state_for_events.lock().unwrap();
                        let removed = state.focus_mode_config.remove_from_blocklist(&domain);
                        if removed {
                            if let Err(e) = state.save_focus_config() {
                                warn!("Failed to save focus config: {}", e);
                            }
                        }
                        let blocklist = state.focus_mode_config.blocklist.clone();
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "focus_blocklist_updated",
                            "blocklist": blocklist,
                            "removed": removed,
                            "domain": domain
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    // Vault settings events
                    UserEvent::GetVaultStatus => {
                        let state = state_for_events.lock().unwrap();
                        let unlocked = state.vault.is_unlocked();
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "vault_status_result",
                            "unlocked": unlocked
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::UnlockVault(password) => {
                        let mut state = state_for_events.lock().unwrap();
                        let result = state.vault.unlock(&password);
                        drop(state);

                        let result_json = match result {
                            Ok(_) => {
                                info!("Vault unlocked from settings");
                                serde_json::json!({
                                    "type": "vault_unlock_result",
                                    "success": true
                                })
                            }
                            Err(e) => {
                                warn!("Failed to unlock vault: {}", e);
                                serde_json::json!({
                                    "type": "vault_unlock_result",
                                    "success": false,
                                    "error": "Invalid password"
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::LockVault => {
                        let mut state = state_for_events.lock().unwrap();
                        state.vault.lock();
                        info!("Vault locked from settings");
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "vault_lock_result",
                            "success": true
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::GetAllCredentials => {
                        let state = state_for_events.lock().unwrap();
                        let result = state.vault.get_all_credentials();
                        drop(state);

                        let result_json = match result {
                            Ok(creds) => {
                                let cred_list: Vec<serde_json::Value> = creds
                                    .iter()
                                    .map(|c| serde_json::json!({
                                        "id": c.id,
                                        "url": c.url,
                                        "username": c.username,
                                    }))
                                    .collect();
                                serde_json::json!({
                                    "type": "credentials_result",
                                    "credentials": cred_list
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "type": "credentials_result",
                                    "error": format!("{}", e)
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::SaveCredential { url, username, password } => {
                        let mut state = state_for_events.lock().unwrap();
                        let url_parsed = url::Url::parse(&url)
                            .or_else(|_| url::Url::parse(&format!("https://{}", url)));

                        let result = match url_parsed {
                            Ok(parsed_url) => state.vault.save_credential(&parsed_url, &username, &password),
                            Err(e) => Err(hiwave_core::HiWaveError::Vault(format!("Invalid URL: {}", e))),
                        };
                        drop(state);

                        let result_json = match result {
                            Ok(id) => {
                                info!("Credential saved from settings");
                                serde_json::json!({
                                    "type": "credential_saved",
                                    "success": true,
                                    "id": id
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "type": "credential_saved",
                                    "success": false,
                                    "error": format!("{}", e)
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::DeleteCredential(id) => {
                        let mut state = state_for_events.lock().unwrap();
                        let result = state.vault.delete_credential(id);
                        drop(state);

                        let result_json = match result {
                            Ok(_) => {
                                info!("Credential {} deleted from settings", id);
                                serde_json::json!({
                                    "type": "credential_deleted",
                                    "success": true,
                                    "id": id
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "type": "credential_deleted",
                                    "success": false,
                                    "error": format!("{}", e)
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    // Analytics settings events
                    UserEvent::GetAnalyticsSettings => {
                        let state = state_for_events.lock().unwrap();
                        let settings = state.analytics.get_settings();
                        drop(state);

                        let result_json = serde_json::json!({
                            "type": "analytics_settings_result",
                            "data": {
                                "enabled": settings.enabled,
                                "retention_days": settings.retention_days,
                                "weekly_report": settings.weekly_report,
                                "report_day": settings.report_day,
                            }
                        }).to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::UpdateAnalyticsSettings { enabled, retention_days, weekly_report, report_day } => {
                        use hiwave_analytics::ReportSettings;

                        let new_settings = ReportSettings {
                            enabled,
                            retention_days,
                            weekly_report,
                            report_day: report_day.clone(),
                        };

                        let state = state_for_events.lock().unwrap();
                        let result = state.analytics.update_settings(new_settings);
                        drop(state);

                        let result_json = match result {
                            Ok(_) => {
                                info!("Analytics settings updated");
                                serde_json::json!({
                                    "type": "analytics_settings_updated",
                                    "success": true
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "type": "analytics_settings_updated",
                                    "success": false,
                                    "error": format!("{}", e)
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ClearAnalyticsData => {
                        let state = state_for_events.lock().unwrap();
                        let result = state.analytics.clear_all_data();
                        drop(state);

                        let result_json = match result {
                            Ok(_) => {
                                info!("Analytics data cleared");
                                serde_json::json!({
                                    "type": "analytics_data_cleared",
                                    "success": true
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "type": "analytics_data_cleared",
                                    "success": false,
                                    "error": format!("{}", e)
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::ExportAnalyticsData { format } => {
                        let state = state_for_events.lock().unwrap();

                        let result = match format.to_lowercase().as_str() {
                            "json" => {
                                state.analytics.generate_monthly_report()
                                    .and_then(|report| serde_json::to_string_pretty(&report).map_err(|e| {
                                        hiwave_core::error::HiWaveError::analytics(e.to_string())
                                    }))
                                    .map(|data| (data, "json"))
                            }
                            "csv" => {
                                state.analytics.get_last_n_days_stats(30)
                                    .map(|stats| {
                                        let mut csv = String::from("Date,Trackers Blocked,Ads Blocked,Popups Blocked,Pages Visited,Tabs Opened,Tabs Closed,Browsing Time (s),Focus Time (s),Workspace Switches\n");
                                        for stat in stats {
                                            csv.push_str(&format!(
                                                "{},{},{},{},{},{},{},{},{},{}\n",
                                                stat.date,
                                                stat.trackers_blocked,
                                                stat.ads_blocked,
                                                stat.popups_blocked,
                                                stat.pages_visited,
                                                stat.tabs_opened,
                                                stat.tabs_closed,
                                                stat.browsing_time,
                                                stat.focus_time,
                                                stat.workspace_switches
                                            ));
                                        }
                                        (csv, "csv")
                                    })
                            }
                            _ => Err(hiwave_core::error::HiWaveError::analytics(format!("Unsupported format: {}", format)))
                        };
                        drop(state);

                        let result_json = match result {
                            Ok((data, fmt)) => {
                                serde_json::json!({
                                    "type": "analytics_export_result",
                                    "success": true,
                                    "data": data,
                                    "format": fmt
                                })
                            }
                            Err(e) => {
                                serde_json::json!({
                                    "type": "analytics_export_result",
                                    "success": false,
                                    "error": format!("{}", e)
                                })
                            }
                        }.to_string();

                        if let Some((_, ref settings_wv)) = *settings_window_for_events.lock().unwrap() {
                            let script = format!("window.postMessage({}, '*');", result_json);
                            let _ = settings_wv.evaluate_script(&script);
                        }
                    }
                    UserEvent::DecayTick => {
                        // Get decay settings and calculate decay levels for all tabs
                        let state = state_for_events.lock().unwrap();
                        let decay_days = state.user_settings.tab_decay_days;
                        let auto_shelf_days = state.user_settings.auto_shelf_days;
                        let mode = state.user_settings.default_mode.clone();

                        // Get active workspace
                        let active_ws = state.shell.get_active_workspace().map(|ws| ws.id);

                        // Calculate decay info for tabs in active workspace
                        if let Some(ws_id) = active_ws {
                            let tabs_with_decay = state.shell.tabs_with_decay(Some(ws_id), decay_days);

                            // Build decay info JSON
                            let decay_info: Vec<serde_json::Value> = tabs_with_decay
                                .iter()
                                .map(|(tab, level)| {
                                    serde_json::json!({
                                        "id": tab.id.0.to_string(),
                                        "url": tab.url.to_string(),
                                        "decay_level": level
                                    })
                                })
                                .collect();

                            drop(state);

                            // Send decay updates to chrome UI
                            let update_json = serde_json::json!({
                                "type": "decay_update",
                                "tabs": decay_info
                            }).to_string();

                            let script = format!(
                                "if(window.hiwave && window.hiwave.updateDecay) {{ window.hiwave.updateDecay({}); }}",
                                update_json
                            );
                            chrome_for_events.evaluate_script(&script);
                        }

                        // Auto-shelf expired tabs in Zen mode
                        if mode == "zen" {
                            if let Some(shelf_days) = auto_shelf_days {
                                if shelf_days > 0 {
                                    let mut state = state_for_events.lock().unwrap();
                                    let expired = state.shell.expired_tabs(None, shelf_days);

                                    for tab in expired {
                                        // Add to shelf
                                        let workspace_name = state.shell
                                            .get_workspace(tab.workspace_id)
                                            .map(|ws| ws.name.clone())
                                            .unwrap_or_else(|| "Default".to_string());

                                        let shelf_item = crate::state::ShelfItem {
                                            id: format!("shelf-{}", crate::state::AppState::current_timestamp_secs()),
                                            url: tab.url.to_string(),
                                            title: tab.title.clone(),
                                            workspace: workspace_name.clone(),
                                            added_at: crate::state::AppState::current_timestamp_secs(),
                                        };

                                        state.shelves
                                            .entry(workspace_name)
                                            .or_default()
                                            .push(shelf_item);

                                        // Close the tab
                                        let _ = state.shell.close_tab(tab.id);
                                    }
                                }
                            }
                        }
                    }
                    UserEvent::EvaluateScript(script) => {
                        info!("EvaluateScript on chrome: {}", &script[..script.len().min(100)]);
                        chrome_for_events.evaluate_script(&script);
                    }
                    UserEvent::EvaluateContentScript(script) => {
                        content_for_events.evaluate_script(&script);
                    }
                    UserEvent::LoadAboutPage => {
                        info!("Loading about page");
                        load_about_page(&content_for_events);
                    }
                    UserEvent::LoadReportPage => {
                        info!("Loading analytics report page");
                        load_report_page(&content_for_events);
                    }
                    UserEvent::ReportRefusedNavigation(url) => {
                        let line = format!("Not allowed to load local resource: {url}");
                        content_for_events.evaluate_script(&format!("console.error({});", js_string(&line)));
                    }
                    UserEvent::PrintPage => {
                        info!("Triggering print dialog");
                        content_for_events.evaluate_script("window.print();");
                    }
                    UserEvent::ZoomIn => {
                        let mut s = state_for_events.lock().unwrap();
                        if let Some(tab) = s.shell.get_active_tab() {
                            let tab_id = tab.id.0.to_string();
                            let level = s.zoom_in(&tab_id);
                            let zoom_pct = (level * 100.0).round() as u32;
                            info!("Zoom in: {}%", zoom_pct);
                            content_for_events.evaluate_script(&format!(
                                "document.body.style.zoom = '{}%';",
                                zoom_pct
                            ));
                            // Notify chrome of zoom level change
                            chrome_for_events.evaluate_script(&format!(
                                "if(window.updateZoomIndicator) {{ updateZoomIndicator({}); }}",
                                level
                            ));
                        }
                    }
                    UserEvent::ZoomOut => {
                        let mut s = state_for_events.lock().unwrap();
                        if let Some(tab) = s.shell.get_active_tab() {
                            let tab_id = tab.id.0.to_string();
                            let level = s.zoom_out(&tab_id);
                            let zoom_pct = (level * 100.0).round() as u32;
                            info!("Zoom out: {}%", zoom_pct);
                            content_for_events.evaluate_script(&format!(
                                "document.body.style.zoom = '{}%';",
                                zoom_pct
                            ));
                            chrome_for_events.evaluate_script(&format!(
                                "if(window.updateZoomIndicator) {{ updateZoomIndicator({}); }}",
                                level
                            ));
                        }
                    }
                    UserEvent::ResetZoom => {
                        let mut s = state_for_events.lock().unwrap();
                        if let Some(tab) = s.shell.get_active_tab() {
                            let tab_id = tab.id.0.to_string();
                            let level = s.reset_zoom(&tab_id);
                            let zoom_pct = (level * 100.0).round() as u32;
                            info!("Reset zoom: {}%", zoom_pct);
                            content_for_events.evaluate_script(&format!(
                                "document.body.style.zoom = '{}%';",
                                zoom_pct
                            ));
                            chrome_for_events.evaluate_script(&format!(
                                "if(window.updateZoomIndicator) {{ updateZoomIndicator({}); }}",
                                level
                            ));
                        }
                    }
                    UserEvent::SyncZoomLevel => {
                        let s = state_for_events.lock().unwrap();
                        if let Some(tab) = s.shell.get_active_tab() {
                            let tab_id = tab.id.0.to_string();
                            let level = s.get_zoom_level(&tab_id);
                            let zoom_pct = (level * 100.0).round() as u32;
                            content_for_events.evaluate_script(&format!(
                                "document.body.style.zoom = '{}%';",
                                zoom_pct
                            ));
                            chrome_for_events.evaluate_script(&format!(
                                "if(window.updateZoomIndicator) {{ updateZoomIndicator({}); }}",
                                level
                            ));
                        }
                    }
                    UserEvent::TabAudioStateChanged { playing } => {
                        let mut s = state_for_events.lock().unwrap();
                        if let Some(tab) = s.shell.get_active_tab() {
                            let tab_id = tab.id.0.to_string();
                            s.set_tab_audio_playing(&tab_id, playing);
                            info!("Tab {} audio state: playing={}", tab_id, playing);
                            // Sync tabs to update UI
                            drop(s);
                            let _ = proxy.send_event(UserEvent::SyncTabs);
                        }
                    }
                    UserEvent::ToggleTabMute { ref id } => {
                        let mut s = state_for_events.lock().unwrap();
                        let new_muted = s.toggle_tab_mute(id);
                        info!("Tab {} mute toggled: now {}", id, if new_muted { "muted" } else { "unmuted" });
                        let is_active = s.shell.get_active_tab()
                            .map(|t| t.id.0.to_string() == *id)
                            .unwrap_or(false);
                        drop(s);
                        if is_active {
                            let script = if new_muted {
                                "document.querySelectorAll('video, audio').forEach(el => el.muted = true);"
                            } else {
                                "document.querySelectorAll('video, audio').forEach(el => el.muted = false);"
                            };
                            content_for_events.evaluate_script(script);
                        }
                        let _ = proxy.send_event(UserEvent::SyncTabs);
                    }
                    UserEvent::CloseActiveTab => {
                        info!("Close active tab from content WebView");
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { hiwaveChrome.closeActiveTab(); }"
                        );
                    }
                    UserEvent::FocusAddressBar => {
                        info!("Focus address bar from content WebView");
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { hiwaveChrome.focusAddressBar(); }"
                        );
                    }
                    UserEvent::OpenFind => {
                        info!("Open find from content WebView");
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { hiwaveChrome.openFind(); }"
                        );
                    }
                    UserEvent::ToggleSidebar => {
                        info!("Toggle sidebar from content WebView");
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { hiwaveChrome.toggleSidebar(); }"
                        );
                    }
                    UserEvent::OpenCommandPalette => {
                        info!("Open command palette from content WebView");
                        chrome_for_events.evaluate_script(
                            "if(window.hiwaveChrome) { hiwaveChrome.openCommandPalette(); }"
                        );
                    }
                    UserEvent::ActivateTabByIndex { index } => {
                        info!("Activate tab by index {} from content WebView", index);
                        chrome_for_events.evaluate_script(&format!(
                            "if(window.hiwaveChrome) {{ hiwaveChrome.activateTabByIndex({}); }}",
                            index
                        ));
                    }
                    UserEvent::TriggerAutofill => {
                        info!("Triggering autofill");
                        // Focus the password field if any
                        content_for_events.evaluate_script(
                            "if(window.hiwaveAutofill) { hiwaveAutofill.trigger(); }"
                        );
                    }
                    UserEvent::SendAutofillCredentials { credentials_json: _ } => {
                        // Autofill hardening: nothing is delivered into a content page.
                        warn!("Autofill delivery to a content page is disabled");
                    }
                }
            }
            _ => {}
        }

    });
    } // end cfg(not(feature = "native-linux"))
}

#[cfg(test)]
mod url_log_tests {
    use super::*;
    const SECRET: &str = "hunter2-a4-secret";

    #[test]
    fn a_url_in_the_log_has_no_query_or_fragment() {
        assert_eq!(url_for_log(&format!("https://a.test/p/q?x={SECRET}#{SECRET}")), "https://a.test/p/q");
        assert_eq!(url_for_log(&format!("https://a.test/p#{SECRET}")), "https://a.test/p");
        assert_eq!(url_for_log("about:blank"), "about:blank");
    }

    #[test]
    fn a_page_url_in_the_log_keeps_scheme_host_and_path_only() {
        // userinfo is a credential; a data: URL's payload is the page.
        assert_eq!(url_for_log(&format!("https://u:{SECRET}@a.test/p?q=1")), "https://a.test/p");
        assert_eq!(url_for_log(&format!("https://{SECRET}@a.test")), "https://a.test/");
        // An '@' in the path is not userinfo.
        assert_eq!(url_for_log("https://a.test/@user/x"), "https://a.test/@user/x");
        assert_eq!(url_for_log(&format!("data:text/html,<p>{SECRET}</p>")), format!("data: URL ({} bytes)", "data:text/html,<p></p>".len() + SECRET.len()));
        assert!(url_for_log(&format!("data:text/html;base64,{SECRET}")).starts_with("data: URL ("));
        assert_eq!(url_for_log("hiwave://newtab"), "hiwave://newtab");
    }

    // Ported from hiwave-windows #162 (content_ipc_guard.rs): the URL parser decides what is logged.
    #[test]
    fn url_for_log_does_not_panic_when_byte_five_is_inside_a_multibyte_char() {
        let url = "abc\u{20AC}tail";
        assert!(!url.is_char_boundary(5));
        assert_eq!(url_for_log(url), format!("unparsed URL ({} bytes)", url.len()));
        assert_eq!(url_for_log("DATA:text/html,x"), "data: URL (16 bytes)");
    }

    #[test]
    fn a_logged_url_is_scheme_host_and_path() {
        assert_eq!(url_for_log("https://user:pw@example.com/a/b?token=SECRET#frag"), "https://example.com/a/b");
        assert_eq!(url_for_log("http://x.test/?q=1"), "http://x.test/");
        assert_eq!(url_for_log("https://example.com/p#section"), "https://example.com/p");
        assert_eq!(url_for_log("hiwave://report"), "hiwave://report");
        assert_eq!(url_for_log("https://example.com:8443/a?x"), "https://example.com:8443/a");
        assert_eq!(url_for_log("about:blank"), "about:blank");
    }

    /// The URL parser decides where the userinfo ends, as the browser does: a backslash is a path
    /// separator in an http(s) URL, so a hand-split at the first '/' would keep the password.
    #[test]
    fn a_logged_url_follows_the_url_parser_not_a_hand_split() {
        assert_eq!(url_for_log("https://user:SECRET@example.com"), "https://example.com/");
        assert_eq!(url_for_log(&format!("https://user:SECRET@evil.test{}p", '\\')), "https://evil.test/p");
        assert_eq!(url_for_log("  data:text/html,secret"), "data: URL (21 bytes)");
        assert_eq!(url_for_log("not a url SECRET"), "unparsed URL (16 bytes)");
    }

    #[test]
    fn a_data_url_is_logged_by_size_and_a_long_one_is_cut() {
        let report = format!("data:text/html;charset=utf-8,<script>const REPORT_NONCE = '{}'</script>", "ab12".repeat(8));
        let logged = url_for_log(&report);
        assert!(!logged.contains("ab12") && !logged.contains("REPORT_NONCE"), "no page content: {logged}");
        assert!(logged.starts_with("data: URL ("));
        assert_eq!(url_for_log("https://example.com/a"), "https://example.com/a");
        let long = format!("https://example.com/{}", "x".repeat(500));
        assert!(url_for_log(&long).len() < 260);
    }

    #[test]
    fn a_navigation_line_has_the_full_url_only_when_bodies_are_logged() {
        let url = format!("https://a.test/login?token={SECRET}#{SECRET}");
        assert_eq!(page_url_for_log_as(&url, false), "https://a.test/login");
        assert_eq!(page_url_for_log_as(&url, true), url);
    }
}

#[cfg(test)]
mod content_ipc_scope_tests {
    const MAIN_RS: &str = include_str!("main.rs");

    /// The content view's IPC handler: the closure every page in the content view can reach.
    fn content_handler() -> &'static str {
        let start = MAIN_RS
            .find("let content_ipc: std::rc::Rc<dyn Fn(&str)>")
            .expect("the content view's IPC handler");
        let rest = &MAIN_RS[start..];
        &rest[..rest.find("\n    });").expect("the end of the content handler")]
    }

    /// A page does not open things outside the browser: the content view's handler has no
    /// `open_external` arm (macOS's has none either).
    #[test]
    fn a_page_cannot_open_an_external_url() {
        assert!(!content_handler().contains("IpcMessage::OpenExternal"));
    }

    /// The context menu closes the active tab; the content view closes no tab by its id.
    #[test]
    fn a_page_closes_only_the_active_tab() {
        let handler = content_handler();
        let arm = &handler[handler.find("IpcMessage::CloseTab { id } =>").expect("the close-tab arm")..];
        let arm = &arm[..arm.find("IpcMessage::Refresh").unwrap_or(arm.len())];
        assert!(!arm.contains("handle_message"), "{arm}");
    }
}

#[cfg(all(test, not(feature = "native-linux")))]
mod ipc_shape_tests {
    const MAIN_RS: &str = include_str!("main.rs");

    /// An IPC answer or message is logged by its shape (command, kind, size), never whole: the
    /// answer can carry what the vault or the history holds.
    #[test]
    fn no_ipc_answer_or_message_is_logged_whole() {
        let whole: Vec<&str> = MAIN_RS
            .lines()
            .filter(|l| l.contains("info!(") && l.contains("{:?}"))
            .filter(|l| l.contains(", response)") || l.contains(", ipc_msg)"))
            .collect();
        assert!(whole.is_empty(), "logged whole: {whole:#?}");
    }

    /// The picked element's path reaches the Inspector as a JavaScript string literal built by
    /// one function, not by hand-escaping quotes in place.
    #[test]
    fn the_picked_element_script_is_built_by_one_function() {
        let picked = MAIN_RS
            .split("UserEvent::InspectorElementPicked { path, styles } =>")
            .nth(1)
            .expect("the picked-element handler");
        let handler = &picked[..picked.find("UserEvent::").unwrap_or(picked.len())];
        assert!(!handler.contains("replace("), "hand-escaped path: {handler}");
        assert!(handler.contains("inspector_picked_script("), "{handler}");
    }

    /// The log line names the command and the answer's kind and size, never the answer's data.
    #[test]
    fn an_answer_is_logged_by_command_kind_and_size() {
        let secret = "pw-for-the-test";
        let answer = crate::ipc::IpcResponse::success(serde_json::json!({ "password": secret }));
        let line = super::ipc_log_shape(r#"{"cmd":"get_credentials"}"#, &answer);
        assert!(line.starts_with("get_credentials: success, "), "{line}");
        assert!(!line.contains(secret), "{line}");
        let odd = super::ipc_log_shape(r#"{"cmd":"Not A Command <x>"}"#, &answer);
        assert!(odd.starts_with("unknown: "), "{odd}");
    }

    /// A page's title or URL reaches the chrome's script as one string literal, whatever it holds.
    #[test]
    fn a_page_title_reaches_the_chrome_script_as_one_string_literal() {
        for title in ["it's", "ends in a backslash \\", "a\\'); window.__flag = 1; //", "two\nlines\r\u{2028}"] {
            let script = super::chrome_update_script("updateTitle", title);
            let literal = serde_json::to_string(title).unwrap();
            assert_eq!(script, format!("if(window.hiwaveChrome) {{ hiwaveChrome.updateTitle({literal}); }}"), "{title:?}");
        }
    }

    /// A path with quotes, a backslash or line breaks reaches the script as one string literal.
    #[test]
    fn the_picked_path_is_one_string_literal() {
        for path in ["div\\'); window.__flag = 1; } //", "div[title=\"a\\\\b\"] > p\n'x'\r\u{2028}"] {
            let script = super::inspector_picked_script(path, "null");
            let literal = serde_json::to_string(path).unwrap();
            assert!(script.contains(&format!("selectElementByPath({literal})")), "{script}");
        }
    }
}

#[cfg(all(test, not(feature = "native-linux")))]
mod content_ipc_scope_2_tests {
    const MAIN_RS: &str = include_str!("main.rs");

    fn content_handler() -> &'static str {
        let start = MAIN_RS
            .find("let content_ipc: std::rc::Rc<dyn Fn(&str)>")
            .expect("the content view's IPC handler");
        let rest = &MAIN_RS[start..];
        &rest[..rest.find("\n    });").expect("the end of the content handler")]
    }

    fn arm(name: &str) -> Option<&'static str> {
        let handler = content_handler();
        let at = handler.find(&format!("IpcMessage::{name}"))?;
        let rest = &handler[at..];
        let end = rest[1..].find("IpcMessage::").map(|i| i + 1).unwrap_or(rest.len());
        Some(&rest[..end])
    }

    /// The context menu shelves the active tab; a page names no other tab.
    #[test]
    fn a_page_shelves_only_the_active_tab() {
        let arm = arm("AddToShelf").expect("the add-to-shelf arm");
        assert!(!arm.contains("tab_id.clone()"), "{arm}");
    }

    /// A page's new tab opens only an http(s) URL, or none (Ctrl+T), checked before the tab exists.
    #[test]
    fn a_page_new_tab_url_is_checked_first() {
        let arm = arm("CreateTab").expect("the create-tab arm");
        let check = arm.find("content_new_tab_url_allowed(").expect("the URL check");
        assert!(check < arm.find("handle_message").unwrap_or(usize::MAX), "{arm}");
    }

    /// Autofill, and entering or toggling focus mode, are not served from the content view (Escape's
    /// exit_focus_mode is).
    #[test]
    fn a_page_neither_fills_nor_switches_focus_mode() {
        for name in ["TriggerAutofill =>", "EnterFocusMode =>", "ToggleFocusMode =>"] {
            assert!(arm(name).is_none(), "{name} is served to pages");
        }
    }

    /// The keys context_menu.js forwarded, now taken at window level; Escape stays the page's.
    #[test]
    fn the_window_takes_the_focus_mode_and_autofill_keys() {
        use super::{window_shortcut, WindowShortcut::*};
        assert_eq!(window_shortcut("F11", false, false, false), Some(ToggleFocusMode));
        assert_eq!(window_shortcut("F", true, true, false), Some(ToggleFocusMode));
        assert_eq!(window_shortcut("f", true, true, false), Some(ToggleFocusMode));
        assert_eq!(window_shortcut("L", true, true, false), Some(TriggerAutofill));
        for (key, ctrl, shift, alt) in [("Escape", false, false, false), ("f", true, false, false), ("L", false, true, false), ("L", true, true, true), ("F11", true, false, false), ("t", true, false, false)] {
            assert_eq!(window_shortcut(key, ctrl, shift, alt), None, "{key} ctrl={ctrl} shift={shift} alt={alt}");
        }
    }

    /// A page's window.open / target=_blank opens a tab under the same rule as its CreateTab: only
    /// an http(s) URL, checked before the tab exists. (The popup filter passes `hiwave:`, and the
    /// browser's Navigate would then open its own page for the page.)
    #[test]
    fn a_page_new_window_url_is_checked_before_its_tab_exists() {
        let start = MAIN_RS.find(".with_new_window_req_handler(").expect("the new-window handler");
        let handler = &MAIN_RS[start..];
        let handler = &handler[..handler.find(".with_ipc_handler(").expect("the end of the new-window handler")];
        let tab = handler.find("IpcMessage::CreateTab").expect("the new tab it opens");
        let check = handler.find("content_new_tab_url_allowed(Some(").unwrap_or(usize::MAX);
        assert!(check < tab, "the new-window handler opens a tab without checking its URL");
    }

    fn navigation_handler() -> &'static str {
        let start = MAIN_RS.find(".with_navigation_handler(move |url| {").expect("the navigation handler");
        let handler = &MAIN_RS[start..];
        &handler[..handler.find(".with_on_page_load_handler(").expect("its end")]
    }

    /// WebKitGTK calls the navigation handler for every frame's navigation, an iframe's too, with
    /// only its URL: the address bar, the tab's stored URL and the loading state are not set there.
    #[test]
    fn a_frames_navigation_sets_neither_the_address_bar_nor_the_tab_url() {
        let handler = navigation_handler();
        for event in ["UserEvent::UpdateUrl(", "UserEvent::UpdateActiveTabUrl(", "UserEvent::SetLoading(true)"] {
            assert!(!handler.contains(event), "the navigation handler sends {event}");
        }
    }

    /// The main frame's committed load (wry's page-load Started: load-changed, webview.uri()) is
    /// where the address bar and the tab's URL are set.
    #[test]
    fn the_main_frames_committed_load_sets_the_address_bar_and_tab_url() {
        let start = MAIN_RS.find(".with_on_page_load_handler(move |event, url| match event {").expect("the page-load handler");
        let handler = &MAIN_RS[start..];
        let started = &handler[handler.find("wry::PageLoadEvent::Started").expect("Started")..handler.find("wry::PageLoadEvent::Finished").expect("Finished")];
        assert!(started.contains("main_frame_url_update("), "{started}");
    }

    #[test]
    fn a_committed_page_url_follows_the_new_tab_rules() {
        use super::{main_frame_url_update as update, TabUrlUpdate::*};
        assert_eq!(update("https://a.test/x", false, false), Page("https://a.test/x".into()));
        assert_eq!(update("https://a.test/x", true, false), Page("https://a.test/x".into()));
        // The app's own pages commit as about:blank: on About the tab keeps hiwave://about.
        assert_eq!(update("about:blank", false, true), NewTab { store: true });
        assert_eq!(update("about:blank", true, true), NewTab { store: false });
        assert_eq!(update("hiwave://newtab", true, false), NewTab { store: true });
    }

    /// A page's own navigation to about:blank is that page, as in Chrome: the address bar shows
    /// about:blank and the tab keeps it, not the new-tab page's empty bar and hiwave://newtab.
    #[test]
    fn a_pages_own_about_blank_is_not_the_new_tab_page() {
        use super::{main_frame_url_update as update, TabUrlUpdate::*};
        assert_eq!(update("about:blank", false, false), Page("about:blank".into()));
        assert_eq!(update("about:blank", true, false), Page("about:blank".into()));
    }

    /// The marker: each app load is taken by the next main-frame about:blank commit; a page's own
    /// commit (any other URL) means the pending app loads were superseded.
    #[test]
    fn an_app_load_is_told_from_a_pages_about_blank() {
        use super::{mark_app_load, take_app_load, APP_LOADS_PENDING};
        use std::sync::atomic::Ordering;
        APP_LOADS_PENDING.store(0, Ordering::SeqCst);
        assert!(!take_app_load("about:blank"), "no app load pending");
        mark_app_load();
        assert!(take_app_load("about:blank"), "the app's load");
        assert!(!take_app_load("about:blank"), "taken once");
        mark_app_load();
        mark_app_load();
        assert!(!take_app_load("https://a.test/"), "a page committed");
        assert!(!take_app_load("about:blank"), "the superseded loads are gone");
    }

    /// Every way the app loads its own page into the content view marks the load first.
    #[test]
    fn every_app_page_load_is_marked() {
        for loader in ["fn load_new_tab(", "fn load_about_page(", "fn load_report_page("] {
            let at = MAIN_RS.find(loader).unwrap_or_else(|| panic!("{loader}"));
            let body = &MAIN_RS[at..at + MAIN_RS[at..].find("\n}\n").expect("its end")];
            assert!(body.contains("mark_app_load();"), "{loader} does not mark its load");
        }
        let start = MAIN_RS.find(".with_html(ABOUT_HTML)").expect("the start page");
        let before = &MAIN_RS[start.saturating_sub(1500)..start];
        assert!(before.contains("mark_app_load();"), "the start page's load is not marked");
    }

    /// A page's navigation to a browser page is refused whatever its case or leading blanks; web,
    /// blank and data URLs are not this check's business.
    #[test]
    fn a_page_cannot_navigate_to_the_browsers_own_pages() {
        // The URL parser also drops a tab or newline ANYWHERE in the input, not only leading.
        for bad in ["hiwave://report", "hiwave://about", "HIWAVE://About", "  hiwave://report", "\t\nhiwave:newtab", "hiwave:x", "hi\twave://report", "hiwave\n:x", "h\ri\nw\tave://about"] {
            assert!(!super::page_navigation_allowed(bad), "{bad:?}");
        }
        for ok in ["https://example.com/", "http://a.test/hiwave://x", "about:blank", "data:text/html,x", "file:///tmp/a.html"] {
            assert!(super::page_navigation_allowed(ok), "{ok:?}");
        }
    }

    /// A tab restored on About or Report reopens through the browser's own loader (not load_url,
    /// which the page-navigation check refuses); other browser URLs keep the start page.
    #[test]
    fn a_restored_browser_page_opens_through_the_browsers_own_loader() {
        use super::{start_page, StartPage::*};
        assert_eq!(start_page("hiwave://newtab"), Keep);
        assert_eq!(start_page("about:blank"), Keep);
        assert_eq!(start_page("hiwave://about"), About);
        assert_eq!(start_page("hiwave://report"), Report);
        assert_eq!(start_page("hiwave://settings"), Keep);
        assert_eq!(start_page("https://example.com/"), Url);
    }

    /// Each door a page navigates through checks first: the WebKit view's navigation handler (which
    /// no longer serves About or Report itself), and (below) the RustKit view's script and link
    /// navigations, which otherwise reach the browser's own Navigate.
    #[test]
    fn the_webkit_views_navigation_handler_checks_the_url_first() {
        let start = MAIN_RS.find(".with_navigation_handler(move |url| {").expect("the navigation handler");
        let handler = &MAIN_RS[start..];
        let handler = &handler[..handler.find(".with_on_page_load_handler(").expect("its end")];
        let check = handler.find("page_navigation_allowed(&url)").unwrap_or(usize::MAX);
        assert!(check < handler.find("is_new_tab_url(&url)").expect("the new-tab arm"), "navigation handler");
        assert!(!handler.contains("LoadAboutPage") && !handler.contains("LoadReportPage"), "the handler serves a browser page");
    }

    #[test]
    fn the_rustkit_views_page_navigations_check_the_url_first() {
        // The pump sends what page_navigation_event makes of the URL, and nothing else.
        let pump = include_str!("rustkit_content.rs");
        let taken = pump.find("take_script_navigation()").expect("the RustKit view's page navigations");
        let pump = &pump[taken..pump[taken..].find("\n        }\n").map(|e| taken + e).expect("its end")];
        assert_eq!(pump.matches("send_event(").count(), 1, "{pump}");
        assert!(pump.contains("send_event(page_navigation_event(url))"), "{pump}");
    }

    #[cfg(all(target_os = "linux", feature = "rustkit"))]
    #[test]
    fn a_rustkit_page_navigation_to_a_browser_page_becomes_a_refusal() {
        use super::UserEvent;
        use crate::rustkit_content::page_navigation_event;
        assert!(matches!(page_navigation_event("hiwave://report".into()), UserEvent::ReportRefusedNavigation(u) if u == "hiwave://report"));
        assert!(matches!(page_navigation_event("https://a.test/".into()), UserEvent::Navigate(u) if u == "https://a.test/"));
    }

    #[test]
    fn only_http_and_https_or_no_url_open_from_a_page() {
        for ok in [None, Some("https://example.com/"), Some("http://a.test/x"), Some("HTTPS://A.TEST")] {
            assert!(super::content_new_tab_url_allowed(ok), "{ok:?}");
        }
        for bad in ["javascript:alert(1)", "file:///etc/passwd", "data:text/html,x", "hiwave://settings", "about:blank", " https://a.test", "httpx://a", ""] {
            assert!(!super::content_new_tab_url_allowed(Some(bad)), "{bad}");
        }
    }
}

#[cfg(all(test, not(feature = "native-linux")))]
mod page_visibility_tests {
    use super::window_shows_pages;

    /// Upstream `a_minimised_or_hidden_window_shows_no_page`.
    #[test]
    fn a_minimised_or_hidden_window_shows_no_page() {
        assert!(window_shows_pages(true, false));
        assert!(!window_shows_pages(true, true), "minimised");
        assert!(!window_shows_pages(false, false), "hidden");
        assert!(!window_shows_pages(false, true));
    }

    /// The main window's focus and resize events tell its RustKit pages whether they are shown
    /// (a minimise and a restore each move the focus, and a restore resizes), as upstream's do.
    #[test]
    fn focus_and_resize_events_sync_page_visibility() {
        let main = include_str!("main.rs");
        let event_loop = &main[main.find("event_loop.run(").expect("the event loop")..];
        for event in ["WindowEvent::Focused(", "WindowEvent::Resized("] {
            let at = event_loop.find(event).unwrap_or_else(|| panic!("no {event} arm"));
            let arm = &event_loop[at..];
            let arm = &arm[..arm.find("Event::WindowEvent {").unwrap_or(arm.len())];
            assert!(arm.contains("sync_page_visibility("), "{event} arm does not sync page visibility");
        }
    }
}

/// Upstream (hiwave-macos) tests for main.rs, ported by LINUX-MAIN-AUDIT.
#[cfg(all(test, not(feature = "native-linux")))]
mod main_audit_port_tests {
    use super::*;
    use tracing_subscriber::filter::LevelFilter;

    /// Upstream `defaults_to_info_and_honors_a_rust_log_directive`.
    #[test]
    fn defaults_to_info_and_honors_a_rust_log_directive() {
        assert_eq!(rust_log_filter(None).max_level_hint(), Some(LevelFilter::INFO));
        assert_eq!(rust_log_filter(Some("")).max_level_hint(), Some(LevelFilter::INFO));
        assert_eq!(rust_log_filter(Some("   ")).max_level_hint(), Some(LevelFilter::INFO));
        assert_eq!(rust_log_filter(Some("===bad===")).max_level_hint(), Some(LevelFilter::INFO));
        assert_eq!(rust_log_filter(Some("debug")).max_level_hint(), Some(LevelFilter::DEBUG));
        assert_eq!(rust_log_filter(Some("warn")).max_level_hint(), Some(LevelFilter::WARN));
        assert_eq!(rust_log_filter(Some("info,rustkit_engine=debug")).max_level_hint(), Some(LevelFilter::DEBUG));
    }

    /// The keys `collectSettingsFromUI()` in settings.html puts in the `save_settings` payload, read
    /// from the page the app ships (upstream's helper).
    fn page_save_keys() -> Vec<String> {
        let start = SETTINGS_HTML
            .find("function collectSettingsFromUI()")
            .expect("settings.html has collectSettingsFromUI");
        let body = &SETTINGS_HTML[start..];
        let open = body.find("return {").expect("collectSettingsFromUI returns an object literal") + "return {".len();
        let close = open + body[open..].find("};").expect("the object literal ends");
        body[open..close]
            .lines()
            .filter_map(|line| {
                let key = line.trim().split(':').next()?.trim();
                (!key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')).then(|| key.to_string())
            })
            .collect()
    }

    /// Upstream `the_page_sends_every_settings_field`: a field added to one side and not the other
    /// fails here.
    #[test]
    fn the_page_sends_every_settings_field() {
        let keys = page_save_keys();
        let defaults = serde_json::to_value(crate::state::UserSettings::default()).unwrap();
        let fields: Vec<&String> = defaults.as_object().unwrap().keys().collect();
        let missing: Vec<&&String> = fields.iter().filter(|f| !keys.contains(f)).collect();
        let unknown: Vec<&String> = keys.iter().filter(|k| !fields.contains(k)).collect();
        assert!(missing.is_empty() && unknown.is_empty(), "page omits {missing:?}, sends unknown {unknown:?}");
    }

    /// Upstream's credential-delivery pin: no statement in main.rs or ipc/autofill.rs posts
    /// credentials into a page (`window.ipc.postMessage(..)`, page to app, is the other direction),
    /// and the credential message type is not in the app.
    #[test]
    fn nothing_in_the_app_posts_credentials_into_a_page() {
        let main = include_str!("main.rs").replace("\r\n", "\n");
        let production = &main[..main.find("#[cfg(test)]\nmod ").unwrap_or(main.len())];
        let autofill = include_str!("ipc/autofill.rs").replace("\r\n", "\n");
        let autofill_production = autofill.split("#[cfg(test)]").next().unwrap_or("");
        for (file, text) in [("main.rs", production), ("ipc/autofill.rs", autofill_production)] {
            for statement in text.split(';') {
                let posts_to_page = statement.contains("postMessage") && !statement.contains("ipc.postMessage");
                assert!(
                    !(posts_to_page && (statement.contains("credentials") || statement.contains("password"))),
                    "{file} posts credentials into a page: {}",
                    statement.trim().lines().next().unwrap_or("")
                );
            }
        }
        let marker = ["hiwave_autofill", "_credentials"].concat();
        assert!(!production.contains(&marker) && !autofill_production.contains(&marker));
    }
}

/// Linux: every engine-backed test in this binary takes `engine_test_turn` (software Vulkan
/// deadlocks or crashes with several GPU devices alive at once; SOFTWARE-GPU-TEARDOWN).
#[cfg(all(test, target_os = "linux", feature = "rustkit", not(feature = "native-linux")))]
mod engine_test_turn_pin_tests {
    /// Each Inspector page test that builds an engine (`inspector()`) takes its turn first.
    #[test]
    fn every_inspector_page_test_takes_its_turn_before_its_engine() {
        let src = include_str!("inspector_page_tests.rs");
        let mut missing = Vec::new();
        for test in src.split("#[test]").skip(1) {
            let name = test.split("fn ").nth(1).and_then(|s| s.split('(').next()).unwrap_or("?");
            let body = &test[..test.find("\n}\n").unwrap_or(test.len())];
            if let Some(engine) = body.find("inspector()") {
                match body.find("one_engine_at_a_time()") {
                    Some(turn) if turn < engine => {}
                    _ => missing.push(name.to_string()),
                }
            }
        }
        assert!(missing.is_empty(), "build an engine without the turn: {missing:?}");
    }
}


/// Text a page controls (its title and URL, an element path, console input) reaches a script the app
/// builds only as a `js_string` literal; and the inspector data a content page posts is read into the
/// shape the injected scripts send before it is forwarded (`ipc::inspector_payload`), as upstream.
#[cfg(all(test, not(feature = "native-linux")))]
mod script_string_tests {
    const MAIN_RS: &str = include_str!("main.rs");

    /// main.rs without its test modules.
    fn app_code() -> &'static str {
        &MAIN_RS[..MAIN_RS.find("\n#[cfg(test)]\nmod ").expect("the first test module")]
    }

    #[test]
    fn no_script_is_built_by_escaping_quotes_by_hand() {
        let code = app_code();
        for (n, line) in code.lines().enumerate() {
            assert!(!line.contains(r#".replace("'", "\\'")"#), "main.rs:{}: {}", n + 1, line.trim());
            assert!(!line.contains("('{}')"), "main.rs:{}: {}", n + 1, line.trim());
        }
    }

    fn content_handler() -> &'static str {
        let start = MAIN_RS.find("let content_ipc: std::rc::Rc<dyn Fn(&str)>").expect("the content view's IPC handler");
        let rest = &MAIN_RS[start..];
        &rest[..rest.find("\n    });").expect("the end of the content handler")]
    }

    fn arm(name: &str) -> &'static str {
        let handler = content_handler();
        let at = handler.find(&format!("IpcMessage::{name} ")).unwrap_or_else(|| panic!("the {name} arm"));
        let rest = &handler[at..];
        let end = rest[1..].find("IpcMessage::").map(|i| i + 1).unwrap_or(rest.len());
        &rest[..end]
    }

    #[test]
    fn inspector_data_from_a_content_page_is_read_before_it_is_forwarded() {
        for (name, reader) in [
            ("InspectorDomTreeResult", "inspector_payload::dom_tree("),
            ("InspectorElementStylesResult", "inspector_payload::element_styles("),
            ("InspectorElementPicked", "inspector_payload::element_styles("),
            ("InspectorConsoleMessage", "inspector_payload::console_entry("),
        ] {
            assert!(arm(name).contains(reader), "{name} must go through {reader}");
        }
    }
}
