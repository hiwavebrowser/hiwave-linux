//! The browser chrome as a RustKit view: opt-in, X11 only (stage S2).
//!
//! `HIWAVE_RUSTKIT_CHROME=1` runs the same `ui/chrome.html` the WebKit chrome runs in a RustKit view, as the
//! macOS app does. The view is a full-window X11 child created AFTER the page, so it stacks above it, and an
//! XShape region (window less the page column, plus whatever the chrome paints over it) lets the page show
//! through and take the pointer. With the variable unset nothing here runs.
//!
//! Known gap, stated in the PR: this is the OPAQUE route. macOS's default is an alpha chrome whose transparent
//! pixels reveal the page; X11 without an ARGB visual and a compositor cannot do that, so the shape mask does the
//! job. Overlays that extend over the page (dropdowns, find, palette, modals) are only covered as far as the
//! region follows the frame's painted boxes (`RustKitView::apply_region`); the full growth rules are S3.

use crate::rustkit_content::{decide, flag_requested, handle_kind, Engagement};
use crate::webview_rustkit::RustKitView;
use rustkit_viewhost::Bounds;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// The environment variable that asks for the RustKit chrome.
pub const FLAG: &str = "HIWAVE_RUSTKIT_CHROME";

/// The whole gate, pure. The RustKit chrome's `window.ipc` messages reach the app only through the content
/// thread's event harvest (the same constraint the macOS app documents), so without `HIWAVE_CONTENT_THREAD=1`
/// the chrome would draw but every button would be dead: that case falls back to the WebKit chrome instead.
pub fn decide_chrome(requested: bool, handle: crate::rustkit_content::HandleKind, content_thread: bool) -> Engagement {
    match decide(requested, handle) {
        Engagement::On if !content_thread => Engagement::FallBack(
            "the RustKit chrome needs the content thread (its messages arrive through it); start the app with HIWAVE_CONTENT_THREAD=1",
        ),
        other => other,
    }
}

/// Decide for this window and say so once. Off is silent at info level (it is the default).
pub fn choose(window: &tao::window::Window) -> Engagement {
    let requested = flag_requested(std::env::var(FLAG).ok().as_deref());
    let engagement = decide_chrome(requested, handle_kind(window), rustkit_engine::content_thread_enabled());
    match &engagement {
        Engagement::Off => debug!("RustKit chrome: off ({FLAG} is not 1)"),
        Engagement::FallBack(why) => warn!("{FLAG}=1 ignored, using the WebKit chrome: {why}"),
        Engagement::On => info!("Chrome: RustKit ({FLAG}=1, X11 window, content thread on)"),
    }
    engagement
}

/// The script the page-hole click helper installs: chrome.html closes its menus on a click outside them, and
/// a click that lands in the page (a different window now) never reaches the chrome's document.
pub const OUTSIDE_CLICK: &str = r#"
window.__hiwaveOutsideClick = function () {
    var ev;
    try { ev = new MouseEvent('click', { bubbles: true }); } catch (e) { ev = new Event('click', { bubbles: true }); }
    document.body.dispatchEvent(ev);
};
"#;

/// A document for a RustKit view: `bridge` (the WebKit views run it as an initialization script) as the head's
/// first script, and `tail`, when there is one, as the body's last.
pub fn html_with_bridge(html: &str, bridge: &str, tail: &str) -> String {
    let html = if tail.is_empty() {
        html.to_string()
    } else {
        let tail = format!("<script>{}</script>", tail);
        match html.rfind("</body>") {
            Some(at) => format!("{}{}{}", &html[..at], tail, &html[at..]),
            None => format!("{}{}", html, tail),
        }
    };
    let script = format!("<script>{}</script>", bridge);
    match html.find("<head>") {
        Some(at) => {
            let at = at + "<head>".len();
            format!("{}{}{}", &html[..at], script, &html[at..])
        }
        None => format!("{}{}", script, html),
    }
}

/// Build the chrome view over `bounds` and load `document` into it as an app-owned view (only the app's own
/// documents get `window.ipc` and `window.hiwaveChrome`; the page view is never passed here).
pub fn create(window: &tao::window::Window, bounds: Bounds, document: &str) -> Result<Arc<RustKitView>, String> {
    // The region route needs XShape: without it the full-window chrome would cover the page for good.
    if !rustkit_viewhost::x11_shape_supported() {
        return Err("the X server has no SHAPE extension".to_string());
    }
    let started = std::time::Instant::now();
    let view = RustKitView::new(window, bounds).map_err(|e| e.to_string())?;
    view.set_app_owned();
    view.load_html_internal(document).map_err(|e| e.to_string())?;
    info!(elapsed_ms = started.elapsed().as_millis() as u64, "RustKit chrome: created and loaded");
    Ok(Arc::new(view))
}

/// The app window's X id, when it is an X11 window.
pub fn toplevel_xid(window: &tao::window::Window) -> Option<u64> {
    use tao::rwh_06::{HasWindowHandle, RawWindowHandle};
    match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xlib(h)) => Some(h.window as u64),
        Ok(RawWindowHandle::Xcb(h)) => Some(h.window.get() as u64),
        _ => None,
    }
}

/// With the RustKit chrome, keyboard focus sits on the chrome's X window after the chrome was clicked (that is how
/// its text fields get keys). A click on a WebKit page does not move it, so the page's keys and the shortcuts it
/// forwards would still go to the chrome. When the WebKit page is pressed this gives the X input focus back to the
/// app window with a raw `XSetInputFocus`: the exact inverse of the call that took it (not `gdk_window_focus`, which
/// under a window manager only sends it an activation request that it may ignore for a window already active).
/// WebKit then takes GTK focus as it always did. A RustKit page does this itself in its input pump.
pub fn return_focus_on_page_click(content: &wry::WebView, chrome: Arc<RustKitView>, window: &tao::window::Window) {
    use gtk::prelude::*;
    use wry::WebViewExtUnix;
    let Some(toplevel) = toplevel_xid(window) else {
        warn!("RustKit chrome: the app window has no X11 id, keyboard focus will not return after a page click");
        return;
    };
    content.webview().connect_button_press_event(move |_, _| {
        chrome.focus_x11_window(toplevel);
        gtk::glib::Propagation::Proceed
    });
}

/// Drive the chrome on the GTK main loop: step its load, deliver its X11 input, run its timers, hand its
/// `window.ipc` messages to the shared chrome IPC handler, draw a frame.
pub fn install_pump(view: Arc<RustKitView>, ipc: Rc<dyn Fn(&str)>) {
    gtk::glib::timeout_add_local(Duration::from_millis(16), move || {
        view.step_load();
        view.pump_x11_input();
        let _ = view.process_events();
        for body in view.take_ipc_messages() {
            ipc(&body);
        }
        view.render();
        gtk::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rustkit_content::HandleKind;

    #[test]
    fn unset_engages_nothing_whatever_the_window_or_thread() {
        for h in [HandleKind::X11, HandleKind::Other("Wayland")] {
            for ct in [true, false] {
                assert_eq!(decide_chrome(false, h_clone(&h), ct), Engagement::Off);
            }
        }
    }

    fn h_clone(h: &HandleKind) -> HandleKind {
        match h {
            HandleKind::X11 => HandleKind::X11,
            HandleKind::Other(s) => HandleKind::Other(s),
        }
    }

    #[test]
    fn x11_with_the_content_thread_engages() {
        assert_eq!(decide_chrome(true, HandleKind::X11, true), Engagement::On);
    }

    #[test]
    fn without_the_content_thread_it_falls_back_and_says_what_to_set() {
        match decide_chrome(true, HandleKind::X11, false) {
            Engagement::FallBack(why) => assert!(why.contains("HIWAVE_CONTENT_THREAD=1"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn on_wayland_it_falls_back_to_the_x11_hint() {
        match decide_chrome(true, HandleKind::Other("Wayland"), true) {
            Engagement::FallBack(why) => assert!(why.contains("GDK_BACKEND=x11"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_bridge_is_the_first_script_in_the_head_and_the_tail_the_last_in_the_body() {
        let html = "<html><head><title>t</title></head><body><p>x</p></body></html>";
        let out = html_with_bridge(html, "BRIDGE", "TAIL");
        let bridge = out.find("<script>BRIDGE</script>").unwrap();
        assert_eq!(bridge, out.find("<head>").unwrap() + "<head>".len());
        let tail = out.find("<script>TAIL</script>").unwrap();
        assert_eq!(tail + "<script>TAIL</script>".len(), out.find("</body>").unwrap());
    }

    #[test]
    fn a_document_with_no_head_or_body_still_gets_the_bridge_and_tail() {
        let out = html_with_bridge("<p>x</p>", "B", "T");
        assert!(out.starts_with("<script>B</script>"));
        assert!(out.contains("<script>T</script>"));
    }

    #[test]
    fn an_empty_tail_adds_no_script() {
        let out = html_with_bridge("<head></head><body></body>", "B", "");
        assert_eq!(out.matches("<script>").count(), 1);
    }
}
