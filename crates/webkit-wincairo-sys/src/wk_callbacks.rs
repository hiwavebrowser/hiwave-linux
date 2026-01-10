//! WebKit callback types and client structures
//!
//! WebKit uses a client-based callback system where you register callback
//! functions for various events. This module defines the callback function
//! types and client structures.
//!
//! IMPORTANT: These struct layouts MUST match the WebKit C API headers exactly.
//! The structs are defined in:
//! - WKPageLoaderClient.h
//! - WKPageNavigationClient.h
//! - WKPageUIClient.h
//! - WKPageInjectedBundleClient.h

use super::wk_types::*;
use std::ffi::c_void;

/// Base client structure - all clients have version and clientInfo
#[repr(C)]
pub struct WKClientBase {
    pub version: i32, // Note: int in C, which is i32
    pub client_info: *const c_void,
}

// ============================================================================
// Callback Function Types - Loader Client
// ============================================================================

/// Generic loader client callback (for progress, process state, etc.)
pub type WKPageLoaderClientCallback =
    Option<unsafe extern "C" fn(page: WKPageRef, client_info: *const c_void)>;

/// Callback for provisional load start
pub type WKPageDidStartProvisionalLoadForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for server redirect during provisional load
pub type WKPageDidReceiveServerRedirectForProvisionalLoadForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for provisional load failure
pub type WKPageDidFailProvisionalLoadWithErrorForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        error: WKErrorRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for load commit
pub type WKPageDidCommitLoadForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for document load finish
pub type WKPageDidFinishDocumentLoadForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for load finish
pub type WKPageDidFinishLoadForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for load failure
pub type WKPageDidFailLoadWithErrorForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        error: WKErrorRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for same-document navigation
pub type WKPageDidSameDocumentNavigationForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        navigation_type: WKSameDocumentNavigationType,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for title changes
pub type WKPageDidReceiveTitleForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        title: WKStringRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for first layout
pub type WKPageDidFirstLayoutForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for first visually non-empty layout
pub type WKPageDidFirstVisuallyNonEmptyLayoutForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for frame removal
pub type WKPageDidRemoveFrameFromHierarchyCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for insecure content display
pub type WKPageDidDisplayInsecureContentForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for insecure content run
pub type WKPageDidRunInsecureContentForFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for protection space authentication
pub type WKPageCanAuthenticateAgainstProtectionSpaceInFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        protection_space: WKProtectionSpaceRef,
        client_info: *const c_void,
    ) -> bool,
>;

/// Callback for authentication challenge
pub type WKPageDidReceiveAuthenticationChallengeInFrameCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        frame: WKFrameRef,
        authentication_challenge: WKAuthenticationChallengeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for back/forward list changes
pub type WKPageDidChangeBackForwardListCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        added_item: WKBackForwardListItemRef,
        removed_items: WKArrayRef,
        client_info: *const c_void,
    ),
>;

/// Callback for back/forward navigation decision
pub type WKPageShouldGoToBackForwardListItemCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        item: WKBackForwardListItemRef,
        client_info: *const c_void,
    ) -> bool,
>;

/// Deprecated callback for plugin initialization failure
pub type WKPageDidFailToInitializePluginCallback_deprecatedForUseWithV0 = Option<
    unsafe extern "C" fn(page: WKPageRef, mime_type: WKStringRef, client_info: *const c_void),
>;

// ============================================================================
// Callback Function Types - Navigation Client
// ============================================================================

/// Callback for navigation action policy decision
pub type WKPageNavigationDecidePolicyForNavigationActionCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation_action: WKNavigationActionRef,
        listener: WKFramePolicyListenerRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation response policy decision
pub type WKPageNavigationDecidePolicyForNavigationResponseCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation_response: WKNavigationResponseRef,
        listener: WKFramePolicyListenerRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for plugin load policy
pub type WKPageNavigationDecidePolicyForPluginLoadCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        current_policy: u32, // WKPluginLoadPolicy
        plugin_info: WKDictionaryRef,
        unavailability_description: *mut WKStringRef,
        client_info: *const c_void,
    ) -> u32, // WKPluginLoadPolicy
>;

/// Callback for navigation start
pub type WKPageNavigationDidStartProvisionalNavigationCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for server redirect during navigation
pub type WKPageNavigationDidReceiveServerRedirectCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation failure (provisional)
pub type WKPageNavigationDidFailProvisionalNavigationCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        error: WKErrorRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation commit
pub type WKPageNavigationDidCommitNavigationCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation finish
pub type WKPageNavigationDidFinishNavigationCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation failure
pub type WKPageNavigationDidFailNavigationCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        error: WKErrorRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for subframe provisional load failure
pub type WKPageNavigationDidFailProvisionalLoadInSubframeCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        subframe: WKFrameInfoRef,
        error: WKErrorRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for document load finish
pub type WKPageNavigationDidFinishDocumentLoadCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for same-document navigation
pub type WKPageNavigationDidSameDocumentNavigationCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        navigation: WKNavigationRef,
        navigation_type: WKSameDocumentNavigationType,
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for rendering progress changes
pub type WKPageNavigationRenderingProgressDidChangeCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        progress_events: u32, // WKPageRenderingProgressEvents
        user_data: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for protection space check
pub type WKPageNavigationCanAuthenticateAgainstProtectionSpaceCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        protection_space: WKProtectionSpaceRef,
        client_info: *const c_void,
    ) -> bool,
>;

/// Callback for authentication challenge
pub type WKPageNavigationDidReceiveAuthenticationChallengeCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        challenge: WKAuthenticationChallengeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for web process crash
pub type WKPageNavigationWebProcessDidCrashCallback =
    Option<unsafe extern "C" fn(page: WKPageRef, client_info: *const c_void)>;

/// Callback for copying WebCrypto master key
pub type WKPageNavigationCopyWebCryptoMasterKeyCallback =
    Option<unsafe extern "C" fn(page: WKPageRef, client_info: *const c_void) -> WKDataRef>;

/// Callback for navigation gesture begin
pub type WKPageNavigationDidBeginNavigationGestureCallback =
    Option<unsafe extern "C" fn(page: WKPageRef, client_info: *const c_void)>;

/// Callback for navigation gesture will end
pub type WKPageNavigationWillEndNavigationGestureCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        back_forward_item: WKBackForwardListItemRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation gesture end
pub type WKPageNavigationDidEndNavigationGestureCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        back_forward_item: WKBackForwardListItemRef,
        client_info: *const c_void,
    ),
>;

/// Callback for navigation gesture snapshot removal
pub type WKPageNavigationDidRemoveNavigationGestureSnapshotCallback =
    Option<unsafe extern "C" fn(page: WKPageRef, client_info: *const c_void)>;

// ============================================================================
// Callback Function Types - Injected Bundle Client
// ============================================================================

/// Callback for IPC messages from injected bundle
pub type WKPageDidReceiveMessageFromInjectedBundleCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        message_name: WKStringRef,
        message_body: WKTypeRef,
        client_info: *const c_void,
    ),
>;

/// Callback for synchronous IPC messages from injected bundle
pub type WKPageDidReceiveSynchronousMessageFromInjectedBundleCallback = Option<
    unsafe extern "C" fn(
        page: WKPageRef,
        message_name: WKStringRef,
        message_body: WKTypeRef,
        return_data: *mut WKTypeRef,
        client_info: *const c_void,
    ),
>;

// ============================================================================
// Callback Function Types - JavaScript
// ============================================================================

/// Callback for JavaScript execution results
pub type WKPageRunJavaScriptFunction = Option<
    unsafe extern "C" fn(
        result: WKSerializedScriptValueRef,
        error: WKErrorRef,
        context: *mut c_void,
    ),
>;

// ============================================================================
// Client Structures - Matching WebKit Header Layouts EXACTLY
// ============================================================================

/// Loader client V0 - handles page load events
/// MUST match WKPageLoaderClientV0 in WKPageLoaderClient.h exactly (24 callbacks)
#[repr(C)]
pub struct WKPageLoaderClientV0 {
    pub base: WKClientBase,

    // Version 0 callbacks (24 total)
    pub did_start_provisional_load_for_frame: WKPageDidStartProvisionalLoadForFrameCallback,
    pub did_receive_server_redirect_for_provisional_load_for_frame:
        WKPageDidReceiveServerRedirectForProvisionalLoadForFrameCallback,
    pub did_fail_provisional_load_with_error_for_frame:
        WKPageDidFailProvisionalLoadWithErrorForFrameCallback,
    pub did_commit_load_for_frame: WKPageDidCommitLoadForFrameCallback,
    pub did_finish_document_load_for_frame: WKPageDidFinishDocumentLoadForFrameCallback,
    pub did_finish_load_for_frame: WKPageDidFinishLoadForFrameCallback,
    pub did_fail_load_with_error_for_frame: WKPageDidFailLoadWithErrorForFrameCallback,
    pub did_same_document_navigation_for_frame: WKPageDidSameDocumentNavigationForFrameCallback,
    pub did_receive_title_for_frame: WKPageDidReceiveTitleForFrameCallback,
    pub did_first_layout_for_frame: WKPageDidFirstLayoutForFrameCallback,
    pub did_first_visually_non_empty_layout_for_frame:
        WKPageDidFirstVisuallyNonEmptyLayoutForFrameCallback,
    pub did_remove_frame_from_hierarchy: WKPageDidRemoveFrameFromHierarchyCallback,
    pub did_display_insecure_content_for_frame: WKPageDidDisplayInsecureContentForFrameCallback,
    pub did_run_insecure_content_for_frame: WKPageDidRunInsecureContentForFrameCallback,
    pub can_authenticate_against_protection_space_in_frame:
        WKPageCanAuthenticateAgainstProtectionSpaceInFrameCallback,
    pub did_receive_authentication_challenge_in_frame:
        WKPageDidReceiveAuthenticationChallengeInFrameCallback,

    // Progress callbacks
    pub did_start_progress: WKPageLoaderClientCallback,
    pub did_change_progress: WKPageLoaderClientCallback,
    pub did_finish_progress: WKPageLoaderClientCallback,

    // Process state callbacks
    pub process_did_become_unresponsive: WKPageLoaderClientCallback,
    pub process_did_become_responsive: WKPageLoaderClientCallback,
    pub process_did_crash: WKPageLoaderClientCallback,

    // Back/forward list
    pub did_change_back_forward_list: WKPageDidChangeBackForwardListCallback,
    pub should_go_to_back_forward_list_item: WKPageShouldGoToBackForwardListItemCallback,
    pub did_fail_to_initialize_plugin_deprecated:
        WKPageDidFailToInitializePluginCallback_deprecatedForUseWithV0,
}

impl Default for WKPageLoaderClientV0 {
    fn default() -> Self {
        Self {
            base: WKClientBase {
                version: 0,
                client_info: std::ptr::null(),
            },
            did_start_provisional_load_for_frame: None,
            did_receive_server_redirect_for_provisional_load_for_frame: None,
            did_fail_provisional_load_with_error_for_frame: None,
            did_commit_load_for_frame: None,
            did_finish_document_load_for_frame: None,
            did_finish_load_for_frame: None,
            did_fail_load_with_error_for_frame: None,
            did_same_document_navigation_for_frame: None,
            did_receive_title_for_frame: None,
            did_first_layout_for_frame: None,
            did_first_visually_non_empty_layout_for_frame: None,
            did_remove_frame_from_hierarchy: None,
            did_display_insecure_content_for_frame: None,
            did_run_insecure_content_for_frame: None,
            can_authenticate_against_protection_space_in_frame: None,
            did_receive_authentication_challenge_in_frame: None,
            did_start_progress: None,
            did_change_progress: None,
            did_finish_progress: None,
            process_did_become_unresponsive: None,
            process_did_become_responsive: None,
            process_did_crash: None,
            did_change_back_forward_list: None,
            should_go_to_back_forward_list_item: None,
            did_fail_to_initialize_plugin_deprecated: None,
        }
    }
}

/// Navigation client V0 - handles navigation events
/// MUST match WKPageNavigationClientV0 in WKPageNavigationClient.h exactly (21 callbacks)
#[repr(C)]
pub struct WKPageNavigationClientV0 {
    pub base: WKClientBase,

    // Version 0 callbacks (21 total)
    pub decide_policy_for_navigation_action:
        WKPageNavigationDecidePolicyForNavigationActionCallback,
    pub decide_policy_for_navigation_response:
        WKPageNavigationDecidePolicyForNavigationResponseCallback,
    pub decide_policy_for_plugin_load: WKPageNavigationDecidePolicyForPluginLoadCallback,
    pub did_start_provisional_navigation: WKPageNavigationDidStartProvisionalNavigationCallback,
    pub did_receive_server_redirect_for_provisional_navigation:
        WKPageNavigationDidReceiveServerRedirectCallback,
    pub did_fail_provisional_navigation: WKPageNavigationDidFailProvisionalNavigationCallback,
    pub did_commit_navigation: WKPageNavigationDidCommitNavigationCallback,
    pub did_finish_navigation: WKPageNavigationDidFinishNavigationCallback,
    pub did_fail_navigation: WKPageNavigationDidFailNavigationCallback,
    pub did_fail_provisional_load_in_subframe:
        WKPageNavigationDidFailProvisionalLoadInSubframeCallback,
    pub did_finish_document_load: WKPageNavigationDidFinishDocumentLoadCallback,
    pub did_same_document_navigation: WKPageNavigationDidSameDocumentNavigationCallback,
    pub rendering_progress_did_change: WKPageNavigationRenderingProgressDidChangeCallback,
    pub can_authenticate_against_protection_space:
        WKPageNavigationCanAuthenticateAgainstProtectionSpaceCallback,
    pub did_receive_authentication_challenge:
        WKPageNavigationDidReceiveAuthenticationChallengeCallback,
    pub web_process_did_crash: WKPageNavigationWebProcessDidCrashCallback,
    pub copy_web_crypto_master_key: WKPageNavigationCopyWebCryptoMasterKeyCallback,
    pub did_begin_navigation_gesture: WKPageNavigationDidBeginNavigationGestureCallback,
    pub will_end_navigation_gesture: WKPageNavigationWillEndNavigationGestureCallback,
    pub did_end_navigation_gesture: WKPageNavigationDidEndNavigationGestureCallback,
    pub did_remove_navigation_gesture_snapshot:
        WKPageNavigationDidRemoveNavigationGestureSnapshotCallback,
}

impl Default for WKPageNavigationClientV0 {
    fn default() -> Self {
        Self {
            base: WKClientBase {
                version: 0,
                client_info: std::ptr::null(),
            },
            decide_policy_for_navigation_action: None,
            decide_policy_for_navigation_response: None,
            decide_policy_for_plugin_load: None,
            did_start_provisional_navigation: None,
            did_receive_server_redirect_for_provisional_navigation: None,
            did_fail_provisional_navigation: None,
            did_commit_navigation: None,
            did_finish_navigation: None,
            did_fail_navigation: None,
            did_fail_provisional_load_in_subframe: None,
            did_finish_document_load: None,
            did_same_document_navigation: None,
            rendering_progress_did_change: None,
            can_authenticate_against_protection_space: None,
            did_receive_authentication_challenge: None,
            web_process_did_crash: None,
            copy_web_crypto_master_key: None,
            did_begin_navigation_gesture: None,
            will_end_navigation_gesture: None,
            did_end_navigation_gesture: None,
            did_remove_navigation_gesture_snapshot: None,
        }
    }
}

/// Injected Bundle client V0 - handles IPC messages from injected bundle
/// MUST match WKPageInjectedBundleClientV0 in WKPageInjectedBundleClient.h exactly (2 callbacks)
#[repr(C)]
pub struct WKPageInjectedBundleClientV0 {
    pub base: WKClientBase,

    // Version 0 callbacks (2 total)
    pub did_receive_message_from_injected_bundle: WKPageDidReceiveMessageFromInjectedBundleCallback,
    pub did_receive_synchronous_message_from_injected_bundle:
        WKPageDidReceiveSynchronousMessageFromInjectedBundleCallback,
}

impl Default for WKPageInjectedBundleClientV0 {
    fn default() -> Self {
        Self {
            base: WKClientBase {
                version: 0,
                client_info: std::ptr::null(),
            },
            did_receive_message_from_injected_bundle: None,
            did_receive_synchronous_message_from_injected_bundle: None,
        }
    }
}

// ============================================================================
// Additional Types
// ============================================================================

/// Opaque reference to a URL request
pub type WKURLRequestRef = *mut c_void;

// Note: WKFrameInfoRef is defined in wk_types.rs

/// Opaque reference to a frame policy listener
pub type WKFramePolicyListenerRef = *mut c_void;

// ============================================================================
// External Functions
// ============================================================================

extern "C" {
    // Client registration functions
    pub fn WKPageSetPageLoaderClient(page: WKPageRef, client: *const WKClientBase);
    pub fn WKPageSetPageNavigationClient(page: WKPageRef, client: *const WKClientBase);
    pub fn WKPageSetPageInjectedBundleClient(page: WKPageRef, client: *const WKClientBase);

    // Navigation action functions
    pub fn WKNavigationActionGetTypeID() -> WKTypeID;
    pub fn WKNavigationActionCopyRequest(
        navigation_action: WKNavigationActionRef,
    ) -> WKURLRequestRef;
    pub fn WKNavigationActionCopyTargetFrameInfo(
        navigation_action: WKNavigationActionRef,
    ) -> WKFrameInfoRef;
    pub fn WKNavigationActionGetNavigationType(
        navigation_action: WKNavigationActionRef,
    ) -> WKNavigationType;
    pub fn WKNavigationActionGetShouldOpenExternalSchemes(
        navigation_action: WKNavigationActionRef,
    ) -> bool;
    pub fn WKNavigationActionHasUnconsumedUserGesture(
        navigation_action: WKNavigationActionRef,
    ) -> bool;
    pub fn WKNavigationActionShouldPerformDownload(
        navigation_action: WKNavigationActionRef,
    ) -> bool;

    // URL request functions
    pub fn WKURLRequestGetTypeID() -> WKTypeID;
    pub fn WKURLRequestCopyURL(request: WKURLRequestRef) -> WKURLRef;
    pub fn WKURLRequestCopyFirstPartyForCookies(request: WKURLRequestRef) -> WKURLRef;
    pub fn WKURLRequestCopyHTTPMethod(request: WKURLRequestRef) -> WKStringRef;
    pub fn WKURLRequestCreateWithWKURL(url: WKURLRef) -> WKURLRequestRef;

    // Frame policy listener functions
    pub fn WKFramePolicyListenerUse(listener: WKFramePolicyListenerRef);
    pub fn WKFramePolicyListenerDownload(listener: WKFramePolicyListenerRef);
    pub fn WKFramePolicyListenerIgnore(listener: WKFramePolicyListenerRef);

    // Frame info functions
    pub fn WKFrameInfoGetTypeID() -> WKTypeID;
    pub fn WKFrameInfoGetIsMainFrame(frame_info: WKFrameInfoRef) -> bool;
    pub fn WKFrameInfoGetPage(frame_info: WKFrameInfoRef) -> WKPageRef;
    pub fn WKFrameInfoCopySecurityOrigin(frame_info: WKFrameInfoRef) -> WKTypeRef;
}
