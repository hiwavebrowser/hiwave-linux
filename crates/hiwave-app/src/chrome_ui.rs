//! The browser chrome's view and its panels, behind one small interface (stage S1b).
//!
//! Ported from the macOS app's `ChromeUi` / `ChromeView` / `LazyPanel` (hiwave-macos `main.rs`) so the Linux app
//! has the same seams the RustKit chrome plugs into (S2). With WebKit as the only backend this is behaviour
//! neutral: every call goes to the same `wry::WebView` it went to before.
//!
//! What is not ported yet, on purpose: `send_state` / `flush_state` (the coalesced state bridge needs
//! `chrome_state`, which is a later stage), and the RustKit variant of `ChromeView`.

use crate::webview::IWebView;
#[cfg(feature = "rustkit")]
use crate::webview::IWebContent;
#[cfg(feature = "rustkit")]
use crate::webview_rustkit::RustKitView;
use std::sync::Arc;
use wry::Rect;

/// The chrome's (or a panel's) view. The app only runs script in it and places it.
pub trait ChromeUi {
    /// Run `js` in the chrome's document, without waiting for its value.
    fn eval(&self, js: &str) -> Result<(), String>;
    /// Move and size the view.
    fn place(&self, rect: Rect);
    /// Route the keyboard to the chrome after a shortcut's script focused one of its fields (the address bar,
    /// find, the command palette). The WebKit view: nothing, as before.
    fn take_keyboard(&self) {}
    /// The page column (x, y, width, height) the chrome must leave to the page below it, in a `width` x
    /// `height` window. The WebKit view: nothing, as before.
    fn compose(&self, _width: f64, _height: f64, _hole: (f64, f64, f64, f64)) {}
    /// Stay the frontmost of the window's views, over panels built after it. The WebKit view: nothing.
    fn keep_on_top(&self) {}
}

impl ChromeUi for wry::WebView {
    fn eval(&self, js: &str) -> Result<(), String> {
        wry::WebView::evaluate_script(self, js).map_err(|e| e.to_string())
    }

    fn place(&self, rect: Rect) {
        let _ = wry::WebView::set_bounds(self, rect);
    }
}

/// The chrome's view as built: WebKit by default, or a RustKit view with `HIWAVE_RUSTKIT_CHROME=1` (S2).
#[derive(Clone)]
pub enum ChromeView {
    WebKit(Arc<wry::WebView>),
    #[cfg(feature = "rustkit")]
    RustKit(Arc<RustKitView>),
}

/// A panel's view (the shelf, later the Inspector and settings): the same backends as the chrome.
#[allow(dead_code)]
pub type PanelView = ChromeView;

impl ChromeUi for ChromeView {
    fn eval(&self, js: &str) -> Result<(), String> {
        match self {
            ChromeView::WebKit(view) => ChromeUi::eval(&**view, js),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => view.wry_evaluate_script(js),
        }
    }

    fn place(&self, rect: Rect) {
        match self {
            ChromeView::WebKit(view) => ChromeUi::place(&**view, rect),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => {
                let _ = view.wry_set_bounds(rect);
            }
        }
    }

    fn take_keyboard(&self) {
        #[cfg(feature = "rustkit")]
        if let ChromeView::RustKit(view) = self {
            view.grab_keyboard();
        }
    }

    fn compose(&self, width: f64, height: f64, hole: (f64, f64, f64, f64)) {
        #[cfg(feature = "rustkit")]
        if let ChromeView::RustKit(view) = self {
            view.compose_region(width, height, rustkit_viewhost::RegionRect::new(hole.0, hole.1, hole.2, hole.3));
        }
        #[cfg(not(feature = "rustkit"))]
        let _ = (width, height, hole);
    }

    fn keep_on_top(&self) {
        #[cfg(feature = "rustkit")]
        if let ChromeView::RustKit(view) = self {
            view.raise();
        }
    }
}

/// The app's existing call sites (`apply_layout`, the `sync_*_to_chrome` helpers, the event loop) speak
/// `IWebView`; the chrome view answers them with the same calls it made as a bare `wry::WebView`.
impl IWebView for ChromeView {
    fn load_url(&self, url: &str) {
        match self {
            ChromeView::WebKit(view) => IWebView::load_url(&**view, url),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => {
                let _ = view.wry_load_url(url);
            }
        }
    }

    fn load_html(&self, html: &str) {
        match self {
            ChromeView::WebKit(view) => IWebView::load_html(&**view, html),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => {
                let _ = view.wry_load_html(html);
            }
        }
    }

    fn evaluate_script(&self, script: &str) {
        match self {
            ChromeView::WebKit(view) => IWebView::evaluate_script(&**view, script),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => {
                let _ = view.wry_evaluate_script(script);
            }
        }
    }

    fn set_bounds(&self, rect: Rect) {
        match self {
            ChromeView::WebKit(view) => IWebView::set_bounds(&**view, rect),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => {
                let _ = view.wry_set_bounds(rect);
            }
        }
    }

    fn url(&self) -> Option<String> {
        match self {
            ChromeView::WebKit(view) => IWebView::url(&**view),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => IWebContent::get_url(&**view).map(|u| u.to_string()),
        }
    }

    fn set_zoom(&self, level: f64) {
        match self {
            ChromeView::WebKit(view) => IWebView::set_zoom(&**view, level),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(_) => {}
        }
    }

    fn print(&self) {
        match self {
            ChromeView::WebKit(view) => IWebView::print(&**view),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(_) => {}
        }
    }

    fn focus(&self) {
        match self {
            ChromeView::WebKit(view) => IWebView::focus(&**view),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(view) => view.grab_keyboard(),
        }
    }

    fn clear_all_browsing_data(&self) {
        match self {
            ChromeView::WebKit(view) => IWebView::clear_all_browsing_data(&**view),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(_) => {}
        }
    }

    fn set_visible(&self, visible: bool) {
        match self {
            ChromeView::WebKit(view) => IWebView::set_visible(&**view, visible),
            #[cfg(feature = "rustkit")]
            ChromeView::RustKit(_) => {}
        }
    }
}

/// How many scripts a panel that is not built yet keeps for when it is. Past it the oldest go: a panel shows
/// what it was last told.
#[allow(dead_code)]
pub const LAZY_PANEL_HELD_MAX: usize = 256;

/// A panel's view, built when the panel first opens, so a panel most sessions never open costs nothing until
/// then. Once built it stays. Scripts sent before it exists are held and run when it is built. Used from S2
/// (the RustKit panels); the WebKit shelf is built at launch and wrapped with [`LazyPanel::built`].
#[allow(dead_code)]
pub struct LazyPanel<V = PanelView> {
    view: std::cell::OnceCell<V>,
    /// Scripts run on it before it was built, oldest first.
    held: std::cell::RefCell<std::collections::VecDeque<String>>,
    /// Its document and its name in the log.
    pub html: &'static str,
    pub name: &'static str,
}

#[allow(dead_code)]
impl<V: ChromeUi> LazyPanel<V> {
    /// A panel whose view is built now.
    pub fn built(view: V, html: &'static str, name: &'static str) -> Self {
        let panel = Self::deferred(html, name);
        let _ = panel.view.set(view);
        panel
    }

    /// A panel built when it first opens.
    pub fn deferred(html: &'static str, name: &'static str) -> Self {
        LazyPanel {
            view: std::cell::OnceCell::new(),
            held: std::cell::RefCell::new(std::collections::VecDeque::new()),
            html,
            name,
        }
    }

    /// Its view, if built.
    pub fn get(&self) -> Option<&V> {
        self.view.get()
    }

    /// The layout gives the panel `rect`: a panel with a height is open, and one not built yet is built there
    /// by `build` (its document and name), then hears the scripts it was sent meanwhile. A closed one is only
    /// placed.
    pub fn lay_out(&self, rect: Rect, build: impl FnOnce(Rect, &'static str, &'static str) -> Option<V>) {
        let open = rect.size.to_logical::<f64>(1.0).height > 0.0;
        if open && self.view.get().is_none() {
            if let Some(view) = build(rect, self.html, self.name) {
                for js in self.held.take() {
                    let _ = view.eval(&js);
                }
                let _ = self.view.set(view);
            }
        }
        if let Some(view) = self.view.get() {
            view.place(rect);
        }
    }
}

impl<V: ChromeUi> ChromeUi for LazyPanel<V> {
    fn eval(&self, js: &str) -> Result<(), String> {
        match self.view.get() {
            Some(view) => view.eval(js),
            None => {
                let mut held = self.held.borrow_mut();
                if held.len() == LAZY_PANEL_HELD_MAX {
                    held.pop_front();
                }
                held.push_back(js.to_string());
                Ok(())
            }
        }
    }

    fn place(&self, rect: Rect) {
        if let Some(view) = self.view.get() {
            view.place(rect);
        }
    }

    fn take_keyboard(&self) {
        if let Some(view) = self.view.get() {
            view.take_keyboard();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use wry::dpi::{LogicalPosition, LogicalSize};

    #[derive(Default)]
    struct Fake {
        log: Rc<RefCell<Vec<String>>>,
    }

    impl ChromeUi for Fake {
        fn eval(&self, js: &str) -> Result<(), String> {
            self.log.borrow_mut().push(format!("eval:{js}"));
            Ok(())
        }
        fn place(&self, rect: Rect) {
            let s = rect.size.to_logical::<f64>(1.0);
            self.log.borrow_mut().push(format!("place:{}x{}", s.width, s.height));
        }
        fn take_keyboard(&self) {
            self.log.borrow_mut().push("keyboard".into());
        }
    }

    fn rect(h: f64) -> Rect {
        Rect { position: LogicalPosition::new(0, 0).into(), size: LogicalSize::new(100.0, h).into() }
    }

    #[test]
    fn the_defaults_do_nothing_for_a_view_that_does_not_compose() {
        let f = Fake::default();
        f.compose(1.0, 1.0, (0.0, 0.0, 1.0, 1.0));
        f.keep_on_top();
        assert!(f.log.borrow().is_empty());
    }

    #[test]
    fn a_deferred_panel_holds_scripts_and_replays_them_in_order_when_built() {
        let panel: LazyPanel<Fake> = LazyPanel::deferred("shelf.html", "shelf");
        assert!(panel.eval("a").is_ok());
        assert!(panel.eval("b").is_ok());
        assert!(panel.get().is_none());
        let log = Rc::new(RefCell::new(Vec::new()));
        let l2 = Rc::clone(&log);
        panel.lay_out(rect(40.0), move |_, html, name| {
            assert_eq!((html, name), ("shelf.html", "shelf"));
            Some(Fake { log: l2 })
        });
        assert_eq!(*log.borrow(), vec!["eval:a", "eval:b", "place:100x40"]);
        // Built panels pass scripts straight through.
        panel.eval("c").unwrap();
        assert_eq!(log.borrow().last().unwrap(), "eval:c");
    }

    #[test]
    fn a_closed_panel_is_not_built_and_a_built_one_is_only_placed() {
        let panel: LazyPanel<Fake> = LazyPanel::deferred("x", "x");
        panel.lay_out(rect(0.0), |_, _, _| panic!("a closed panel must not be built"));
        assert!(panel.get().is_none());
        let f = Fake::default();
        let log = Rc::clone(&f.log);
        let built = LazyPanel::built(f, "x", "x");
        built.lay_out(rect(0.0), |_, _, _| panic!("already built"));
        assert_eq!(*log.borrow(), vec!["place:100x0"]);
    }

    #[test]
    fn held_scripts_are_capped_and_the_oldest_go() {
        let panel: LazyPanel<Fake> = LazyPanel::deferred("x", "x");
        for i in 0..(LAZY_PANEL_HELD_MAX + 5) {
            panel.eval(&format!("s{i}")).unwrap();
        }
        let log = Rc::new(RefCell::new(Vec::new()));
        let l2 = Rc::clone(&log);
        panel.lay_out(rect(10.0), move |_, _, _| Some(Fake { log: l2 }));
        let evals: Vec<String> = log.borrow().iter().filter(|l| l.starts_with("eval:")).cloned().collect();
        assert_eq!(evals.len(), LAZY_PANEL_HELD_MAX);
        assert_eq!(evals.first().unwrap(), "eval:s5");
        assert_eq!(evals.last().unwrap(), &format!("eval:s{}", LAZY_PANEL_HELD_MAX + 4));
    }

    #[test]
    fn take_keyboard_reaches_a_built_panel_only() {
        let panel: LazyPanel<Fake> = LazyPanel::deferred("x", "x");
        panel.take_keyboard(); // not built: nothing, no panic
        let f = Fake::default();
        let log = Rc::clone(&f.log);
        let built = LazyPanel::built(f, "x", "x");
        built.take_keyboard();
        assert_eq!(*log.borrow(), vec!["keyboard"]);
    }
}
