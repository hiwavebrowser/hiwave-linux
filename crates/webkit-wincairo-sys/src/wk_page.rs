//! WebKit page functions
//!
//! WKPage represents a web page. It provides methods for navigation,
//! JavaScript execution, and handling page events.
//!
//! Note: Function signatures verified against WebKit2.dll WinCairo exports.

use super::wk_callbacks::WKPageRunJavaScriptFunction;
use super::wk_types::*;
use std::ffi::c_void;

extern "C" {
    // ========== Type Info ==========

    /// Get the WKPage type ID
    pub fn WKPageGetTypeID() -> WKTypeID;

    // ========== Navigation ==========

    /// Load a URL in the page
    pub fn WKPageLoadURL(page: WKPageRef, url: WKURLRef);

    /// Load a URL request
    pub fn WKPageLoadURLRequest(page: WKPageRef, url_request: WKTypeRef);

    /// Load a URL with user data
    pub fn WKPageLoadURLWithUserData(page: WKPageRef, url: WKURLRef, user_data: WKTypeRef);

    /// Load HTML string with a base URL
    pub fn WKPageLoadHTMLString(page: WKPageRef, html_string: WKStringRef, base_url: WKURLRef);

    /// Load HTML string with user data
    pub fn WKPageLoadHTMLStringWithUserData(
        page: WKPageRef,
        html_string: WKStringRef,
        base_url: WKURLRef,
        user_data: WKTypeRef,
    );

    /// Load plain text
    pub fn WKPageLoadPlainTextString(page: WKPageRef, text: WKStringRef);

    /// Load data with MIME type
    pub fn WKPageLoadData(
        page: WKPageRef,
        data: WKDataRef,
        mime_type: WKStringRef,
        encoding: WKStringRef,
        base_url: WKURLRef,
    );

    /// Reload the page
    pub fn WKPageReload(page: WKPageRef);

    /// Reload the page, revalidating content data
    pub fn WKPageReloadFromOrigin(page: WKPageRef);

    /// Reload without content blockers
    pub fn WKPageReloadWithoutContentBlockers(page: WKPageRef);

    /// Reload expired content only
    pub fn WKPageReloadExpiredOnly(page: WKPageRef);

    /// Stop loading
    pub fn WKPageStopLoading(page: WKPageRef);

    /// Go back in history
    pub fn WKPageGoBack(page: WKPageRef);

    /// Can we go back?
    pub fn WKPageCanGoBack(page: WKPageRef) -> bool;

    /// Go forward in history
    pub fn WKPageGoForward(page: WKPageRef);

    /// Can we go forward?
    pub fn WKPageCanGoForward(page: WKPageRef) -> bool;

    /// Get the back-forward list
    pub fn WKPageGetBackForwardList(page: WKPageRef) -> WKBackForwardListRef;

    /// Go to a specific back-forward list item
    pub fn WKPageGoToBackForwardListItem(page: WKPageRef, item: WKBackForwardListItemRef);

    /// Close the page
    pub fn WKPageClose(page: WKPageRef);

    /// Check if page is closed
    pub fn WKPageIsClosed(page: WKPageRef) -> bool;

    /// Try to close the page (may trigger beforeunload)
    pub fn WKPageTryClose(page: WKPageRef);

    /// Terminate the page process
    pub fn WKPageTerminate(page: WKPageRef);

    // ========== Page Info ==========

    /// Get the page context
    pub fn WKPageGetContext(page: WKPageRef) -> WKContextRef;

    /// Get the page group
    pub fn WKPageGetPageGroup(page: WKPageRef) -> WKPageGroupRef;

    /// Get the main frame
    pub fn WKPageGetMainFrame(page: WKPageRef) -> WKFrameRef;

    /// Get the focused frame
    pub fn WKPageGetFocusedFrame(page: WKPageRef) -> WKFrameRef;

    /// Copy the active URL (currently displayed URL)
    pub fn WKPageCopyActiveURL(page: WKPageRef) -> WKURLRef;

    /// Copy the committed URL
    pub fn WKPageCopyCommittedURL(page: WKPageRef) -> WKURLRef;

    /// Copy the provisional URL (URL being loaded)
    pub fn WKPageCopyProvisionalURL(page: WKPageRef) -> WKURLRef;

    /// Get the page title
    pub fn WKPageCopyTitle(page: WKPageRef) -> WKStringRef;

    /// Get the estimated loading progress (0.0 to 1.0)
    pub fn WKPageGetEstimatedProgress(page: WKPageRef) -> f64;

    /// Get the user agent
    pub fn WKPageCopyUserAgent(page: WKPageRef) -> WKStringRef;

    /// Get the custom user agent
    pub fn WKPageCopyCustomUserAgent(page: WKPageRef) -> WKStringRef;

    /// Set the custom user agent
    pub fn WKPageSetCustomUserAgent(page: WKPageRef, user_agent: WKStringRef);

    /// Copy the application name for user agent
    pub fn WKPageCopyApplicationNameForUserAgent(page: WKPageRef) -> WKStringRef;

    /// Set the application name for user agent
    pub fn WKPageSetApplicationNameForUserAgent(page: WKPageRef, name: WKStringRef);

    /// Copy the page configuration
    pub fn WKPageCopyPageConfiguration(page: WKPageRef) -> WKPageConfigurationRef;

    /// Get the inspector
    pub fn WKPageGetInspector(page: WKPageRef) -> WKInspectorRef;

    // ========== JavaScript ==========

    /// Evaluate JavaScript in the main frame
    /// The callback will be called with the result
    pub fn WKPageEvaluateJavaScriptInMainFrame(
        page: WKPageRef,
        script: WKStringRef,
        context: *mut c_void,
        callback: WKPageRunJavaScriptFunction,
    );

    /// Evaluate JavaScript in a specific frame
    pub fn WKPageEvaluateJavaScriptInFrame(
        page: WKPageRef,
        frame: WKFrameRef,
        script: WKStringRef,
        context: *mut c_void,
        callback: WKPageRunJavaScriptFunction,
    );

    /// Call async JavaScript (newer API)
    pub fn WKPageCallAsyncJavaScript(
        page: WKPageRef,
        frame: WKFrameRef,
        script: WKStringRef,
        arguments: WKTypeRef,
        force_user_gesture: bool,
        context: *mut c_void,
        callback: WKPageRunJavaScriptFunction,
    );

    // ========== Client Setup ==========
    // Note: Main client registration functions are in wk_callbacks.rs
    // These are additional clients that use opaque pointers

    /// Set the page UI client (handles UI events like alerts, new windows)
    /// Note: We don't have the full struct for this yet, so using c_void
    pub fn WKPageSetPageUIClient(page: WKPageRef, client: *const c_void);

    /// Set the page policy client (handles navigation decisions)
    /// Note: We don't have the full struct for this yet, so using c_void
    pub fn WKPageSetPagePolicyClient(page: WKPageRef, client: *const c_void);

    /// Set the page find client
    pub fn WKPageSetPageFindClient(page: WKPageRef, client: *const c_void);

    /// Set the page find matches client
    pub fn WKPageSetPageFindMatchesClient(page: WKPageRef, client: *const c_void);

    /// Set the page context menu client
    pub fn WKPageSetPageContextMenuClient(page: WKPageRef, client: *const c_void);

    /// Set the page form client
    pub fn WKPageSetPageFormClient(page: WKPageRef, client: *const c_void);

    /// Set the page diagnostic logging client
    pub fn WKPageSetPageDiagnosticLoggingClient(page: WKPageRef, client: *const c_void);

    // ========== Zoom ==========

    /// Set the page zoom factor
    pub fn WKPageSetPageZoomFactor(page: WKPageRef, zoom_factor: f64);

    /// Get the page zoom factor
    pub fn WKPageGetPageZoomFactor(page: WKPageRef) -> f64;

    /// Set the text zoom factor
    pub fn WKPageSetTextZoomFactor(page: WKPageRef, zoom_factor: f64);

    /// Get the text zoom factor
    pub fn WKPageGetTextZoomFactor(page: WKPageRef) -> f64;

    /// Check if text zoom is supported
    pub fn WKPageSupportsTextZoom(page: WKPageRef) -> bool;

    /// Set both page and text zoom factors
    pub fn WKPageSetPageAndTextZoomFactors(page: WKPageRef, page_zoom: f64, text_zoom: f64);

    /// Get the scale factor
    pub fn WKPageGetScaleFactor(page: WKPageRef) -> f64;

    /// Set the scale factor
    pub fn WKPageSetScaleFactor(page: WKPageRef, scale: f64, origin_x: i32, origin_y: i32);

    /// Get the backing scale factor
    pub fn WKPageGetBackingScaleFactor(page: WKPageRef) -> f64;

    /// Set custom backing scale factor
    pub fn WKPageSetCustomBackingScaleFactor(page: WKPageRef, scale: f64);

    // ========== Find ==========

    /// Find text in the page
    pub fn WKPageFindString(
        page: WKPageRef,
        string: WKStringRef,
        find_options: u32,
        max_match_count: u32,
    );

    /// Hide find UI
    pub fn WKPageHideFindUI(page: WKPageRef);

    /// Count matches for a string
    pub fn WKPageCountStringMatches(
        page: WKPageRef,
        string: WKStringRef,
        find_options: u32,
        max_match_count: u32,
    );

    /// Find all string matches
    pub fn WKPageFindStringMatches(
        page: WKPageRef,
        string: WKStringRef,
        find_options: u32,
        max_match_count: u32,
    );

    // ========== Editing Commands ==========

    /// Execute a command (like "copy", "paste", "selectAll")
    pub fn WKPageExecuteCommand(page: WKPageRef, command_name: WKStringRef, argument: WKStringRef);

    /// Validate a command (check if it can be executed)
    pub fn WKPageValidateCommand(
        page: WKPageRef,
        command_name: WKStringRef,
        context: *mut c_void,
        callback: extern "C" fn(*mut c_void, WKStringRef, bool, i32, WKTypeRef),
    );

    /// Check if can delete
    pub fn WKPageCanDelete(page: WKPageRef) -> bool;

    /// Check if has selected range
    pub fn WKPageHasSelectedRange(page: WKPageRef) -> bool;

    /// Check if content is editable
    pub fn WKPageIsContentEditable(page: WKPageRef) -> bool;

    // ========== Print ==========

    /// Begin printing
    pub fn WKPageBeginPrinting(page: WKPageRef, frame: WKFrameRef, print_info: WKTypeRef);

    /// End printing
    pub fn WKPageEndPrinting(page: WKPageRef);

    /// Compute pages for printing
    pub fn WKPageComputePagesForPrinting(
        page: WKPageRef,
        frame: WKFrameRef,
        print_info: WKTypeRef,
        context: *mut c_void,
        callback: extern "C" fn(*mut c_void, *const c_void, u32, f64, WKTypeRef),
    );

    // ========== Scrollbars ==========

    /// Check if horizontal scrollbar exists
    pub fn WKPageHasHorizontalScrollbar(page: WKPageRef) -> bool;

    /// Check if vertical scrollbar exists
    pub fn WKPageHasVerticalScrollbar(page: WKPageRef) -> bool;

    /// Suppress scrollbar animations
    pub fn WKPageSetSuppressScrollbarAnimations(page: WKPageRef, suppress: bool);

    /// Check if scrollbar animations are suppressed
    pub fn WKPageAreScrollbarAnimationsSuppressed(page: WKPageRef) -> bool;

    // ========== Layout ==========

    /// Use fixed layout mode
    pub fn WKPageUseFixedLayout(page: WKPageRef) -> bool;

    /// Set fixed layout mode
    pub fn WKPageSetUseFixedLayout(page: WKPageRef, use_fixed: bool);

    /// Get the fixed layout size
    pub fn WKPageFixedLayoutSize(page: WKPageRef) -> WKSize;

    /// Set the fixed layout size
    pub fn WKPageSetFixedLayoutSize(page: WKPageRef, size: WKSize);

    /// Listen for layout milestones
    pub fn WKPageListenForLayoutMilestones(page: WKPageRef, milestones: u32);

    // ========== Media ==========

    /// Set media volume
    pub fn WKPageSetMediaVolume(page: WKPageRef, volume: f32);

    /// Set muted
    pub fn WKPageSetMuted(page: WKPageRef, muted_state: u32);

    /// Check if page is playing audio
    pub fn WKPageIsPlayingAudio(page: WKPageRef) -> bool;

    /// Get media state
    pub fn WKPageGetMediaState(page: WKPageRef) -> u32;

    /// Set media capture enabled
    pub fn WKPageSetMediaCaptureEnabled(page: WKPageRef, enabled: bool);

    /// Get media capture enabled
    pub fn WKPageGetMediaCaptureEnabled(page: WKPageRef) -> bool;

    // ========== Misc ==========

    /// Force a repaint
    pub fn WKPageForceRepaint(
        page: WKPageRef,
        context: *mut c_void,
        callback: extern "C" fn(*mut c_void, WKTypeRef),
    );

    /// Set allows remote inspection (Web Inspector)
    pub fn WKPageSetAllowsRemoteInspection(page: WKPageRef, allow: bool);

    /// Get allows remote inspection
    pub fn WKPageGetAllowsRemoteInspection(page: WKPageRef) -> bool;

    /// Try to restore scroll position
    pub fn WKPageTryRestoreScrollPosition(page: WKPageRef);

    /// Update website policies
    pub fn WKPageUpdateWebsitePolicies(page: WKPageRef, policies: WKTypeRef);

    /// Background extends beyond page
    pub fn WKPageBackgroundExtendsBeyondPage(page: WKPageRef) -> bool;

    /// Set background extends beyond page
    pub fn WKPageSetBackgroundExtendsBeyondPage(page: WKPageRef, extends: bool);

    /// Get process identifier
    pub fn WKPageGetProcessIdentifier(page: WKPageRef) -> u32;
}

// ========== Frame Functions ==========

extern "C" {
    /// Get the frame type ID
    pub fn WKFrameGetTypeID() -> WKTypeID;

    /// Check if a frame is the main frame
    pub fn WKFrameIsMainFrame(frame: WKFrameRef) -> bool;

    /// Get the URL of a frame
    pub fn WKFrameCopyURL(frame: WKFrameRef) -> WKURLRef;

    /// Get the provisional URL of a frame (during navigation)
    pub fn WKFrameCopyProvisionalURL(frame: WKFrameRef) -> WKURLRef;

    /// Get the title of a frame
    pub fn WKFrameCopyTitle(frame: WKFrameRef) -> WKStringRef;

    /// Get the MIME type
    pub fn WKFrameCopyMIMEType(frame: WKFrameRef) -> WKStringRef;

    /// Get the unreachable URL
    pub fn WKFrameCopyUnreachableURL(frame: WKFrameRef) -> WKURLRef;

    /// Get the frame load state
    pub fn WKFrameGetFrameLoadState(frame: WKFrameRef) -> u32;

    /// Get the page for this frame
    pub fn WKFrameGetPage(frame: WKFrameRef) -> WKPageRef;

    /// Check if frame can provide source
    pub fn WKFrameCanProvideSource(frame: WKFrameRef) -> bool;

    /// Check if frame can show MIME type
    pub fn WKFrameCanShowMIMEType(frame: WKFrameRef, mime_type: WKStringRef) -> bool;

    /// Stop loading the frame
    pub fn WKFrameStopLoading(frame: WKFrameRef);
}

// ========== Back/Forward List Functions ==========

extern "C" {
    /// Get the back-forward list type ID
    pub fn WKBackForwardListGetTypeID() -> WKTypeID;

    /// Get the current item in the back-forward list
    pub fn WKBackForwardListGetCurrentItem(list: WKBackForwardListRef) -> WKBackForwardListItemRef;

    /// Get the back item
    pub fn WKBackForwardListGetBackItem(list: WKBackForwardListRef) -> WKBackForwardListItemRef;

    /// Get the forward item
    pub fn WKBackForwardListGetForwardItem(list: WKBackForwardListRef) -> WKBackForwardListItemRef;

    /// Get item at index
    pub fn WKBackForwardListGetItemAtIndex(
        list: WKBackForwardListRef,
        index: i32,
    ) -> WKBackForwardListItemRef;

    /// Get the back list count
    pub fn WKBackForwardListGetBackListCount(list: WKBackForwardListRef) -> u32;

    /// Get the forward list count
    pub fn WKBackForwardListGetForwardListCount(list: WKBackForwardListRef) -> u32;

    /// Clear the back-forward list
    pub fn WKBackForwardListClear(list: WKBackForwardListRef);
}

extern "C" {
    /// Get the back-forward list item type ID
    pub fn WKBackForwardListItemGetTypeID() -> WKTypeID;

    /// Get the URL of a back-forward list item
    pub fn WKBackForwardListItemCopyURL(item: WKBackForwardListItemRef) -> WKURLRef;

    /// Get the original URL of a back-forward list item
    pub fn WKBackForwardListItemCopyOriginalURL(item: WKBackForwardListItemRef) -> WKURLRef;

    /// Get the title of a back-forward list item
    pub fn WKBackForwardListItemCopyTitle(item: WKBackForwardListItemRef) -> WKStringRef;
}

// ========== Page Configuration Functions ==========

extern "C" {
    /// Get the page configuration type ID
    pub fn WKPageConfigurationGetTypeID() -> WKTypeID;

    /// Create a page configuration
    pub fn WKPageConfigurationCreate() -> WKPageConfigurationRef;

    /// Get the context from configuration
    pub fn WKPageConfigurationGetContext(config: WKPageConfigurationRef) -> WKContextRef;

    /// Set the context on configuration
    pub fn WKPageConfigurationSetContext(config: WKPageConfigurationRef, context: WKContextRef);

    /// Get the page group from configuration
    pub fn WKPageConfigurationGetPageGroup(config: WKPageConfigurationRef) -> WKPageGroupRef;

    /// Set the page group on configuration
    pub fn WKPageConfigurationSetPageGroup(config: WKPageConfigurationRef, group: WKPageGroupRef);

    /// Get the preferences from configuration
    pub fn WKPageConfigurationGetPreferences(config: WKPageConfigurationRef) -> WKPreferencesRef;

    /// Set the preferences on configuration
    pub fn WKPageConfigurationSetPreferences(
        config: WKPageConfigurationRef,
        prefs: WKPreferencesRef,
    );

    /// Get the user content controller from configuration
    pub fn WKPageConfigurationGetUserContentController(
        config: WKPageConfigurationRef,
    ) -> WKUserContentControllerRef;

    /// Set the user content controller on configuration
    pub fn WKPageConfigurationSetUserContentController(
        config: WKPageConfigurationRef,
        controller: WKUserContentControllerRef,
    );

    /// Get the related page
    pub fn WKPageConfigurationGetRelatedPage(config: WKPageConfigurationRef) -> WKPageRef;

    /// Set the related page
    pub fn WKPageConfigurationSetRelatedPage(config: WKPageConfigurationRef, page: WKPageRef);

    /// Get the website data store
    pub fn WKPageConfigurationGetWebsiteDataStore(
        config: WKPageConfigurationRef,
    ) -> WKWebsiteDataStoreRef;

    /// Set the website data store
    pub fn WKPageConfigurationSetWebsiteDataStore(
        config: WKPageConfigurationRef,
        store: WKWebsiteDataStoreRef,
    );
}

// Note: Page Group functions are defined in wk_context.rs

/// Find options flags
pub mod find_options {
    pub const WK_FIND_OPTIONS_CASE_INSENSITIVE: u32 = 1 << 0;
    pub const WK_FIND_OPTIONS_AT_WORD_STARTS: u32 = 1 << 1;
    pub const WK_FIND_OPTIONS_TREAT_MEDIA_CAPTIONS_AS_TEXT: u32 = 1 << 2;
    pub const WK_FIND_OPTIONS_BACKWARDS: u32 = 1 << 3;
    pub const WK_FIND_OPTIONS_WRAP_AROUND: u32 = 1 << 4;
    pub const WK_FIND_OPTIONS_SHOW_OVERLAY: u32 = 1 << 5;
    pub const WK_FIND_OPTIONS_SHOW_FIND_INDICATOR: u32 = 1 << 6;
    pub const WK_FIND_OPTIONS_SHOW_HIGHLIGHT: u32 = 1 << 7;
}
