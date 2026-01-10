//! WebKit string handling functions
//!
//! WKString is WebKit's internal string type. These functions allow
//! conversion between Rust strings and WKString.

use super::wk_types::*;
use std::ffi::c_char;

extern "C" {
    /// Create a WKString from a UTF-8 C string
    pub fn WKStringCreateWithUTF8CString(string: *const c_char) -> WKStringRef;

    /// Get the length of a WKString in UTF-8 bytes
    pub fn WKStringGetLength(string: WKStringRef) -> usize;

    /// Get the maximum UTF-8 buffer size needed
    pub fn WKStringGetMaximumUTF8CStringSize(string: WKStringRef) -> usize;

    /// Copy WKString contents to a UTF-8 buffer
    /// Returns the number of bytes written (including null terminator)
    pub fn WKStringGetUTF8CString(
        string: WKStringRef,
        buffer: *mut c_char,
        buffer_size: usize,
    ) -> usize;

    /// Check if a WKString is empty
    pub fn WKStringIsEmpty(string: WKStringRef) -> bool;

    /// Check if two WKStrings are equal
    pub fn WKStringIsEqual(a: WKStringRef, b: WKStringRef) -> bool;

    /// Check if a WKString equals a UTF-8 C string
    pub fn WKStringIsEqualToUTF8CString(a: WKStringRef, string: *const c_char) -> bool;

    /// Retain (increment reference count)
    pub fn WKRetain(ref_: WKTypeRef) -> WKTypeRef;

    /// Release (decrement reference count)
    pub fn WKRelease(ref_: WKTypeRef);

    /// Create a URL from a UTF-8 C string
    pub fn WKURLCreateWithUTF8CString(string: *const c_char) -> WKURLRef;

    /// Get the string representation of a URL
    pub fn WKURLCopyString(url: WKURLRef) -> WKStringRef;

    /// Copy the URL host
    pub fn WKURLCopyHostName(url: WKURLRef) -> WKStringRef;

    /// Copy the URL path
    pub fn WKURLCopyPath(url: WKURLRef) -> WKStringRef;

    /// Copy the URL scheme
    pub fn WKURLCopyScheme(url: WKURLRef) -> WKStringRef;
}

/// Helper to create a WKString from a Rust &str
///
/// # Safety
/// The returned WKStringRef must be released with WKRelease when no longer needed.
#[inline]
pub unsafe fn wk_string_from_str(s: &str) -> WKStringRef {
    let c_string = std::ffi::CString::new(s).unwrap_or_default();
    WKStringCreateWithUTF8CString(c_string.as_ptr())
}

/// Helper to convert a WKString to a Rust String
///
/// # Safety
/// The caller must ensure the WKStringRef is valid.
#[inline]
pub unsafe fn wk_string_to_string(wk_str: WKStringRef) -> Option<String> {
    if wk_str.is_null() {
        return None;
    }

    let size = WKStringGetMaximumUTF8CStringSize(wk_str);
    if size == 0 {
        return Some(String::new());
    }

    let mut buffer = vec![0i8; size];
    let written = WKStringGetUTF8CString(wk_str, buffer.as_mut_ptr(), size);

    if written == 0 {
        return Some(String::new());
    }

    // Convert buffer to string (exclude null terminator)
    let bytes: Vec<u8> = buffer[..written - 1].iter().map(|&b| b as u8).collect();

    String::from_utf8(bytes).ok()
}

/// Helper to create a WKURL from a Rust &str
///
/// # Safety
/// The returned WKURLRef must be released with WKRelease when no longer needed.
#[inline]
pub unsafe fn wk_url_from_str(s: &str) -> WKURLRef {
    let c_string = std::ffi::CString::new(s).unwrap_or_default();
    WKURLCreateWithUTF8CString(c_string.as_ptr())
}

/// Helper to convert a WKURL to a Rust String
///
/// # Safety
/// The caller must ensure the WKURLRef is valid.
#[inline]
pub unsafe fn wk_url_to_string(wk_url: WKURLRef) -> Option<String> {
    if wk_url.is_null() {
        return None;
    }

    let wk_str = WKURLCopyString(wk_url);
    let result = wk_string_to_string(wk_str);
    WKRelease(wk_str);
    result
}
