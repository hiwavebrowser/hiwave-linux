//! # RustKit Engine
//!
//! Browser engine orchestration layer that integrates all RustKit components
//! to provide a complete multi-view browser engine.
//!
//! ## Design Goals
//!
//! 1. **Multi-view support**: Manage multiple independent browser views
//! 2. **Unified API**: Single entry point for all browser functionality
//! 3. **Event coordination**: Route events between views and host
//! 4. **Resource sharing**: Share compositor and network resources

use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use rustkit_bindings::DomBindings;
// Re-export types for external use
pub use rustkit_bindings::IpcMessage;
pub use rustkit_renderer::{RenderStats, ScreenshotMetadata};
use rustkit_compositor::Compositor;
use rustkit_core::{LoadEvent, NavigationRequest, NavigationStateMachine};
use rustkit_css::{ComputedStyle, PropertyValue, Stylesheet};
use rustkit_dom::{Document, Node, NodeType};
use rustkit_image::ImageManager;
use rustkit_js::JsRuntime;
use rustkit_layout::{BoxType, Dimensions, DisplayList, LayoutBox, Rect};
use rustkit_net::{LoaderConfig, NetError, Request, ResourceLoader};
use rustkit_renderer::Renderer;
use rustkit_viewhost::{Bounds, ViewHost, ViewId};
use thiserror::Error;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};
use url::Url;
#[cfg(windows)]
use windows::Win32::Foundation::HWND;

/// Errors that can occur in the engine.
#[derive(Error, Debug)]
pub enum EngineError {
    #[error("View error: {0}")]
    ViewError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] NetError),

    #[error("Navigation error: {0}")]
    NavigationError(String),

    #[error("Render error: {0}")]
    RenderError(String),

    #[error("JS error: {0}")]
    JsError(String),

    #[error("View not found: {0:?}")]
    ViewNotFound(EngineViewId),
}

/// Unique identifier for an engine view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EngineViewId(u64);

impl EngineViewId {
    fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        Self(COUNTER.fetch_add(1, Ordering::Relaxed))
    }

    pub fn raw(&self) -> u64 {
        self.0
    }
}

/// Engine events emitted to the host application.
#[derive(Debug, Clone)]
pub enum EngineEvent {
    /// Navigation started.
    NavigationStarted { view_id: EngineViewId, url: Url },
    /// Navigation committed (first bytes received).
    NavigationCommitted { view_id: EngineViewId, url: Url },
    /// Page fully loaded.
    PageLoaded {
        view_id: EngineViewId,
        url: Url,
        title: Option<String>,
    },
    /// Navigation failed.
    NavigationFailed {
        view_id: EngineViewId,
        url: Url,
        error: String,
    },
    /// Title changed.
    TitleChanged {
        view_id: EngineViewId,
        title: String,
    },
    /// Console message from JavaScript.
    ConsoleMessage {
        view_id: EngineViewId,
        level: String,
        message: String,
    },
    /// View resized.
    ViewResized {
        view_id: EngineViewId,
        width: u32,
        height: u32,
    },
    /// View received focus.
    ViewFocused { view_id: EngineViewId },
    /// Download started.
    DownloadStarted { url: Url, filename: String },
    /// Image loaded.
    ImageLoaded {
        view_id: EngineViewId,
        url: Url,
        width: u32,
        height: u32,
    },
    /// Image failed to load.
    ImageError {
        view_id: EngineViewId,
        url: Url,
        error: String,
    },
    /// Favicon detected.
    FaviconDetected {
        view_id: EngineViewId,
        url: Url,
    },
}

/// View state.
#[allow(dead_code)]
struct ViewState {
    id: EngineViewId,
    viewhost_id: ViewId,
    url: Option<Url>,
    title: Option<String>,
    document: Option<Rc<Document>>,
    #[allow(dead_code)]
    layout: Option<LayoutBox>,
    #[allow(dead_code)]
    display_list: Option<DisplayList>,
    #[allow(dead_code)]
    bindings: Option<DomBindings>,
    navigation: NavigationStateMachine,
    #[allow(dead_code)]
    nav_event_rx: mpsc::UnboundedReceiver<LoadEvent>,
    /// Currently focused DOM node.
    focused_node: Option<rustkit_dom::NodeId>,
    /// Whether the view itself has focus.
    view_focused: bool,
    /// CSS text of every successfully fetched `<link rel="stylesheet">`, in
    /// document order. Held on the view rather than re-fetched at layout time
    /// because relayout is synchronous and runs on every resize.
    external_css: String,
}

impl ViewState {
    /// Attach a newly-loaded document to this view, resetting the state that
    /// belongs to the document being REPLACED.
    ///
    /// This exists because assigning `document` alone was a cross-document
    /// style leak: `external_css` outlives a single navigation by design (it
    /// must survive the synchronous relayout a resize triggers), so nothing
    /// cleared it when the next document had no `<link rel="stylesheet">` of
    /// its own. Navigating from a styled page to an unstyled one left the old
    /// page's rules cascading, and nothing logged anything.
    ///
    /// The reset lives HERE, at the single point where a document is
    /// attached, rather than in the stylesheet loader. Putting it in the
    /// loader would only work for load paths that remember to call the loader
    /// - and `load_html` does not call it at all, which was the second door
    /// into the same bug. Any per-document cache added to `ViewState` later
    /// should be cleared in this method and nowhere else.
    fn attach_document(&mut self, document: Rc<Document>) {
        self.document = Some(document);
        self.external_css = String::new();
    }

    /// A view with no window and no compositor surface, for tests that
    /// exercise per-view STATE rather than presentation. `create_view` needs a
    /// real window handle and a GPU surface; requiring those to test a String
    /// field is what put GPU init on the test path and produced the parallel
    /// SIGSEGV earlier in this port.
    #[cfg(test)]
    fn headless_for_test() -> Self {
        let (nav_tx, nav_rx) = mpsc::unbounded_channel();
        ViewState {
            id: EngineViewId::new(),
            viewhost_id: ViewId::new(),
            url: None,
            title: None,
            document: None,
            layout: None,
            display_list: None,
            bindings: None,
            navigation: NavigationStateMachine::new(nav_tx),
            nav_event_rx: nav_rx,
            focused_node: None,
            view_focused: false,
            external_css: String::new(),
        }
    }
}

/// Engine configuration.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// User agent string.
    pub user_agent: String,
    /// Enable JavaScript.
    pub javascript_enabled: bool,
    /// Enable cookies.
    pub cookies_enabled: bool,
    /// Default background color.
    pub background_color: [f64; 4],
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            user_agent: "RustKit/1.0 HiWave/1.0".to_string(),
            javascript_enabled: true,
            cookies_enabled: true,
            background_color: [1.0, 1.0, 1.0, 1.0], // White
        }
    }
}

/// The main browser engine.
pub struct Engine {
    config: EngineConfig,
    viewhost: ViewHost,
    compositor: Compositor,
    renderer: Option<Renderer>,
    loader: Arc<ResourceLoader>,
    image_manager: Arc<ImageManager>,
    views: HashMap<EngineViewId, ViewState>,
    event_tx: mpsc::UnboundedSender<EngineEvent>,
    event_rx: Option<mpsc::UnboundedReceiver<EngineEvent>>,
}

/// Minimal identity of an ancestor element, captured while walking the DOM so
/// descendant selectors (`.card p`) can verify the ancestor chain instead of
/// matching on the subject alone. (= Windows/macOS `ElementCtx`.)
/// How one compound selector is joined to the compound on its right.
///
/// Sibling combinators (`+`, `~`) are deliberately absent: they need the
/// element's previous siblings, which the style walk does not carry yet.
/// Adding the variant without the context would be a promise the matcher
/// cannot keep.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Combinator {
    /// `a b` - some ancestor matches.
    Descendant,
    /// `a > b` - the immediate parent matches.
    Child,
}

#[derive(Clone)]
struct ElementCtx {
    tag: String,
    classes: Vec<String>,
    id: Option<String>,
}

impl Engine {
    /// Create a new browser engine.
    pub fn new(config: EngineConfig) -> Result<Self, EngineError> {
        Self::with_interceptor(config, None)
    }

    /// Create a new browser engine with an optional request interceptor.
    pub fn with_interceptor(
        config: EngineConfig,
        interceptor: Option<rustkit_net::RequestInterceptor>,
    ) -> Result<Self, EngineError> {
        info!("Initializing RustKit Engine");

        // Initialize ViewHost
        let viewhost = ViewHost::new();

        // Initialize Compositor
        let compositor = Compositor::new().map_err(|e| EngineError::RenderError(e.to_string()))?;

        // Initialize ResourceLoader
        let loader_config = LoaderConfig {
            user_agent: config.user_agent.clone(),
            cookies_enabled: config.cookies_enabled,
            ..Default::default()
        };
        let loader = if let Some(interceptor) = interceptor {
            info!("ResourceLoader initialized with request interceptor");
            Arc::new(
                ResourceLoader::with_interceptor(loader_config, interceptor)
                    .map_err(EngineError::NetworkError)?,
            )
        } else {
            Arc::new(ResourceLoader::new(loader_config).map_err(EngineError::NetworkError)?)
        };

        // Initialize ImageManager
        let image_manager = Arc::new(ImageManager::new());

        // Initialize Renderer
        let renderer = Renderer::new(
            compositor.device_arc(),
            compositor.queue_arc(),
            compositor.surface_format(),
        ).map_err(|e| EngineError::RenderError(e.to_string()))?;

        // Event channel
        let (event_tx, event_rx) = mpsc::unbounded_channel();

        info!(
            adapter = ?compositor.adapter_info().name,
            "Engine initialized with GPU renderer"
        );

        Ok(Self {
            config,
            viewhost,
            compositor,
            renderer: Some(renderer),
            loader,
            image_manager,
            views: HashMap::new(),
            event_tx,
            event_rx: Some(event_rx),
        })
    }

    /// Take the event receiver.
    pub fn take_event_receiver(&mut self) -> Option<mpsc::UnboundedReceiver<EngineEvent>> {
        self.event_rx.take()
    }

    /// Create a new view.
    #[cfg(windows)]
    pub fn create_view(
        &mut self,
        parent: HWND,
        bounds: Bounds,
    ) -> Result<EngineViewId, EngineError> {
        let id = EngineViewId::new();

        debug!(?id, ?bounds, "Creating view");

        // Create viewhost view
        let viewhost_id = self
            .viewhost
            .create_view(parent, bounds)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        // Create compositor surface
        let hwnd = self
            .viewhost
            .get_hwnd(viewhost_id)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        unsafe {
            self.compositor
                .create_surface_for_hwnd(viewhost_id, hwnd, bounds.width, bounds.height)
                .map_err(|e| EngineError::RenderError(e.to_string()))?;
        }

        // Create navigation state machine
        let (nav_tx, nav_rx) = mpsc::unbounded_channel();
        let navigation = NavigationStateMachine::new(nav_tx);

        // Create view state
        let view_state = ViewState {
            id,
            viewhost_id,
            url: None,
            title: None,
            document: None,
            layout: None,
            display_list: None,
            bindings: None,
            navigation,
            nav_event_rx: nav_rx,
            focused_node: None,
            view_focused: false,
            external_css: String::new(),
        };

        self.views.insert(id, view_state);

        // Render initial background
        self.compositor
            .render_solid_color(viewhost_id, self.config.background_color)
            .map_err(|e| EngineError::RenderError(e.to_string()))?;

        info!(?id, "View created");
        Ok(id)
    }

    #[cfg(not(windows))]
    pub fn create_view(
        &mut self,
        _parent: usize,
        _bounds: Bounds,
    ) -> Result<EngineViewId, EngineError> {
        Err(EngineError::RenderError("create_view is only supported on Windows".to_string()))
    }

    /// Destroy a view.
    pub fn destroy_view(&mut self, id: EngineViewId) -> Result<(), EngineError> {
        let view = self
            .views
            .remove(&id)
            .ok_or(EngineError::ViewNotFound(id))?;

        // Destroy compositor surface
        let _ = self.compositor.destroy_surface(view.viewhost_id);

        // Destroy viewhost view
        let _ = self.viewhost.destroy_view(view.viewhost_id);

        info!(?id, "View destroyed");
        Ok(())
    }

    /// Resize a view.
    pub fn resize_view(&mut self, id: EngineViewId, bounds: Bounds) -> Result<(), EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;

        debug!(?id, ?bounds, "Resizing view");

        // Resize viewhost
        self.viewhost
            .set_bounds(view.viewhost_id, bounds)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        // Resize compositor surface
        self.compositor
            .resize_surface(view.viewhost_id, bounds.width, bounds.height)
            .map_err(|e| EngineError::RenderError(e.to_string()))?;

        // Re-layout if we have content
        if self.views.get(&id).unwrap().document.is_some() {
            self.relayout(id)?;
        }

        // Emit event
        let _ = self.event_tx.send(EngineEvent::ViewResized {
            view_id: id,
            width: bounds.width,
            height: bounds.height,
        });

        Ok(())
    }

    /// Focus a view.
    pub fn focus_view(&self, id: EngineViewId) -> Result<(), EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;

        debug!(?id, "Focusing view");

        self.viewhost
            .focus(view.viewhost_id)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        Ok(())
    }

    /// Set view visibility.
    pub fn set_view_visible(&self, id: EngineViewId, visible: bool) -> Result<(), EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;

        debug!(?id, visible, "Setting view visibility");

        self.viewhost
            .set_visible(view.viewhost_id, visible)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        Ok(())
    }

    /// Load a URL in a view.
    pub async fn load_url(&mut self, id: EngineViewId, url: Url) -> Result<(), EngineError> {
        let view = self
            .views
            .get_mut(&id)
            .ok_or(EngineError::ViewNotFound(id))?;

        info!(?id, %url, "Loading URL");

        // Start navigation
        let request = NavigationRequest::new(url.clone());
        view.navigation
            .start_navigation(request)
            .map_err(|e| EngineError::NavigationError(e.to_string()))?;

        // Emit event
        let _ = self.event_tx.send(EngineEvent::NavigationStarted {
            view_id: id,
            url: url.clone(),
        });

        // Fetch the URL
        let request = Request::get(url.clone());
        let response = self.loader.fetch(request).await?;

        if !response.ok() {
            let error = format!("HTTP {}", response.status);
            let view = self.views.get_mut(&id).unwrap();
            view.navigation
                .fail_navigation(error.clone())
                .map_err(|e| EngineError::NavigationError(e.to_string()))?;

            let _ = self.event_tx.send(EngineEvent::NavigationFailed {
                view_id: id,
                url,
                error,
            });

            return Err(EngineError::NavigationError("HTTP error".into()));
        }

        // Commit navigation
        let view = self.views.get_mut(&id).unwrap();
        view.navigation
            .commit_navigation()
            .map_err(|e| EngineError::NavigationError(e.to_string()))?;

        let _ = self.event_tx.send(EngineEvent::NavigationCommitted {
            view_id: id,
            url: url.clone(),
        });

        // Parse HTML
        let html = response.text().await?;
        let document =
            Document::parse_html(&html).map_err(|e| EngineError::RenderError(e.to_string()))?;
        let document = Rc::new(document);

        // Get title
        let title = document.title();

        // Store in view
        let view = self.views.get_mut(&id).unwrap();
        view.url = Some(url.clone());
        view.attach_document(document.clone());
        view.title = title.clone();

        // Initialize JavaScript if enabled
        if self.config.javascript_enabled {
            let js_runtime = JsRuntime::new().map_err(|e| EngineError::JsError(e.to_string()))?;

            let bindings =
                DomBindings::new(js_runtime).map_err(|e| EngineError::JsError(e.to_string()))?;

            bindings
                .set_document(document.clone())
                .map_err(|e| EngineError::JsError(e.to_string()))?;

            bindings
                .set_location(&url)
                .map_err(|e| EngineError::JsError(e.to_string()))?;

            let view = self.views.get_mut(&id).unwrap();
            view.bindings = Some(bindings);
        }

        // Fetch <link rel="stylesheet"> BEFORE layout, so external rules take
        // part in the very first cascade instead of appearing on a later
        // repaint (a flash of unstyled content).
        self.load_external_stylesheets(id, &document, &url).await;

        // Layout and render
        self.relayout(id)?;

        // Finish navigation
        let view = self.views.get_mut(&id).unwrap();
        view.navigation
            .finish_navigation()
            .map_err(|e| EngineError::NavigationError(e.to_string()))?;

        // Emit events
        if let Some(ref title) = title {
            let _ = self.event_tx.send(EngineEvent::TitleChanged {
                view_id: id,
                title: title.clone(),
            });
        }

        let _ = self.event_tx.send(EngineEvent::PageLoaded {
            view_id: id,
            url,
            title: view.title.clone(),
        });

        Ok(())
    }

    /// Load HTML content directly into a view.
    ///
    /// This is used for loading inline HTML content like the Chrome UI,
    /// without making an HTTP request.
    pub fn load_html(&mut self, id: EngineViewId, html: &str) -> Result<(), EngineError> {
        let view = self
            .views
            .get_mut(&id)
            .ok_or(EngineError::ViewNotFound(id))?;

        info!(?id, len = html.len(), "HTML: loading content");
        
        // Log first 100 chars of HTML for debugging
        let preview: String = html.chars().take(100).collect();
        info!(?id, preview = %preview, "HTML: preview");

        // Use a synthetic about:blank URL for inline content
        let url = Url::parse("about:blank").unwrap();

        // Start navigation
        let request = NavigationRequest::new(url.clone());
        view.navigation
            .start_navigation(request)
            .map_err(|e| EngineError::NavigationError(e.to_string()))?;

        // Emit event
        let _ = self.event_tx.send(EngineEvent::NavigationStarted {
            view_id: id,
            url: url.clone(),
        });

        // Commit navigation
        view.navigation
            .commit_navigation()
            .map_err(|e| EngineError::NavigationError(e.to_string()))?;

        let _ = self.event_tx.send(EngineEvent::NavigationCommitted {
            view_id: id,
            url: url.clone(),
        });

        // Parse HTML
        let document =
            Document::parse_html(html).map_err(|e| EngineError::RenderError(e.to_string()))?;
        let document = Rc::new(document);

        // Get title
        let title = document.title();

        // Store in view
        let view = self.views.get_mut(&id).unwrap();
        view.url = Some(url.clone());
        view.attach_document(document.clone());
        view.title = title.clone();

        // Initialize JavaScript if enabled
        if self.config.javascript_enabled {
            let js_runtime = JsRuntime::new().map_err(|e| EngineError::JsError(e.to_string()))?;

            let bindings =
                DomBindings::new(js_runtime).map_err(|e| EngineError::JsError(e.to_string()))?;

            bindings
                .set_document(document.clone())
                .map_err(|e| EngineError::JsError(e.to_string()))?;

            bindings
                .set_location(&url)
                .map_err(|e| EngineError::JsError(e.to_string()))?;

            let view = self.views.get_mut(&id).unwrap();
            view.bindings = Some(bindings);
        }

        // Layout and render
        self.relayout(id)?;

        // Finish navigation
        let view = self.views.get_mut(&id).unwrap();
        view.navigation
            .finish_navigation()
            .map_err(|e| EngineError::NavigationError(e.to_string()))?;

        // Emit events
        if let Some(ref title) = title {
            let _ = self.event_tx.send(EngineEvent::TitleChanged {
                view_id: id,
                title: title.clone(),
            });
        }

        let _ = self.event_tx.send(EngineEvent::PageLoaded {
            view_id: id,
            url,
            title: view.title.clone(),
        });

        Ok(())
    }

    /// Re-layout a view.
    fn relayout(&mut self, id: EngineViewId) -> Result<(), EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;

        let document = view
            .document
            .as_ref()
            .ok_or(EngineError::RenderError("No document".into()))?
            .clone();

        // Get view bounds
        let bounds = self
            .viewhost
            .get_bounds(view.viewhost_id)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        info!(
            ?id,
            width = bounds.width,
            height = bounds.height,
            "Layout: starting"
        );

        // Create containing block
        // NOTE: content.height is used as a cursor for vertical positioning, so it starts at 0.
        // The available viewport size is stored in the rect's width/height.
        let containing_block = Dimensions {
            content: Rect::new(0.0, 0.0, bounds.width as f32, 0.0), // height=0 means cursor at top
            ..Default::default()
        };

        // Build layout tree from DOM
        // Use the CSS fetched for this view by load_external_stylesheets, so a
        // resize re-lays-out with the external rules still applied rather than
        // silently dropping them (relayout is synchronous and cannot re-fetch).
        let external_css = self
            .views
            .get(&id)
            .map(|v| v.external_css.clone())
            .unwrap_or_default();
        let mut root_box = self.build_layout_with_external_css(&document, &external_css);

        // Count children for debugging
        let child_count = root_box.children.len();
        info!(?id, child_count, "Layout: built tree from DOM");

        // Layout
        root_box.layout(&containing_block);

        // Generate display list
        let display_list = DisplayList::build(&root_box);

        // Count command types for debugging
        let mut solid_count = 0;
        let mut text_count = 0;
        let mut border_count = 0;
        let mut other_count = 0;
        for cmd in &display_list.commands {
            match cmd {
                rustkit_layout::DisplayCommand::SolidColor(_, _) => solid_count += 1,
                rustkit_layout::DisplayCommand::Text { .. } => text_count += 1,
                rustkit_layout::DisplayCommand::Border { .. } => border_count += 1,
                _ => other_count += 1,
            }
        }
        
        info!(
            ?id,
            num_commands = display_list.commands.len(),
            solid_count,
            text_count,
            border_count,
            other_count,
            "Layout: generated display list"
        );
        
        // Print first few text commands for debugging
        for (i, cmd) in display_list.commands.iter().enumerate() {
            if let rustkit_layout::DisplayCommand::Text { text, x, y, font_size, .. } = cmd {
                if i < 5 {
                    info!(
                        ?id,
                        index = i,
                        text = %text,
                        x = x,
                        y = y,
                        font_size = font_size,
                        "Layout: text command"
                    );
                }
            }
        }

        // Store
        let view = self.views.get_mut(&id).unwrap();
        view.layout = Some(root_box);
        view.display_list = Some(display_list);

        // Render
        self.render(id)?;

        Ok(())
    }

    /// Build a layout tree from a DOM document.
    fn build_layout_from_document(&self, document: &Document) -> LayoutBox {
        self.build_layout_with_external_css(document, "")
    }

    /// Same, plus the CSS text of any fetched external stylesheets.
    ///
    /// `external_css` is placed BEFORE the inline `<style>` text so that an
    /// inline rule wins over an external one at equal specificity. That is the
    /// common authoring order (`<link>` then `<style>` in `<head>`) but it is a
    /// SIMPLIFICATION: true CSS cascade order follows the document position of
    /// each element, so a `<style>` appearing BEFORE a `<link>` is applied in
    /// the wrong order here. Stated rather than glossed - fixing it needs
    /// per-element source ordering, which this change does not add and which
    /// Prometheus scoped as a separate correctness PR for all trees.
    /// (= the declared simplification in Athena's Windows #54.)
    fn build_layout_with_external_css(
        &self,
        document: &Document,
        external_css: &str,
    ) -> LayoutBox {
        // L0 SUBSTRATE: collect and parse author stylesheets from <style>
        // elements so their rules can reach the cascade. Before this, Linux had
        // no author-stylesheet path at all - <style> was skipped like <link>,
        // so a page was styled ONLY by inline style attributes.
        let mut css_text = String::new();
        if !external_css.is_empty() {
            css_text.push_str(external_css);
            css_text.push('\n');
        }
        self.collect_style_text(&document.root(), &mut css_text);
        let sheet = Stylesheet::parse(&css_text).unwrap_or_else(|_| Stylesheet::new());
        info!(
            rule_count = sheet.rules.len(),
            css_len = css_text.len(),
            "CSS: author stylesheet parsed"
        );

        // The initial (root) style every top-level box inherits from. Colour
        // starts BLACK here - once, at the root - rather than being forced on
        // every element, which is what previously defeated inheritance.
        let mut root_inherited = ComputedStyle::new();
        root_inherited.color = rustkit_css::Color::BLACK;

        // COMPUTE THE <html> ELEMENT'S STYLE so its inherited properties reach
        // <body> and everything below.
        //
        // Layout starts at <body>, and this tree previously built body from a
        // bare root_inherited - so every inherited property an author set on
        // <html> was silently dropped. Measured before the fix, with
        // `html { font-size:20px; color:#f00; line-height:1.5;
        // font-family:Georgia; text-align:center }`:
        //
        //   font_size    Px(16.0)      expected Px(20)
        //   color        black         expected red
        //   line_height  1.2           expected 1.5
        //   font_family  "sans-serif"  expected Georgia
        //   text_align   Left          expected Center
        //
        // ALL FIVE dropped. That matters more than it looks: parity-reset.css
        // and most real stylesheets set inherited properties on `html`, so the
        // page starts from the wrong baseline before a single author rule for
        // an element is considered.
        //
        // The reference already does this (macos rustkit-engine
        // build_layout_from_document) and carries the scar comment for the same
        // bug: "building body with parent_style = None silently dropped them
        // (e.g. html { line-height: 1.5 } never reached any heading)". PORT
        // DEFECT - measured on both sides this time, behaviour not grep.
        //
        // Found from Argos's soft note on #46: he observed that Linux builds
        // from body with root_inherited when html is not body's parent. He
        // flagged it as pre-existing and non-blocking; it turned out to drop
        // every inherited property on the element.
        if let Some(html_el) = document.root().children().iter().find(|c| {
            matches!(&c.node_type, NodeType::Element { tag_name, .. }
                     if tag_name.eq_ignore_ascii_case("html"))
        }) {
            if let NodeType::Element { tag_name, attributes, .. } = &html_el.node_type {
                root_inherited = self.compute_style_for_element(
                    tag_name,
                    attributes,
                    &sheet,
                    &root_inherited,
                    &[],
                );
            }
        }

        // Create root layout box for the document
        let mut root_style = ComputedStyle::new();
        root_style.background_color = rustkit_css::Color::WHITE;
        let mut root_box = LayoutBox::new(BoxType::Block, root_style);

        // Debug: print root children to understand DOM structure
        let root_children = document.root().children();
        info!(
            root_children = root_children.len(),
            "DOM: document root children count"
        );
        for (i, child) in root_children.iter().take(5).enumerate() {
            if let NodeType::Element { tag_name, .. } = &child.node_type {
                info!(index = i, tag = %tag_name, "DOM: root child");
                // Print grandchildren too
                for (j, grandchild) in child.children().iter().take(3).enumerate() {
                    if let NodeType::Element { tag_name, .. } = &grandchild.node_type {
                        info!(index = j, tag = %tag_name, "DOM: grandchild of root");
                    }
                }
            } else if let NodeType::DocumentType { name, .. } = &child.node_type {
                info!(index = i, name = %name, "DOM: root child (doctype)");
            }
        }

        // Get the body element and build layout from it
        if let Some(body) = document.body() {
            // Debug: count body's children
            let body_children = body.children();
            info!(
                body_children = body_children.len(),
                "DOM: body element found"
            );
            
            // Debug: print first few children tags
            for (i, child) in body_children.iter().take(5).enumerate() {
                if let NodeType::Element { tag_name, .. } = &child.node_type {
                    info!(index = i, tag = %tag_name, "DOM: body child");
                } else if let NodeType::Text(text) = &child.node_type {
                    let preview: String = text.chars().take(30).collect();
                    info!(index = i, text = %preview, "DOM: body child (text)");
                }
            }
            
            let body_box = self.build_layout_from_node(&body, &sheet, &root_inherited, &[]);
            info!(
                layout_children = body_box.children.len(),
                "Layout: body box built"
            );
            root_box.children.push(body_box);
        } else if let Some(html) = document.document_element() {
            // Fallback: use html element if no body
            info!("DOM: no body found, using html element");
            // Debug: print html's children
            let html_children = html.children();
            info!(html_children = html_children.len(), "DOM: html element children");
            for (i, child) in html_children.iter().take(5).enumerate() {
                if let NodeType::Element { tag_name, .. } = &child.node_type {
                    info!(index = i, tag = %tag_name, "DOM: html child");
                }
            }
            let html_box = self.build_layout_from_node(&html, &sheet, &root_inherited, &[]);
            root_box.children.push(html_box);
        } else {
            warn!("DOM: no body or html element found");
        }

        root_box
    }

    /// Build a layout box from a DOM node.
    fn build_layout_from_node(
        &self,
        node: &Rc<Node>,
        sheet: &Stylesheet,
        parent_style: &ComputedStyle,
        ancestors: &[ElementCtx],
    ) -> LayoutBox {
        match &node.node_type {
            NodeType::Element { tag_name, attributes, .. } => {
                // Determine box type based on tag
                let is_inline = matches!(
                    tag_name.to_lowercase().as_str(),
                    "a" | "span" | "strong" | "b" | "em" | "i" | "u" | "code" | "small" | "big" | "sub" | "sup" | "abbr" | "cite" | "q" | "mark" | "label"
                );

                // Skip rendering for certain elements
                let is_hidden = matches!(
                    tag_name.to_lowercase().as_str(),
                    "head" | "title" | "meta" | "link" | "script" | "style" | "noscript"
                );

                if is_hidden {
                    // Return an empty block for hidden elements
                    return LayoutBox::new(BoxType::Block, ComputedStyle::new());
                }

                let box_type = if is_inline {
                    BoxType::Inline
                } else {
                    BoxType::Block
                };

                // Create computed style based on element and attributes
                let style = self.compute_style_for_element(tag_name, attributes, sheet, parent_style, ancestors);

                let mut layout_box = LayoutBox::new(box_type, style);
                Self::apply_position_to_layout_box(&mut layout_box);

                // Extend the ancestor chain with THIS element so descendant
                // selectors (`.card p`) can verify the chain when the children
                // are styled. Built once per element, not per child.
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.push(ElementCtx {
                    tag: tag_name.to_lowercase(),
                    classes: attributes
                        .get("class")
                        .map(|c| c.split_whitespace().map(|s| s.to_string()).collect())
                        .unwrap_or_default(),
                    id: attributes.get("id").cloned(),
                });

                // Get DOM children for processing
                let dom_children = node.children();
                trace!(tag = %tag_name, dom_children = dom_children.len(), "Processing element");

                // Process children
                for child in dom_children {
                    let child_box = self.build_layout_from_node(&child, sheet, &layout_box.style, &child_ancestors);
                    // Add all boxes - don't filter based on children
                    // The display list builder will handle empty boxes
                    layout_box.children.push(child_box);
                }

                layout_box
            }
            NodeType::Text(text) => {
                // Create text box for non-empty text
                let trimmed = text.trim();
                if trimmed.is_empty() {
                    // Return minimal box for whitespace-only text
                    LayoutBox::new(BoxType::Block, ComputedStyle::new())
                } else {
                    // Text nodes inherit from the element that contains them,
                    // so `p { color: red }` colours the words inside the <p>
                    // rather than only the box.
                    let mut style = ComputedStyle::inherit_from(parent_style);

                    // NOT CSS INHERITANCE - FEATURE PLUMBING, and the SECOND
                    // BREAK behind the text-decoration arms.
                    //
                    // text-decoration is correctly NOT inherited. But layout
                    // emits the decoration commands from the TEXT box's style
                    // (rustkit-layout lib.rs ~1714), and the text box is built
                    // by inherit_from, which resets the field. So the arms
                    // alone make the property writable - enough for the
                    // reachability metric to drop the name - while the page
                    // still renders undecorated.
                    //
                    // Copy it onto the text run explicitly, GATED on the
                    // parent actually having a line so an undecorated page
                    // carries none of this. Same shape as the macOS reference
                    // (engine lib.rs ~1749) and Athena's Windows #64.
                    let pl = parent_style.text_decoration_line;
                    if pl.underline || pl.overline || pl.line_through {
                        style.text_decoration_line = pl;
                        style.text_decoration_color = parent_style.text_decoration_color;
                        style.text_decoration_style = parent_style.text_decoration_style;
                        style.text_decoration_thickness =
                            parent_style.text_decoration_thickness.clone();
                    }
                    LayoutBox::new(BoxType::Text(trimmed.to_string()), style)
                }
            }
            _ => {
                // For other node types (Document, Comment, etc.), return empty box
                LayoutBox::new(BoxType::Block, ComputedStyle::new())
            }
        }
    }

    /// Compute a basic style for an element based on its tag and attributes.
    /// Discover every `<link rel="stylesheet">` href in the document, resolved
    /// against the document's own URL.
    ///
    /// Associated fn - a pure function of the document - so the tests exercise
    /// the real discovery path without constructing an Engine (and therefore
    /// without a GPU adapter). Same reasoning as the Option A wire tests.
    ///
    /// Relative hrefs need `base_url`; with no base, only absolute hrefs
    /// resolve. An href that will not parse is SKIPPED, not guessed at.
    /// (= Athena's Windows #54.)
    fn discover_external_stylesheets(document: &Document, base_url: Option<&Url>) -> Vec<Url> {
        let mut urls = Vec::new();
        for link in document.get_elements_by_tag_name("link") {
            // `rel` is an unordered SET in HTML ("stylesheet", "alternate
            // stylesheet"), so match any token rather than the whole attribute,
            // and case-insensitively - REL="StyleSheet" is legal.
            let is_stylesheet = link
                .get_attribute("rel")
                .map(|rel| {
                    rel.split_whitespace()
                        .any(|t| t.eq_ignore_ascii_case("stylesheet"))
                })
                .unwrap_or(false);
            if !is_stylesheet {
                continue;
            }
            let Some(href) = link.get_attribute("href") else {
                continue;
            };
            if href.trim().is_empty() {
                continue;
            }
            let resolved = match base_url {
                Some(base) => base.join(&href).ok(),
                None => Url::parse(&href).ok(),
            };
            match resolved {
                Some(url) => urls.push(url),
                None => warn!(href = %href, "stylesheet href did not resolve; skipping"),
            }
        }
        urls
    }

    /// Fetch every `<link rel="stylesheet">` and store the CSS on the view so
    /// the next layout can cascade it. Returns how many sheets loaded.
    ///
    /// FAIL-SOFT PER SHEET, DELIBERATELY: one 404 stylesheet must not fail the
    /// whole navigation - that is how a real browser behaves. But each failure
    /// is logged at warn WITH ITS URL, because a silently-dropped stylesheet
    /// looks exactly like a page that renders wrong for no reason. Fail-soft
    /// but never silent.
    async fn load_external_stylesheets(
        &mut self,
        id: EngineViewId,
        document: &Document,
        base_url: &Url,
    ) -> usize {
        let urls = Self::discover_external_stylesheets(document, Some(base_url));
        // NO EARLY RETURN on an empty list. Returning here would leave
        // whatever `external_css` already held, which for a reused view is the
        // PREVIOUS document's stylesheets. The empty case must ASSIGN an empty
        // string. (attach_document already clears it; this is the second lock
        // on the same door, because this function is also reachable directly.)
        let mut css = String::new();
        let mut loaded = 0usize;
        for url in urls {
            match self.loader.fetch(Request::get(url.clone())).await {
                Ok(response) if response.ok() => match response.text().await {
                    Ok(text) => {
                        css.push_str(&text);
                        css.push('\n');
                        loaded += 1;
                    }
                    Err(e) => warn!(%url, error = %e, "stylesheet body was not readable"),
                },
                Ok(response) => {
                    warn!(%url, status = ?response.status, "stylesheet fetch returned non-OK")
                }
                Err(e) => warn!(%url, error = %e, "stylesheet fetch failed"),
            }
        }
        if let Some(view) = self.views.get_mut(&id) {
            view.external_css = css;
        }
        info!(?id, loaded, "external stylesheets loaded");
        loaded
    }

    /// Walk the document collecting the text of every `<style>` element.
    ///
    /// Linux previously had NO author-stylesheet path at all: `<style>` sat in
    /// the engine's skip list beside `<link>`, so every rule an author wrote in
    /// a style block was dropped as silently as an external sheet. This is the
    /// L0 substrate the other two trees already had. (= Windows/macOS
    /// `collect_style_text`.)
    fn collect_style_text(&self, node: &Rc<Node>, out: &mut String) {
        if let NodeType::Element { tag_name, .. } = &node.node_type {
            if tag_name.eq_ignore_ascii_case("style") {
                for child in node.children() {
                    if let NodeType::Text(t) = &child.node_type {
                        out.push_str(t);
                        out.push('\n');
                    }
                }
                return;
            }
        }
        for child in node.children() {
            self.collect_style_text(&child, out);
        }
    }

    /// Match a selector against an element, returning its specificity.
    ///
    /// Supports comma groups, descendant chains (and `>` treated as descendant
    /// — stated simplification), type/class/id compounds and `*`. NOT covered,
    /// deliberately, and left to the B1 selector campaign: pseudo-classes,
    /// attribute selectors, and sibling combinators. A selector using those
    /// simply does not match rather than matching wrongly.
    /// Split one complex selector into compounds, recording for each the
    /// combinator that joins it to the compound on its RIGHT.
    ///
    /// `>` is recognised with OR WITHOUT surrounding whitespace, because
    /// authors write all four of `a>b`, `a> b`, `a >b` and `a > b` and they
    /// are the same selector. That detail is not cosmetic here: the previous
    /// tokenizer split on whitespace alone, so `a>b` arrived as a single
    /// compound whose type part was the literal string "a>b", matched no tag,
    /// and the whole rule was silently dead.
    ///
    /// Returns None for shapes this matcher cannot honour (`> p`, `div >`,
    /// `a > > b`). Refusing to match is the safe read of a selector we do not
    /// understand - applying it on a guess would style the wrong elements.
    fn tokenize_selector(group: &str) -> Option<Vec<(String, Combinator)>> {
        let spaced = group.replace('>', " > ");
        let mut out: Vec<(String, Combinator)> = Vec::new();
        let mut next_is_child = false;

        for tok in spaced.split_whitespace() {
            if tok == ">" {
                if out.is_empty() || next_is_child {
                    return None;
                }
                next_is_child = true;
                continue;
            }
            if let Some(last) = out.last_mut() {
                last.1 = if next_is_child {
                    Combinator::Child
                } else {
                    Combinator::Descendant
                };
            }
            next_is_child = false;
            // The combinator recorded here is a placeholder; it is overwritten
            // when (and only if) another compound follows. The subject's own
            // value is never read.
            out.push((tok.to_string(), Combinator::Descendant));
        }

        if next_is_child || out.is_empty() {
            return None;
        }
        Some(out)
    }

    fn selector_matches(
        selector: &str,
        tag: &str,
        classes: &[&str],
        id: Option<&str>,
        ancestors: &[ElementCtx],
    ) -> Option<u32> {
        let mut best: Option<u32> = None;
        for group in selector.split(',') {
            let group = group.trim();
            if group.is_empty() {
                continue;
            }
            let Some(compounds) = Self::tokenize_selector(group) else {
                continue;
            };
            let Some((subject, ancestor_sels)) = compounds.split_last() else {
                continue;
            };
            let Some(subject_spec) = Self::simple_selector_match(&subject.0, tag, classes, id)
            else {
                continue;
            };
            let mut spec = subject_spec;
            let mut idx = ancestors.len();
            let mut matched_all = true;
            // Walk the ancestor compounds RIGHT to LEFT. Each carries the
            // combinator joining it to the compound on its right, which is
            // exactly the relation to test when walking in this direction.
            for (sel, comb) in ancestor_sels.iter().rev() {
                let mut found = false;
                match comb {
                    // `a > b`: ONE candidate, the immediate parent. Before
                    // this, `>` was stripped from the token list and the
                    // relation silently relaxed to descendant, so
                    // `.nav > li` also styled every li nested any depth below.
                    Combinator::Child => {
                        if idx > 0 {
                            idx -= 1;
                            let a = &ancestors[idx];
                            let a_classes: Vec<&str> =
                                a.classes.iter().map(|s| s.as_str()).collect();
                            if let Some(s) = Self::simple_selector_match(
                                sel,
                                &a.tag,
                                &a_classes,
                                a.id.as_deref(),
                            ) {
                                spec += s;
                                found = true;
                            }
                        }
                    }
                    // `a b`: the nearest matching ancestor wins, and the
                    // cursor stays there for the compounds further left.
                    Combinator::Descendant => {
                        while idx > 0 {
                            idx -= 1;
                            let a = &ancestors[idx];
                            let a_classes: Vec<&str> =
                                a.classes.iter().map(|s| s.as_str()).collect();
                            if let Some(s) = Self::simple_selector_match(
                                sel,
                                &a.tag,
                                &a_classes,
                                a.id.as_deref(),
                            ) {
                                spec += s;
                                found = true;
                                break;
                            }
                        }
                    }
                }
                if !found {
                    matched_all = false;
                    break;
                }
            }
            if matched_all {
                best = Some(best.map_or(spec, |b| b.max(spec)));
            }
        }
        best
    }

    /// Match one compound selector (`div.card#main`), returning specificity.
    fn simple_selector_match(
        sel: &str,
        tag: &str,
        classes: &[&str],
        id: Option<&str>,
    ) -> Option<u32> {
        let mut spec = 0u32;
        let first_special = sel.find(['.', '#']).unwrap_or(sel.len());
        let type_sel = &sel[..first_special];
        if !type_sel.is_empty() && type_sel != "*" {
            if !type_sel.eq_ignore_ascii_case(tag) {
                return None;
            }
            spec += 1;
        }
        let rest = &sel[first_special..];
        let bytes = rest.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            let kind = bytes[i];
            let start = i + 1;
            let mut j = start;
            while j < bytes.len() && bytes[j] != b'.' && bytes[j] != b'#' {
                j += 1;
            }
            let name = &rest[start..j];
            if name.is_empty() {
                return None;
            }
            match kind {
                b'.' => {
                    if !classes.iter().any(|c| *c == name) {
                        return None;
                    }
                    spec += 10;
                }
                b'#' => {
                    if id != Some(name) {
                        return None;
                    }
                    spec += 100;
                }
                _ => return None,
            }
            i = j;
        }
        Some(spec)
    }

    /// Carry `position` and the offsets from the computed style onto the
    /// LayoutBox, so rustkit-layout's positioned-layout code can actually run.
    ///
    /// This is the THIRD break in the chain. ComputedStyle gained the fields
    /// and the applier gained the arms, but until a box carries them, layout
    /// still sees `Position::Static` everywhere and nothing on screen moves.
    /// A test that only checked computed values would pass at that point,
    /// which is why the tests for this unit are split into two groups.
    ///
    /// `rustkit_css::Position` and `rustkit_layout::Position` are SEPARATE
    /// enums with identical variants and no `From` impl. Not merged here -
    /// that is a wider refactor and this is a wire. The conversion below is an
    /// EXHAUSTIVE match, so adding a variant to either enum breaks the build
    /// rather than silently mapping to Static. Duplication flagged for
    /// whoever does the cleanup.
    fn apply_position_to_layout_box(layout_box: &mut LayoutBox) {
        use rustkit_layout::Position as LP;
        layout_box.position = match layout_box.style.position {
            rustkit_css::Position::Static => LP::Static,
            // Relative and Sticky map to Static ON PURPOSE, mirroring the
            // macOS reference. Entering the positioned paint path for these
            // wrecks pages whose relative boxes are only z-index anchors,
            // until the stacking pipeline matures. Deviating here would be a
            // DIVERGENCE, not an improvement - a relative box with no offsets
            // is visually identical to a static one either way.
            rustkit_css::Position::Relative => LP::Static,
            rustkit_css::Position::Sticky => LP::Static,
            rustkit_css::Position::Absolute => LP::Absolute,
            rustkit_css::Position::Fixed => LP::Fixed,
        };
        layout_box.z_index = layout_box.style.z_index;
        if layout_box.position != LP::Static {
            let font_size_px = match layout_box.style.font_size {
                rustkit_css::Length::Px(p) => p,
                _ => 16.0,
            };
            // Percentages resolve against the containing block, which is not
            // known here, so they stay None (auto) rather than becoming an
            // invented pixel value. Same restriction as the reference.
            let px = |l: &Option<rustkit_css::Length>| match l {
                Some(rustkit_css::Length::Px(v)) => Some(*v),
                Some(rustkit_css::Length::Zero) => Some(0.0),
                Some(rustkit_css::Length::Rem(r)) => Some(r * 16.0),
                Some(rustkit_css::Length::Em(e)) => Some(e * font_size_px),
                _ => None,
            };
            let (t, r, b, l) = (
                px(&layout_box.style.top),
                px(&layout_box.style.right),
                px(&layout_box.style.bottom),
                px(&layout_box.style.left),
            );
            layout_box.set_offsets(t, r, b, l);
        }
    }

    fn compute_style_for_element(
        &self,
        tag_name: &str,
        attributes: &std::collections::HashMap<String, String>,
        sheet: &Stylesheet,
        parent: &ComputedStyle,
        ancestors: &[ElementCtx],
    ) -> ComputedStyle {
        // INHERITANCE (unit 2): start from the parent's inherited properties
        // instead of a fresh default. Before this, every element computed from
        // ComputedStyle::new() with an unconditional BLACK, so `body { color:
        // red; font-family: Georgia }` reached the body element and stopped
        // dead there - no descendant ever saw it. ComputedStyle::inherit_from
        // already existed in rustkit-css; the engine simply never called it.
        let mut style = ComputedStyle::inherit_from(parent);

        // Apply tag-specific default styles
        match tag_name.to_lowercase().as_str() {
            "body" => {
                style.background_color = rustkit_css::Color::WHITE;
                style.margin_top = rustkit_css::Length::Px(8.0);
                style.margin_right = rustkit_css::Length::Px(8.0);
                style.margin_bottom = rustkit_css::Length::Px(8.0);
                style.margin_left = rustkit_css::Length::Px(8.0);
            }
            "h1" => {
                style.font_size = rustkit_css::Length::Px(32.0);
                style.font_weight = rustkit_css::FontWeight::BOLD;
                style.margin_top = rustkit_css::Length::Px(21.44);
                style.margin_bottom = rustkit_css::Length::Px(21.44);
            }
            "h2" => {
                style.font_size = rustkit_css::Length::Px(24.0);
                style.font_weight = rustkit_css::FontWeight::BOLD;
                style.margin_top = rustkit_css::Length::Px(19.92);
                style.margin_bottom = rustkit_css::Length::Px(19.92);
            }
            "h3" => {
                style.font_size = rustkit_css::Length::Px(18.72);
                style.font_weight = rustkit_css::FontWeight::BOLD;
                style.margin_top = rustkit_css::Length::Px(18.72);
                style.margin_bottom = rustkit_css::Length::Px(18.72);
            }
            "p" => {
                style.margin_top = rustkit_css::Length::Px(16.0);
                style.margin_bottom = rustkit_css::Length::Px(16.0);
            }
            "div" => {
                // Block element with no special styling
            }
            "a" => {
                style.color = rustkit_css::Color::new(0, 0, 238, 1.0); // Blue
            }
            "strong" | "b" => {
                style.font_weight = rustkit_css::FontWeight::BOLD;
            }
            "em" | "i" => {
                style.font_style = rustkit_css::FontStyle::Italic;
            }
            "pre" | "code" => {
                style.font_family = "monospace".to_string();
            }
            "ul" | "ol" => {
                style.margin_top = rustkit_css::Length::Px(16.0);
                style.margin_bottom = rustkit_css::Length::Px(16.0);
                style.padding_left = rustkit_css::Length::Px(40.0);
            }
            "li" => {
                // List items are blocks
            }
            "blockquote" => {
                style.margin_top = rustkit_css::Length::Px(16.0);
                style.margin_bottom = rustkit_css::Length::Px(16.0);
                style.margin_left = rustkit_css::Length::Px(40.0);
                style.margin_right = rustkit_css::Length::Px(40.0);
            }
            "hr" => {
                style.border_top_width = rustkit_css::Length::Px(1.0);
                style.border_top_color = rustkit_css::Color::new(128, 128, 128, 1.0);
                style.margin_top = rustkit_css::Length::Px(8.0);
                style.margin_bottom = rustkit_css::Length::Px(8.0);
            }
            _ => {}
        }

        // Author stylesheet rules, selector-matched, applied in
        // (specificity, source order) so later / more specific rules win.
        // These land AFTER the UA defaults above and BEFORE the inline style
        // attribute below, which is the correct cascade order for this layer.
        if !sheet.rules.is_empty() {
            let classes: Vec<&str> = attributes
                .get("class")
                .map(|c| c.split_whitespace().collect())
                .unwrap_or_default();
            let id = attributes.get("id").map(|s| s.as_str());
            let mut matched: Vec<(u32, usize)> = Vec::new();
            for (i, rule) in sheet.rules.iter().enumerate() {
                if let Some(spec) =
                    Self::selector_matches(&rule.selector, tag_name, &classes, id, ancestors)
                {
                    matched.push((spec, i));
                }
            }
            matched.sort_by_key(|&(spec, i)| (spec, i));
            for (_, i) in matched {
                for decl in &sheet.rules[i].declarations {
                    // Custom properties (--x) and var() resolution are NOT in
                    // L0 - stated, not silently skipped. They need the
                    // inherited custom-property map the other trees carry.
                    if decl.property.starts_with("--") {
                        continue;
                    }
                    if let PropertyValue::Specified(v) = &decl.value {
                        apply_inline_style_decls(
                            &mut style,
                            &format!("{}: {}", decl.property, v),
                        );
                    }
                }
            }
        }

        // Parse inline style attribute if present (highest priority).
        if let Some(style_attr) = attributes.get("style") {
            self.apply_inline_style(&mut style, style_attr);
        }

        // ABSOLUTISE FONT-SIZE. One chokepoint, AFTER every source that can
        // set it - UA defaults, author rules, inline style - so no path can
        // leave a relative unit behind.
        //
        // Until now this tree stored `font-size: 2rem` as Rem(2.0) and handed
        // it to layout, where ~6 consumers each did
        // `match font_size { Px(px) => px, _ => 16.0 }`. Every rem/em/%
        // font-size on every page laid out at 16px. Measured on Atlas's
        // fixture: 12 of 16 context/unit combinations unresolved.
        //
        // The reference already does exactly this (macos rustkit-engine
        // lib.rs ~1344) and carries the scar comment for the same bug:
        // "leaving Em here made h1 { font-size: 2em } render at 16px". This is
        // a PORT DEFECT - macOS is clean, we are behind - not a shared one.
        // My earlier classification of it as shared came from instance-
        // counting the reference instead of running it, which is how Atlas got
        // assigned a PR with nothing to fix.
        //
        // NOTE ON THE CONSUMER FALLBACKS: the ~6 `_ => 16.0` arms in
        // rustkit-layout are deliberately LEFT IN PLACE. Once the cascade
        // absolutises they become unreached sentinels, exactly as the
        // reference's 26 are. Deleting them would remove the safety net for
        // any future path that forgets to absolutise, and the fleet ruling is
        // to keep them until the cascade is enforced everywhere.
        let parent_font_px = match parent.font_size {
            rustkit_css::Length::Px(px) => px,
            // Only the root can legitimately reach here: every non-root parent
            // has been through this same block. 16 is the initial font size.
            _ => 16.0,
        };
        style.font_size = match style.font_size {
            // `em` resolves against the PARENT's used font size, not the root.
            rustkit_css::Length::Em(em) => rustkit_css::Length::Px(em * parent_font_px),
            // A percentage font-size is the same relation expressed differently.
            rustkit_css::Length::Percent(pct) => {
                rustkit_css::Length::Px(pct / 100.0 * parent_font_px)
            }
            // `rem` is always against the ROOT font size. Hardcoded 16 mirrors
            // the reference; a real root-font-size lookup is a separate unit on
            // all three trees and inventing one here would diverge.
            rustkit_css::Length::Rem(rem) => rustkit_css::Length::Px(rem * 16.0),
            other => other,
        };

        style
    }

    /// Apply inline style attribute to computed style.
    ///
    /// Delegates to `apply_inline_style_decls` - see that function for why the
    /// implementation is free-standing.
    fn apply_inline_style(&self, style: &mut ComputedStyle, style_attr: &str) {
        apply_inline_style_decls(style, style_attr)
    }

    /// Render a view (public API for continuous rendering).
    pub fn render_view(&mut self, id: EngineViewId) -> Result<(), EngineError> {
        self.render(id)
    }

    /// Render all views.
    pub fn render_all_views(&mut self) {
        let view_ids: Vec<_> = self.views.keys().copied().collect();
        for id in view_ids {
            if let Err(e) = self.render(id) {
                trace!(?id, error = %e, "Failed to render view");
            }
        }
    }

    /// Get render statistics from the renderer.
    pub fn get_render_stats(&self) -> RenderStats {
        self.renderer
            .as_ref()
            .map(|r| r.get_render_stats())
            .unwrap_or_default()
    }

    /// Capture a screenshot of a view to a PNG file.
    ///
    /// This renders the view to an offscreen texture and reads back the pixels.
    pub fn capture_view_screenshot(
        &mut self,
        id: EngineViewId,
        output_path: &std::path::Path,
    ) -> Result<ScreenshotMetadata, EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;
        let display_list = view.display_list.as_ref();
        let viewhost_id = view.viewhost_id;

        // Get view bounds for viewport
        let bounds = self
            .viewhost
            .get_bounds(viewhost_id)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        if bounds.width == 0 || bounds.height == 0 {
            return Err(EngineError::RenderError(format!(
                "Cannot capture screenshot of zero-sized view: {}x{}",
                bounds.width, bounds.height
            )));
        }

        if let Some(renderer) = &mut self.renderer {
            // Update viewport size
            renderer.set_viewport_size(bounds.width, bounds.height);
            
            // Get commands from display list or use empty
            let commands = display_list
                .map(|dl| dl.commands.as_slice())
                .unwrap_or(&[]);
            
            // Capture to file
            renderer
                .execute_and_capture(commands, output_path)
                .map_err(|e| EngineError::RenderError(e.to_string()))
        } else {
            Err(EngineError::RenderError("No renderer available".to_string()))
        }
    }

    /// Get the native window handle (HWND) for a view.
    #[cfg(windows)]
    pub fn get_view_hwnd(&self, id: EngineViewId) -> Result<HWND, EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;
        self.viewhost
            .get_hwnd(view.viewhost_id)
            .map_err(|e| EngineError::ViewError(e.to_string()))
    }

    /// Render a view (internal).
    fn render(&mut self, id: EngineViewId) -> Result<(), EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;
        let viewhost_id = view.viewhost_id;
        let display_list = view.display_list.as_ref();

        trace!(?id, "Rendering view");

        // Get view bounds for viewport
        let bounds = self
            .viewhost
            .get_bounds(viewhost_id)
            .map_err(|e| EngineError::ViewError(e.to_string()))?;

        // Get surface texture
        let (output, texture_view) = self.compositor
            .get_surface_texture(viewhost_id)
            .map_err(|e| EngineError::RenderError(e.to_string()))?;

        // Render using display list if available, otherwise just clear to background
        if let (Some(renderer), Some(display_list)) = (&mut self.renderer, display_list) {
            // Update viewport size before rendering
            renderer.set_viewport_size(bounds.width, bounds.height);
            renderer.execute(&display_list.commands, &texture_view)
                .map_err(|e| EngineError::RenderError(e.to_string()))?;
        } else if let Some(renderer) = &mut self.renderer {
            // No display list, render empty (will clear to white)
            renderer.set_viewport_size(bounds.width, bounds.height);
            renderer.execute(&[], &texture_view)
                .map_err(|e| EngineError::RenderError(e.to_string()))?;
        } else {
            // Fallback to compositor solid color (shouldn't normally happen)
            drop(output); // Release the texture
            self.compositor
                .render_solid_color(viewhost_id, self.config.background_color)
                .map_err(|e| EngineError::RenderError(e.to_string()))?;
            return Ok(());
        }

        // Present
        self.compositor.present(output);

        Ok(())
    }

    /// Execute JavaScript in a view.
    pub fn execute_script(
        &mut self,
        id: EngineViewId,
        script: &str,
    ) -> Result<String, EngineError> {
        let view = self.views.get(&id).ok_or(EngineError::ViewNotFound(id))?;

        let bindings = view
            .bindings
            .as_ref()
            .ok_or(EngineError::JsError("JavaScript not initialized".into()))?;

        let result = bindings
            .evaluate(script)
            .map_err(|e| EngineError::JsError(e.to_string()))?;

        Ok(format!("{:?}", result))
    }

    /// Get the current URL of a view.
    pub fn get_url(&self, id: EngineViewId) -> Option<Url> {
        self.views.get(&id).and_then(|v| v.url.clone())
    }

    /// Get the title of a view.
    pub fn get_title(&self, id: EngineViewId) -> Option<String> {
        self.views.get(&id).and_then(|v| v.title.clone())
    }

    /// Check if a view can go back.
    pub fn can_go_back(&self, id: EngineViewId) -> bool {
        self.views
            .get(&id)
            .map(|v| v.navigation.can_go_back())
            .unwrap_or(false)
    }

    /// Check if a view can go forward.
    pub fn can_go_forward(&self, id: EngineViewId) -> bool {
        self.views
            .get(&id)
            .map(|v| v.navigation.can_go_forward())
            .unwrap_or(false)
    }

    /// Get the number of views.
    pub fn view_count(&self) -> usize {
        self.views.len()
    }

    /// Get the download manager.
    pub fn download_manager(&self) -> Arc<rustkit_net::DownloadManager> {
        self.loader.download_manager()
    }

    /// Get GPU info.
    pub fn gpu_info(&self) -> String {
        format!("{:?}", self.compositor.adapter_info())
    }

    /// Handle a view event from the viewhost.
    #[cfg(windows)]
    pub fn handle_view_event(&mut self, event: rustkit_viewhost::ViewEvent) {
        use rustkit_viewhost::ViewEvent;

        match event {
            ViewEvent::Resized {
                view_id: viewhost_id,
                bounds,
                dpi: _,
            } => {
                // Find engine view id for this viewhost id
                if let Some((id, _)) = self
                    .views
                    .iter()
                    .find(|(_, v)| v.viewhost_id == viewhost_id)
                {
                    let id = *id;
                    let _ = self.resize_view(
                        id,
                        rustkit_viewhost::Bounds::new(
                            bounds.x,
                            bounds.y,
                            bounds.width,
                            bounds.height,
                        ),
                    );
                }
            }
            ViewEvent::Focused {
                view_id: viewhost_id,
            } => {
                if let Some((id, view)) = self
                    .views
                    .iter_mut()
                    .find(|(_, v)| v.viewhost_id == viewhost_id)
                {
                    view.view_focused = true;
                    let _ = self
                        .event_tx
                        .send(EngineEvent::ViewFocused { view_id: *id });
                }
            }
            ViewEvent::Blurred {
                view_id: viewhost_id,
            } => {
                if let Some(view) = self
                    .views
                    .values_mut()
                    .find(|v| v.viewhost_id == viewhost_id)
                {
                    view.view_focused = false;
                }
            }
            ViewEvent::Input {
                view_id: viewhost_id,
                event: input_event,
            } => {
                self.handle_input_event(viewhost_id, input_event);
            }
            _ => {}
        }
    }

    /// Handle an input event.
    #[cfg(windows)]
    fn handle_input_event(&mut self, viewhost_id: ViewId, event: rustkit_core::InputEvent) {
        use rustkit_core::InputEvent;

        // Find the view
        let engine_id = self
            .views
            .iter()
            .find(|(_, v)| v.viewhost_id == viewhost_id)
            .map(|(id, _)| *id);

        let Some(engine_id) = engine_id else {
            return;
        };

        match event {
            InputEvent::Mouse(mouse_event) => {
                self.handle_mouse_event(engine_id, mouse_event);
            }
            InputEvent::Key(key_event) => {
                self.handle_key_event(engine_id, key_event);
            }
            InputEvent::Focus(focus_event) => {
                // Focus events are handled via ViewEvent::Focused/Blurred
                let _ = focus_event;
            }
        }
    }

    /// Handle a mouse event.
    #[cfg(windows)]
    fn handle_mouse_event(&mut self, view_id: EngineViewId, event: rustkit_core::MouseEvent) {
        use rustkit_core::MouseEventType;
        use rustkit_dom::MouseEventData;

        let view = match self.views.get_mut(&view_id) {
            Some(v) => v,
            None => return,
        };

        // Perform hit testing if we have layout
        let hit_result = view
            .layout
            .as_ref()
            .and_then(|layout| layout.hit_test(event.position.x as f32, event.position.y as f32));

        // Convert to DOM event
        let dom_event_type = match event.event_type {
            MouseEventType::MouseDown => "mousedown",
            MouseEventType::MouseUp => "mouseup",
            MouseEventType::MouseMove => "mousemove",
            MouseEventType::MouseEnter => "mouseenter",
            MouseEventType::MouseLeave => "mouseleave",
            MouseEventType::Wheel => "wheel",
            MouseEventType::ContextMenu => "contextmenu",
        };

        let _mouse_data = MouseEventData {
            client_x: event.position.x,
            client_y: event.position.y,
            screen_x: event.screen_position.x,
            screen_y: event.screen_position.y,
            offset_x: hit_result.as_ref().map(|r| r.local_x as f64).unwrap_or(0.0),
            offset_y: hit_result.as_ref().map(|r| r.local_y as f64).unwrap_or(0.0),
            button: event.button.button_index(),
            buttons: event.buttons,
            ctrl_key: event.modifiers.ctrl,
            alt_key: event.modifiers.alt,
            shift_key: event.modifiers.shift,
            meta_key: event.modifiers.meta,
            related_target: None,
        };

        // If we have a hit and a document, dispatch the event
        if let (Some(_hit), Some(_document)) = (hit_result, &view.document) {
            // TODO: Map hit result to DOM node and dispatch event
            // For now, just log
            trace!(?view_id, event_type = dom_event_type, "Mouse event");
        }

        // Handle click focus change
        if event.event_type == MouseEventType::MouseDown {
            // TODO: Focus the clicked element if focusable
        }
    }

    /// Handle a keyboard event.
    #[cfg(windows)]
    fn handle_key_event(&mut self, view_id: EngineViewId, event: rustkit_core::KeyEvent) {
        use rustkit_core::{KeyCode, KeyEventType};

        let view = match self.views.get_mut(&view_id) {
            Some(v) => v,
            None => return,
        };

        // Only process keyboard events if the view has focus
        if !view.view_focused {
            return;
        }

        trace!(?view_id, key = ?event.key_code, event_type = ?event.event_type, "Key event");

        // Handle Tab key for focus navigation
        if event.event_type == KeyEventType::KeyDown && event.key_code == KeyCode::Tab {
            // TODO: Implement Tab navigation between focusable elements
        }

        // Dispatch to focused element via DOM events
        // TODO: Dispatch KeyboardEvent to focused DOM node
    }

    /// Focus a DOM node in a view.
    pub fn focus_element(
        &mut self,
        view_id: EngineViewId,
        node_id: rustkit_dom::NodeId,
    ) -> Result<(), EngineError> {
        let view = self
            .views
            .get_mut(&view_id)
            .ok_or(EngineError::ViewNotFound(view_id))?;

        let old_focused = view.focused_node;
        view.focused_node = Some(node_id);

        // TODO: Dispatch blur event to old focused element
        // TODO: Dispatch focus event to new focused element

        debug!(?view_id, ?node_id, ?old_focused, "Focus changed");
        Ok(())
    }

    /// Blur the currently focused element.
    pub fn blur_element(&mut self, view_id: EngineViewId) -> Result<(), EngineError> {
        let view = self
            .views
            .get_mut(&view_id)
            .ok_or(EngineError::ViewNotFound(view_id))?;

        let old_focused = view.focused_node.take();

        // TODO: Dispatch blur event to old focused element

        debug!(?view_id, ?old_focused, "Element blurred");
        Ok(())
    }

    /// Get the currently focused node in a view.
    pub fn get_focused_element(&self, view_id: EngineViewId) -> Option<rustkit_dom::NodeId> {
        self.views.get(&view_id).and_then(|v| v.focused_node)
    }

    /// Load an image from a URL.
    pub async fn load_image(&self, view_id: EngineViewId, url: Url) -> Result<(), EngineError> {
        let image_manager = self.image_manager.clone();
        let event_tx = self.event_tx.clone();

        match image_manager.load(url.clone()).await {
            Ok(image) => {
                let _ = event_tx.send(EngineEvent::ImageLoaded {
                    view_id,
                    url,
                    width: image.natural_width,
                    height: image.natural_height,
                });
                Ok(())
            }
            Err(e) => {
                let error = e.to_string();
                let _ = event_tx.send(EngineEvent::ImageError {
                    view_id,
                    url: url.clone(),
                    error: error.clone(),
                });
                Err(EngineError::RenderError(format!("Image load failed: {}", error)))
            }
        }
    }

    /// Preload an image (non-blocking).
    pub fn preload_image(&self, url: Url) {
        self.image_manager.preload(url);
    }

    /// Check if an image is cached.
    pub fn is_image_cached(&self, url: &Url) -> bool {
        self.image_manager.is_cached(url)
    }

    /// Get a cached image's dimensions.
    pub fn get_image_dimensions(&self, url: &Url) -> Option<(u32, u32)> {
        self.image_manager
            .get_cached(url)
            .map(|img| (img.natural_width, img.natural_height))
    }

    /// Get the image manager for direct access.
    pub fn image_manager(&self) -> Arc<ImageManager> {
        self.image_manager.clone()
    }

    /// Clear the image cache.
    pub fn clear_image_cache(&self) {
        self.image_manager.clear_cache();
    }

    /// Drain IPC messages from all views.
    ///
    /// Returns a Vec of (EngineViewId, IpcMessage) tuples for messages received
    /// via `window.ipc.postMessage()` from JavaScript in any view.
    ///
    /// This should be called periodically (e.g., during the message loop) to
    /// process IPC messages from the Chrome UI, Shelf, and Content views.
    pub fn drain_ipc_messages(&self) -> Vec<(EngineViewId, IpcMessage)> {
        let mut messages = Vec::new();

        for (&view_id, view_state) in &self.views {
            if let Some(ref bindings) = view_state.bindings {
                for ipc_msg in bindings.drain_ipc_queue() {
                    messages.push((view_id, ipc_msg));
                }
            }
        }

        messages
    }

    /// Check if any view has pending IPC messages.
    pub fn has_pending_ipc(&self) -> bool {
        self.views.values().any(|v| {
            v.bindings
                .as_ref()
                .map(|b| b.has_pending_ipc())
                .unwrap_or(false)
        })
    }
}

/// Builder for Engine.
pub struct EngineBuilder {
    config: EngineConfig,
    interceptor: Option<rustkit_net::RequestInterceptor>,
}

impl EngineBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            config: EngineConfig::default(),
            interceptor: None,
        }
    }

    /// Set a request interceptor for filtering network requests.
    pub fn request_interceptor(mut self, interceptor: rustkit_net::RequestInterceptor) -> Self {
        self.interceptor = Some(interceptor);
        self
    }

    /// Set the user agent.
    pub fn user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.config.user_agent = user_agent.into();
        self
    }

    /// Enable or disable JavaScript.
    pub fn javascript_enabled(mut self, enabled: bool) -> Self {
        self.config.javascript_enabled = enabled;
        self
    }

    /// Enable or disable cookies.
    pub fn cookies_enabled(mut self, enabled: bool) -> Self {
        self.config.cookies_enabled = enabled;
        self
    }

    /// Set the default background color.
    pub fn background_color(mut self, color: [f64; 4]) -> Self {
        self.config.background_color = color;
        self
    }

    /// Build the engine.
    pub fn build(self) -> Result<Engine, EngineError> {
        Engine::with_interceptor(self.config, self.interceptor)
    }
}

impl Default for EngineBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse a color value from CSS.
fn parse_color(value: &str) -> Option<rustkit_css::Color> {
    let value = value.trim().to_lowercase();

    // Named colors
    match value.as_str() {
        "black" => return Some(rustkit_css::Color::BLACK),
        "white" => return Some(rustkit_css::Color::WHITE),
        "red" => return Some(rustkit_css::Color::new(255, 0, 0, 1.0)),
        "green" => return Some(rustkit_css::Color::new(0, 128, 0, 1.0)),
        "blue" => return Some(rustkit_css::Color::new(0, 0, 255, 1.0)),
        "yellow" => return Some(rustkit_css::Color::new(255, 255, 0, 1.0)),
        "cyan" => return Some(rustkit_css::Color::new(0, 255, 255, 1.0)),
        "magenta" => return Some(rustkit_css::Color::new(255, 0, 255, 1.0)),
        "gray" | "grey" => return Some(rustkit_css::Color::new(128, 128, 128, 1.0)),
        "transparent" => return Some(rustkit_css::Color::TRANSPARENT),
        _ => {}
    }

    // Hex colors
    if let Some(hex) = value.strip_prefix('#') {
        let (r, g, b) = match hex.len() {
            3 => {
                let r = u8::from_str_radix(&hex[0..1], 16).ok()? * 17;
                let g = u8::from_str_radix(&hex[1..2], 16).ok()? * 17;
                let b = u8::from_str_radix(&hex[2..3], 16).ok()? * 17;
                (r, g, b)
            }
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b)
            }
            _ => return None,
        };
        return Some(rustkit_css::Color::from_rgb(r, g, b));
    }

    // rgb() and rgba()
    if value.starts_with("rgb(") || value.starts_with("rgba(") {
        let inner = value
            .trim_start_matches("rgba(")
            .trim_start_matches("rgb(")
            .trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if parts.len() >= 3 {
            let r: u8 = parts[0].trim().parse().ok()?;
            let g: u8 = parts[1].trim().parse().ok()?;
            let b: u8 = parts[2].trim().parse().ok()?;
            let a: f32 = if parts.len() >= 4 {
                parts[3].trim().parse().ok()?
            } else {
                1.0
            };
            return Some(rustkit_css::Color::new(r, g, b, a));
        }
    }

    None
}

/// Parse a length value from CSS.
/// Parse a CSS transform value into a TransformList.
fn parse_transform(value: &str) -> Option<rustkit_css::TransformList> {
    let value = value.trim();
    if value == "none" {
        return Some(rustkit_css::TransformList::none());
    }

    let mut ops = Vec::new();
    let mut remaining = value;

    while !remaining.is_empty() {
        remaining = remaining.trim_start();

        // Find the function name
        if let Some(paren_pos) = remaining.find('(') {
            let func_name = &remaining[..paren_pos];
            let after_paren = &remaining[paren_pos + 1..];

            // Find matching closing paren
            if let Some(close_pos) = find_matching_paren(after_paren) {
                let args = &after_paren[..close_pos];
                remaining = &after_paren[close_pos + 1..];

                if let Some(op) = parse_transform_op(func_name, args) {
                    ops.push(op);
                }
            } else {
                break;
            }
        } else {
            break;
        }
    }

    if ops.is_empty() {
        None
    } else {
        Some(rustkit_css::TransformList { ops })
    }
}

/// Find the index of the matching closing paren (depth-aware).
fn find_matching_paren(s: &str) -> Option<usize> {
    let mut depth = 1;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parse a single transform operation.
fn parse_transform_op(func: &str, args: &str) -> Option<rustkit_css::TransformOp> {
    let args = args.trim();
    let parts: Vec<&str> = args.split(',').map(|s| s.trim()).collect();

    match func.trim() {
        "translate" => {
            let x = parse_length(parts.first()?)?;
            let y = parts
                .get(1)
                .and_then(|s| parse_length(s))
                .unwrap_or(rustkit_css::Length::Zero);
            Some(rustkit_css::TransformOp::Translate(x, y))
        }
        "translateX" => {
            let x = parse_length(parts.first()?)?;
            Some(rustkit_css::TransformOp::TranslateX(x))
        }
        "translateY" => {
            let y = parse_length(parts.first()?)?;
            Some(rustkit_css::TransformOp::TranslateY(y))
        }
        "scale" => {
            let sx = parts.first()?.parse::<f32>().ok()?;
            let sy = parts
                .get(1)
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(sx);
            Some(rustkit_css::TransformOp::Scale(sx, sy))
        }
        "scaleX" => {
            let s = parts.first()?.parse::<f32>().ok()?;
            Some(rustkit_css::TransformOp::ScaleX(s))
        }
        "scaleY" => {
            let s = parts.first()?.parse::<f32>().ok()?;
            Some(rustkit_css::TransformOp::ScaleY(s))
        }
        "rotate" => {
            let angle = parse_angle(parts.first()?)?;
            Some(rustkit_css::TransformOp::Rotate(angle))
        }
        "skew" => {
            let ax = parse_angle(parts.first()?)?;
            let ay = parts.get(1).and_then(|s| parse_angle(s)).unwrap_or(0.0);
            Some(rustkit_css::TransformOp::Skew(ax, ay))
        }
        "skewX" => {
            let angle = parse_angle(parts.first()?)?;
            Some(rustkit_css::TransformOp::SkewX(angle))
        }
        "skewY" => {
            let angle = parse_angle(parts.first()?)?;
            Some(rustkit_css::TransformOp::SkewY(angle))
        }
        "matrix" => {
            if parts.len() >= 6 {
                let a = parts[0].parse::<f32>().ok()?;
                let b = parts[1].parse::<f32>().ok()?;
                let c = parts[2].parse::<f32>().ok()?;
                let d = parts[3].parse::<f32>().ok()?;
                let e = parts[4].parse::<f32>().ok()?;
                let f = parts[5].parse::<f32>().ok()?;
                Some(rustkit_css::TransformOp::Matrix(a, b, c, d, e, f))
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Parse a CSS angle value (e.g., "45deg", "1rad", "0.5turn") into degrees.
/// Parse a `flex-basis` value.
///
/// `content` and `auto` are distinct: `auto` defers to the item's own
/// width/height, `content` sizes to content regardless of them. Collapsing
/// the two would silently change layout for any item that sets both a width
/// and `flex-basis: content`.
// ---------------------------------------------------------------------------
// GRID VALUE PARSERS - ported verbatim from the macOS reference
// (rustkit-engine/src/lib.rs). Linux had none of these: the ComputedStyle
// grid fields existed and rustkit-layout's grid.rs consumed them, but nothing
// could parse a track list, so `grid-template-columns` was unreachable.
//
// Kept byte-faithful to the reference rather than rewritten. A port that
// "improves" the grammar while porting cannot be diffed against its source,
// and any behavioural difference would be an undeclared divergence.
// ---------------------------------------------------------------------------

/// Parse a single track size (e.g., "1fr", "100px", "auto", "minmax(...)").
fn parse_track_size(value: &str) -> Option<rustkit_css::TrackSize> {
    let value = value.trim();

    if value == "auto" {
        return Some(rustkit_css::TrackSize::Auto);
    }

    if value == "min-content" {
        return Some(rustkit_css::TrackSize::MinContent);
    }

    if value == "max-content" {
        return Some(rustkit_css::TrackSize::MaxContent);
    }

    // Check for fr unit
    if let Some(fr_str) = value.strip_suffix("fr") {
        if let Ok(fr) = fr_str.trim().parse::<f32>() {
            return Some(rustkit_css::TrackSize::Fr(fr));
        }
    }

    // Check for px unit
    if let Some(px_str) = value.strip_suffix("px") {
        if let Ok(px) = px_str.trim().parse::<f32>() {
            return Some(rustkit_css::TrackSize::Px(px));
        }
    }

    // Check for percent
    if let Some(pct_str) = value.strip_suffix('%') {
        if let Ok(pct) = pct_str.trim().parse::<f32>() {
            return Some(rustkit_css::TrackSize::Percent(pct));
        }
    }

    // Check for minmax()
    if value.starts_with("minmax(") {
        if let Some(close) = find_matching_paren(&value[7..]) {
            let content = &value[7..7 + close];
            if let Some(comma) = content.find(',') {
                let min_str = content[..comma].trim();
                let max_str = content[comma + 1..].trim();
                if let (Some(min), Some(max)) =
                    (parse_track_size(min_str), parse_track_size(max_str))
                {
                    return Some(rustkit_css::TrackSize::MinMax(Box::new(min), Box::new(max)));
                }
            }
        }
    }

    // Check for fit-content()
    if value.starts_with("fit-content(") {
        if let Some(close) = find_matching_paren(&value[12..]) {
            let content = &value[12..12 + close];
            if let Some(length) = parse_length(content) {
                return Some(rustkit_css::TrackSize::FitContent(
                    length.to_px(16.0, 16.0, 0.0),
                ));
            }
        }
    }

    None
}

/// Parse a grid-template-columns or grid-template-rows value.
/// Supports: repeat(N, 1fr), explicit track sizes, and combinations.
fn parse_grid_template(value: &str) -> Option<rustkit_css::GridTemplate> {
    let value = value.trim();

    if value == "none" || value.is_empty() {
        return Some(rustkit_css::GridTemplate::none());
    }

    let mut tracks = Vec::new();

    // Check for repeat() function
    if let Some(repeat_start) = value.find("repeat(") {
        let after_repeat = &value[repeat_start + 7..];
        if let Some(close_paren) = find_matching_paren(after_repeat) {
            let repeat_content = &after_repeat[..close_paren];

            // Parse repeat(count, track-size)
            if let Some(comma_pos) = repeat_content.find(',') {
                let count_str = repeat_content[..comma_pos].trim();
                let track_str = repeat_content[comma_pos + 1..].trim();

                // Parse count (could be number, auto-fill, auto-fit)
                let count: Option<u32> = if count_str == "auto-fill" || count_str == "auto-fit" {
                    // For now, default to a reasonable number
                    Some(4)
                } else {
                    count_str.parse().ok()
                };

                if let (Some(count), Some(track_size)) = (count, parse_track_size(track_str)) {
                    for _ in 0..count {
                        tracks.push(rustkit_css::TrackDefinition::simple(track_size.clone()));
                    }
                }
            }
        }
    } else {
        // Parse space-separated track sizes
        for part in value.split_whitespace() {
            if let Some(track_size) = parse_track_size(part) {
                tracks.push(rustkit_css::TrackDefinition::simple(track_size));
            }
        }
    }

    if tracks.is_empty() {
        return None;
    }

    Some(rustkit_css::GridTemplate {
        tracks,
        repeats: Vec::new(),
        final_line_names: Vec::new(),
    })
}

/// Parse a grid line value (e.g., "1", "span 2", "auto").
fn parse_grid_line(value: &str) -> Option<rustkit_css::GridLine> {
    let value = value.trim();

    if value == "auto" {
        return Some(rustkit_css::GridLine::Auto);
    }

    // Check for "span N"
    if let Some(span_str) = value.strip_prefix("span") {
        let span_str = span_str.trim();
        if let Ok(span) = span_str.parse::<u32>() {
            return Some(rustkit_css::GridLine::Span(span));
        }
    }

    // Try as a number
    if let Ok(num) = value.parse::<i32>() {
        return Some(rustkit_css::GridLine::Number(num));
    }

    // Could be a named line (just use auto for now)
    Some(rustkit_css::GridLine::Auto)
}

/// Parse a grid-column or grid-row shorthand (e.g., "1 / 3", "span 2").
fn parse_grid_line_shorthand(
    value: &str,
) -> Option<(rustkit_css::GridLine, rustkit_css::GridLine)> {
    let value = value.trim();

    // Check for "start / end" format
    if let Some(slash_pos) = value.find('/') {
        let start_str = value[..slash_pos].trim();
        let end_str = value[slash_pos + 1..].trim();

        let start = parse_grid_line(start_str)?;
        let end = parse_grid_line(end_str)?;

        return Some((start, end));
    }

    // Single value - applies to start, end is auto
    let start = parse_grid_line(value)?;
    Some((start, rustkit_css::GridLine::Auto))
}

fn parse_flex_basis(value: &str) -> Option<rustkit_css::FlexBasis> {
    let v = value.trim();
    match v {
        "auto" => Some(rustkit_css::FlexBasis::Auto),
        "content" => Some(rustkit_css::FlexBasis::Content),
        _ => {
            if let Some(pct) = v.strip_suffix('%') {
                return pct.trim().parse::<f32>().ok().map(rustkit_css::FlexBasis::Percent);
            }
            match parse_length(v)? {
                rustkit_css::Length::Px(px) => Some(rustkit_css::FlexBasis::Length(px)),
                // Other units reach FlexBasis only as a raw f32, which has no
                // room for the unit. Refusing beats silently treating `2em`
                // as 2 pixels.
                _ => None,
            }
        }
    }
}

fn parse_angle(value: &str) -> Option<f32> {
    let value = value.trim();
    // Suffixes are tested LONGEST-FIRST because they overlap: "grad" ends with
    // "rad". Testing "rad" first makes the grad branch below unreachable - it
    // strips "rad" from "200grad", leaving "200g", which fails to parse, and
    // the whole declaration is dropped. Identical shape to the rem-before-em
    // bug (Linux #3): an overlapping-suffix chain in the wrong order silently
    // discards the longer unit. (= Windows #48 fix, applied on arrival.)
    if value.ends_with("grad") {
        value[..value.len() - 4]
            .parse::<f32>()
            .ok()
            .map(|g| g * 0.9)
    } else if value.ends_with("turn") {
        value[..value.len() - 4]
            .parse::<f32>()
            .ok()
            .map(|t| t * 360.0)
    } else if value.ends_with("deg") {
        value[..value.len() - 3].parse().ok()
    } else if value.ends_with("rad") {
        value[..value.len() - 3]
            .parse::<f32>()
            .ok()
            .map(|r| r.to_degrees())
    } else {
        // Try parsing as number (defaults to degrees)
        value.parse().ok()
    }
}

/// Parse transform-origin value.
fn parse_transform_origin(value: &str) -> Option<rustkit_css::TransformOrigin> {
    let parts: Vec<&str> = value.split_whitespace().collect();

    let parse_component = |s: &str| -> Option<rustkit_css::Length> {
        match s {
            "left" => Some(rustkit_css::Length::Percent(0.0)),
            "center" => Some(rustkit_css::Length::Percent(50.0)),
            "right" => Some(rustkit_css::Length::Percent(100.0)),
            "top" => Some(rustkit_css::Length::Percent(0.0)),
            "bottom" => Some(rustkit_css::Length::Percent(100.0)),
            _ => parse_length(s),
        }
    };

    match parts.len() {
        1 => {
            let x = parse_component(parts[0])?;
            Some(rustkit_css::TransformOrigin {
                x,
                y: rustkit_css::Length::Percent(50.0),
            })
        }
        2 | 3 => {
            let x = parse_component(parts[0])?;
            let y = parse_component(parts[1])?;
            Some(rustkit_css::TransformOrigin { x, y })
        }
        _ => None,
    }
}


/// Serialise GPU device construction across tests.
///
/// Concurrent `Compositor::new()` SIGSEGVs on this box — Argos's parallel R1
/// on #21 found it, and Prometheus's follow-up made the sharper point: a green
/// run at low collision density is NOT a guard. The cascade wire suites no
/// longer need a Compositor at all (see `apply_inline_style_decls`); this
/// exists for the tests that genuinely need a real Engine.
///
/// Poison-tolerant: one panicking test must not brick every later one.
/// (= Athena's Windows #52 `test_compositor`.)
#[cfg(test)]
fn test_compositor() -> Compositor {
    static ENGINE_INIT: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _init_guard = ENGINE_INIT.lock().unwrap_or_else(|e| e.into_inner());
    Compositor::new().expect("failed to create compositor for test")
}

/// Apply the declarations in an inline `style="..."` attribute to a
/// ComputedStyle.
///
/// FREE FUNCTION BY DESIGN: this path needs no engine state - no compositor,
/// no loader, no renderer - so the wire tests can exercise the real cascade
/// without allocating a GPU device per test. Concurrent Compositor::new()
/// under default-parallel `cargo test` SIGSEGVs on this box (found by Argos
/// R1 on PR #21). Engine::apply_inline_style delegates here.
fn apply_inline_style_decls(style: &mut ComputedStyle, style_attr: &str) {
    for declaration in style_attr.split(';') {
        let declaration = declaration.trim();
        if declaration.is_empty() {
            continue;
        }
        if let Some((property, value)) = declaration.split_once(':') {
            let property = property.trim().to_lowercase();
            let value = value.trim();

            match property.as_str() {
                // Transform family WIRE (engine Slice-1 / Cluster A,
                // = Windows #48). The types landed INERT in Linux #10;
                // these arms are what make the properties compute. The
                // renderer does not consume style.transform yet.
                "transform" => {
                    if let Some(list) = parse_transform(value) {
                        style.transform = list;
                    }
                }
                "transform-origin" => {
                    if let Some(origin) = parse_transform_origin(value) {
                        style.transform_origin = origin;
                    }
                }
                // Shadow/Filter family WIRE (Cluster A2, = Windows #49).
                // BoxShadow landed INERT in Linux #11.
                //
                // `none` CLEARS the list rather than pushing nothing, so a
                // later rule can cancel an earlier shadow. This is a
                // deliberate divergence from the macOS reference, which
                // only pushes on successful parse and therefore leaks the
                // earlier value; Athena flagged it on Windows #49 and Atlas
                // confirmed the reference defect. If Prometheus rules for
                // bug-compatibility, this branch and its test revert on
                // BOTH destination trees together.
                "box-shadow" => {
                    if value.trim() == "none" {
                        style.box_shadows.clear();
                    } else if let Some(shadow) = parse_box_shadow(value) {
                        style.box_shadows.push(shadow);
                    }
                }
                // Animation/transition family WIRE (Cluster A3, = Windows
                // #50). Enums landed INERT in Linux #12. PARSED, NOT
                // EXECUTED - nothing animates as a result; the values
                // simply survive the cascade for a future driver.
                "transition-property" => {
                    style.transition_property = value.trim().to_string();
                }
                "transition-duration" => {
                    if let Some(dur) = parse_time(value) {
                        style.transition_duration = dur;
                    }
                }
                "transition-timing-function" => {
                    style.transition_timing_function = parse_timing_function(value);
                }
                "transition-delay" => {
                    if let Some(delay) = parse_time(value) {
                        style.transition_delay = delay;
                    }
                }
                "animation-name" => {
                    style.animation_name = value.trim().to_string();
                }
                "animation-duration" => {
                    if let Some(dur) = parse_time(value) {
                        style.animation_duration = dur;
                    }
                }
                "animation-timing-function" => {
                    style.animation_timing_function = parse_timing_function(value);
                }
                "animation-delay" => {
                    if let Some(delay) = parse_time(value) {
                        style.animation_delay = delay;
                    }
                }
                "animation-iteration-count" => {
                    let v = value.trim();
                    if v == "infinite" {
                        style.animation_iteration_count =
                            rustkit_css::AnimationIterationCount::Infinite;
                    } else if let Ok(n) = v.parse::<f32>() {
                        style.animation_iteration_count =
                            rustkit_css::AnimationIterationCount::Count(n);
                    }
                }
                "animation-direction" => {
                    style.animation_direction = match value.trim() {
                        "normal" => rustkit_css::AnimationDirection::Normal,
                        "reverse" => rustkit_css::AnimationDirection::Reverse,
                        "alternate" => rustkit_css::AnimationDirection::Alternate,
                        "alternate-reverse" => rustkit_css::AnimationDirection::AlternateReverse,
                        _ => rustkit_css::AnimationDirection::Normal,
                    };
                }
                "animation-fill-mode" => {
                    style.animation_fill_mode = match value.trim() {
                        "none" => rustkit_css::AnimationFillMode::None,
                        "forwards" => rustkit_css::AnimationFillMode::Forwards,
                        "backwards" => rustkit_css::AnimationFillMode::Backwards,
                        "both" => rustkit_css::AnimationFillMode::Both,
                        _ => rustkit_css::AnimationFillMode::None,
                    };
                }
                "animation-play-state" => {
                    style.animation_play_state = match value.trim() {
                        "running" => rustkit_css::AnimationPlayState::Running,
                        "paused" => rustkit_css::AnimationPlayState::Paused,
                        _ => rustkit_css::AnimationPlayState::Running,
                    };
                }
                // ---- L0.props.tier1: layout-critical arms ----------
                // Ported from the Windows applier (104 arms) into this tree's
                // 30. Without these, author rules REACHED elements (the L0
                // substrate) but could not take effect for the properties that
                // decide layout - width/height/display were unknown words.
                "width" => {
                    if let Some(l) = parse_length(value) { style.width = l; }
                }
                "height" => {
                    if let Some(l) = parse_length(value) { style.height = l; }
                }
                "min-width" => {
                    if let Some(l) = parse_length(value) { style.min_width = l; }
                }
                "min-height" => {
                    if let Some(l) = parse_length(value) { style.min_height = l; }
                }
                "max-width" => {
                    if let Some(l) = parse_length(value) { style.max_width = l; }
                }
                "max-height" => {
                    if let Some(l) = parse_length(value) { style.max_height = l; }
                }
                // FLEX (tier2). rustkit-layout has had a complete, tested
                // flex container since the port began - layout_flex_container,
                // dispatched from Display::Flex - and `display: flex` already
                // reached it. But not one flex PROPERTY did, so every flex
                // container on Linux rendered at its defaults: row direction,
                // flex-start on both axes, no gap, no grow. The engine was
                // there; nothing could steer it.
                "flex-direction" => {
                    style.flex_direction = match value.trim() {
                        "row-reverse" => rustkit_css::FlexDirection::RowReverse,
                        "column" => rustkit_css::FlexDirection::Column,
                        "column-reverse" => rustkit_css::FlexDirection::ColumnReverse,
                        _ => rustkit_css::FlexDirection::Row,
                    };
                }
                "flex-wrap" => {
                    style.flex_wrap = match value.trim() {
                        "wrap" => rustkit_css::FlexWrap::Wrap,
                        "wrap-reverse" => rustkit_css::FlexWrap::WrapReverse,
                        _ => rustkit_css::FlexWrap::NoWrap,
                    };
                }
                "justify-content" => {
                    style.justify_content = match value.trim() {
                        "flex-end" | "end" | "right" => rustkit_css::JustifyContent::FlexEnd,
                        "center" => rustkit_css::JustifyContent::Center,
                        "space-between" => rustkit_css::JustifyContent::SpaceBetween,
                        "space-around" => rustkit_css::JustifyContent::SpaceAround,
                        "space-evenly" => rustkit_css::JustifyContent::SpaceEvenly,
                        _ => rustkit_css::JustifyContent::FlexStart,
                    };
                }
                "align-items" => {
                    style.align_items = match value.trim() {
                        "flex-start" | "start" => rustkit_css::AlignItems::FlexStart,
                        "flex-end" | "end" => rustkit_css::AlignItems::FlexEnd,
                        "center" => rustkit_css::AlignItems::Center,
                        "baseline" => rustkit_css::AlignItems::Baseline,
                        _ => rustkit_css::AlignItems::Stretch,
                    };
                }
                "align-content" => {
                    style.align_content = match value.trim() {
                        "flex-start" | "start" => rustkit_css::AlignContent::FlexStart,
                        "flex-end" | "end" => rustkit_css::AlignContent::FlexEnd,
                        "center" => rustkit_css::AlignContent::Center,
                        "space-between" => rustkit_css::AlignContent::SpaceBetween,
                        "space-around" => rustkit_css::AlignContent::SpaceAround,
                        "space-evenly" => rustkit_css::AlignContent::SpaceEvenly,
                        _ => rustkit_css::AlignContent::Stretch,
                    };
                }
                "align-self" => {
                    style.align_self = match value.trim() {
                        "flex-start" | "start" => rustkit_css::AlignSelf::FlexStart,
                        "flex-end" | "end" => rustkit_css::AlignSelf::FlexEnd,
                        "center" => rustkit_css::AlignSelf::Center,
                        "baseline" => rustkit_css::AlignSelf::Baseline,
                        "stretch" => rustkit_css::AlignSelf::Stretch,
                        _ => rustkit_css::AlignSelf::Auto,
                    };
                }
                "flex-grow" => {
                    if let Ok(n) = value.trim().parse::<f32>() {
                        style.flex_grow = n;
                    }
                }
                "flex-shrink" => {
                    if let Ok(n) = value.trim().parse::<f32>() {
                        style.flex_shrink = n;
                    }
                }
                "flex-basis" => {
                    if let Some(b) = parse_flex_basis(value) {
                        style.flex_basis = b;
                    }
                }
                "order" => {
                    if let Ok(n) = value.trim().parse::<i32>() {
                        style.order = n;
                    }
                }
                // `gap` is a shorthand: one value sets both axes, two set
                // row then column. Note the ORDER - row-gap comes first,
                // which is the opposite of the row/column reading order
                // people expect from `flex-direction`.
                "gap" | "grid-gap" => {
                    let parts: Vec<&str> = value.split_whitespace().collect();
                    match parts.as_slice() {
                        [one] => {
                            if let Some(l) = parse_length(one) {
                                style.row_gap = l.clone();
                                style.column_gap = l;
                            }
                        }
                        [row, col] => {
                            if let Some(l) = parse_length(row) { style.row_gap = l; }
                            if let Some(l) = parse_length(col) { style.column_gap = l; }
                        }
                        _ => {}
                    }
                }
                "row-gap" => {
                    if let Some(l) = parse_length(value) { style.row_gap = l; }
                }
                "column-gap" => {
                    if let Some(l) = parse_length(value) { style.column_gap = l; }
                }
                // `flex` shorthand. Per spec a single unitless number is
                // grow, and basis becomes 0 - NOT auto. Getting that wrong
                // makes `flex: 1` size to content instead of filling, which
                // is the single most common flex declaration on the web.
                "flex" => {
                    let v = value.trim();
                    match v {
                        "none" => {
                            style.flex_grow = 0.0;
                            style.flex_shrink = 0.0;
                            style.flex_basis = rustkit_css::FlexBasis::Auto;
                        }
                        "auto" => {
                            style.flex_grow = 1.0;
                            style.flex_shrink = 1.0;
                            style.flex_basis = rustkit_css::FlexBasis::Auto;
                        }
                        "initial" => {
                            style.flex_grow = 0.0;
                            style.flex_shrink = 1.0;
                            style.flex_basis = rustkit_css::FlexBasis::Auto;
                        }
                        _ => {
                            let parts: Vec<&str> = v.split_whitespace().collect();
                            match parts.as_slice() {
                                [g] => {
                                    if let Ok(n) = g.parse::<f32>() {
                                        style.flex_grow = n;
                                        style.flex_shrink = 1.0;
                                        style.flex_basis = rustkit_css::FlexBasis::Length(0.0);
                                    } else if let Some(b) = parse_flex_basis(g) {
                                        style.flex_grow = 1.0;
                                        style.flex_shrink = 1.0;
                                        style.flex_basis = b;
                                    }
                                }
                                [g, s] => {
                                    if let Ok(n) = g.parse::<f32>() { style.flex_grow = n; }
                                    if let Ok(n) = s.parse::<f32>() {
                                        style.flex_shrink = n;
                                        style.flex_basis = rustkit_css::FlexBasis::Length(0.0);
                                    } else if let Some(b) = parse_flex_basis(s) {
                                        style.flex_shrink = 1.0;
                                        style.flex_basis = b;
                                    }
                                }
                                [g, s, b] => {
                                    if let Ok(n) = g.parse::<f32>() { style.flex_grow = n; }
                                    if let Ok(n) = s.parse::<f32>() { style.flex_shrink = n; }
                                    if let Some(fb) = parse_flex_basis(b) { style.flex_basis = fb; }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                // POSITION + OFFSETS. rustkit-layout implements positioned
                // layout - 30 Position:: references, Absolute/Fixed branches,
                // out-of-flow skipping in flex.rs and grid.rs - and until now
                // nothing could set style.position, so every page on this tree
                // rendered position:static regardless of what the author wrote
                // and all of that layout code was unreachable.
                //
                // The chain was broken in THREE places: no offset fields on
                // ComputedStyle, no arms here, and no layout assignment.
                // Fixing any ONE alone yields a green computed-value test and
                // no visible change, which is why the tests below are split
                // into a computed-value group and a layout-reaching group.
                "position" => {
                    style.position = match value.trim() {
                        "relative" => rustkit_css::Position::Relative,
                        "absolute" => rustkit_css::Position::Absolute,
                        "fixed" => rustkit_css::Position::Fixed,
                        "sticky" => rustkit_css::Position::Sticky,
                        _ => rustkit_css::Position::Static,
                    };
                }
                // A percentage offset resolves against the containing block,
                // which is not known while the tree is being built. It stays
                // None (auto) rather than becoming an invented pixel value -
                // same restriction as the macOS reference. parse_length
                // returning None for `%` is what enforces that, and there is
                // a test asserting the REFUSAL rather than only the happy path.
                "top" => { if let Some(l) = parse_length(value) { style.top = Some(l); } }
                "right" => { if let Some(l) = parse_length(value) { style.right = Some(l); } }
                "bottom" => { if let Some(l) = parse_length(value) { style.bottom = Some(l); } }
                "left" => { if let Some(l) = parse_length(value) { style.left = Some(l); } }
                // Garbage z-index is IGNORED, not flattened to 0. Flattening
                // would silently restack the page - a wrong answer wearing the
                // costume of a decision.
                "z-index" => {
                    if let Ok(z) = value.trim().parse::<i32>() { style.z_index = z; }
                }
                // TEXT-DECORATION, taken FROM the reachability list rather
                // than invented: rustkit-layout emits decoration commands and
                // nothing could set the property.
                //
                // A shorthand whose tokens arrive in any order and may mix a
                // line with a colour: `underline red`, `red underline`,
                // `underline line-through`. Walk the tokens and take the line
                // flags; an unrecognised token (a colour) must NOT clear a
                // line that another token set.
                "text-decoration" | "text-decoration-line" => {
                    let mut line = rustkit_css::TextDecorationLine::NONE;
                    let mut saw_none = false;
                    for tok in value.split_whitespace() {
                        match tok {
                            "underline" => line.underline = true,
                            "overline" => line.overline = true,
                            "line-through" => line.line_through = true,
                            "none" => saw_none = true,
                            _ => {}
                        }
                    }
                    if saw_none || line != rustkit_css::TextDecorationLine::NONE {
                        style.text_decoration_line = line;
                    }
                }
                "text-decoration-color" => {
                    if let Some(c) = rustkit_css::parse_color(value) {
                        style.text_decoration_color = Some(c);
                    }
                }
                "text-decoration-style" => {
                    style.text_decoration_style = match value.trim() {
                        "double" => rustkit_css::TextDecorationStyle::Double,
                        "dotted" => rustkit_css::TextDecorationStyle::Dotted,
                        "dashed" => rustkit_css::TextDecorationStyle::Dashed,
                        "wavy" => rustkit_css::TextDecorationStyle::Wavy,
                        _ => rustkit_css::TextDecorationStyle::Solid,
                    };
                }
                // NO `text-decoration-thickness` ARM, deliberately. The macOS
                // reference has arms for text-decoration/-line/-color/-style
                // and NONE for thickness - it never assigns the field. Adding
                // one here would be a DIVERGENCE dressed as progress: the
                // metric drops a name and this tree renders thicknesses the
                // reference cannot express.
                //
                // I had already written that arm and declared DIVERGENCE:
                // NONE. My own wireability tool's reference column caught it.
                // The family-level check passed - macOS HAS text-decoration -
                // and the per-property check is what failed it.
                //
                // The propagation below still copies thickness onto the text
                // run: inert while nothing can set it, and it keeps the copy
                // one coherent block for whenever the fleet wires thickness
                // together.
                // GRID (11 arms / 9 fields). Taken FROM the reachability
                // list and verified against BOTH peers before writing:
                // hiwave-windows master and hiwave-macos each implement all
                // eleven. rustkit-layout's layout_grid_container already
                // consumed these fields and is dispatched from
                // display.is_grid(), so the consumer was live and only the
                // producer was missing - the same shape as flex in #30.
                //
                // NOT WIRED, deliberately: justify-items, justify-self and
                // grid-template-areas. My wireability tool calls them
                // WIREABLE - layout reads them and the reader is live - and
                // that is correct and insufficient. NEITHER peer implements
                // them, so they are SHARED LIMITS and wiring them here alone
                // would be a divergence dressed as progress.
                "grid-template-columns" => {
                    if let Some(t) = parse_grid_template(value) { style.grid_template_columns = t; }
                }
                "grid-template-rows" => {
                    if let Some(t) = parse_grid_template(value) { style.grid_template_rows = t; }
                }
                "grid-auto-columns" => {
                    if let Some(t) = parse_track_size(value) { style.grid_auto_columns = t; }
                }
                "grid-auto-rows" => {
                    if let Some(t) = parse_track_size(value) { style.grid_auto_rows = t; }
                }
                "grid-auto-flow" => {
                    style.grid_auto_flow = match value.trim() {
                        "column" => rustkit_css::GridAutoFlow::Column,
                        "row dense" | "dense row" => rustkit_css::GridAutoFlow::RowDense,
                        "column dense" | "dense column" => rustkit_css::GridAutoFlow::ColumnDense,
                        "dense" => rustkit_css::GridAutoFlow::RowDense,
                        _ => rustkit_css::GridAutoFlow::Row,
                    };
                }
                // Shorthands first in source order is irrelevant to matching,
                // but they matter to authors: `grid-column: 1 / 3` is far more
                // common than the longhand pair, and omitting them would leave
                // the common spelling silently dead - the same under-match the
                // child combinator had for `.nav>li`.
                "grid-column" => {
                    if let Some((a, b)) = parse_grid_line_shorthand(value) {
                        style.grid_column_start = a;
                        style.grid_column_end = b;
                    }
                }
                "grid-row" => {
                    if let Some((a, b)) = parse_grid_line_shorthand(value) {
                        style.grid_row_start = a;
                        style.grid_row_end = b;
                    }
                }
                "grid-column-start" => {
                    if let Some(l) = parse_grid_line(value) { style.grid_column_start = l; }
                }
                "grid-column-end" => {
                    if let Some(l) = parse_grid_line(value) { style.grid_column_end = l; }
                }
                "grid-row-start" => {
                    if let Some(l) = parse_grid_line(value) { style.grid_row_start = l; }
                }
                "grid-row-end" => {
                    if let Some(l) = parse_grid_line(value) { style.grid_row_end = l; }
                }
                "display" => {
                    if let Some(d) = rustkit_css::parse_display(value) { style.display = d; }
                }
                "text-align" => {
                    style.text_align = match value.trim() {
                        "center" => rustkit_css::TextAlign::Center,
                        "right" => rustkit_css::TextAlign::Right,
                        "justify" => rustkit_css::TextAlign::Justify,
                        _ => rustkit_css::TextAlign::Left,
                    };
                }
                "line-height" => {
                    if let Ok(n) = value.trim().parse::<f32>() {
                        style.line_height = n;
                    } else if let Some(rustkit_css::Length::Px(px)) = parse_length(value) {
                        style.line_height = px;
                    }
                }
                "font-family" => {
                    let fam = value.split(',').next().unwrap_or(value).trim();
                    let fam = fam.trim_matches(['"', '\'']);
                    if !fam.is_empty() { style.font_family = fam.to_string(); }
                }
                "font-style" => {
                    if value == "italic" || value == "oblique" {
                        style.font_style = rustkit_css::FontStyle::Italic;
                    }
                }
                // Box-model longhands. The `margin`/`padding` SHORTHANDS
                // already existed; a rule setting only one side was dropped.
                "margin-top" => { if let Some(l) = parse_length(value) { style.margin_top = l; } }
                "margin-right" => { if let Some(l) = parse_length(value) { style.margin_right = l; } }
                "margin-bottom" => { if let Some(l) = parse_length(value) { style.margin_bottom = l; } }
                "margin-left" => { if let Some(l) = parse_length(value) { style.margin_left = l; } }
                "padding-top" => { if let Some(l) = parse_length(value) { style.padding_top = l; } }
                "padding-right" => { if let Some(l) = parse_length(value) { style.padding_right = l; } }
                "padding-bottom" => { if let Some(l) = parse_length(value) { style.padding_bottom = l; } }
                "padding-left" => { if let Some(l) = parse_length(value) { style.padding_left = l; } }
                "border-width" => {
                    if let Some(l) = parse_length(value) {
                        style.border_top_width = l.clone();
                        style.border_right_width = l.clone();
                        style.border_bottom_width = l.clone();
                        style.border_left_width = l;
                    }
                }
                "border-color" => {
                    if let Some(c) = parse_color(value) {
                        style.border_top_color = c;
                        style.border_right_color = c;
                        style.border_bottom_color = c;
                        style.border_left_color = c;
                    }
                }
                "color" => {
                    if let Some(color) = parse_color(value) {
                        style.color = color;
                    }
                }
                "background-color" | "background" => {
                    if let Some(color) = parse_color(value) {
                        style.background_color = color;
                    }
                }
                "font-size" => {
                    if let Some(length) = parse_length(value) {
                        style.font_size = length;
                    }
                }
                "font-weight" => {
                    if value == "bold" || value == "700" || value == "800" || value == "900" {
                        style.font_weight = rustkit_css::FontWeight::BOLD;
                    }
                }
                "margin" => {
                    if let Some(length) = parse_length(value) {
                        // Length is not Copy: clone for all but the last
                        // assignment, which still moves (= Windows #42).
                        style.margin_top = length.clone();
                        style.margin_right = length.clone();
                        style.margin_bottom = length.clone();
                        style.margin_left = length;
                    }
                }
                "padding" => {
                    if let Some(length) = parse_length(value) {
                        style.padding_top = length.clone();
                        style.padding_right = length.clone();
                        style.padding_bottom = length.clone();
                        style.padding_left = length;
                    }
                }
                _ => {}
            }
        }
    }
}

/// Parse a CSS time value (e.g. "0.3s", "300ms") into SECONDS.
fn parse_time(value: &str) -> Option<f32> {
    let value = value.trim();
    // "ms" BEFORE "s": "300ms".ends_with('s') is true, so an s-first chain
    // would strip one char, leave "300m", and fail - the overlapping-suffix
    // class (rem/em Linux #3, grad/rad Linux #18). parse_time was already
    // correct on the reference; this ordering is PINNED by a test below so a
    // future edit cannot silently reverse it.
    if value.ends_with("ms") {
        value[..value.len() - 2]
            .parse::<f32>()
            .ok()
            .map(|v| v / 1000.0)
    } else if value.ends_with('s') {
        value[..value.len() - 1].parse::<f32>().ok()
    } else {
        None
    }
}

/// Parse a CSS timing function. Unknown values fall back to Ease, per macOS.
fn parse_timing_function(value: &str) -> rustkit_css::TimingFunction {
    let value = value.trim();
    match value {
        "ease" => rustkit_css::TimingFunction::Ease,
        "linear" => rustkit_css::TimingFunction::Linear,
        "ease-in" => rustkit_css::TimingFunction::EaseIn,
        "ease-out" => rustkit_css::TimingFunction::EaseOut,
        "ease-in-out" => rustkit_css::TimingFunction::EaseInOut,
        "step-start" => rustkit_css::TimingFunction::StepStart,
        "step-end" => rustkit_css::TimingFunction::StepEnd,
        _ if value.starts_with("cubic-bezier(") => {
            let inner = value
                .trim_start_matches("cubic-bezier(")
                .trim_end_matches(')');
            let parts: Vec<f32> = inner
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if parts.len() == 4 {
                rustkit_css::TimingFunction::CubicBezier(parts[0], parts[1], parts[2], parts[3])
            } else {
                rustkit_css::TimingFunction::Ease
            }
        }
        _ if value.starts_with("steps(") => {
            let inner = value.trim_start_matches("steps(").trim_end_matches(')');
            let parts: Vec<&str> = inner.split(',').map(|s| s.trim()).collect();
            if let Some(count) = parts.first().and_then(|s| s.parse::<u32>().ok()) {
                let jump_start = parts
                    .get(1)
                    .map(|s| *s == "jump-start" || *s == "start")
                    .unwrap_or(false);
                rustkit_css::TimingFunction::Steps(count, jump_start)
            } else {
                rustkit_css::TimingFunction::StepEnd
            }
        }
        _ => rustkit_css::TimingFunction::Ease,
    }
}

/// Parse a CSS box-shadow value.
/// Supports: offset-x offset-y [blur [spread]] color [inset]
fn parse_box_shadow(value: &str) -> Option<rustkit_css::BoxShadow> {
    let value = value.trim();
    if value.is_empty() || value == "none" {
        return None;
    }
    let mut shadow = rustkit_css::BoxShadow::new();
    // Check for "inset" keyword
    let (value, inset) = if value.starts_with("inset") {
        // SAFETY: strip_prefix succeeds because we just checked starts_with
        (value.strip_prefix("inset").unwrap().trim(), true)
    } else if value.ends_with("inset") {
        // SAFETY: strip_suffix succeeds because we just checked ends_with
        (value.strip_suffix("inset").unwrap().trim(), true)
    } else {
        (value, false)
    };
    shadow.inset = inset;
    // Split into tokens, being careful about rgba() which contains commas.
    let mut parts: Vec<&str> = Vec::new();
    let mut current_start = 0;
    let mut paren_depth = 0;
    for (i, ch) in value.char_indices() {
        match ch {
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            ' ' if paren_depth == 0 => {
                let part = value[current_start..i].trim();
                if !part.is_empty() {
                    parts.push(part);
                }
                current_start = i + 1;
            }
            _ => {}
        }
    }
    let last_part = value[current_start..].trim();
    if !last_part.is_empty() {
        parts.push(last_part);
    }
    // Format: offset-x offset-y [blur [spread]] color
    let mut lengths: Vec<f32> = Vec::new();
    let mut color_value = None;
    for part in parts {
        if let Some(length) = parse_length(part) {
            lengths.push(length.to_px(16.0, 16.0, 0.0));
        } else if let Some(c) = parse_color(part) {
            color_value = Some(c);
        }
    }
    if lengths.len() >= 2 {
        shadow.offset_x = lengths[0];
        shadow.offset_y = lengths[1];
    } else {
        return None; // Need at least offset-x and offset-y
    }
    if lengths.len() >= 3 {
        shadow.blur_radius = lengths[2].max(0.0);
    }
    if lengths.len() >= 4 {
        shadow.spread_radius = lengths[3];
    }
    shadow.color = color_value.unwrap_or(rustkit_css::Color::new(0, 0, 0, 0.5));
    Some(shadow)
}

fn parse_length(value: &str) -> Option<rustkit_css::Length> {
    let value = value.trim();

    if value == "0" || value == "auto" {
        return Some(if value == "auto" {
            rustkit_css::Length::Auto
        } else {
            rustkit_css::Length::Zero
        });
    }

    if value.ends_with("px") {
        let num: f32 = value.trim_end_matches("px").trim().parse().ok()?;
        return Some(rustkit_css::Length::Px(num));
    }

    // Check "rem" before "em" since "rem" ends with "em"
    if value.ends_with("rem") {
        let num: f32 = value.trim_end_matches("rem").trim().parse().ok()?;
        return Some(rustkit_css::Length::Rem(num));
    }

    if value.ends_with("em") {
        let num: f32 = value.trim_end_matches("em").trim().parse().ok()?;
        return Some(rustkit_css::Length::Em(num));
    }

    if value.ends_with('%') {
        let num: f32 = value.trim_end_matches('%').trim().parse().ok()?;
        return Some(rustkit_css::Length::Percent(num));
    }

    // Bare number (treat as pixels)
    if let Ok(num) = value.parse::<f32>() {
        return Some(rustkit_css::Length::Px(num));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_engine_view_id_uniqueness() {
        let id1 = EngineViewId::new();
        let id2 = EngineViewId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_engine_config_default() {
        let config = EngineConfig::default();
        assert!(config.javascript_enabled);
        assert!(config.cookies_enabled);
    }

    #[test]
    fn test_engine_builder() {
        let builder = EngineBuilder::new()
            .user_agent("Test/1.0")
            .javascript_enabled(false);

        assert_eq!(builder.config.user_agent, "Test/1.0");
        assert!(!builder.config.javascript_enabled);
    }

    #[test]
    fn test_layout_tree_from_document() {
        // Parse a simple HTML document
        let html = r#"<!DOCTYPE html>
            <html>
            <head><title>Test</title></head>
            <body>
                <h1>Hello World</h1>
                <p>This is a paragraph.</p>
            </body>
            </html>"#;
        
        let document = Document::parse_html(html).expect("Failed to parse HTML");
        let document = Rc::new(document);
        
        // Verify document structure
        assert!(document.body().is_some(), "Document should have a body");
        
        // Create a dummy engine using the new() constructor pattern
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let engine = Engine {
            config: EngineConfig::default(),
            views: HashMap::new(),
            viewhost: ViewHost::new(),
            compositor: test_compositor(),
            renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("Failed to create loader")),
            image_manager: Arc::new(ImageManager::new()),
            event_tx,
            event_rx: Some(event_rx),
        };
        
        // Build layout tree from document
        let layout = engine.build_layout_from_document(&document);
        
        // Verify layout tree is not empty
        assert!(!layout.children.is_empty(), "Layout tree should have children from body");
        
        // The body should contain h1 and p elements
        let body_box = &layout.children[0];
        
        // Count text boxes (h1 content "Hello World" and p content "This is a paragraph.")
        fn count_text_boxes(layout_box: &LayoutBox) -> usize {
            let mut count = if matches!(layout_box.box_type, BoxType::Text(_)) {
                1
            } else {
                0
            };
            for child in &layout_box.children {
                count += count_text_boxes(child);
            }
            count
        }
        
        let text_count = count_text_boxes(body_box);
        assert!(text_count >= 2, "Should have at least 2 text boxes (h1 and p content), got {}", text_count);
    }

    #[test]
    fn test_display_list_generation() {
        // Parse a document with styled content
        let html = r#"<!DOCTYPE html>
            <html>
            <body style="background-color: white">
                <h1>Title</h1>
            </body>
            </html>"#;
        
        let document = Document::parse_html(html).expect("Failed to parse HTML");
        let document = Rc::new(document);
        
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let engine = Engine {
            config: EngineConfig::default(),
            views: HashMap::new(),
            viewhost: ViewHost::new(),
            compositor: test_compositor(),
            renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("Failed to create loader")),
            image_manager: Arc::new(ImageManager::new()),
            event_tx,
            event_rx: Some(event_rx),
        };
        
        let mut layout = engine.build_layout_from_document(&document);
        
        // Perform layout with a containing block
        let containing_block = Dimensions {
            content: Rect::new(0.0, 0.0, 800.0, 600.0),
            ..Default::default()
        };
        layout.layout(&containing_block);
        
        // Generate display list
        let display_list = DisplayList::build(&layout);
        
        // Display list should have commands (at least background colors)
        assert!(!display_list.commands.is_empty(), "Display list should have commands, got {:?}", display_list.commands);
    }

    #[test]
    fn test_parse_color() {
        // Test named colors
        assert_eq!(parse_color("black"), Some(rustkit_css::Color::BLACK));
        assert_eq!(parse_color("white"), Some(rustkit_css::Color::WHITE));
        
        // Test hex colors
        assert_eq!(parse_color("#fff"), Some(rustkit_css::Color::from_rgb(255, 255, 255)));
        assert_eq!(parse_color("#000000"), Some(rustkit_css::Color::from_rgb(0, 0, 0)));
        assert_eq!(parse_color("#ff0000"), Some(rustkit_css::Color::from_rgb(255, 0, 0)));
        
        // Test rgb colors
        assert_eq!(parse_color("rgb(255, 0, 0)"), Some(rustkit_css::Color::new(255, 0, 0, 1.0)));
    }

    #[test]
    fn test_parse_length() {
        assert_eq!(parse_length("0"), Some(rustkit_css::Length::Zero));
        assert_eq!(parse_length("auto"), Some(rustkit_css::Length::Auto));
        assert_eq!(parse_length("10px"), Some(rustkit_css::Length::Px(10.0)));
        assert_eq!(parse_length("1.5em"), Some(rustkit_css::Length::Em(1.5)));
        assert_eq!(parse_length("2rem"), Some(rustkit_css::Length::Rem(2.0)));
        assert_eq!(parse_length("50%"), Some(rustkit_css::Length::Percent(50.0)));
    }
}

#[cfg(test)]
mod transform_wire_tests {
    use super::*;
    use rustkit_css::{Length, TransformOp};

    /// Build a headless Engine the way the existing layout tests in this file
    /// do - struct literal, real Compositor (works headless on this box, and
    /// under lavapipe in CI). No lighter constructor exists; the wire path
    /// under test is apply_inline_style, which needs `self`.

    // ---- parser correctness -------------------------------------------------

    #[test]
    fn parses_none_as_identity() {
        let t = parse_transform("none").expect("none must parse");
        assert!(t.is_identity());
    }

    #[test]
    fn parses_a_multi_op_transform_in_source_order() {
        // Order matters for composition, so the ops must not be reordered or
        // deduplicated by the parser.
        let t = parse_transform("translate(10px, 20px) scale(2) rotate(45deg)")
            .expect("multi-op must parse");
        assert_eq!(t.ops.len(), 3);
        assert!(matches!(t.ops[0], TransformOp::Translate(..)));
        assert!(matches!(t.ops[1], TransformOp::Scale(..)));
        assert!(matches!(t.ops[2], TransformOp::Rotate(_)));
    }

    #[test]
    fn scale_with_one_arg_applies_to_both_axes() {
        let t = parse_transform("scale(3)").expect("parse");
        match t.ops[0] {
            TransformOp::Scale(x, y) => assert_eq!((x, y), (3.0, 3.0)),
            ref other => panic!("expected Scale, got {:?}", other),
        }
    }

    #[test]
    fn angle_units_all_convert_to_degrees() {
        // A parser that only handled `deg` would pass the common case and
        // silently mis-render rad/turn/grad.
        assert_eq!(parse_angle("90deg"), Some(90.0));
        assert_eq!(parse_angle("1turn"), Some(360.0));
        // REGRESSION GUARD: "200grad".ends_with("rad") is true. With rad
        // tested first, the grad branch is unreachable and every grad angle
        // becomes None - dropping the whole transform declaration. Same shape
        // as rem-before-em (Linux #3); fix applied ON ARRIVAL per Windows #48.
        assert_eq!(parse_angle("200grad"), Some(180.0));
        assert_eq!(parse_angle("100grad"), Some(90.0));
        assert_eq!(parse_angle("45"), Some(45.0), "bare number defaults to deg");
        let rad = parse_angle("3.14159265rad").expect("rad must parse");
        assert!((rad - 180.0).abs() < 0.01, "1 pi rad == 180deg, got {}", rad);
    }

    #[test]
    fn every_angle_unit_expressing_the_same_angle_converges() {
        // THE GENERALISING GUARD (Prometheus, macOS #72 review): per-unit
        // assertions cannot see suffix-eating, which is exactly why the
        // grad/rad bug survived per-unit tests on the reference tree. Asserting
        // that every spelling of ~90 degrees CONVERGES catches the class - any
        // future unit added to parse_angle whose suffix overlaps an existing
        // one fails here without anyone having to predict the collision.
        let ninety = ["90deg", "100grad", "0.25turn", "1.5708rad", "90"];
        for spelling in ninety {
            let got = parse_angle(spelling)
                .unwrap_or_else(|| panic!("{spelling} must parse, got None (suffix eaten?)"));
            assert!(
                (got - 90.0).abs() < 0.01,
                "{spelling} should be ~90 degrees, got {got}"
            );
        }
    }

    #[test]
    fn transform_origin_keywords_map_to_percentages() {
        let o = parse_transform_origin("left top").expect("parse");
        assert_eq!(o.x, Length::Percent(0.0));
        assert_eq!(o.y, Length::Percent(0.0));
        let c = parse_transform_origin("center").expect("parse");
        assert_eq!(c.x, Length::Percent(50.0));
        assert_eq!(c.y, Length::Percent(50.0), "single value defaults y to 50%");
    }

    #[test]
    fn garbage_does_not_panic_and_yields_none_or_identity() {
        for bad in ["translate(", "rotate(abc)", "notafunction(1)", ""] {
            let _ = parse_transform(bad);
        }
    }

    // ---- the WIRE: properties must now COMPUTE ------------------------------
    // Linux's single declaration-application path is apply_inline_style, so
    // the wire receipts drive that real path (Windows #48 used its
    // apply_declaration refactor; Linux has no such method - not invented).

    #[test]
    fn transform_declaration_computes_into_style() {
        // THIS is the wire receipt. Before this PR the declaration was dropped
        // on the floor: no "transform" arm, no ComputedStyle field.
        let mut style = ComputedStyle::default();
        assert!(style.transform.is_identity(), "default must be identity");

        apply_inline_style_decls(&mut style, "transform: scale(2)");
        assert!(
            !style.transform.is_identity(),
            "transform: scale(2) must compute into ComputedStyle"
        );
        assert_eq!(style.transform.ops.len(), 1);
    }

    #[test]
    fn transform_origin_declaration_computes_into_style() {
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(&mut style, "transform-origin: left top");
        assert_eq!(style.transform_origin.x, Length::Percent(0.0));
        assert_eq!(style.transform_origin.y, Length::Percent(0.0));
    }

    #[test]
    fn an_invalid_transform_leaves_the_previous_value_untouched() {
        // CSS: an invalid declaration is ignored, not reset to initial.
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(&mut style, "transform: scale(2)");
        let before = style.transform.ops.len();
        apply_inline_style_decls(&mut style, "transform: !!!garbage!!!");
        assert_eq!(
            style.transform.ops.len(), before,
            "invalid value must not clobber the computed transform"
        );
    }
}

#[cfg(test)]
mod shadow_wire_tests {
    use super::*;

    /// Same headless Engine literal the transform wire tests use.

    #[test]
    fn parses_offsets_blur_and_colour() {
        let s = parse_box_shadow("2px 4px 6px rgb(255, 0, 0)").expect("must parse");
        assert_eq!((s.offset_x, s.offset_y, s.blur_radius), (2.0, 4.0, 6.0));
        assert_eq!(s.color.r, 255);
        assert!(!s.inset);
    }

    #[test]
    fn rgba_commas_do_not_split_the_token_list() {
        // The parser tracks paren depth precisely because rgba() contains
        // commas and spaces; a naive split would shred the colour into
        // fragments and lose it.
        let s = parse_box_shadow("1px 2px 3px rgba(0, 0, 0, 0.5)").expect("must parse");
        assert_eq!((s.offset_x, s.offset_y, s.blur_radius), (1.0, 2.0, 3.0));
        assert!(s.color.a < 1.0, "alpha must survive, got {}", s.color.a);
    }

    #[test]
    fn inset_keyword_is_recognised() {
        let s = parse_box_shadow("0 0 4px #000 inset").expect("must parse");
        assert!(s.inset);
    }

    #[test]
    fn none_and_empty_yield_no_shadow() {
        assert!(parse_box_shadow("none").is_none());
        assert!(parse_box_shadow("").is_none());
    }

    // ---- the WIRE (via apply_inline_style, Linux's real declaration path) ---

    #[test]
    fn box_shadow_declaration_computes_into_style() {
        let mut style = ComputedStyle::default();
        assert!(style.box_shadows.is_empty(), "default has no shadows");
        apply_inline_style_decls(&mut style, "box-shadow: 2px 4px 6px #000");
        assert_eq!(style.box_shadows.len(), 1, "box-shadow must compute");
        assert_eq!(style.box_shadows[0].offset_x, 2.0);
    }

    #[test]
    fn box_shadow_none_clears_a_previously_computed_shadow() {
        // A later rule must be able to cancel an earlier one. If `none` were
        // simply "parse fails, push nothing", the earlier shadow would survive
        // and the element would keep a shadow the author removed.
        //
        // DELIBERATE DIVERGENCE from the macOS reference (which has that
        // defect — confirmed by Atlas from source). Reverts on both trees
        // together if Prometheus rules for bug-compatibility.
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(&mut style, "box-shadow: 2px 4px 6px #000");
        assert_eq!(style.box_shadows.len(), 1);
        apply_inline_style_decls(&mut style, "box-shadow: none");
        assert!(style.box_shadows.is_empty(), "none must clear the list");
    }

    #[test]
    fn shadow_is_visible_predicate_agrees_with_the_parsed_value() {
        // Ties the wire back to the INERT type's own logic from Linux #11.
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(&mut style, "box-shadow: 0 0 0 rgba(0,0,0,0)");
        if let Some(s) = style.box_shadows.first() {
            assert!(!s.is_visible(), "fully transparent, zero geometry: not visible");
        }
        let mut style2 = ComputedStyle::default();
        apply_inline_style_decls(&mut style2, "box-shadow: 3px 3px 5px #000");
        assert!(style2.box_shadows[0].is_visible());
    }
}

#[cfg(test)]
mod animation_wire_tests {
    use super::*;
    use rustkit_css::{AnimationDirection, AnimationFillMode, AnimationIterationCount,
                      AnimationPlayState, TimingFunction};


    #[test]
    fn ms_suffix_is_tested_before_s() {
        // PIN, not a fix: parse_time is already correct. "300ms".ends_with('s')
        // is TRUE, so an s-first chain would strip one char, leave "300m", and
        // return None - the same overlapping-suffix class as rem/em (#3) and
        // grad/rad (#18). Athena's fleet sweep confirmed ms/s was clean; this
        // pins it so a future edit cannot silently reverse the order.
        assert_eq!(parse_time("300ms"), Some(0.3));
        assert_eq!(parse_time("0.3s"), Some(0.3));
        assert_eq!(parse_time("1s"), Some(1.0));
        assert_eq!(parse_time("bogus"), None);
    }

    #[test]
    fn durations_are_stored_in_seconds_regardless_of_authoring() {
        // Same duration written two ways must land on the same number, or
        // downstream code silently sees a 1000x difference.
        let mut a = ComputedStyle::default();
        let mut b = ComputedStyle::default();
        apply_inline_style_decls(&mut a, "animation-duration: 250ms");
        apply_inline_style_decls(&mut b, "animation-duration: 0.25s");
        assert_eq!(a.animation_duration, b.animation_duration);
        assert_eq!(a.animation_duration, 0.25);
    }

    #[test]
    fn fractional_iteration_counts_survive() {
        // 2.5 is legal CSS. An integer-typed wire truncates it silently.
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(&mut style, "animation-iteration-count: 2.5");
        assert_eq!(style.animation_iteration_count, AnimationIterationCount::Count(2.5));
        apply_inline_style_decls(&mut style, "animation-iteration-count: infinite");
        assert_eq!(style.animation_iteration_count, AnimationIterationCount::Infinite);
    }

    #[test]
    fn transition_and_animation_timing_compute_independently() {
        // Both share TimingFunction, so a crossed wire is INVISIBLE unless
        // both are asserted in one test with different values.
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(
            &mut style,
            "transition-timing-function: linear; animation-timing-function: ease-in",
        );
        assert_eq!(style.transition_timing_function, TimingFunction::Linear);
        assert_eq!(style.animation_timing_function, TimingFunction::EaseIn);
    }

    #[test]
    fn transition_and_animation_durations_do_not_cross() {
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(
            &mut style,
            "transition-duration: 100ms; animation-duration: 900ms",
        );
        assert_eq!(style.transition_duration, 0.1);
        assert_eq!(style.animation_duration, 0.9);
    }

    #[test]
    fn parametric_timing_functions_parse() {
        assert_eq!(
            parse_timing_function("cubic-bezier(0.25, 0.1, 0.25, 1.0)"),
            TimingFunction::CubicBezier(0.25, 0.1, 0.25, 1.0)
        );
        assert_eq!(parse_timing_function("steps(4, jump-start)"), TimingFunction::Steps(4, true));
        assert_eq!(parse_timing_function("steps(4)"), TimingFunction::Steps(4, false));
        // Unknown falls back to Ease, matching the reference.
        assert_eq!(parse_timing_function("nonsense"), TimingFunction::Ease);
    }

    #[test]
    fn animation_keywords_compute() {
        let mut style = ComputedStyle::default();
        apply_inline_style_decls(
            &mut style,
            "animation-direction: alternate-reverse; animation-fill-mode: both; \
             animation-play-state: paused; animation-name: slide",
        );
        assert_eq!(style.animation_direction, AnimationDirection::AlternateReverse);
        assert_eq!(style.animation_fill_mode, AnimationFillMode::Both);
        assert_eq!(style.animation_play_state, AnimationPlayState::Paused);
        assert_eq!(style.animation_name, "slide");
    }

    #[test]
    fn the_whole_family_computes_from_defaults() {
        // THE WIRE RECEIPT: before this PR every one of these declarations was
        // dropped on the floor - no arms, no fields.
        let mut style = ComputedStyle::default();
        assert_eq!(style.animation_duration, 0.0, "default is zero");
        apply_inline_style_decls(
            &mut style,
            "transition-property: opacity; transition-delay: 50ms; \
             animation-delay: 2s; animation-duration: 1s",
        );
        assert_eq!(style.transition_property, "opacity");
        assert_eq!(style.transition_delay, 0.05);
        assert_eq!(style.animation_delay, 2.0);
        assert_eq!(style.animation_duration, 1.0);
    }
}

#[cfg(test)]
mod author_stylesheet_tests {
    use super::*;
    use rustkit_css::Length;

    fn doc(html: &str) -> Document {
        Document::parse_html(html).expect("parse")
    }

    /// Compute the style the engine would give the FIRST element matching
    /// `tag` at any depth, exercising the real collect -> parse -> match ->
    /// apply path. No Engine and no GPU: the pieces under test are associated
    /// functions and a &self method that touches no engine state.
    fn style_of(html: &str, tag: &str) -> ComputedStyle {
        let d = doc(html);
        let mut css = String::new();
        collect_style_text_free(&d.root(), &mut css);
        let sheet = Stylesheet::parse(&css).unwrap_or_else(|_| Stylesheet::new());
        let mut found = None;
        walk(&d.root(), &sheet, &[], tag, &mut found);
        found.unwrap_or_else(|| panic!("no <{tag}> in fixture"))
    }

    // Free mirrors of the engine's walk, so the tests need no Engine instance.
    fn collect_style_text_free(node: &Rc<Node>, out: &mut String) {
        if let NodeType::Element { tag_name, .. } = &node.node_type {
            if tag_name.eq_ignore_ascii_case("style") {
                for child in node.children() {
                    if let NodeType::Text(t) = &child.node_type {
                        out.push_str(t);
                        out.push('\n');
                    }
                }
                return;
            }
        }
        for child in node.children() {
            collect_style_text_free(&child, out);
        }
    }

    fn walk(
        node: &Rc<Node>,
        sheet: &Stylesheet,
        ancestors: &[ElementCtx],
        want: &str,
        out: &mut Option<ComputedStyle>,
    ) {
        if out.is_some() {
            return;
        }
        if let NodeType::Element { tag_name, attributes, .. } = &node.node_type {
            let classes: Vec<&str> = attributes
                .get("class")
                .map(|c| c.split_whitespace().collect())
                .unwrap_or_default();
            let id = attributes.get("id").map(|s| s.as_str());
            if tag_name.eq_ignore_ascii_case(want) {
                // Reproduce the engine's cascade order: UA-ish base, then
                // author rules by (specificity, source order), then inline.
                let mut style = ComputedStyle::new();
                let mut matched: Vec<(u32, usize)> = Vec::new();
                for (i, rule) in sheet.rules.iter().enumerate() {
                    if let Some(spec) =
                        Engine::selector_matches(&rule.selector, tag_name, &classes, id, ancestors)
                    {
                        matched.push((spec, i));
                    }
                }
                matched.sort_by_key(|&(spec, i)| (spec, i));
                for (_, i) in matched {
                    for decl in &sheet.rules[i].declarations {
                        if decl.property.starts_with("--") {
                            continue;
                        }
                        if let PropertyValue::Specified(v) = &decl.value {
                            apply_inline_style_decls(
                                &mut style,
                                &format!("{}: {}", decl.property, v),
                            );
                        }
                    }
                }
                if let Some(attr) = attributes.get("style") {
                    apply_inline_style_decls(&mut style, attr);
                }
                *out = Some(style);
                return;
            }
            let mut next = ancestors.to_vec();
            next.push(ElementCtx {
                tag: tag_name.to_lowercase(),
                classes: classes.iter().map(|s| s.to_string()).collect(),
                id: id.map(|s| s.to_string()),
            });
            for child in node.children() {
                walk(&child, sheet, &next, want, out);
            }
            return;
        }
        for child in node.children() {
            walk(&child, sheet, ancestors, want, out);
        }
    }

    // ---- THE RECEIPT: a <style> rule reaches a DESCENDANT element ----------

    #[test]
    fn a_style_rule_computes_onto_a_descendant_element() {
        // Prometheus's required receipt shape: assert a DESCENDANT's computed
        // style, not the root. Before L0 this width was dropped entirely -
        // <style> was in the engine's skip list, so author CSS never existed.
        let s = style_of(
            r#"<html><head><style>p { font-size: 123px; }</style></head>
               <body><div><p>hi</p></div></body></html>"#,
            "p",
        );
        assert_eq!(s.font_size, Length::Px(123.0), "author rule must reach the <p>");
    }

    #[test]
    fn descendant_selector_requires_the_ancestor_chain() {
        let html = r#"<html><head><style>.card p { font-size: 50px; }</style></head>
            <body><div class="card"><p>in</p></div></body></html>"#;
        assert_eq!(style_of(html, "p").font_size, Length::Px(50.0));
        // Same rule, no .card ancestor -> must NOT match.
        let outside = r#"<html><head><style>.card p { font-size: 50px; }</style></head>
            <body><div><p>out</p></div></body></html>"#;
        assert_ne!(style_of(outside, "p").font_size, Length::Px(50.0));
    }

    #[test]
    fn specificity_orders_the_cascade_not_source_order_alone() {
        // #id (100) must beat .class (10) must beat tag (1), regardless of the
        // order the rules appear in.
        let s = style_of(
            r#"<html><head><style>
                 #only { font-size: 300px; }
                 p { font-size: 100px; }
                 .c { font-size: 200px; }
               </style></head>
               <body><p class="c" id="only">x</p></body></html>"#,
            "p",
        );
        assert_eq!(s.font_size, Length::Px(300.0), "#id must win");
    }

    #[test]
    fn equal_specificity_falls_back_to_source_order() {
        let s = style_of(
            r#"<html><head><style>p { font-size: 10px; } p { font-size: 20px; }</style></head>
               <body><p>x</p></body></html>"#,
            "p",
        );
        assert_eq!(s.font_size, Length::Px(20.0), "later rule wins at equal specificity");
    }

    #[test]
    fn inline_style_attribute_beats_the_author_sheet() {
        let s = style_of(
            r#"<html><head><style>p { font-size: 10px; }</style></head>
               <body><p style="font-size: 99px">x</p></body></html>"#,
            "p",
        );
        assert_eq!(s.font_size, Length::Px(99.0), "inline must win over the sheet");
    }

    #[test]
    fn comma_groups_and_star_match() {
        let s = style_of(
            r#"<html><head><style>h1, p { font-size: 42px; }</style></head>
               <body><p>x</p></body></html>"#,
            "p",
        );
        assert_eq!(s.font_size, Length::Px(42.0));
        assert!(Engine::selector_matches("*", "div", &[], None, &[]).is_some());
    }

    #[test]
    fn unsupported_selector_forms_do_not_match_rather_than_matching_wrongly() {
        // Pseudo-classes, attribute and sibling selectors are the B1 campaign.
        // The honest failure is NO match; matching them loosely would apply
        // rules the author scoped tightly.
        assert!(Engine::selector_matches("p:hover", "p", &[], None, &[]).is_none());
        assert!(Engine::selector_matches("[data-x]", "p", &[], None, &[]).is_none());
        assert!(Engine::selector_matches("h1 + p", "p", &[], None, &[]).is_none());
    }

    #[test]
    fn a_page_with_no_style_element_is_unchanged() {
        // Regression guard: L0 must not alter pages that have no author CSS.
        let s = style_of(r#"<html><body><p style="font-size: 7px">x</p></body></html>"#, "p");
        assert_eq!(s.font_size, Length::Px(7.0));
    }
}

#[cfg(test)]
mod props_tier1_tests {
    use super::*;
    use rustkit_css::{Display, Length, TextAlign};

    fn applied(decls: &str) -> ComputedStyle {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, decls);
        s
    }

    /// THE RECEIPT PROMETHEUS ASKED FOR: Athena's #54 shape, width:123 on a
    /// descendant via an author rule. This exact assertion FAILED on the L0
    /// branch (width came back Auto) because the applier had no `width` arm —
    /// which is how the property-coverage gap was found. It passes now.
    #[test]
    fn athena_54_shape_width_123_on_a_descendant() {
        let d = Document::parse_html(
            r#"<html><head><style>.card p { width: 123px; }</style></head>
               <body><div class="card"><p>x</p></div></body></html>"#,
        )
        .expect("parse");
        let mut css = String::new();
        collect_free(&d.root(), &mut css);
        let sheet = Stylesheet::parse(&css).expect("sheet");
        let ancestors = vec![
            ElementCtx { tag: "body".into(), classes: vec![], id: None },
            ElementCtx { tag: "div".into(), classes: vec!["card".into()], id: None },
        ];
        let mut style = ComputedStyle::new();
        for rule in &sheet.rules {
            if Engine::selector_matches(&rule.selector, "p", &[], None, &ancestors).is_some() {
                for decl in &rule.declarations {
                    if let PropertyValue::Specified(v) = &decl.value {
                        apply_inline_style_decls(&mut style, &format!("{}: {}", decl.property, v));
                    }
                }
            }
        }
        assert_eq!(style.width, Length::Px(123.0), "width must now take effect");
    }

    fn collect_free(node: &Rc<Node>, out: &mut String) {
        if let NodeType::Element { tag_name, .. } = &node.node_type {
            if tag_name.eq_ignore_ascii_case("style") {
                for c in node.children() {
                    if let NodeType::Text(t) = &c.node_type {
                        out.push_str(t);
                        out.push('\n');
                    }
                }
                return;
            }
        }
        for c in node.children() {
            collect_free(&c, out);
        }
    }

    #[test]
    fn box_dimensions_and_constraints_apply() {
        let s = applied("width: 10px; height: 20px; min-width: 1px; max-width: 99px; \
                         min-height: 2px; max-height: 88px");
        assert_eq!(s.width, Length::Px(10.0));
        assert_eq!(s.height, Length::Px(20.0));
        assert_eq!(s.min_width, Length::Px(1.0));
        assert_eq!(s.max_width, Length::Px(99.0));
        assert_eq!(s.min_height, Length::Px(2.0));
        assert_eq!(s.max_height, Length::Px(88.0));
    }

    #[test]
    fn display_applies_including_the_flex_that_layout_branches_on() {
        assert_eq!(applied("display: flex").display, Display::Flex);
        assert_eq!(applied("display: none").display, Display::None);
        // Unknown value must not clobber the computed value.
        let mut s = applied("display: flex");
        apply_inline_style_decls(&mut s, "display: bogus-value");
        assert_eq!(s.display, Display::Flex, "invalid display must be ignored");
    }

    #[test]
    fn margin_and_padding_longhands_do_not_cross() {
        // One test asserting all eight, with DISTINCT values: a crossed wire
        // (top writing to bottom) is invisible if the values match or if each
        // side is asserted alone.
        let s = applied("margin-top: 1px; margin-right: 2px; margin-bottom: 3px; margin-left: 4px; \
                         padding-top: 5px; padding-right: 6px; padding-bottom: 7px; padding-left: 8px");
        assert_eq!(s.margin_top, Length::Px(1.0));
        assert_eq!(s.margin_right, Length::Px(2.0));
        assert_eq!(s.margin_bottom, Length::Px(3.0));
        assert_eq!(s.margin_left, Length::Px(4.0));
        assert_eq!(s.padding_top, Length::Px(5.0));
        assert_eq!(s.padding_right, Length::Px(6.0));
        assert_eq!(s.padding_bottom, Length::Px(7.0));
        assert_eq!(s.padding_left, Length::Px(8.0));
    }

    #[test]
    fn longhand_after_shorthand_wins_source_order() {
        // `margin: 5px; margin-left: 50px` must leave left=50, others 5.
        let s = applied("margin: 5px; margin-left: 50px");
        assert_eq!(s.margin_left, Length::Px(50.0));
        assert_eq!(s.margin_top, Length::Px(5.0));
    }

    #[test]
    fn text_and_font_properties_apply() {
        let s = applied("text-align: center; line-height: 1.5; font-family: Georgia, serif; \
                         font-style: italic");
        assert_eq!(s.text_align, TextAlign::Center);
        assert_eq!(s.line_height, 1.5);
        assert_eq!(s.font_family, "Georgia", "first family wins, quotes/space trimmed");
        assert_eq!(s.font_style, rustkit_css::FontStyle::Italic);
    }

    #[test]
    fn line_height_accepts_both_a_number_and_a_length() {
        assert_eq!(applied("line-height: 2").line_height, 2.0);
        assert_eq!(applied("line-height: 24px").line_height, 24.0);
    }

    #[test]
    fn border_shorthand_sides_are_all_set() {
        let s = applied("border-width: 3px; border-color: #ff0000");
        assert_eq!(s.border_top_width, Length::Px(3.0));
        assert_eq!(s.border_left_width, Length::Px(3.0));
        assert_eq!(s.border_bottom_color.r, 255);
    }

    #[test]
    fn quoted_font_family_is_unquoted() {
        assert_eq!(applied("font-family: \"Times New Roman\", serif").font_family, "Times New Roman");
    }
}

#[cfg(test)]
mod inheritance_tests {
    use super::*;
    use rustkit_css::{Color, Length, TextAlign};

    /// Drive the REAL layout path — Engine::build_layout_from_document — not a
    /// test-local mirror of the walk. Argos's N1 note on #23 was that mirror
    /// tests cannot see a divergence between the mirror and the real walk; this
    /// unit's receipts answer that by going through the engine itself.
    /// Uses the serialised test_compositor so parallel runs cannot race GPU
    /// init (the #21 lesson).
    fn engine() -> Engine {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Engine {
            config: EngineConfig::default(),
            views: HashMap::new(),
            viewhost: ViewHost::new(),
            compositor: test_compositor(),
            renderer: None,
            loader: Arc::new(
                ResourceLoader::new(LoaderConfig::default()).expect("loader"),
            ),
            image_manager: Arc::new(ImageManager::new()),
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    /// Find the first layout box whose text matches, returning its style.
    fn find_text<'a>(b: &'a LayoutBox, want: &str) -> Option<&'a ComputedStyle> {
        if let BoxType::Text(t) = &b.box_type {
            if t.contains(want) {
                return Some(&b.style);
            }
        }
        b.children.iter().find_map(|c| find_text(c, want))
    }

    /// Find the first box that carries a distinguishing computed value.
    fn find_depth(b: &LayoutBox, depth: usize) -> Option<&LayoutBox> {
        if depth == 0 {
            return Some(b);
        }
        b.children.first().and_then(|c| find_depth(c, depth - 1))
    }

    fn layout(html: &str) -> LayoutBox {
        let e = engine();
        let d = Document::parse_html(html).expect("parse");
        e.build_layout_from_document(&d)
    }

    #[test]
    fn multi_property_inheritance_reaches_a_deep_descendant() {
        // THE RECEIPT: three inherited properties set once on <body>, asserted
        // on a nested element through the real layout build. Before this unit
        // every element started from ComputedStyle::new() with a forced BLACK,
        // so none of these crossed a single level.
        let root = layout(
            r#"<html><head><style>
                 body { color: #ff0000; font-size: 21px; font-family: Georgia; }
               </style></head>
               <body><div><section><p>deep</p></section></div></body></html>"#,
        );
        let s = find_text(&root, "deep").expect("text box");
        assert_eq!(s.color, Color::from_rgb(255, 0, 0), "color must inherit");
        assert_eq!(s.font_size, Length::Px(21.0), "font-size must inherit");
        assert_eq!(s.font_family, "Georgia", "font-family must inherit");
    }

    #[test]
    fn a_non_inherited_property_does_NOT_leak_to_descendants() {
        // The other half of the claim, and the one a naive "copy parent style"
        // implementation gets wrong: width is NOT an inherited property.
        let root = layout(
            r#"<html><head><style>body { width: 500px; color: #00ff00; }</style></head>
               <body><div><p>x</p></div></body></html>"#,
        );
        let s = find_text(&root, "x").expect("text box");
        assert_eq!(s.color, Color::from_rgb(0, 255, 0), "color inherits");
        // POSITIVE residual, per Prometheus's N-width-positive: assert the CSS
        // initial, not merely "not the parent's value". assert_ne would pass if
        // width came back garbage in a NEW way - and it did: before the
        // inherit_from partition fix this was Length::Zero, which is not 500px
        // and is also completely wrong. The weaker assertion shipped the bug.
        assert_eq!(s.width, Length::Auto, "width must reset to its CSS initial");
    }

    #[test]
    fn a_descendant_rule_overrides_the_inherited_value() {
        let root = layout(
            r#"<html><head><style>
                 body { color: #ff0000; }
                 p { color: #0000ff; }
               </style></head>
               <body><div><p>over</p></div></body></html>"#,
        );
        let s = find_text(&root, "over").expect("text box");
        assert_eq!(s.color, Color::from_rgb(0, 0, 255), "own rule beats inherited");
    }

    #[test]
    fn text_nodes_inherit_from_their_containing_element() {
        // Text is where inheritance is actually visible to a user: colouring a
        // <p> must colour the words in it, not just the box.
        let root = layout(
            r#"<html><head><style>p { color: #123456; }</style></head>
               <body><p>words</p></body></html>"#,
        );
        let s = find_text(&root, "words").expect("text box");
        assert_eq!(s.color, Color::from_rgb(0x12, 0x34, 0x56));
    }

    #[test]
    fn text_align_inherits_portably() {
        let root = layout(
            r#"<html><head><style>body { text-align: center; }</style></head>
               <body><div><p>c</p></div></body></html>"#,
        );
        let s = find_text(&root, "c").expect("text box");
        assert_eq!(s.text_align, TextAlign::Center);
    }

    #[test]
    fn default_colour_is_still_black_without_any_author_rule() {
        // Regression guard: dropping the unconditional BLACK must not leave
        // text colourless. The root seeds it once instead.
        let root = layout(r#"<html><body><p>plain</p></body></html>"#);
        let s = find_text(&root, "plain").expect("text box");
        assert_eq!(s.color, Color::BLACK);
    }

    #[test]
    fn ua_defaults_win_over_an_inherited_value() {
        // Prometheus N-ua-stub: this test previously asserted only
        // find_depth(..).is_some() while its NAME promised UA-beats-inherited
        // ordering. A test that names an invariant it does not check makes the
        // invariant look covered - the same defect class as a vacuous root
        // assertion. Now it asserts the ordering.
        //
        // body sets 10px; h1's UA default is 32px and is applied AFTER
        // inheriting, so the h1 must be 32px, not the inherited 10px.
        let root = layout(
            r#"<html><head><style>body { font-size: 10px; }</style></head>
               <body><h1>big</h1></body></html>"#,
        );
        let s = find_text(&root, "big").expect("h1 text box");
        // The text inherits from the h1, so it carries the h1's computed size.
        assert_eq!(s.font_size, Length::Px(32.0), "UA h1 size must beat the inherited 10px");
    }

    #[test]
    fn inheriting_does_not_make_elements_zero_sized_black_or_invisible() {
        // REGRESSION GUARD for the defect this unit nearly shipped. Linux's
        // inherit_from fell through to ..Default::default() for width/height/
        // background/opacity, whose DERIVED defaults are Zero / opaque BLACK /
        // 0.0 - so every inheriting element would have been 0x0, painted black,
        // and fully transparent. Every other test in this file still passed.
        let root = layout(
            r#"<html><head><style>body { color: #ff0000; }</style></head>
               <body><div><p>x</p></div></body></html>"#,
        );
        let s = find_text(&root, "x").expect("text box");
        assert_eq!(s.width, Length::Auto, "must not inherit a Zero width");
        assert_eq!(s.height, Length::Auto, "must not inherit a Zero height");
        assert_eq!(s.background_color, Color::TRANSPARENT, "must not paint black");
        assert_eq!(s.opacity, 1.0, "must not be invisible");
    }
}


#[cfg(test)]
mod external_stylesheet_tests {
    use super::*;
    use rustkit_css::Length;

    fn doc(html: &str) -> Document {
        Document::parse_html(html).expect("parse")
    }
    fn base() -> Url {
        Url::parse("https://example.com/dir/page.html").unwrap()
    }

    #[test]
    fn relative_href_resolves_against_the_document_url() {
        let d = doc(r#"<html><head><link rel="stylesheet" href="site.css"></head><body></body></html>"#);
        let urls = Engine::discover_external_stylesheets(&d, Some(&base()));
        assert_eq!(urls.len(), 1);
        assert_eq!(urls[0].as_str(), "https://example.com/dir/site.css");
    }

    #[test]
    fn root_relative_and_absolute_hrefs_both_resolve() {
        let d = doc(
            r#"<html><head>
               <link rel="stylesheet" href="/a.css">
               <link rel="stylesheet" href="https://cdn.example.org/b.css">
               </head><body></body></html>"#,
        );
        let got: Vec<String> = Engine::discover_external_stylesheets(&d, Some(&base()))
            .iter().map(|u| u.to_string()).collect();
        assert!(got.iter().any(|u| u == "https://example.com/a.css"), "got {got:?}");
        assert!(got.iter().any(|u| u == "https://cdn.example.org/b.css"), "got {got:?}");
    }

    #[test]
    fn non_stylesheet_links_are_ignored() {
        // <link> is also used for icons, preconnect and manifests. Treating
        // every <link href> as CSS would fetch the favicon and parse it as CSS.
        let d = doc(
            r#"<html><head>
               <link rel="icon" href="favicon.ico">
               <link rel="preconnect" href="https://fonts.example.com">
               <link rel="manifest" href="app.webmanifest">
               </head><body></body></html>"#,
        );
        assert!(Engine::discover_external_stylesheets(&d, Some(&base())).is_empty());
    }

    #[test]
    fn rel_is_a_token_set_and_case_insensitive() {
        let d = doc(
            r#"<html><head>
               <link rel="alternate stylesheet" href="alt.css">
               <link REL="StyleSheet" href="caps.css">
               </head><body></body></html>"#,
        );
        // Both match: rel is an unordered token set, matched case-insensitively.
        // KNOWN LIMIT, inherited from the Windows recipe and stated rather than
        // discovered: `alternate stylesheet` sheets are ALTERNATE and should not
        // enter the default cascade. Over-applying (both themes) is milder than
        // under-applying (zero CSS), but it is wrong for theme-switcher pages.
        assert_eq!(Engine::discover_external_stylesheets(&d, Some(&base())).len(), 2);
    }

    #[test]
    fn an_unparseable_href_is_skipped_not_guessed() {
        let d = doc(r#"<html><head><link rel="stylesheet" href="ht!tp://[[bad"></head><body></body></html>"#);
        // With no base there is nothing to resolve against; a bad href must be
        // dropped rather than turned into some invented URL.
        assert!(Engine::discover_external_stylesheets(&d, None).is_empty());
    }

    #[test]
    fn empty_and_missing_href_are_skipped() {
        let d = doc(
            r#"<html><head>
               <link rel="stylesheet" href="">
               <link rel="stylesheet">
               </head><body></body></html>"#,
        );
        assert!(Engine::discover_external_stylesheets(&d, Some(&base())).is_empty());
    }

    #[test]
    fn external_css_cascades_and_inline_style_element_wins_at_equal_specificity() {
        // THE WIRE RECEIPT, through the real layout build: external CSS must
        // reach a descendant, and the <style> block must win at equal
        // specificity because external is placed first.
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let e = Engine {
            config: EngineConfig::default(),
            views: HashMap::new(),
            viewhost: ViewHost::new(),
            compositor: test_compositor(),
            renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()),
            event_tx,
            event_rx: Some(event_rx),
        };
        let d = doc(r#"<html><head><style>p { width: 10px; }</style></head>
                      <body><div><p>x</p></div></body></html>"#);

        // External alone reaches the descendant.
        let ext_only = e.build_layout_with_external_css(&doc(
            r#"<html><body><div><p>x</p></div></body></html>"#), "p { width: 55px; }");
        // Find the ELEMENT box carrying the width, not its text child: width
        // is not an inherited property, so the text box would read Auto no
        // matter what the rule said. (My first version asserted on the text box
        // and failed for that reason - a test bug, not a product one.)
        fn width_of_any_box(b: &LayoutBox) -> Option<Length> {
            if !matches!(b.box_type, BoxType::Text(_)) && b.style.width != Length::Auto {
                return Some(b.style.width.clone());
            }
            b.children.iter().find_map(width_of_any_box)
        }
        assert_eq!(
            width_of_any_box(&ext_only), Some(Length::Px(55.0)),
            "external CSS must reach the element"
        );

        // With both, the <style> element wins at equal specificity.
        let both = e.build_layout_with_external_css(&d, "p { width: 55px; }");
        assert_eq!(
            width_of_any_box(&both), Some(Length::Px(10.0)),
            "inline <style> must win at equal specificity"
        );
    }

    fn engine_for_test() -> Engine {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Engine {
            config: EngineConfig::default(),
            views: HashMap::new(),
            viewhost: ViewHost::new(),
            compositor: test_compositor(),
            renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()),
            event_tx,
            event_rx: Some(event_rx),
        }
    }

    #[tokio::test]
    async fn a_document_without_stylesheets_does_not_inherit_the_previous_one(
    ) {
        // THE SECOND-CALL TEST. Every other test in this module loads exactly
        // ONE document, and a per-view cache cannot be validated by exercising
        // the operation that FILLS it while never exercising the one that must
        // CLEAR it. Found by Argos's R1 HOLD and independently by Athena on
        // Windows (#59) - both from the review question I wrote about a
        // different hazard.
        let mut e = engine_for_test();
        let mut view = ViewState::headless_for_test();
        let id = view.id;
        // Page 1 left external CSS on the view.
        view.external_css = "p { width: 999px; }".to_string();
        e.views.insert(id, view);

        // Page 2 has no <link rel="stylesheet"> at all.
        let d = doc(r#"<html><head></head><body><p>x</p></body></html>"#);
        let loaded = e.load_external_stylesheets(id, &d, &base()).await;

        assert_eq!(loaded, 0);
        assert_eq!(
            e.views[&id].external_css, "",
            "a document with zero stylesheet links must CLEAR the previous \
             document's CSS, not keep it"
        );
    }

    #[test]
    fn attaching_a_document_clears_the_previous_documents_external_css() {
        // The robust door: the reset lives at the point a document is
        // attached, so a load path that never calls the stylesheet loader at
        // all - load_html does not - still gets it. Testing the loader alone
        // would have missed that route entirely.
        let mut view = ViewState::headless_for_test();
        view.external_css = "p { width: 999px; }".to_string();

        view.attach_document(Rc::new(doc(r#"<html><body><p>x</p></body></html>"#)));

        assert_eq!(
            view.external_css, "",
            "attaching a document must reset per-document cached CSS"
        );
        assert!(view.document.is_some(), "and must still attach the document");
    }

    #[test]
    fn stylesheet_discovery_does_not_collect_style_elements_or_vice_versa() {
        // Both passes now run over the same document. If EITHER matched on
        // "element has a URL attribute" rather than on tag name, the page would
        // cross-contaminate. Neither pass's own tests would reveal it - the bug
        // exists only in the interaction. (= Athena's disjointness principle.)
        // THE TRAP ELEMENT. A bare <style> carries no href, so a discovery
        // pass that wrongly matched on "has a URL attribute" could never pick
        // it up and the first assertion below could not fail - it was close to
        // vacuous, which is the exact class of half-assertion this fleet has
        // been stamping out. Giving the <style> a rel and an href makes the
        // assertion FALSIFIABLE: a tag-gated discovery ignores it, an
        // attribute-gated one swallows it. Invalid HTML on purpose; the point
        // is that the gate is on the TAG NAME.
        let d = doc(
            r#"<html><head>
                 <link rel="stylesheet" href="/ext.css">
                 <style rel="stylesheet" href="/trap.css">p { width: 3px; }</style>
               </head><body></body></html>"#,
        );
        let urls = Engine::discover_external_stylesheets(&d, Some(&base()));
        assert_eq!(
            urls.len(), 1,
            "discovery must gate on the <link> TAG, not on carrying a URL attribute; got {urls:?}"
        );
        assert_eq!(urls[0].as_str(), "https://example.com/ext.css");

        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let e = Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        };
        let mut css = String::new();
        e.collect_style_text(&d.root(), &mut css);
        assert!(css.contains("width: 3px"), "collect must find the <style> text");
        assert!(!css.contains("ext.css"), "collect must not pick up the <link> href");
    }
}

#[cfg(test)]
mod flex_property_tests {
    use super::*;
    use rustkit_css::{AlignItems, AlignSelf, FlexBasis, FlexDirection, FlexWrap,
                      JustifyContent, Length};

    fn st(css: &str) -> ComputedStyle {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, css);
        s
    }

    #[test]
    fn direction_wrap_and_alignment_reach_the_style() {
        let s = st("flex-direction: column-reverse; flex-wrap: wrap-reverse; \
                    justify-content: space-evenly; align-items: baseline; \
                    align-self: center; align-content: space-between");
        assert_eq!(s.flex_direction, FlexDirection::ColumnReverse);
        assert_eq!(s.flex_wrap, FlexWrap::WrapReverse);
        assert_eq!(s.justify_content, JustifyContent::SpaceEvenly);
        assert_eq!(s.align_items, AlignItems::Baseline);
        assert_eq!(s.align_self, AlignSelf::Center);
        assert_eq!(s.align_content, rustkit_css::AlignContent::SpaceBetween);
    }

    #[test]
    fn box_alignment_aliases_are_accepted() {
        // `start`/`end` are the box-alignment spellings; pages use both.
        assert_eq!(st("align-items: start").align_items, AlignItems::FlexStart);
        assert_eq!(st("align-items: end").align_items, AlignItems::FlexEnd);
        assert_eq!(st("justify-content: end").justify_content, JustifyContent::FlexEnd);
    }

    #[test]
    fn grow_shrink_basis_and_order() {
        let s = st("flex-grow: 2.5; flex-shrink: 0; flex-basis: 120px; order: -1");
        assert_eq!(s.flex_grow, 2.5);
        assert_eq!(s.flex_shrink, 0.0);
        assert_eq!(s.flex_basis, FlexBasis::Length(120.0));
        assert_eq!(s.order, -1);
        assert_eq!(st("flex-basis: 40%").flex_basis, FlexBasis::Percent(40.0));
        assert_eq!(st("flex-basis: content").flex_basis, FlexBasis::Content);
        assert_eq!(st("flex-basis: auto").flex_basis, FlexBasis::Auto);
    }

    #[test]
    fn flex_one_sets_basis_to_zero_not_auto() {
        // THE SPEC TRAP, and the single most common flex declaration on the
        // web. `flex: 1` means grow 1, shrink 1, basis 0 - the item fills its
        // share of free space. Defaulting basis to Auto instead makes the item
        // size to its content and the layout looks almost-but-not-right, which
        // is far harder to chase than an obvious break.
        let s = st("flex: 1");
        assert_eq!(s.flex_grow, 1.0);
        assert_eq!(s.flex_shrink, 1.0);
        assert_eq!(s.flex_basis, FlexBasis::Length(0.0), "flex: 1 must set basis 0, not auto");
    }

    #[test]
    fn flex_shorthand_keywords_and_arities() {
        let none = st("flex: none");
        assert_eq!((none.flex_grow, none.flex_shrink, none.flex_basis),
                   (0.0, 0.0, FlexBasis::Auto));
        let auto = st("flex: auto");
        assert_eq!((auto.flex_grow, auto.flex_shrink, auto.flex_basis),
                   (1.0, 1.0, FlexBasis::Auto));
        let initial = st("flex: initial");
        assert_eq!((initial.flex_grow, initial.flex_shrink, initial.flex_basis),
                   (0.0, 1.0, FlexBasis::Auto));
        // two-value forms: <grow> <shrink> and <grow> <basis>
        let gs = st("flex: 2 3");
        assert_eq!((gs.flex_grow, gs.flex_shrink), (2.0, 3.0));
        let gb = st("flex: 2 100px");
        assert_eq!((gb.flex_grow, gb.flex_basis), (2.0, FlexBasis::Length(100.0)));
        // three-value form
        let three = st("flex: 3 4 50px");
        assert_eq!((three.flex_grow, three.flex_shrink, three.flex_basis),
                   (3.0, 4.0, FlexBasis::Length(50.0)));
        // a bare length is a basis, with grow/shrink 1
        let b = st("flex: 30px");
        assert_eq!((b.flex_grow, b.flex_basis), (1.0, FlexBasis::Length(30.0)));
    }

    #[test]
    fn gap_shorthand_is_row_then_column() {
        // The order is row-gap THEN column-gap, which is the opposite of the
        // row/column reading order flex-direction trains people to expect.
        let one = st("gap: 12px");
        assert_eq!((one.row_gap.clone(), one.column_gap.clone()),
                   (Length::Px(12.0), Length::Px(12.0)));
        let two = st("gap: 4px 9px");
        assert_eq!(two.row_gap, Length::Px(4.0), "first value is ROW gap");
        assert_eq!(two.column_gap, Length::Px(9.0), "second value is COLUMN gap");
        assert_eq!(st("row-gap: 7px").row_gap, Length::Px(7.0));
        assert_eq!(st("column-gap: 8px").column_gap, Length::Px(8.0));
    }

    #[test]
    fn a_malformed_value_leaves_the_previous_one_alone() {
        // A rule that fails to parse must not silently reset the property to
        // its default - that turns a typo into a layout change somewhere else.
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, "flex-grow: 3");
        apply_inline_style_decls(&mut s, "flex-grow: banana");
        assert_eq!(s.flex_grow, 3.0);
        apply_inline_style_decls(&mut s, "flex-basis: 2em");
        assert_eq!(s.flex_basis, FlexBasis::Auto,
                   "em cannot reach FlexBasis (raw f32, no unit) - must refuse, not treat as 2px");
    }

    #[test]
    fn flex_properties_change_actual_layout_geometry() {
        // THE RECEIPT THAT MATTERS. rustkit-layout has had a complete flex
        // container all along; nothing could steer it. Two items in a 300px
        // row: with flex-grow 1 and 3 they must split the space 75/225, not
        // sit at their content widths.
        use rustkit_layout::{layout_flex_container, LayoutBox as LB};
        let mut container = LB::new(
            BoxType::Block,
            {
                let mut s = ComputedStyle::new();
                apply_inline_style_decls(&mut s, "display: flex; width: 300px; height: 50px");
                s
            },
        );
        for grow in ["flex: 1", "flex: 3"] {
            let mut s = ComputedStyle::new();
            apply_inline_style_decls(&mut s, grow);
            container.children.push(LB::new(BoxType::Block, s));
        }
        let containing = rustkit_layout::Dimensions {
            content: rustkit_layout::Rect::new(0.0, 0.0, 300.0, 50.0),
            ..Default::default()
        };
        layout_flex_container(&mut container, &containing);
        let w: Vec<f32> = container.children.iter().map(|c| c.dimensions.content.width).collect();
        assert_eq!(w.len(), 2);
        assert!((w[0] - 75.0).abs() < 1.0 && (w[1] - 225.0).abs() < 1.0,
                "flex-grow 1 and 3 must split 300px as 75/225; got {w:?}");
    }
}

#[cfg(test)]
mod child_combinator_tests {
    use super::*;

    fn anc(chain: &[(&str, &str)]) -> Vec<ElementCtx> {
        // Root-first, so the LAST entry is the immediate parent.
        chain.iter().map(|(tag, class)| ElementCtx {
            tag: tag.to_string(),
            classes: if class.is_empty() { vec![] } else {
                class.split_whitespace().map(|s| s.to_string()).collect()
            },
            id: None,
        }).collect()
    }
    fn m(sel: &str, tag: &str, classes: &[&str], chain: &[(&str, &str)]) -> Option<u32> {
        Engine::selector_matches(sel, tag, classes, None, &anc(chain))
    }

    #[test]
    fn child_combinator_matches_an_immediate_child() {
        assert!(m("ul > li", "li", &[], &[("body", ""), ("ul", "")]).is_some());
    }

    #[test]
    fn child_combinator_rejects_a_deeper_descendant() {
        // THE BUG. `>` was stripped from the token list, so this relation was
        // silently relaxed to descendant and `.nav > li` also styled every li
        // nested any depth below - the exact shape used to style one menu
        // level without touching its submenus.
        // NOTE ON THE TREE: a submenu `li` still has a `ul` for a parent, so
        // `ul > li` correctly matches it. To exercise the combinator the
        // subject's parent must genuinely not be a `ul` - here a wrapper div.
        // (My first draft used the submenu tree and failed; the matcher was
        // right and the test was wrong.)
        assert!(
            m("ul > li", "li", &[], &[("ul", ""), ("div", "")]).is_none(),
            "a li wrapped in a div is not a child of the ul"
        );
        assert!(
            m(".nav > li", "li", &[], &[("ul", "nav"), ("li", ""), ("ul", "")]).is_none(),
            "submenu items must not inherit the top-level rule"
        );
    }

    #[test]
    fn descendant_combinator_still_matches_at_any_depth() {
        // The fix must not overshoot: plain descendant is unchanged.
        assert!(m("ul li", "li", &[], &[("ul", ""), ("li", ""), ("ul", "")]).is_some());
        assert!(m(".card p", "p", &[], &[("div", "card"), ("div", ""), ("section", "")]).is_some());
    }

    #[test]
    fn child_combinator_parses_without_surrounding_whitespace() {
        // THE SECOND BUG, opposite direction. Whitespace-only splitting left
        // `ul>li` as one compound whose type part was the literal "ul>li",
        // which matched no tag, so the rule was silently DEAD rather than
        // over-applied. Authors write all four spellings.
        for sel in ["ul>li", "ul> li", "ul >li", "ul > li"] {
            assert!(
                m(sel, "li", &[], &[("body", ""), ("ul", "")]).is_some(),
                "{sel:?} must match an immediate child"
            );
            assert!(
                m(sel, "li", &[], &[("ul", ""), ("div", "")]).is_none(),
                "{sel:?} must reject a non-child"
            );
        }
    }

    #[test]
    fn child_at_the_root_has_no_parent_to_match() {
        assert!(m("body > div", "div", &[], &[]).is_none());
    }

    #[test]
    fn mixed_child_and_descendant_chain() {
        // `.page .card > p`: p's immediate parent is .card, and .card has some
        // .page ancestor.
        assert!(m(".page .card > p", "p", &[],
                  &[("div", "page"), ("section", ""), ("div", "card")]).is_some());
        // Same tree, but p is a grandchild of .card - the `>` must reject it.
        assert!(m(".page .card > p", "p", &[],
                  &[("div", "page"), ("div", "card"), ("span", "")]).is_none());
    }

    #[test]
    fn specificity_still_sums_across_the_chain() {
        // `>` must not change how specific a selector is; only which elements
        // it reaches. Both forms are one class + one type.
        assert_eq!(m(".card > p", "p", &[], &[("div", "card")]),
                   m(".card p", "p", &[], &[("div", "card")]));
        assert_eq!(m(".card > p", "p", &[], &[("div", "card")]), Some(11));
    }

    #[test]
    fn malformed_combinators_match_nothing_rather_than_guessing() {
        // Refusing is the safe read: applying a selector we cannot parse would
        // style the wrong elements, which is worse than styling none.
        for sel in ["> p", "div >", "div > > p", ">"] {
            assert!(
                m(sel, "p", &[], &[("div", ""), ("div", "")]).is_none(),
                "{sel:?} must not match"
            );
        }
    }

    #[test]
    fn child_combinator_takes_effect_through_the_real_layout_build() {
        // The unit tests above prove the MATCHER. This proves the matcher is
        // what the cascade actually consults - a correct matcher nothing calls
        // would pass every test above and change nothing on screen.
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let e = Engine {
            config: EngineConfig::default(),
            views: HashMap::new(),
            viewhost: ViewHost::new(),
            compositor: test_compositor(),
            renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()),
            event_tx,
            event_rx: Some(event_rx),
        };
        // Two elements carry .item. Only the FIRST is an immediate child of
        // .menu; the second sits under a nested ul. Before the fix both were
        // 20px. (Targeting `.menu > li` instead would have been a bad fixture:
        // BOTH top-level li are direct children, so the count could not
        // distinguish the bug from correct behaviour.)
        let doc = Document::parse_html(
            r#"<html><head><style>.menu > .item { width: 20px; }</style></head>
               <body><ul class="menu">
                 <li class="item"></li>
                 <li><ul><li class="item"></li></ul></li>
               </ul></body></html>"#,
        ).expect("parse");
        let layout = e.build_layout_from_document(&doc);

        fn widths(b: &LayoutBox, out: &mut Vec<rustkit_css::Length>) {
            if !matches!(b.box_type, BoxType::Text(_)) {
                out.push(b.style.width.clone());
            }
            for c in &b.children { widths(c, out); }
        }
        let mut w = Vec::new();
        widths(&layout, &mut w);
        let styled = w.iter().filter(|l| **l == rustkit_css::Length::Px(20.0)).count();
        assert_eq!(
            styled, 1,
            "only the immediate child may be styled; got {styled} boxes at 20px in {w:?}"
        );
    }

    #[test]
    fn known_limit_greedy_matching_does_not_backtrack() {
        // DOCUMENTED, NOT ASSERTED-CORRECT. The nearest matching ancestor is
        // taken and never reconsidered, so a chain that needs backtracking
        // gives a FALSE NEGATIVE: here .b matches the inner div, and .a is not
        // its parent, so the walk fails - though matching the OUTER .b would
        // have succeeded.
        //
        // This is inherited from the reference implementation, which uses the
        // same forward cursor. Kept identical on purpose: an independently
        // more-correct matcher would be a DIVERGENCE and would make parity
        // comparisons meaningless. Filed as a cross-tree follow-up instead.
        assert!(
            m(".a > .b .c", "span", &["c"],
              &[("div", "a"), ("div", "b"), ("div", "b")]).is_none(),
            "if this now MATCHES, backtracking landed - retire this test"
        );
    }
}

#[cfg(test)]
mod position_wire_tests {
    //! Split into TWO groups on purpose (= Athena's Windows #62 shape).
    //!
    //! GROUP A asserts COMPUTED VALUES - that the applier arms parse. GROUP B
    //! asserts the value REACHED THE LAYOUT BOX - that it does anything.
    //!
    //! The split is the point. This chain was broken in three places, and
    //! fixing only the arms would leave group A fully green while every page
    //! still rendered position:static. A suite of computed-value assertions
    //! alone cannot distinguish "the property parses" from "the property does
    //! something", and that distinction is the entire defect class.
    use super::*;
    use rustkit_css::{Length, Position};

    fn st(css: &str) -> ComputedStyle {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, css);
        s
    }
    fn boxed(css: &str) -> LayoutBox {
        let mut b = LayoutBox::new(BoxType::Block, st(css));
        Engine::apply_position_to_layout_box(&mut b);
        b
    }

    // ---------------- GROUP A: computed values (the arms) ----------------

    #[test]
    fn a_every_position_keyword_parses() {
        assert_eq!(st("position: relative").position, Position::Relative);
        assert_eq!(st("position: absolute").position, Position::Absolute);
        assert_eq!(st("position: fixed").position, Position::Fixed);
        assert_eq!(st("position: sticky").position, Position::Sticky);
        assert_eq!(st("position: static").position, Position::Static);
        assert_eq!(st("position: bogus").position, Position::Static, "unknown falls back to static");
    }

    #[test]
    fn a_offsets_parse_and_unset_stays_auto() {
        let s = st("position: absolute; top: 10px; left: 0");
        assert_eq!(s.top, Some(Length::Px(10.0)));
        // `left: 0` is Some(0) - PINNED to the containing block edge.
        // An unset `right` is None - AUTO, keep the static-flow position.
        // These are different things; a plain Length could not tell them apart
        // because Length::default() is Zero.
        assert!(matches!(s.left, Some(Length::Zero) | Some(Length::Px(0.0))),
                "left:0 must be Some(0) = pinned, got {:?}", s.left);
        assert_eq!(s.right, None, "unset offset must stay auto (None), not become 0");
        assert_eq!(s.bottom, None);
    }

    #[test]
    fn a_percentage_offsets_are_kept_as_percentages_not_flattened() {
        // The COMPUTED value keeps the percentage - that is the honest record
        // of what the author wrote, and matches the reference. The refusal
        // happens later, at the layout wire, where pixels are demanded and the
        // containing block is still unknown (see the group-B twin below).
        //
        // My first draft asserted None here and failed. The product was right:
        // discarding the percentage at parse time would lose information the
        // engine may later be able to resolve.
        assert_eq!(st("position: absolute; top: 50%").top, Some(Length::Percent(50.0)));
    }

    #[test]
    fn a_z_index_garbage_is_ignored_not_flattened() {
        assert_eq!(st("z-index: 7").z_index, 7);
        assert_eq!(st("z-index: -3").z_index, -3);
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, "z-index: 5");
        apply_inline_style_decls(&mut s, "z-index: banana");
        assert_eq!(s.z_index, 5, "garbage must be ignored; flattening to 0 silently restacks the page");
    }

    #[test]
    fn a_offsets_do_not_inherit() {
        let mut parent = ComputedStyle::new();
        parent.top = Some(Length::Px(40.0));
        parent.z_index = 9;
        let child = ComputedStyle::inherit_from(&parent);
        assert_eq!(child.top, None, "a child must not adopt its parent's displacement");
        assert_eq!(child.z_index, 0);
    }

    // ------------- GROUP B: reached the layout box (the wire) -------------

    #[test]
    fn b_position_reaches_the_layout_box() {
        use rustkit_layout::Position as LP;
        assert_eq!(boxed("position: absolute").position, LP::Absolute);
        assert_eq!(boxed("position: fixed").position, LP::Fixed);
        assert_eq!(boxed("").position, LP::Static);
    }

    #[test]
    fn b_offsets_reach_the_layout_box_in_pixels() {
        let b = boxed("position: absolute; top: 10px; left: 20px");
        assert_eq!(b.offsets.top, Some(10.0), "top must reach the box");
        assert_eq!(b.offsets.left, Some(20.0), "left must reach the box");
        assert_eq!(b.offsets.right, None, "an unset offset must stay auto at the box too");
    }

    #[test]
    fn b_relative_units_are_resolved_against_the_element_font_size() {
        // rem is always 16px; em follows the element's own font-size. If these
        // arrived unresolved the box would be offset by 2 pixels instead of 64.
        let b = boxed("position: absolute; font-size: 32px; top: 2em; left: 2rem");
        assert_eq!(b.offsets.top, Some(64.0), "2em at font-size 32px is 64px");
        assert_eq!(b.offsets.left, Some(32.0), "2rem is 32px");
    }

    #[test]
    fn b_percentage_offsets_are_refused_at_the_wire_not_invented() {
        // THE TWIN of the group-A test above, and the one that matters. A
        // percentage resolves against the containing block, which is not known
        // while the tree is built. The box must get None (auto) rather than an
        // invented pixel value - treating `top: 50%` as 50px would place the
        // element somewhere no CSS author asked for, silently.
        let b = boxed("position: absolute; top: 50%; left: 10px");
        assert_eq!(b.offsets.top, None, "a % offset must not become an invented px");
        assert_eq!(b.offsets.left, Some(10.0), "and must not poison its siblings");
    }

    #[test]
    fn b_z_index_reaches_the_layout_box() {
        assert_eq!(boxed("z-index: 4").z_index, 4);
    }

    #[test]
    fn b_a_static_box_gets_no_offsets_even_if_they_are_declared() {
        // Offsets on a static box must not displace it - that is the CSS rule,
        // and it is also what stops a stray `top:` in a stylesheet from
        // shifting unpositioned content.
        let b = boxed("top: 99px; left: 99px");
        assert_eq!(b.offsets.top, None);
        assert_eq!(b.offsets.left, None);
    }

    #[test]
    fn b_relative_and_sticky_map_to_static_deliberately() {
        // Mirrors the macOS reference. Entering the positioned paint path for
        // these wrecks pages whose relative boxes are only z-index anchors,
        // until the stacking pipeline matures. Pinned so a future change is a
        // DECISION rather than a drift; deviating here would be a divergence.
        use rustkit_layout::Position as LP;
        assert_eq!(boxed("position: relative; top: 5px").position, LP::Static);
        assert_eq!(boxed("position: sticky; top: 5px").position, LP::Static);
    }

    #[test]
    fn b_position_reaches_a_box_through_the_real_document_build() {
        // Group B above calls the helper directly. This drives the whole path
        // - author stylesheet, cascade, layout build - so the receipt is not
        // resting on my own helper being called.
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let e = Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        };
        let doc = Document::parse_html(
            r#"<html><head><style>.pin { position: absolute; top: 12px; left: 34px; }</style></head>
               <body><div class="pin"></div></body></html>"#,
        ).expect("parse");
        let layout = e.build_layout_from_document(&doc);

        fn find_positioned(b: &LayoutBox) -> Option<&LayoutBox> {
            if b.position != rustkit_layout::Position::Static { return Some(b); }
            b.children.iter().find_map(find_positioned)
        }
        let p = find_positioned(&layout).expect("an absolutely positioned box must exist");
        assert_eq!(p.position, rustkit_layout::Position::Absolute);
        assert_eq!(p.offsets.top, Some(12.0));
        assert_eq!(p.offsets.left, Some(34.0));
    }
}

#[cfg(test)]
mod text_decoration_tests {
    //! Two-group split (fleet pin). GROUP A = the arms parse. GROUP B = the
    //! value changes what would be PAINTED.
    //!
    //! Group B was written FIRST, on Prometheus's instruction, and run before
    //! any arm existed. It then stayed RED after the arms were added, which is
    //! the whole point: text-decoration has a second break behind the arms.
    //!
    //! SCOPE NOTE: this unit began as overflow + white-space + text-decoration,
    //! mirroring Athena's Windows #64. THE OTHER TWO WERE REMOVED BEFORE
    //! SHIPPING. On this tree `collapse_whitespace` and `is_scroll_container`
    //! exist and are tested, but nothing in production calls them with the
    //! style field: layout never consults `style.white_space` when breaking
    //! lines, and nothing feeds `style.overflow_x` to the scroll code. Adding
    //! those arms would have made both writable, dropped three names from the
    //! reachability list, and changed not one pixel - gaming my own metric.
    //! They wait for their callers.
    use super::*;

    fn engine() -> Engine {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        }
    }
    fn display_list_for(html: &str) -> String {
        let e = engine();
        let doc = Document::parse_html(html).expect("parse");
        format!("{:?}", DisplayList::build(&e.build_layout_from_document(&doc)).commands)
    }
    fn st(css: &str) -> ComputedStyle {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, css);
        s
    }

    // ---------------- GROUP A: the arms parse ----------------

    #[test]
    fn a_shorthand_token_order_does_not_matter() {
        assert!(st("text-decoration: underline red").text_decoration_line.underline);
        assert!(st("text-decoration: red underline").text_decoration_line.underline);
        let multi = st("text-decoration: underline line-through").text_decoration_line;
        assert!(multi.underline && multi.line_through, "both lines must combine");
    }

    #[test]
    fn a_none_clears_but_a_colour_only_value_does_not() {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, "text-decoration: underline");
        apply_inline_style_decls(&mut s, "text-decoration: goldenrod");
        assert!(s.text_decoration_line.underline,
                "a colour-only value must NOT clear an existing line");
        apply_inline_style_decls(&mut s, "text-decoration: none");
        assert!(!s.text_decoration_line.underline, "`none` must clear it");
    }

    #[test]
    fn a_longhands_parse() {
        assert_eq!(st("text-decoration-style: wavy").text_decoration_style,
                   rustkit_css::TextDecorationStyle::Wavy);
        assert!(st("text-decoration-color: #ff0000").text_decoration_color.is_some());
    }

    // -------- GROUP B: the value changes what would be painted --------

    #[test]
    fn b_underline_reaches_the_display_list() {
        let with = display_list_for(
            r#"<html><head><style>p { text-decoration: underline; }</style></head>
               <body><p>hello</p></body></html>"#,
        );
        let without = display_list_for(r#"<html><body><p>hello</p></body></html>"#);
        assert_ne!(with, without,
            "underline must change what is painted; if these are equal the \
             value never reached the text box");
        assert!(with.contains("TextDecoration"),
                "expected a decoration command, got: {with}");
    }

    #[test]
    fn b_an_undecorated_page_emits_no_decoration_commands() {
        // The propagation copy is GATED on the parent having a line. Without
        // the gate every text run would carry decoration it never asked for.
        // Asserting the negative so the gate is a decision, not a leftover.
        let plain = display_list_for(r#"<html><body><p>hello</p></body></html>"#);
        assert!(!plain.contains("TextDecoration"),
                "an undecorated page must emit no decoration commands, got: {plain}");
    }

    #[test]
    fn b_line_through_and_colour_reach_the_display_list_distinctly() {
        let plain_ul = display_list_for(
            r#"<html><head><style>p{text-decoration:underline}</style></head>
               <body><p>hi</p></body></html>"#);
        let coloured = display_list_for(
            r#"<html><head><style>p{text-decoration:underline;text-decoration-color:#ff0000}</style></head>
               <body><p>hi</p></body></html>"#);
        assert_ne!(plain_ul, coloured,
                   "text-decoration-color must reach paint, not just the style struct");
    }
}

#[cfg(test)]
mod grid_arm_tests {
    //! Two-group split (fleet pin). GROUP A = the arms parse. GROUP B =
    //! GEOMETRY through layout_grid_container, per Prometheus's endorsement of
    //! the #30 flex recipe: a computed-value assertion cannot tell a wired
    //! grid from a dead one.
    //!
    //! Group B written FIRST and run before any arm or parser existed.
    use super::*;
    use rustkit_layout::{layout_grid_container, LayoutBox as LB};

    fn st(css: &str) -> ComputedStyle {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, css);
        s
    }

    /// Build a grid container of `width` with `n` children and lay it out.
    fn grid_widths(container_css: &str, n: usize, child_css: &[&str], w: f32) -> Vec<f32> {
        let mut c = LB::new(BoxType::Block, st(container_css));
        for i in 0..n {
            c.children.push(LB::new(
                BoxType::Block,
                st(child_css.get(i).copied().unwrap_or("")),
            ));
        }
        layout_grid_container(&mut c, w, 200.0);
        c.children.iter().map(|k| k.dimensions.content.width).collect()
    }

    // ---------------- GROUP A: the arms parse ----------------

    #[test]
    fn a_display_grid_now_parses_at_all() {
        // THE ROOT FIX. Display::Grid existed, is_grid() existed, and
        // layout_grid_container was dispatched from it - but parse_display had
        // no "grid" arm, so the entire grid engine was unreachable behind one
        // missing match arm. inline-flex and inline-grid were missing too.
        assert!(st("display: grid").display.is_grid(), "display:grid must parse");
        assert!(st("display: inline-grid").display.is_grid());
        assert!(st("display: inline-flex").display.is_flex());
    }

    #[test]
    fn a_track_templates_parse() {
        assert_eq!(st("grid-template-columns: 50px 100px 150px").grid_template_columns.tracks.len(), 3);
        assert_eq!(st("grid-template-rows: 1fr 2fr").grid_template_rows.tracks.len(), 2);
    }

    #[test]
    fn a_auto_flow_keywords_including_dense_spellings() {
        use rustkit_css::GridAutoFlow;
        assert_eq!(st("grid-auto-flow: column").grid_auto_flow, GridAutoFlow::Column);
        assert_eq!(st("grid-auto-flow: row dense").grid_auto_flow, GridAutoFlow::RowDense);
        assert_eq!(st("grid-auto-flow: dense row").grid_auto_flow, GridAutoFlow::RowDense);
        assert_eq!(st("grid-auto-flow: dense").grid_auto_flow, GridAutoFlow::RowDense);
        assert_eq!(st("grid-auto-flow: row").grid_auto_flow, GridAutoFlow::Row);
    }

    #[test]
    fn a_line_placement_longhands_and_shorthand_agree() {
        use rustkit_css::GridLine;
        assert_eq!(st("grid-column-start: 2").grid_column_start, GridLine::Number(2));
        assert_eq!(st("grid-row-end: span 3").grid_row_end, GridLine::Span(3));
        // `grid-column: 1 / 3` is the spelling authors actually write; omitting
        // the shorthand would leave it silently dead, the same under-match the
        // child combinator had for `.nav>li`.
        let sh = st("grid-column: 1 / 3");
        assert_eq!(sh.grid_column_start, GridLine::Number(1));
        assert_eq!(sh.grid_column_end, GridLine::Number(3));
    }

    #[test]
    fn a_shared_limits_are_not_wired() {
        // justify-items / justify-self / grid-template-areas are WIREABLE on
        // this tree (layout reads them, the reader is live) and NEITHER peer
        // implements them. Wiring them here alone would be a divergence.
        // Asserted so a future contributor sees the omission is a decision.
        let s = st("justify-items: center; justify-self: end");
        assert_eq!(s.justify_items, rustkit_css::JustifyItems::default(),
                   "justify-items is a SHARED LIMIT - not wired on purpose");
        assert_eq!(s.justify_self, rustkit_css::JustifySelf::default(),
                   "justify-self is a SHARED LIMIT - not wired on purpose");
    }

    // ------- GROUP B: geometry through layout_grid_container -------

    #[test]
    fn b_explicit_track_template_sizes_the_columns() {
        // THE GEOMETRY RECEIPT. Three fixed columns in a 300px grid must land
        // at their declared widths, not at an even split or at zero.
        let w = grid_widths("display: grid; grid-template-columns: 50px 100px 150px", 3, &[], 300.0);
        assert_eq!(w.len(), 3);
        assert!(
            (w[0] - 50.0).abs() < 1.0 && (w[1] - 100.0).abs() < 1.0 && (w[2] - 150.0).abs() < 1.0,
            "grid-template-columns must size the tracks; got {w:?}"
        );
    }

    #[test]
    fn b_fr_units_split_the_free_space_proportionally() {
        // 1fr 3fr in 400px must be 100/300. If the template never reached
        // layout both boxes would be equal or zero.
        let w = grid_widths("display: grid; grid-template-columns: 1fr 3fr", 2, &[], 400.0);
        assert_eq!(w.len(), 2);
        assert!(
            (w[0] - 100.0).abs() < 2.0 && (w[1] - 300.0).abs() < 2.0,
            "1fr 3fr in 400px must split 100/300; got {w:?}"
        );
    }
}

#[cfg(test)]
mod box_shadow_paint_tests {
    //! Two-group split. GROUP A = `box-shadow` parses into ComputedStyle.
    //! GROUP B = it REACHES THE DISPLAY LIST, i.e. something would be painted.
    //!
    //! Group B written FIRST. Group A already passed before this unit began -
    //! the applier arm and the BoxShadow type have existed since A2/#11 - and
    //! that is exactly the trap: `box-shadow` has been "supported" on this
    //! tree in the sense that it parses, while no shadow has ever been drawn.
    //! A producer with no consumer, which I built myself.
    use super::*;

    fn engine() -> Engine {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        }
    }
    fn display_list_for(html: &str) -> String {
        let e = engine();
        let doc = Document::parse_html(html).expect("parse");
        format!("{:?}", DisplayList::build(&e.build_layout_from_document(&doc)).commands)
    }

    // ---------------- GROUP A: it parses ----------------

    #[test]
    fn a_box_shadow_parses_into_computed_style() {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, "box-shadow: 2px 4px 6px rgba(0,0,0,0.5)");
        assert_eq!(s.box_shadows.len(), 1);
        let sh = &s.box_shadows[0];
        assert_eq!((sh.offset_x, sh.offset_y, sh.blur_radius), (2.0, 4.0, 6.0));
    }

    #[test]
    fn a_none_clears_the_shadow_list() {
        let mut s = ComputedStyle::new();
        apply_inline_style_decls(&mut s, "box-shadow: 2px 2px 2px black");
        apply_inline_style_decls(&mut s, "box-shadow: none");
        assert!(s.box_shadows.is_empty(), "`none` must clear, so a later rule can cancel");
    }

    // -------- GROUP B: it reaches the display list (would be painted) --------

    #[test]
    fn b_box_shadow_reaches_the_display_list() {
        // THE RECEIPT. Group A has passed since A2 while nothing was ever
        // drawn - a shadow that parses and never paints is indistinguishable,
        // on screen, from no support at all.
        let with = display_list_for(
            r#"<html><head><style>div { box-shadow: 4px 4px 8px rgba(0,0,0,0.6); width: 50px; height: 50px; }</style></head>
               <body><div></div></body></html>"#,
        );
        let without = display_list_for(
            r#"<html><head><style>div { width: 50px; height: 50px; }</style></head>
               <body><div></div></body></html>"#,
        );
        assert_ne!(with, without, "box-shadow must change what would be painted");
        assert!(with.contains("BoxShadow"), "expected a BoxShadow command, got: {with}");
    }

    #[test]
    fn b_an_unshadowed_page_emits_no_shadow_commands() {
        let plain = display_list_for(r#"<html><body><div>x</div></body></html>"#);
        assert!(!plain.contains("BoxShadow"),
                "a page with no box-shadow must emit no shadow commands");
    }

    #[test]
    fn b_a_fully_transparent_shadow_is_not_emitted() {
        // is_visible() gates on alpha. Emitting a fully transparent shadow
        // would cost a draw call per box for something nobody can see.
        let t = display_list_for(
            r#"<html><head><style>div { box-shadow: 4px 4px 8px rgba(0,0,0,0); width: 50px; height: 50px; }</style></head>
               <body><div></div></body></html>"#,
        );
        assert!(!t.contains("BoxShadow"), "a transparent shadow must not be emitted");
    }
}

#[cfg(test)]
mod flex_column_cross_stretch {
    //! Guard against the macOS defect Atlas reported 2026-07-31: in a
    //! flex-direction:column container, children were NOT stretched to fill
    //! the cross axis, coming out shrink-to-fit and perfectly SQUARE - the
    //! cross size tracking the MAIN-axis value.
    //!
    //! Linux does NOT have it (measured: children are container-width). This
    //! test exists so it cannot arrive later unnoticed, because the failure is
    //! invisible to every other check we run: the page still renders, nothing
    //! errors, and boxes are merely the wrong size.
    use super::*;

    fn laid_out(html: &str, w: f32) -> Vec<(bool, f32, f32)> {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let e = Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        };
        let doc = Document::parse_html(html).expect("parse");
        let mut layout = e.build_layout_from_document(&doc);
        layout.layout(&rustkit_layout::Dimensions {
            content: rustkit_layout::Rect::new(0.0, 0.0, w, 800.0),
            ..Default::default()
        });
        fn walk(b: &LayoutBox, out: &mut Vec<(bool, f32, f32)>) {
            out.push((
                matches!(b.box_type, BoxType::Text(_)),
                b.dimensions.content.width,
                b.dimensions.content.height,
            ));
            for c in &b.children { walk(c, out); }
        }
        let mut v = Vec::new();
        walk(&layout, &mut v);
        v
    }

    #[test]
    fn column_flex_children_fill_the_cross_axis() {
        // align-items defaults to stretch, so every element box in a 1000px
        // column container must be 1000px wide.
        let boxes = laid_out(
            r#"<html><head><style>body{margin:0;display:flex;flex-direction:column;width:1000px}</style></head>
               <body><div>one</div><div>two words here</div></body></html>"#,
            1000.0,
        );
        let elements: Vec<_> = boxes.iter().filter(|(is_text, _, _)| !is_text).collect();
        assert!(elements.len() >= 4, "expected html/body plus two children, got {elements:?}");
        for (_, w, h) in &elements {
            assert!(
                (*w - 1000.0).abs() < 1.0,
                "a column flex child must stretch to the container width; got w={w} h={h}.                  If w == h the cross size is tracking the MAIN axis - that is the macOS defect."
            );
        }
    }

    // RETIRED: column_flex_children_are_not_square.
    //
    // It shipped in #43 with the T-RED explicitly not firing it, which I named
    // at the time and then let stand. Under FALSIFY_BEFORE_SHIP_GUARD (Athena,
    // who threw away her own unfalsifiable guard rather than ship it with a
    // caveat) that is not good enough, so I went looking for a mutation that
    // would make it scream.
    //
    // There is not one on this tree. I modelled the ACTUAL macOS defect -
    // `item.cross_size = item.target_main_size`, cross tracking main - and the
    // guard still passed, because Linux flex items currently have a main size
    // of 0. Every square this tree can produce is 0x0, and the guard
    // deliberately excludes zero-size boxes so it does not fire on the
    // legitimate current state.
    //
    // So it could not have caught the defect it was written for. Deleted
    // rather than kept with a comment: a guard that cannot fail is the thing
    // this fleet has spent two days removing, and keeping mine because I wrote
    // it would be the worst possible reason.
    //
    // The WIDTH guard below is falsifiable and does catch the defect - macOS
    // showed 16px children in a 1000px container - so coverage is not lost.
    // Worth noting WHY the square guard is unfalsifiable here: it is the same
    // main-size-0 behaviour I reported to Atlas as an open observation. If that
    // resolves, a square guard becomes meaningful and can come back WITH a
    // mutation that proves it.
}

#[cfg(test)]
mod font_size_cascade_absolutise {
    //! THREE groups, not two. This defect class demands the third: a
    //! computed-value test passes the moment the field holds *a* Px, and a
    //! reaching test passes the moment the text box carries *a* number. Only
    //! group 3 catches a Px with the WRONG VALUE - which is what a
    //! resolve-against-the-root-instead-of-the-parent bug produces.
    use super::*;
    use rustkit_css::Length;

    fn engine() -> Engine {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        }
    }

    /// Atlas's fixture: html 16px, body 20px. Returns the <p>'s font-size.
    fn p_font_size(body_extra: &str, p_decl: &str) -> Length {
        let e = engine();
        let html = format!(
            r#"<html><head><style>html{{font-size:16px}}body{{font-size:20px;{body_extra}}}p{{{p_decl}}}</style></head><body><p>x</p></body></html>"#
        );
        let doc = Document::parse_html(&html).expect("parse");
        let lay = e.build_layout_from_document(&doc);
        fn find(b: &LayoutBox) -> Option<Length> {
            if !matches!(b.box_type, BoxType::Text(_)) {
                if let Some(t) = b.children.iter().find(|c| matches!(c.box_type, BoxType::Text(_))) {
                    let _ = t;
                    return Some(b.style.font_size.clone());
                }
            }
            b.children.iter().find_map(find)
        }
        find(&lay).expect("no <p> box")
    }

    // ---- GROUP 1: computed value is ABSOLUTE ----

    #[test]
    fn g1_relative_units_are_stored_as_px() {
        for decl in ["font-size:2rem", "font-size:1.5em", "font-size:200%"] {
            assert!(
                matches!(p_font_size("", decl), Length::Px(_)),
                "{decl} must be absolutised in the cascade, got {:?}", p_font_size("", decl)
            );
        }
    }

    // ---- GROUP 2: it REACHES the text run ----

    #[test]
    fn g2_the_text_box_carries_the_absolute_size() {
        let e = engine();
        let doc = Document::parse_html(
            r#"<html><head><style>html{font-size:16px}body{font-size:20px}p{font-size:2rem}</style></head><body><p>x</p></body></html>"#
        ).expect("parse");
        let lay = e.build_layout_from_document(&doc);
        fn text_fs(b: &LayoutBox) -> Option<Length> {
            if matches!(b.box_type, BoxType::Text(_)) { return Some(b.style.font_size.clone()); }
            b.children.iter().find_map(text_fs)
        }
        assert_eq!(text_fs(&lay), Some(Length::Px(32.0)),
                   "the text run must inherit the ABSOLUTE size, not the relative one");
    }

    // ---- GROUP 3: the VALUE IS CORRECT ----

    #[test]
    fn g3_values_are_right_in_every_context() {
        // The four numbers Atlas specified, across the five paths Prometheus
        // named. `em` resolves against the PARENT (20px), `rem` against the
        // ROOT (16px) - a resolver that used the root for both would pass
        // groups 1 and 2 and fail here on the em case.
        for ctx in ["", "display:flex;flex-direction:row;",
                    "display:flex;flex-direction:column;", "display:grid;"] {
            assert_eq!(p_font_size(ctx, "font-size:2rem"), Length::Px(32.0), "2rem in [{ctx}]");
            assert_eq!(p_font_size(ctx, "font-size:1.5em"), Length::Px(30.0),
                       "1.5em must resolve against the PARENT's 20px = 30, not the root's 16 = 24, in [{ctx}]");
            assert_eq!(p_font_size(ctx, "font-size:200%"), Length::Px(40.0), "200% in [{ctx}]");
            assert_eq!(p_font_size(ctx, "font-size:24px"), Length::Px(24.0), "control in [{ctx}]");
        }
    }

    #[test]
    fn g3_inline_style_attribute_path() {
        // The fifth path. Inline style is applied AFTER author rules, so the
        // absolutise block has to sit after it - if it ran earlier this would
        // still be Rem(2.0).
        let e = engine();
        let doc = Document::parse_html(
            r#"<html><head><style>html{font-size:16px}body{font-size:20px}</style></head><body><p style="font-size:2rem">x</p></body></html>"#
        ).expect("parse");
        let lay = e.build_layout_from_document(&doc);
        fn text_fs(b: &LayoutBox) -> Option<Length> {
            if matches!(b.box_type, BoxType::Text(_)) { return Some(b.style.font_size.clone()); }
            b.children.iter().find_map(text_fs)
        }
        assert_eq!(text_fs(&lay), Some(Length::Px(32.0)), "inline style must absolutise too");
    }

    #[test]
    fn g3_em_chains_compound() {
        // Athena flagged this as unverified on her side: 2em inside 2em must
        // compound, which only works if the parent's font_size was already
        // absolutised when the child reads it. Root 16 -> outer 32 -> inner 64.
        let e = engine();
        let doc = Document::parse_html(
            r#"<html><head><style>html{font-size:16px}body{font-size:16px}.o{font-size:2em}.i{font-size:2em}</style></head>
               <body><div class="o"><div class="i">x</div></div></body></html>"#
        ).expect("parse");
        let lay = e.build_layout_from_document(&doc);
        fn text_fs(b: &LayoutBox) -> Option<Length> {
            if matches!(b.box_type, BoxType::Text(_)) { return Some(b.style.font_size.clone()); }
            b.children.iter().find_map(text_fs)
        }
        assert_eq!(text_fs(&lay), Some(Length::Px(64.0)),
                   "2em inside 2em must compound to 64, not flatten to 32");
    }
}

#[cfg(test)]
mod html_root_inheritance {
    //! Inherited properties set on `<html>` must reach `<body>` and below.
    //!
    //! Layout starts at <body>, so building it from a bare root style dropped
    //! everything an author set on the root element. Found from Argos's soft
    //! note on #46 - he flagged it as pre-existing and non-blocking, and it
    //! turned out to drop EVERY inherited property.
    use super::*;
    use rustkit_css::Length;

    fn text_style(html: &str) -> ComputedStyle {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let e = Engine {
            config: EngineConfig::default(), views: HashMap::new(), viewhost: ViewHost::new(),
            compositor: test_compositor(), renderer: None,
            loader: Arc::new(ResourceLoader::new(LoaderConfig::default()).expect("loader")),
            image_manager: Arc::new(ImageManager::new()), event_tx, event_rx: Some(event_rx),
        };
        let doc = Document::parse_html(html).expect("parse");
        let lay = e.build_layout_from_document(&doc);
        fn t(b: &LayoutBox) -> Option<ComputedStyle> {
            if matches!(b.box_type, BoxType::Text(_)) { return Some(b.style.clone()); }
            b.children.iter().find_map(t)
        }
        t(&lay).expect("no text box")
    }

    #[test]
    fn every_inherited_property_on_html_reaches_the_text() {
        // All five were dropped before this unit. Asserting them together
        // because the defect was not per-property - the whole inherited set
        // was discarded at one seam.
        let s = text_style(
            r#"<html><head><style>html{font-size:20px;color:#ff0000;line-height:1.5;font-family:Georgia;text-align:center}</style></head><body><p>x</p></body></html>"#
        );
        assert_eq!(s.font_size, Length::Px(20.0), "font-size on html must reach text");
        assert_eq!((s.color.r, s.color.g, s.color.b), (255, 0, 0), "color on html must reach text");
        assert_eq!(s.line_height, 1.5, "line-height on html must reach text");
        assert_eq!(s.font_family, "Georgia", "font-family on html must reach text");
        assert_eq!(s.text_align, rustkit_css::TextAlign::Center, "text-align on html must reach text");
    }

    #[test]
    fn em_on_body_resolves_against_the_html_font_size() {
        // The value test, not just the reaching test. html 20px + body 2em
        // must be 40. If html's size never arrives, body resolves 2em against
        // the 16px initial and yields 32 - a plausible-looking wrong number.
        let s = text_style(
            r#"<html><head><style>html{font-size:20px}body{font-size:2em}</style></head><body>x</body></html>"#
        );
        assert_eq!(s.font_size, Length::Px(40.0),
                   "2em on body must resolve against html's 20px = 40, not the 16px initial = 32");
    }

    #[test]
    fn body_still_overrides_html() {
        // Inheritance must not become imposition: a property set on BOTH must
        // take body's value, or this fix would have traded one bug for another.
        let s = text_style(
            r#"<html><head><style>html{color:#ff0000;font-size:20px}body{color:#0000ff;font-size:30px}</style></head><body>x</body></html>"#
        );
        assert_eq!((s.color.r, s.color.g, s.color.b), (0, 0, 255), "body's colour must win over html's");
        assert_eq!(s.font_size, Length::Px(30.0), "body's font-size must win over html's");
    }

    #[test]
    fn a_document_without_an_html_element_still_builds() {
        // Fragment parsing and malformed documents must not panic or regress.
        let s = text_style(r#"<body><p>x</p></body>"#);
        assert_eq!(s.font_size, Length::Px(16.0), "no html element: the initial size still applies");
    }
}
