//! Core WebKit type definitions
//!
//! These are opaque pointer types that represent WebKit objects.
//! The actual struct layouts are not exposed - we only work with pointers.

use std::ffi::c_void;

/// Type ID for WebKit type identification
pub type WKTypeID = usize;

/// Opaque reference to a WebKit string
pub type WKStringRef = *mut c_void;

/// Opaque reference to a WebKit URL
pub type WKURLRef = *mut c_void;

/// Opaque reference to a WebKit data blob
pub type WKDataRef = *mut c_void;

/// Opaque reference to a WebKit array
pub type WKArrayRef = *mut c_void;

/// Opaque reference to a WebKit dictionary
pub type WKDictionaryRef = *mut c_void;

/// Opaque reference to a WebKit type (base class)
pub type WKTypeRef = *mut c_void;

/// Opaque reference to a WebKit context
pub type WKContextRef = *mut c_void;

/// Opaque reference to a WebKit page group
pub type WKPageGroupRef = *mut c_void;

/// Opaque reference to a WebKit page configuration
pub type WKPageConfigurationRef = *mut c_void;

/// Opaque reference to a WebKit page
pub type WKPageRef = *mut c_void;

/// Opaque reference to a WebKit view (Windows-specific)
pub type WKViewRef = *mut c_void;

/// Opaque reference to a WebKit frame
pub type WKFrameRef = *mut c_void;

/// Opaque reference to a WebKit navigation action
pub type WKNavigationActionRef = *mut c_void;

/// Opaque reference to a WebKit navigation response
pub type WKNavigationResponseRef = *mut c_void;

/// Opaque reference to a WebKit navigation
pub type WKNavigationRef = *mut c_void;

/// Opaque reference to a WebKit back/forward list
pub type WKBackForwardListRef = *mut c_void;

/// Opaque reference to a WebKit back/forward list item
pub type WKBackForwardListItemRef = *mut c_void;

/// Opaque reference to a serialized script value (JavaScript result)
pub type WKSerializedScriptValueRef = *mut c_void;

/// Opaque reference to an error
pub type WKErrorRef = *mut c_void;

/// Opaque reference to a download
pub type WKDownloadRef = *mut c_void;

/// Opaque reference to user content controller
pub type WKUserContentControllerRef = *mut c_void;

/// Opaque reference to user script
pub type WKUserScriptRef = *mut c_void;

/// Opaque reference to preferences
pub type WKPreferencesRef = *mut c_void;

/// Opaque reference to website data store
pub type WKWebsiteDataStoreRef = *mut c_void;

/// Opaque reference to web inspector
pub type WKInspectorRef = *mut c_void;

/// Opaque reference to cookie manager
pub type WKCookieManagerRef = *mut c_void;

/// Opaque reference to website policies
pub type WKWebsitePoliciesRef = *mut c_void;

/// Opaque reference to protection space
pub type WKProtectionSpaceRef = *mut c_void;

/// Opaque reference to authentication challenge
pub type WKAuthenticationChallengeRef = *mut c_void;

/// Opaque reference to a script message (from JS to native)
pub type WKScriptMessageRef = *mut c_void;

/// Opaque reference to a completion listener (for async replies)
pub type WKCompletionListenerRef = *mut c_void;

/// Opaque reference to frame info
pub type WKFrameInfoRef = *mut c_void;

/// Same-document navigation type
pub type WKSameDocumentNavigationType = u32;

// Same-document navigation type constants
pub const K_WK_SAME_DOCUMENT_NAVIGATION_ANCHOR_NAVIGATION: WKSameDocumentNavigationType = 0;
pub const K_WK_SAME_DOCUMENT_NAVIGATION_SESSION_STATE_POP: WKSameDocumentNavigationType = 1;
pub const K_WK_SAME_DOCUMENT_NAVIGATION_SESSION_STATE_REPLACE: WKSameDocumentNavigationType = 2;

/// Size structure for layout
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct WKSize {
    pub width: f64,
    pub height: f64,
}

impl WKSize {
    pub fn new(width: f64, height: f64) -> Self {
        Self { width, height }
    }
}

/// Navigation action policy returned by navigation handlers
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WKNavigationActionPolicy {
    /// Allow the navigation
    Allow = 0,
    /// Cancel the navigation
    Cancel = 1,
    /// Download the content
    Download = 2,
}

/// Navigation response policy
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WKNavigationResponsePolicy {
    /// Allow the response
    Allow = 0,
    /// Cancel the response
    Cancel = 1,
    /// Convert to download
    BecomeDownload = 2,
}

/// Navigation type
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WKNavigationType {
    LinkClicked = 0,
    FormSubmitted = 1,
    BackForward = 2,
    Reload = 3,
    FormResubmitted = 4,
    Other = 5,
}

/// Frame load state
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WKFrameLoadState {
    Provisional = 0,
    Committed = 1,
    Finished = 2,
}

/// User script injection time
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WKUserScriptInjectionTime {
    /// Inject at document start
    AtDocumentStart = 0,
    /// Inject at document end
    AtDocumentEnd = 1,
}

/// User content injected frames
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WKUserContentInjectedFrames {
    /// Inject into all frames
    AllFrames = 0,
    /// Inject only into top frame
    TopFrameOnly = 1,
}
