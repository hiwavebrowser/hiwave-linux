//! Raw FFI bindings to WinCairo WebKit C API
//!
//! This crate provides low-level bindings to the WebKit C API for Windows (WinCairo port).
//! These bindings are unsafe and should be wrapped by a higher-level safe Rust API.
//!
//! # Prerequisites
//!
//! WebKit must be installed and the `WEBKIT_PATH` environment variable must point to the
//! installation directory (default: `C:\WebKit`).
//!
//! # Safety
//!
//! All functions in this crate are unsafe as they interact directly with C code.
//! The caller is responsible for:
//! - Ensuring pointers are valid
//! - Managing memory correctly
//! - Calling functions from the correct thread (usually main/UI thread)

#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(dead_code)]

#[cfg(target_os = "windows")]
pub mod wk_callbacks;
#[cfg(target_os = "windows")]
pub mod wk_context;
#[cfg(target_os = "windows")]
pub mod wk_page;
#[cfg(target_os = "windows")]
pub mod wk_string;
#[cfg(target_os = "windows")]
pub mod wk_types;
#[cfg(target_os = "windows")]
pub mod wk_view;

#[cfg(target_os = "windows")]
pub use wk_callbacks::*;
#[cfg(target_os = "windows")]
pub use wk_context::*;
#[cfg(target_os = "windows")]
pub use wk_page::*;
#[cfg(target_os = "windows")]
pub use wk_string::*;
#[cfg(target_os = "windows")]
pub use wk_types::*;
#[cfg(target_os = "windows")]
pub use wk_view::*;

// Re-export windows types for convenience
#[cfg(target_os = "windows")]
pub use windows_sys::Win32::Foundation::{BOOL, FALSE, HWND, RECT, TRUE};
