//! The Inspector panel (stage I1a): a view below the page, built when it first opens.
//!
//! The panel is a [`LazyPanel`] of [`ChromeView`]: nothing exists until the first open (a zero-height X window is
//! invalid), then it is built once by the closure the app gives it, WebKit or RustKit by the chrome's backend,
//! and stays, placed with height 0 when closed. This module only owns that lifecycle and the layout term; what the
//! panel shows (DOM tree, styles, console) is I1b.

use crate::chrome_ui::{ChromeUi, ChromeView, LazyPanel};
use wry::Rect;

/// The panel's height when open, as on macOS.
pub const INSPECTOR_HEIGHT_EXPANDED: u32 = 300;
/// Closed.
pub const INSPECTOR_HEIGHT_DEFAULT: u32 = 0;

pub struct InspectorPanel {
    lazy: LazyPanel<ChromeView>,
    build: Box<dyn Fn(&tao::window::Window, Rect) -> Option<ChromeView>>,
}

impl InspectorPanel {
    pub fn new(html: &'static str, build: impl Fn(&tao::window::Window, Rect) -> Option<ChromeView> + 'static) -> Self {
        InspectorPanel { lazy: LazyPanel::deferred(html, "Inspector"), build: Box::new(build) }
    }

    /// The layout gives the panel `rect`: an open panel (height > 0) not built yet is built there.
    pub fn lay_out(&self, window: &tao::window::Window, rect: Rect) {
        // A built panel that is closed is parked, not given a zero height: X11 rejects a zero-height window and a
        // GTK allocation of 0 leaves stale pixels. One pixel high, far below the window (clipped by its parent).
        let rect = if self.is_built() { parked_if_closed(rect) } else { rect };
        self.lazy.lay_out(rect, |rect, _html, _name| (self.build)(window, rect));
    }

    pub fn take_keyboard(&self) {
        self.lazy.take_keyboard();
    }

    pub fn eval(&self, js: &str) -> Result<(), String> {
        self.lazy.eval(js)
    }

    pub fn is_built(&self) -> bool {
        self.lazy.get().is_some()
    }
}

/// How far below the window a closed panel is parked.
const PARK_OFFSET: i32 = 16_000;

/// `rect` as it is placed: itself when it has a height, else a 1-pixel strip parked below the window.
pub fn parked_if_closed(rect: Rect) -> Rect {
    use wry::dpi::{LogicalPosition, LogicalSize};
    let size = rect.size.to_logical::<f64>(1.0);
    if size.height > 0.0 {
        return rect;
    }
    let at = rect.position.to_logical::<f64>(1.0);
    Rect {
        position: LogicalPosition::new(at.x as i32, at.y as i32 + PARK_OFFSET).into(),
        size: LogicalSize::new(size.width.max(1.0), 1.0).into(),
    }
}

/// The rectangles of a layout turn, pure so the arithmetic is tested: the page, the panel below it.
#[derive(Debug, PartialEq)]
pub struct Column {
    pub content_height: f64,
    pub inspector_y: f64,
}

/// Page height and the panel's top, in a column of `height` under a chrome of `chrome_height`, a shelf of
/// `shelf_height` and a panel of `inspector_height` (the panel sits between the page and the shelf).
pub fn column(height: f64, chrome_height: f64, shelf_height: f64, inspector_height: f64) -> Column {
    let content_height = (height - chrome_height - shelf_height - inspector_height).max(0.0);
    Column { content_height, inspector_y: chrome_height + content_height }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_closed_panel_leaves_the_page_its_full_column() {
        assert_eq!(column(800.0, 104.0, 0.0, 0.0), Column { content_height: 696.0, inspector_y: 800.0 });
    }

    #[test]
    fn an_open_panel_takes_its_height_from_the_page_and_sits_below_it() {
        let c = column(800.0, 104.0, 0.0, 300.0);
        assert_eq!(c, Column { content_height: 396.0, inspector_y: 500.0 });
    }

    #[test]
    fn the_shelf_and_the_panel_both_come_out_of_the_page() {
        let c = column(800.0, 104.0, 280.0, 300.0);
        assert_eq!(c.content_height, 116.0);
    }

    #[test]
    fn a_closed_panel_is_parked_below_the_window_one_pixel_high_and_an_open_one_is_left_alone() {
        use wry::dpi::{LogicalPosition, LogicalSize};
        let rect = |h: f64| Rect { position: LogicalPosition::new(10, 500).into(), size: LogicalSize::new(800.0, h).into() };
        let parked = parked_if_closed(rect(0.0));
        assert_eq!(parked.size.to_logical::<f64>(1.0).height, 1.0);
        assert_eq!(parked.position.to_logical::<f64>(1.0).y, 16_500.0);
        assert_eq!(parked.size.to_logical::<f64>(1.0).width, 800.0);
        let open = parked_if_closed(rect(300.0));
        assert_eq!(open.size.to_logical::<f64>(1.0).height, 300.0);
        assert_eq!(open.position.to_logical::<f64>(1.0).y, 500.0);
    }

    #[test]
    fn a_window_too_short_for_everything_leaves_the_page_nothing_not_a_negative_height() {
        assert_eq!(column(300.0, 104.0, 280.0, 300.0).content_height, 0.0);
    }
}
