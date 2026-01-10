//! Linux ViewHost implementation using X11.
//!
//! This module provides a native Linux window hosting layer using X11.
//! Wayland support is planned for a future iteration.
//!
//! ## Architecture
//!
//! - X11 Display connection
//! - XWindow for each view
//! - Event loop via XNextEvent
//! - Surface integration via raw-window-handle

use crate::{Bounds, EventCallback, MainWindowConfig, ViewEvent, ViewHostError, ViewId};
use rustkit_core::{
    FocusEvent, FocusEventType, InputEvent, KeyCode, KeyEvent, KeyEventType, KeyboardState,
    Modifiers, MouseButton, MouseEvent, MouseEventType, MouseState, Point,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use tracing::{debug, error, info, trace, warn};

// X11 bindings
use x11::xlib::{
    Display, Window, XCloseDisplay, XCreateSimpleWindow, XDefaultRootWindow, XDefaultScreen,
    XDestroyWindow, XFlush, XMapWindow, XMoveResizeWindow, XNextEvent, XOpenDisplay, XPending,
    XRootWindow, XSelectInput, XStoreName, XUnmapWindow, ButtonPressMask, ButtonReleaseMask,
    ExposureMask, FocusChangeMask, KeyPressMask, KeyReleaseMask, PointerMotionMask,
    StructureNotifyMask,
};
use std::ffi::CString;
use std::ptr;

/// View state for Linux X11.
struct X11ViewState {
    id: ViewId,
    window: Window,
    bounds: Bounds,
    visible: bool,
    focused: bool,
    keyboard_state: KeyboardState,
    mouse_state: MouseState,
}

/// Linux X11 ViewHost implementation.
pub struct X11ViewHost {
    display: *mut Display,
    views: RwLock<HashMap<ViewId, Arc<Mutex<X11ViewState>>>>,
    main_window: RwLock<Option<Window>>,
    event_callback: RwLock<Option<EventCallback>>,
    screen: i32,
}

impl X11ViewHost {
    /// Create a new X11 ViewHost.
    pub fn new() -> Result<Self, ViewHostError> {
        info!("Initializing X11 ViewHost");

        let display = unsafe { XOpenDisplay(ptr::null()) };
        if display.is_null() {
            error!("Failed to open X11 display");
            return Err(ViewHostError::WindowCreation("Failed to open X11 display".into()));
        }

        let screen = unsafe { XDefaultScreen(display) };
        debug!(screen, "X11 display opened");

        Ok(Self {
            display,
            views: RwLock::new(HashMap::new()),
            main_window: RwLock::new(None),
            event_callback: RwLock::new(None),
            screen,
        })
    }

    /// Create a top-level main window.
    pub fn create_main_window(&self, config: MainWindowConfig) -> Result<Window, ViewHostError> {
        info!(?config, "Creating X11 main window");

        unsafe {
            let root = XRootWindow(self.display, self.screen);
            
            // Create window
            let window = XCreateSimpleWindow(
                self.display,
                root,
                0, 0,                              // x, y (will be positioned by WM)
                config.width,                       // width
                config.height,                      // height
                1,                                  // border width
                0,                                  // border color (black)
                0xFFFFFF,                          // background color (white)
            );

            if window == 0 {
                error!("Failed to create X11 window");
                return Err(ViewHostError::WindowCreation("XCreateSimpleWindow failed".into()));
            }

            // Set window title
            let title = CString::new(config.title.as_str()).unwrap();
            XStoreName(self.display, window, title.as_ptr());

            // Select events
            XSelectInput(
                self.display,
                window,
                ExposureMask
                    | KeyPressMask
                    | KeyReleaseMask
                    | ButtonPressMask
                    | ButtonReleaseMask
                    | PointerMotionMask
                    | StructureNotifyMask
                    | FocusChangeMask,
            );

            // Map (show) the window
            XMapWindow(self.display, window);
            XFlush(self.display);

            *self.main_window.write().unwrap() = Some(window);

            info!(window, "X11 main window created");
            Ok(window)
        }
    }

    /// Create a child view in the given parent.
    pub fn create_view(&self, parent: Window, bounds: Bounds) -> Result<ViewId, ViewHostError> {
        let view_id = ViewId::new();
        debug!(?view_id, ?bounds, "Creating X11 view");

        unsafe {
            let window = XCreateSimpleWindow(
                self.display,
                parent,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
                0,          // border width
                0,          // border color
                0xFFFFFF,   // background color
            );

            if window == 0 {
                error!(?view_id, "Failed to create X11 child window");
                return Err(ViewHostError::WindowCreation("XCreateSimpleWindow failed".into()));
            }

            // Select events
            XSelectInput(
                self.display,
                window,
                ExposureMask
                    | KeyPressMask
                    | KeyReleaseMask
                    | ButtonPressMask
                    | ButtonReleaseMask
                    | PointerMotionMask
                    | FocusChangeMask,
            );

            XMapWindow(self.display, window);

            let state = X11ViewState {
                id: view_id,
                window,
                bounds,
                visible: true,
                focused: false,
                keyboard_state: KeyboardState::default(),
                mouse_state: MouseState::default(),
            };

            self.views
                .write()
                .unwrap()
                .insert(view_id, Arc::new(Mutex::new(state)));

            debug!(?view_id, window, "X11 view created");
            Ok(view_id)
        }
    }

    /// Resize a view.
    pub fn resize_view(&self, view_id: ViewId, bounds: Bounds) -> Result<(), ViewHostError> {
        let views = self.views.read().unwrap();
        let state = views
            .get(&view_id)
            .ok_or(ViewHostError::ViewNotFound(view_id))?;

        let mut state = state.lock().unwrap();
        state.bounds = bounds;

        unsafe {
            XMoveResizeWindow(
                self.display,
                state.window,
                bounds.x,
                bounds.y,
                bounds.width,
                bounds.height,
            );
        }

        trace!(?view_id, ?bounds, "X11 view resized");
        Ok(())
    }

    /// Destroy a view.
    pub fn destroy_view(&self, view_id: ViewId) -> Result<(), ViewHostError> {
        let state = self
            .views
            .write()
            .unwrap()
            .remove(&view_id)
            .ok_or(ViewHostError::ViewNotFound(view_id))?;

        let state = state.lock().unwrap();
        unsafe {
            XDestroyWindow(self.display, state.window);
        }

        debug!(?view_id, "X11 view destroyed");
        Ok(())
    }

    /// Pump events from X11 (non-blocking).
    pub fn pump_messages(&self) -> bool {
        unsafe {
            while XPending(self.display) > 0 {
                let mut event = std::mem::zeroed();
                XNextEvent(self.display, &mut event);
                
                // TODO: Handle events and emit via callback
                // For now, just process them to prevent queue backup
            }
        }
        true
    }

    /// Set the event callback.
    pub fn set_event_callback(&self, callback: EventCallback) {
        *self.event_callback.write().unwrap() = Some(callback);
    }
}

impl Drop for X11ViewHost {
    fn drop(&mut self) {
        info!("Shutting down X11 ViewHost");
        
        // Destroy all views
        let views: Vec<_> = self.views.read().unwrap().keys().copied().collect();
        for view_id in views {
            let _ = self.destroy_view(view_id);
        }

        // Close display
        if !self.display.is_null() {
            unsafe {
                XCloseDisplay(self.display);
            }
        }
    }
}

// Safety: X11 Display can be sent between threads if properly synchronized
unsafe impl Send for X11ViewHost {}
unsafe impl Sync for X11ViewHost {}

#[cfg(test)]
mod tests {
    // X11 tests require a display, which may not be available in CI
    // These are integration tests that should be run manually
}

