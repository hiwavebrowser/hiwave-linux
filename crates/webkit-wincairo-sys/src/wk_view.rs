//! WebKit view functions (Windows-specific)
//!
//! WKView is the Windows-specific view class that embeds WebKit in an HWND.
//! This is the primary way to display web content on Windows.
//!
//! Note: The WinCairo WebKit2 API provides minimal WKView functions.
//! View positioning, sizing, focus, and visibility are managed through
//! the native HWND returned by WKViewGetWindow().

use super::wk_types::*;
use windows_sys::Win32::Foundation::{HWND, RECT};

extern "C" {
    // ========== View Creation ==========

    /// Create a new WKView
    ///
    /// # Parameters
    /// - `rect`: Initial bounds (position and size) of the view
    /// - `configuration`: Page configuration (from WKPageConfigurationCreate)
    /// - `parent`: Parent HWND to embed in
    ///
    /// Reference: WebKit/Source/WebKit/UIProcess/API/C/win/WKView.cpp
    pub fn WKViewCreate(
        rect: RECT,
        configuration: WKPageConfigurationRef,
        parent: HWND,
    ) -> WKViewRef;

    // ========== View Properties ==========

    /// Get the page associated with this view
    pub fn WKViewGetPage(view: WKViewRef) -> WKPageRef;

    /// Get the native window handle (HWND) for this view
    ///
    /// Use this HWND to control the view's position, size, visibility, and focus
    /// via standard Windows API calls (SetWindowPos, ShowWindow, SetFocus, etc.)
    pub fn WKViewGetWindow(view: WKViewRef) -> HWND;

    /// Set the parent window
    pub fn WKViewSetParentWindow(view: WKViewRef, parent: HWND);

    /// Notify that the view is in/out of a window
    pub fn WKViewSetIsInWindow(view: WKViewRef, is_in_window: bool);

    /// Notify that the window ancestry has changed
    pub fn WKViewWindowAncestryDidChange(view: WKViewRef);

    // ========== Rendering Options ==========

    /// Set whether to use offscreen rendering
    pub fn WKViewSetUsesOffscreenRendering(view: WKViewRef, uses_offscreen: bool);

    /// Set scroll offset on next resize
    pub fn WKViewSetScrollOffsetOnNextResize(view: WKViewRef, offset_x: f64, offset_y: f64);
}

/// Helper struct for working with view bounds
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct ViewBounds {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

impl ViewBounds {
    pub fn new(x: i32, y: i32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn to_rect(&self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width as i32,
            bottom: self.y + self.height as i32,
        }
    }

    pub fn from_rect(rect: RECT) -> Self {
        Self {
            x: rect.left,
            y: rect.top,
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
        }
    }
}
