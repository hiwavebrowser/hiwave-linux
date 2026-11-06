//! The RustKit page view as the Linux content view: opt-in, X11 only.
//!
//! `HIWAVE_RUSTKIT_CONTENT=1` asks for it. It engages only on an X11 window (the RustKit view is an X11 child of
//! the app window); on a Wayland window it logs one clear line and the WebKit view is used. With the variable
//! unset nothing here runs: no `RustKitView`, no X child window, no content thread (`decide` is the whole
//! gate, and it is unit-tested).
//!
//! Stage S1a. The page renders, navigates and syncs its title and URL; it is deliberately NON-interactive
//! until the X11 input stage (S1c) lands, because the Linux viewhost does not yet deliver mouse or keyboard
//! events to the engine.

use crate::webview::IWebContent as _;
use crate::webview_rustkit::RustKitView;
use crate::UserEvent;
use rustkit_viewhost::Bounds;
use std::rc::Rc;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tao::event_loop::EventLoopProxy;
use tao::rwh_06::{HasWindowHandle, RawWindowHandle};
use tracing::{debug, info, warn};

/// The environment variable that asks for the RustKit page view.
pub const FLAG: &str = "HIWAVE_RUSTKIT_CONTENT";

/// What kind of native window the app window is, as far as the RustKit view cares.
#[derive(Debug, PartialEq, Eq)]
pub enum HandleKind {
    X11,
    Other(&'static str),
}

/// The decision, made once at startup.
#[derive(Debug, PartialEq, Eq)]
pub enum Engagement {
    /// Not requested: nothing RustKit is created.
    Off,
    /// Requested but not possible here: log why, use WebKit.
    FallBack(&'static str),
    /// Requested and possible: build the RustKit page view.
    On,
}

/// Only the exact value `1` asks for it.
pub fn flag_requested(value: Option<&str>) -> bool {
    value == Some("1")
}

/// The whole gate. Pure, so the default-off and fallback behaviour are testable without a display.
pub fn decide(requested: bool, handle: HandleKind) -> Engagement {
    if !requested {
        return Engagement::Off;
    }
    match handle {
        HandleKind::X11 => Engagement::On,
        HandleKind::Other("Wayland") => Engagement::FallBack(
            "the app window is a Wayland window and the RustKit view needs an X11 window; start the app with GDK_BACKEND=x11",
        ),
        HandleKind::Other(_) => Engagement::FallBack("the app window has no X11 handle the RustKit view can attach to"),
    }
}

/// Classify the app window's native handle.
pub fn handle_kind(window: &tao::window::Window) -> HandleKind {
    match window.window_handle().map(|h| h.as_raw()) {
        Ok(RawWindowHandle::Xlib(_)) | Ok(RawWindowHandle::Xcb(_)) => HandleKind::X11,
        Ok(RawWindowHandle::Wayland(_)) => HandleKind::Other("Wayland"),
        Ok(_) => HandleKind::Other("unsupported"),
        Err(_) => HandleKind::Other("unavailable"),
    }
}

/// Decide for this window and say so once. Off is silent at info level (it is the default).
pub fn choose(window: &tao::window::Window) -> Engagement {
    let requested = flag_requested(std::env::var(FLAG).ok().as_deref());
    let engagement = decide(requested, handle_kind(window));
    match &engagement {
        Engagement::Off => debug!("RustKit content view: off ({FLAG} is not 1)"),
        Engagement::FallBack(why) => warn!("{FLAG}=1 ignored, using the WebKit page view: {why}"),
        Engagement::On => info!("RustKit content view: engaged ({FLAG}=1, X11 window)"),
    }
    engagement
}

/// Build the RustKit page view. Logs how long creation took (a number for the S4 frame-time work, not a profile).
pub fn create(
    window: &tao::window::Window,
    bounds: Bounds,
    blocked_counter: Option<Arc<AtomicU64>>,
) -> Result<(Arc<RustKitView>, Instant), String> {
    let started = Instant::now();
    info!(?bounds.x, ?bounds.y, ?bounds.width, ?bounds.height, "RustKit content view: creating");
    let view = RustKitView::with_shield_counter(window, bounds, blocked_counter).map_err(|e| e.to_string())?;
    info!(elapsed_ms = started.elapsed().as_millis() as u64, "RustKit content view: created");
    Ok((Arc::new(view), Instant::now()))
}

/// Drive the view on the GTK main loop: its clock and late fetches, its `window.ipc` messages to the shared
/// content IPC handler, navigations the page started, title / URL / loading changes into the tab state (the
/// same `UserEvent`s the WebKit view's handlers send), and a frame. Mirrors the macOS per-tick pump.
pub fn install_pump(view: Arc<RustKitView>, proxy: EventLoopProxy<UserEvent>, ipc: Rc<dyn Fn(&str)>, created: Instant) {
    let mut first_load_logged = false;
    gtk::glib::timeout_add_local(Duration::from_millis(16), move || {
        let _ = view.process_events();

        for body in view.take_ipc_messages() {
            ipc(&body);
        }

        if let Some((url, from_user)) = view.take_script_navigation() {
            if from_user {
                info!(url = %crate::page_url_for_log(&url), "Link clicked");
            } else {
                info!(url = %crate::page_url_for_log(&url), "Script navigation");
            }
            let _ = proxy.send_event(UserEvent::Navigate(url));
        }

        let updates = view.take_chrome_updates();
        if let Some(url) = updates.url {
            let _ = proxy.send_event(UserEvent::UpdateUrl(url.clone()));
            let _ = proxy.send_event(UserEvent::UpdateActiveTabUrl(url));
            let _ = proxy.send_event(UserEvent::NavigationStateChanged {
                can_go_back: view.can_go_back(),
                can_go_forward: view.can_go_forward(),
            });
        }
        if let Some(title) = updates.title {
            let _ = proxy.send_event(UserEvent::UpdateTitle(title));
        }
        if updates.started {
            let _ = proxy.send_event(UserEvent::SetLoading(true));
        }
        if updates.loading == Some(false) {
            let _ = proxy.send_event(UserEvent::SetLoading(false));
            let _ = proxy.send_event(UserEvent::SyncShieldStats);
            if !first_load_logged {
                first_load_logged = true;
                info!(since_view_created_ms = created.elapsed().as_millis() as u64, "RustKit content view: first load finished");
            }
        }
        if let Some(url) = updates.finished {
            let _ = proxy.send_event(UserEvent::RecordVisit { url });
        }

        view.render();
        gtk::glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_value_one_asks_for_it() {
        assert!(flag_requested(Some("1")));
        for v in [None, Some(""), Some("0"), Some("true"), Some("yes"), Some("2")] {
            assert!(!flag_requested(v), "{v:?}");
        }
    }

    /// The default-off proof: with the flag unset nothing is engaged on ANY window kind, so no view, no X
    /// child window and no content thread are ever created.
    #[test]
    fn flag_unset_engages_nothing_on_any_window() {
        for h in [HandleKind::X11, HandleKind::Other("Wayland"), HandleKind::Other("unsupported")] {
            assert_eq!(decide(false, h), Engagement::Off);
        }
    }

    #[test]
    fn requested_on_x11_engages() {
        assert_eq!(decide(true, HandleKind::X11), Engagement::On);
    }

    #[test]
    fn requested_on_wayland_falls_back_and_says_how_to_fix_it() {
        match decide(true, HandleKind::Other("Wayland")) {
            Engagement::FallBack(why) => assert!(why.contains("GDK_BACKEND=x11"), "{why}"),
            other => panic!("expected a fallback, got {other:?}"),
        }
    }

    #[test]
    fn requested_on_an_unknown_handle_falls_back() {
        assert!(matches!(decide(true, HandleKind::Other("unavailable")), Engagement::FallBack(_)));
    }
}
