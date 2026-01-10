//! WebKit context and configuration functions
//!
//! WKContext is the main entry point for WebKit. It manages shared resources
//! like the process pool, cookie storage, and caches.
//!
//! Note: Function signatures verified against WebKit2.dll WinCairo exports.

use super::wk_types::*;

/// Opaque reference to context configuration
pub type WKContextConfigurationRef = *mut std::ffi::c_void;

extern "C" {
    // ========== WKContext ==========

    /// Get the type ID of WKContext
    pub fn WKContextGetTypeID() -> WKTypeID;

    /// Create a new WebKit context
    pub fn WKContextCreate() -> WKContextRef;

    /// Create a context with configuration
    pub fn WKContextCreateWithConfiguration(
        configuration: WKContextConfigurationRef,
    ) -> WKContextRef;

    /// Create a context with injected bundle path
    pub fn WKContextCreateWithInjectedBundlePath(bundle_path: WKStringRef) -> WKContextRef;

    /// Set the client for context
    pub fn WKContextSetClient(context: WKContextRef, client: *const std::ffi::c_void);

    /// Set additional plugins directory
    pub fn WKContextSetAdditionalPluginsDirectory(
        context: WKContextRef,
        plugins_directory: WKStringRef,
    );

    /// Set the cache model
    pub fn WKContextSetCacheModel(context: WKContextRef, cache_model: u32);

    /// Get the cache model
    pub fn WKContextGetCacheModel(context: WKContextRef) -> u32;

    /// Get the website data store
    pub fn WKContextGetWebsiteDataStore(context: WKContextRef) -> WKWebsiteDataStoreRef;

    /// Garbage collect JavaScript objects
    pub fn WKContextGarbageCollectJavaScriptObjects(context: WKContextRef);

    /// Post message to injected bundle
    pub fn WKContextPostMessageToInjectedBundle(
        context: WKContextRef,
        message_name: WKStringRef,
        message_body: WKTypeRef,
    );

    // ========== WKContextConfiguration ==========

    /// Create a context configuration
    pub fn WKContextConfigurationCreate() -> WKContextConfigurationRef;

    /// Set the injected bundle path on configuration
    pub fn WKContextConfigurationSetInjectedBundlePath(
        configuration: WKContextConfigurationRef,
        bundle_path: WKStringRef,
    );

    /// Copy the injected bundle path from configuration
    pub fn WKContextConfigurationCopyInjectedBundlePath(
        configuration: WKContextConfigurationRef,
    ) -> WKStringRef;

    // ========== WKPageGroup ==========

    /// Get the type ID of WKPageGroup
    pub fn WKPageGroupGetTypeID() -> WKTypeID;

    /// Create a page group with an identifier
    pub fn WKPageGroupCreateWithIdentifier(identifier: WKStringRef) -> WKPageGroupRef;

    /// Get preferences for page group
    pub fn WKPageGroupGetPreferences(page_group: WKPageGroupRef) -> WKPreferencesRef;

    /// Set preferences for page group
    pub fn WKPageGroupSetPreferences(page_group: WKPageGroupRef, preferences: WKPreferencesRef);

    /// Get user content controller for page group
    pub fn WKPageGroupGetUserContentController(
        page_group: WKPageGroupRef,
    ) -> WKUserContentControllerRef;

    /// Add a user script to page group
    pub fn WKPageGroupAddUserScript(page_group: WKPageGroupRef, user_script: WKUserScriptRef);

    /// Remove all user scripts from page group
    pub fn WKPageGroupRemoveAllUserScripts(page_group: WKPageGroupRef);

    // ========== WKPageConfiguration ==========
    // (Already defined in wk_page.rs)

    // ========== WKUserContentController ==========

    /// Get the type ID of WKUserContentController
    pub fn WKUserContentControllerGetTypeID() -> WKTypeID;

    /// Create a user content controller
    pub fn WKUserContentControllerCreate() -> WKUserContentControllerRef;

    /// Add a user script to the controller
    pub fn WKUserContentControllerAddUserScript(
        controller: WKUserContentControllerRef,
        user_script: WKUserScriptRef,
    );

    /// Remove all user scripts
    pub fn WKUserContentControllerRemoveAllUserScripts(controller: WKUserContentControllerRef);

    /// Copy user scripts from controller
    pub fn WKUserContentControllerCopyUserScripts(
        controller: WKUserContentControllerRef,
    ) -> WKArrayRef;

    /// Add script message handler
    /// Callback signature: fn(message: WKScriptMessageRef, reply: WKCompletionListenerRef, context: *const c_void)
    pub fn WKUserContentControllerAddScriptMessageHandler(
        controller: WKUserContentControllerRef,
        name: WKStringRef,
        callback: Option<
            unsafe extern "C" fn(
                WKScriptMessageRef,
                WKCompletionListenerRef,
                *const std::ffi::c_void,
            ),
        >,
        context: *const std::ffi::c_void,
    );

    /// Remove all script message handlers
    pub fn WKUserContentControllerRemoveAllUserMessageHandlers(
        controller: WKUserContentControllerRef,
    );

    // ========== WKScriptMessage ==========

    /// Get the type ID of WKScriptMessage
    pub fn WKScriptMessageGetTypeID() -> WKTypeID;

    /// Get the message body (usually a WKStringRef or other WKTypeRef)
    pub fn WKScriptMessageGetBody(message: WKScriptMessageRef) -> WKTypeRef;

    /// Get frame info from script message
    pub fn WKScriptMessageGetFrameInfo(message: WKScriptMessageRef) -> WKFrameInfoRef;

    // ========== WKCompletionListener ==========

    /// Complete a completion listener with a result
    pub fn WKCompletionListenerComplete(listener: WKCompletionListenerRef, result: WKTypeRef);

    // ========== WKUserScript ==========

    /// Get the type ID of WKUserScript
    pub fn WKUserScriptGetTypeID() -> WKTypeID;

    /// Create a user script
    /// Note: The actual signature uses individual bool for mainFrameOnly instead of enum
    pub fn WKUserScriptCreateWithSource(
        source: WKStringRef,
        injection_time: WKUserScriptInjectionTime,
        main_frame_only: bool,
    ) -> WKUserScriptRef;

    /// Get the source of a user script
    pub fn WKUserScriptCopySource(user_script: WKUserScriptRef) -> WKStringRef;

    /// Get the injection time of a user script
    pub fn WKUserScriptGetInjectionTime(user_script: WKUserScriptRef) -> WKUserScriptInjectionTime;

    /// Check if script is main frame only
    pub fn WKUserScriptGetMainFrameOnly(user_script: WKUserScriptRef) -> bool;
}

/// Cache model enum
pub mod cache_model {
    /// Document viewer (minimal caching)
    pub const WK_CACHE_MODEL_DOCUMENT_VIEWER: u32 = 0;
    /// Document browser (moderate caching)
    pub const WK_CACHE_MODEL_DOCUMENT_BROWSER: u32 = 1;
    /// Primary web browser (aggressive caching)
    pub const WK_CACHE_MODEL_PRIMARY_WEB_BROWSER: u32 = 2;
}

// ========== WKPreferences ==========

extern "C" {
    /// Get the type ID of WKPreferences
    pub fn WKPreferencesGetTypeID() -> WKTypeID;

    /// Create preferences
    pub fn WKPreferencesCreate() -> WKPreferencesRef;

    /// Set whether JavaScript is enabled
    pub fn WKPreferencesSetJavaScriptEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether JavaScript is enabled
    pub fn WKPreferencesGetJavaScriptEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether developer extras (inspector) are enabled
    pub fn WKPreferencesSetDeveloperExtrasEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether developer extras are enabled
    pub fn WKPreferencesGetDeveloperExtrasEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether accelerated compositing is enabled (REQUIRED for rendering on Windows!)
    pub fn WKPreferencesSetAcceleratedCompositingEnabled(
        preferences: WKPreferencesRef,
        enabled: bool,
    );

    /// Get whether accelerated compositing is enabled
    pub fn WKPreferencesGetAcceleratedCompositingEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether JavaScript can access clipboard
    pub fn WKPreferencesSetJavaScriptCanAccessClipboard(
        preferences: WKPreferencesRef,
        enabled: bool,
    );

    /// Get whether JavaScript can access clipboard
    pub fn WKPreferencesGetJavaScriptCanAccessClipboard(preferences: WKPreferencesRef) -> bool;

    /// Set whether JavaScript can open windows automatically
    pub fn WKPreferencesSetJavaScriptCanOpenWindowsAutomatically(
        preferences: WKPreferencesRef,
        enabled: bool,
    );

    /// Get whether JavaScript can open windows automatically
    pub fn WKPreferencesGetJavaScriptCanOpenWindowsAutomatically(
        preferences: WKPreferencesRef,
    ) -> bool;

    /// Set whether local storage is enabled
    pub fn WKPreferencesSetLocalStorageEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether local storage is enabled
    pub fn WKPreferencesGetLocalStorageEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether databases are enabled
    pub fn WKPreferencesSetDatabasesEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether databases are enabled
    pub fn WKPreferencesGetDatabasesEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether fullscreen is enabled
    pub fn WKPreferencesSetFullScreenEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether fullscreen is enabled
    pub fn WKPreferencesGetFullScreenEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether WebGL is enabled
    pub fn WKPreferencesSetWebGLEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether WebGL is enabled
    pub fn WKPreferencesGetWebGLEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether media source is enabled
    pub fn WKPreferencesSetMediaSourceEnabled(preferences: WKPreferencesRef, enabled: bool);

    /// Get whether media source is enabled
    pub fn WKPreferencesGetMediaSourceEnabled(preferences: WKPreferencesRef) -> bool;

    /// Set whether media playback requires user gesture
    pub fn WKPreferencesSetMediaPlaybackRequiresUserGesture(
        preferences: WKPreferencesRef,
        requires: bool,
    );

    /// Get whether media playback requires user gesture
    pub fn WKPreferencesGetMediaPlaybackRequiresUserGesture(preferences: WKPreferencesRef) -> bool;

    /// Set the default font size
    pub fn WKPreferencesSetDefaultFontSize(preferences: WKPreferencesRef, size: u32);

    /// Get the default font size
    pub fn WKPreferencesGetDefaultFontSize(preferences: WKPreferencesRef) -> u32;

    /// Set the minimum font size
    pub fn WKPreferencesSetMinimumFontSize(preferences: WKPreferencesRef, size: u32);

    /// Get the minimum font size
    pub fn WKPreferencesGetMinimumFontSize(preferences: WKPreferencesRef) -> u32;
}
