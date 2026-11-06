//! Unified content webview for Linux.
//!
//! WebKit (wry) is the default content view. The RustKit page view is the other variant, built only when
//! `HIWAVE_RUSTKIT_CONTENT=1` engages it on an X11 window (see `rustkit_content.rs`). Both engines are compiled
//! into one binary and the choice is made once at startup.
//!
//! The enum implements the app's `IWebView`, so every existing call site (`apply_layout`, `load_new_tab`, the
//! event loop's `evaluate_script`) works on either engine unchanged.

#[cfg(target_os = "linux")]
use super::content_webview_trait::ContentWebViewOps;
#[cfg(all(target_os = "linux", feature = "rustkit"))]
use super::webview::IWebContent;
use super::webview::IWebView;
#[cfg(all(target_os = "linux", feature = "rustkit"))]
use super::webview_rustkit::RustKitView;
use std::sync::Arc;
use wry::Rect;

/// Unified content webview type.
#[cfg(target_os = "linux")]
#[derive(Clone)]
pub enum ContentWebView {
    Wry(Arc<wry::WebView>),
    #[cfg(feature = "rustkit")]
    RustKit(Arc<RustKitView>),
}

#[cfg(not(target_os = "linux"))]
pub type ContentWebView = Arc<wry::WebView>;

#[cfg(target_os = "linux")]
impl IWebView for ContentWebView {
    fn load_url(&self, url: &str) {
        match self {
            ContentWebView::Wry(v) => IWebView::load_url(v, url),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => {
                let _ = v.wry_load_url(url);
            }
        }
    }

    fn load_html(&self, html: &str) {
        match self {
            ContentWebView::Wry(v) => IWebView::load_html(v, html),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => {
                let _ = v.wry_load_html(html);
            }
        }
    }

    fn evaluate_script(&self, script: &str) {
        match self {
            ContentWebView::Wry(v) => IWebView::evaluate_script(v, script),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => {
                let _ = v.wry_evaluate_script(script);
            }
        }
    }

    /// `rect` is already in WINDOW coordinates (apply_layout adds the container's origin), which is what the
    /// RustKit X11 child window, a child of the window itself, needs.
    fn set_bounds(&self, rect: Rect) {
        match self {
            ContentWebView::Wry(v) => IWebView::set_bounds(v, rect),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => {
                let _ = v.wry_set_bounds(rect);
            }
        }
    }

    fn url(&self) -> Option<String> {
        match self {
            ContentWebView::Wry(v) => IWebView::url(v),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => IWebContent::get_url(&**v).map(|u| u.to_string()),
        }
    }

    fn set_zoom(&self, level: f64) {
        match self {
            ContentWebView::Wry(v) => IWebView::set_zoom(v, level),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(_) => tracing::debug!("RustKit content: zoom is not wired yet"),
        }
    }

    fn print(&self) {
        match self {
            ContentWebView::Wry(v) => IWebView::print(v),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(_) => tracing::debug!("RustKit content: print is not wired yet"),
        }
    }

    fn focus(&self) {
        match self {
            ContentWebView::Wry(v) => IWebView::focus(v),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => v.grab_keyboard(),
        }
    }

    fn clear_all_browsing_data(&self) {
        match self {
            ContentWebView::Wry(v) => IWebView::clear_all_browsing_data(v),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(_) => tracing::debug!("RustKit content: clearing browsing data is not wired yet"),
        }
    }

    fn set_visible(&self, visible: bool) {
        match self {
            ContentWebView::Wry(v) => IWebView::set_visible(v, visible),
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(_) => tracing::debug!(visible, "RustKit content: visibility is not wired yet"),
        }
    }
}

#[cfg(target_os = "linux")]
impl ContentWebViewOps for ContentWebView {
    fn load_url(&self, url: &str) -> Result<(), String> {
        match self {
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => v.wry_load_url(url),
            ContentWebView::Wry(v) => wry::WebView::load_url(v, url).map_err(|e| e.to_string()),
        }
    }

    fn load_html(&self, html: &str) -> Result<(), String> {
        match self {
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => v.wry_load_html(html),
            ContentWebView::Wry(v) => wry::WebView::load_html(v, html).map_err(|e| e.to_string()),
        }
    }

    fn evaluate_script(&self, script: &str) -> Result<(), String> {
        match self {
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => v.wry_evaluate_script(script),
            ContentWebView::Wry(v) => wry::WebView::evaluate_script(v, script).map_err(|e| e.to_string()),
        }
    }

    fn set_bounds(&self, rect: Rect) -> Result<(), String> {
        match self {
            #[cfg(feature = "rustkit")]
            ContentWebView::RustKit(v) => v.wry_set_bounds(rect),
            ContentWebView::Wry(v) => wry::WebView::set_bounds(v, rect).map_err(|e| e.to_string()),
        }
    }
}
