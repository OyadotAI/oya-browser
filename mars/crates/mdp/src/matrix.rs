/**
 * The conformance matrix: for every command in the pinned protocol, whether Mars implements it
 * natively, accepts it without effect, cannot apply it (and why), or has not built it yet. The
 * session's dispatch table is the source of truth for "implemented"; this module renders the
 * table and the reasons into `docs/cdp.md` and `/json/protocol`.
 */
use crate::catalog::Catalog;
use std::collections::BTreeMap;

/** Where one command stands. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Translated into native Mars operations with Chrome's semantics.
    Implemented,
    /// Accepted and answered, but nothing to switch: Mars already behaves that way or has no
    /// such state (enables of domains whose events Mars sends regardless, and the like).
    Accepted,
    /// Cannot apply to Mars; the reason says what Mars lacks or refuses by design.
    NotApplicable(&'static str),
    /// Not built yet.
    NotYet,
}

impl Status {
    /** The column text. */
    pub fn label(self) -> &'static str {
        match self {
            Self::Implemented => "Implemented natively",
            Self::Accepted => "Accepted (no effect)",
            Self::NotApplicable(_) => "Not applicable",
            Self::NotYet => "Not yet",
        }
    }
}

/// Reasons shared by whole domains, so the matrix explains groups once.
const NO_DEBUGGER: &str = "Mars has no JavaScript debugger: V9 runs pages, it does not pause them";
const NO_PROFILER: &str = "no sampling profiler or heap snapshotter in V9";
const NO_FRAMES: &str = "Mars renders one document per tab; there are no frames or workers";
const NO_CSS_MODEL: &str =
    "Eye's cascade is internal; stylesheets are not exposed as editable objects";
const NO_SERVICE_WORKERS: &str = "no service workers, cache storage or background services";
const NO_DEVICES: &str = "no device, sensor, Bluetooth, WebAuthn or payment emulation";
const NO_OVERLAY: &str = "no DevTools overlay, inspect mode or paint highlighting";
const NO_TRACING: &str = "no Chrome tracing or layer tree";
const NO_MEDIA: &str = "no media playback, WebAudio or casting";
const NO_STORAGE_INSPECTION: &str = "IndexedDB, Cache Storage and DOM Storage are not inspected";
const NO_EXTENSIONS: &str = "no extensions, PWAs or autofill";
const SINGLE_PROFILE: &str = "one profile per browser; Mars has no browser contexts";
const NO_PDF: &str = "no print pipeline";
const NO_NETWORK_SHAPING: &str = "Relay does not throttle or fake network conditions";
const NO_CERT_OVERRIDE: &str =
    "Relay refuses invalid certificates; the policy cannot be switched off";
const NO_DIALOGS: &str = "V9 answers alert, confirm and prompt itself; no dialog is ever showing";
const NO_PERMISSIONS: &str = "no permission prompts or geolocation";
const NO_FILE_INPUT: &str = "file inputs cannot be filled through Mars";
const NO_SUBRESOURCE_PAUSE: &str = "subresource requests cannot pause; only navigations do";
const NO_AUTH: &str = "no HTTP authentication challenges";
const NO_SCREENCAST: &str = "no screencast or screen recording";
const NO_PRELOAD: &str = "no speculation rules or prerendering";

/// What differs from Chrome by design or by what the engines expose today; the document says it
/// once, above the tables.
const LIMITS: &str = "## Where Mars differs\n\n- **Ownership.** Every mutation (navigation, input, script evaluation, attribute writes, cookies, emulation) carries the control epoch this connection observed. When the person takes a tab back, the next mutation fails with `control_denied: ...`; reads keep answering. An evaluation on a tab the connection does not hold runs in place and keeps nothing, so its remote objects do not outlive the call.\n- **One realm per page.** `Page.createIsolatedWorld` announces a context, but it shares the page's realm with the main world: a client's objects are visible to page scripts of the same page. Scripts registered with `Page.addScriptToEvaluateOnNewDocument` run right after each commit, after the page's own scripts.\n- **Client code is page code.** Evaluated expressions and function declarations are appended to the page's script list, compiled at a stable offset, and dropped again when they left no live function or global behind. They never count against the page's own `eval` budget.\n- **Navigations commit whole.** Every event of a navigation (request, response, frameNavigated, contexts, lifecycle through `networkIdle`) is sent before the command's answer. Subresource requests are not reported on the Network domain, and `Fetch` pauses navigation requests only: a `fetch()` or image inside the page cannot be intercepted yet (it needs a pause point in the runtime's script-request path).\n- **Geometry is Eye's.** `DOM.getBoxModel`, `getContentQuads` and `scrollIntoViewIfNeeded` answer from the painted display. `getBoundingClientRect`, `elementFromPoint`, `offsetWidth` and `document.activeElement` inside V9 still answer without layout or focus, so Playwright's default actionability checks (visible, receives events) time out; `force: true`, `state: 'attached'` and Puppeteer's point-based clicks work. Input.insertText goes to the node a click or `DOM.focus` focused, else to the element the client last passed to `Runtime.callFunctionOn`.\n- **Shared viewport.** `Emulation.setDeviceMetricsOverride` and `setEmulatedMedia` change the browser's one viewport and colour scheme, for every tab. `setUserAgentOverride`, `setTimezoneOverride` and `setLocaleOverride` change what the page's scripts read from the next step on; the request headers Relay sends are unchanged, and `Network.setExtraHTTPHeaders` is recorded, not sent.\n- **Screenshots** are PNG, of the viewport or a clip cut from it. **Cookies** are listed with HttpOnly values for the client, as Chrome does; page scripts never see them.\n\n";

/** Domains refused as a whole, with the reason. */
const DOMAINS: &[(&str, &str)] = &[
    ("Debugger", NO_DEBUGGER),
    ("Profiler", NO_PROFILER),
    ("HeapProfiler", NO_PROFILER),
    ("CSS", NO_CSS_MODEL),
    ("Animation", NO_CSS_MODEL),
    ("LayerTree", NO_TRACING),
    ("Tracing", NO_TRACING),
    ("Overlay", NO_OVERLAY),
    ("ServiceWorker", NO_SERVICE_WORKERS),
    ("CacheStorage", NO_SERVICE_WORKERS),
    ("BackgroundService", NO_SERVICE_WORKERS),
    ("IndexedDB", NO_STORAGE_INSPECTION),
    ("DOMStorage", NO_STORAGE_INSPECTION),
    ("Media", NO_MEDIA),
    ("WebAudio", NO_MEDIA),
    ("Cast", NO_MEDIA),
    ("WebAuthn", NO_DEVICES),
    ("BluetoothEmulation", NO_DEVICES),
    ("DeviceAccess", NO_DEVICES),
    ("DeviceOrientation", NO_DEVICES),
    ("SmartCardEmulation", NO_DEVICES),
    ("DigitalCredentials", NO_DEVICES),
    ("FedCm", NO_DEVICES),
    ("Extensions", NO_EXTENSIONS),
    ("PWA", NO_EXTENSIONS),
    ("Autofill", NO_EXTENSIONS),
    ("Ads", "no ad tagging"),
    (
        "Audits",
        "no issue auditing; Log carries the diagnostics Mars has",
    ),
    ("CrashReportContext", "no crash reporting"),
    ("DOMDebugger", NO_DEBUGGER),
    ("EventBreakpoints", NO_DEBUGGER),
    (
        "HeadlessExperimental",
        "no BeginFrame control; Mars lays out when a command needs it",
    ),
    ("Memory", "no memory pressure simulation or sampling"),
    ("PerformanceTimeline", "no PerformanceObserver timeline"),
    ("Preload", NO_PRELOAD),
    ("SystemInfo", "no GPU or process inquiry"),
    ("Tethering", "no port tethering"),
    ("WebMCP", "no in-page WebMCP tools"),
    ("FileSystem", "no file system access"),
    ("IO", "no stream handles; bodies come back inline"),
];

/** Single commands refused with a reason of their own. */
const COMMANDS: &[(&str, &str)] = &[
    ("Page.printToPDF", NO_PDF),
    ("Page.handleJavaScriptDialog", NO_DIALOGS),
    ("Page.startScreencast", NO_SCREENCAST),
    ("Page.stopScreencast", NO_SCREENCAST),
    ("Page.screencastFrameAck", NO_SCREENCAST),
    ("Page.startScreenRecording", NO_SCREENCAST),
    ("Page.stopScreenRecording", NO_SCREENCAST),
    (
        "Page.setDownloadBehavior",
        "downloads are saved by the person, never by a client",
    ),
    ("Page.crash", "Mars does not crash its own renderer"),
    ("Page.setWebLifecycleState", "no frozen or discarded pages"),
    ("Page.setSPCTransactionMode", NO_DEVICES),
    (
        "Page.setRPHRegistrationMode",
        "no protocol handler registration",
    ),
    ("Page.generateTestReport", "no reporting API"),
    ("Page.waitForDebugger", NO_DEBUGGER),
    ("Page.setInterceptFileChooserDialog", NO_FILE_INPUT),
    ("Page.setPrerenderingAllowed", NO_PRELOAD),
    ("Page.getAppManifest", NO_EXTENSIONS),
    ("Page.getInstallabilityErrors", NO_EXTENSIONS),
    ("Page.getManifestIcons", NO_EXTENSIONS),
    ("Page.getAppId", NO_EXTENSIONS),
    ("Page.getAdScriptAncestry", "no ad tagging"),
    ("Page.getOriginTrials", "no origin trials"),
    ("Page.getPermissionsPolicyState", NO_PERMISSIONS),
    (
        "Page.setFontFamilies",
        "fonts come from the operating system, not an override",
    ),
    (
        "Page.setFontSizes",
        "fonts come from the operating system, not an override",
    ),
    ("Page.addCompilationCache", "V9 keeps its own code cache"),
    ("Page.clearCompilationCache", "V9 keeps its own code cache"),
    (
        "Page.produceCompilationCache",
        "V9 keeps its own code cache",
    ),
    (
        "Page.searchInResource",
        "resources are not retained after they load",
    ),
    (
        "Page.getResourceContent",
        "resources are not retained after they load",
    ),
    (
        "Page.getResourceTree",
        "resources are not retained after they load",
    ),
    ("Page.captureSnapshot", "no MHTML serialization"),
    (
        "Page.setBypassCSP",
        "Mars enforces the page's Content-Security-Policy for every client",
    ),
    ("Page.setAdBlockingEnabled", "no ad blocking"),
    ("Page.setTouchEmulationEnabled", "no touch events"),
    (
        "Page.setDeviceMetricsOverride",
        "use Emulation.setDeviceMetricsOverride",
    ),
    (
        "Page.clearDeviceMetricsOverride",
        "use Emulation.clearDeviceMetricsOverride",
    ),
    ("Page.setDeviceOrientationOverride", NO_DEVICES),
    ("Page.clearDeviceOrientationOverride", NO_DEVICES),
    ("Page.setGeolocationOverride", NO_PERMISSIONS),
    ("Page.clearGeolocationOverride", NO_PERMISSIONS),
    ("Page.deleteCookie", "use Network.deleteCookies"),
    ("Page.getCookies", "use Network.getCookies"),
    (
        "Page.stopLoading",
        "navigations finish before they answer; there is nothing in flight to stop",
    ),
    ("Page.getFrameTree", ""),
    (
        "Runtime.addBinding",
        "no page-to-client bindings yet: V9 needs a host function hook",
    ),
    (
        "Runtime.removeBinding",
        "no page-to-client bindings yet: V9 needs a host function hook",
    ),
    ("Runtime.compileScript", NO_DEBUGGER),
    ("Runtime.runScript", NO_DEBUGGER),
    ("Runtime.queryObjects", NO_PROFILER),
    ("Runtime.globalLexicalScopeNames", NO_DEBUGGER),
    ("Runtime.getHeapUsage", NO_PROFILER),
    (
        "Runtime.setCustomObjectFormatterEnabled",
        "no custom formatters",
    ),
    ("Runtime.setMaxCallStackSizeToCapture", NO_DEBUGGER),
    (
        "Runtime.terminateExecution",
        "V9 bounds every script by its own deadline",
    ),
    (
        "Runtime.getExceptionDetails",
        "exceptions carry their details when thrown",
    ),
    ("Runtime.getIsolateId", "one realm per page; no isolate ids"),
    ("Network.emulateNetworkConditions", NO_NETWORK_SHAPING),
    ("Network.emulateNetworkConditionsByRule", NO_NETWORK_SHAPING),
    ("Network.canEmulateNetworkConditions", NO_NETWORK_SHAPING),
    (
        "Network.setBlockedURLs",
        "block requests through Fetch.failRequest",
    ),
    ("Network.setRequestInterception", "use the Fetch domain"),
    ("Network.continueInterceptedRequest", "use the Fetch domain"),
    ("Network.setBypassServiceWorker", NO_SERVICE_WORKERS),
    ("Network.setAttachDebugStack", NO_DEBUGGER),
    (
        "Network.setCookieControls",
        "SameSite and third-party cookie rules are fixed",
    ),
    ("Network.streamResourceContent", "no stream handles"),
    (
        "Network.getSecurityIsolationStatus",
        "no cross-origin isolation controls",
    ),
    ("Network.enableReportingApi", "no reporting API"),
    (
        "Network.loadNetworkResource",
        "no out-of-page loads for clients",
    ),
    (
        "Network.setAcceptedEncodings",
        "Relay negotiates encodings itself",
    ),
    (
        "Network.clearAcceptedEncodingsOverride",
        "Relay negotiates encodings itself",
    ),
    (
        "Network.takeResponseBodyForInterceptionAsStream",
        "no stream handles",
    ),
    (
        "Network.searchInResponseBody",
        "response bodies are not retained",
    ),
    ("Network.replayXHR", "requests are not replayed"),
    (
        "Network.getCertificate",
        "certificate chains are not exposed",
    ),
    (
        "Network.getRequestPostData",
        "only the committed document is retained",
    ),
    ("Network.canClearBrowserCache", ""),
    ("Network.canClearBrowserCookies", ""),
    ("Network.setCacheDisabled", ""),
    ("Fetch.continueWithAuth", NO_AUTH),
    ("Fetch.getResponseBody", NO_SUBRESOURCE_PAUSE),
    ("Fetch.takeResponseBodyAsStream", NO_SUBRESOURCE_PAUSE),
    ("Fetch.continueResponse", NO_SUBRESOURCE_PAUSE),
    ("Security.setIgnoreCertificateErrors", NO_CERT_OVERRIDE),
    ("Security.setOverrideCertificateErrors", NO_CERT_OVERRIDE),
    ("Security.handleCertificateError", NO_CERT_OVERRIDE),
    ("Target.createBrowserContext", SINGLE_PROFILE),
    ("Target.disposeBrowserContext", SINGLE_PROFILE),
    (
        "Target.exposeDevToolsProtocol",
        "pages never get the protocol",
    ),
    (
        "Target.sendMessageToTarget",
        "flattened sessions only, as Chrome's browser-level sessions",
    ),
    ("Target.setRemoteLocations", "no remote targets"),
    ("Target.autoAttachRelated", NO_FRAMES),
    ("Target.getDevToolsTarget", "no DevTools frontend"),
    ("Target.openDevTools", "no DevTools frontend"),
    ("Browser.crash", "Mars does not crash itself on request"),
    ("Browser.crashGpuProcess", "no GPU process"),
    ("Browser.getHistograms", "no histograms"),
    ("Browser.getHistogram", "no histograms"),
    (
        "Browser.getBrowserCommandLine",
        "the command line is the person's, not the client's",
    ),
    ("Browser.grantPermissions", NO_PERMISSIONS),
    ("Browser.resetPermissions", NO_PERMISSIONS),
    ("Browser.setPermission", NO_PERMISSIONS),
    (
        "Browser.cancelDownload",
        "downloads are saved by the person, never by a client",
    ),
    ("Browser.setDockTile", "no dock tile control"),
    ("Browser.executeBrowserCommand", "no browser commands"),
    (
        "Browser.addPrivacySandboxEnrollmentOverride",
        "no Privacy Sandbox",
    ),
    (
        "Browser.addPrivacySandboxCoordinatorKeyConfig",
        "no Privacy Sandbox",
    ),
    (
        "Browser.setContentsSize",
        "the viewport is set through Emulation.setDeviceMetricsOverride",
    ),
    ("Emulation.setGeolocationOverride", NO_PERMISSIONS),
    ("Emulation.clearGeolocationOverride", NO_PERMISSIONS),
    ("Emulation.setIdleOverride", NO_DEVICES),
    ("Emulation.clearIdleOverride", NO_DEVICES),
    ("Emulation.setEmitTouchEventsForMouse", "no touch events"),
    ("Emulation.setSensorOverrideEnabled", NO_DEVICES),
    ("Emulation.setSensorOverrideReadings", NO_DEVICES),
    ("Emulation.setPressureSourceOverrideEnabled", NO_DEVICES),
    ("Emulation.setPressureStateOverride", NO_DEVICES),
    ("Emulation.setPressureDataOverride", NO_DEVICES),
    ("Emulation.setDevicePostureOverride", NO_DEVICES),
    ("Emulation.clearDevicePostureOverride", NO_DEVICES),
    ("Emulation.setDisplayFeaturesOverride", NO_DEVICES),
    ("Emulation.clearDisplayFeaturesOverride", NO_DEVICES),
    (
        "Emulation.setVirtualTimePolicy",
        "page time is virtual already and advanced per command",
    ),
    ("Emulation.setCPUThrottlingRate", "no CPU throttling"),
    (
        "Emulation.setEmulatedVisionDeficiency",
        "no vision deficiency filters",
    ),
    ("Emulation.setEmulatedOSTextScale", "no OS text scaling"),
    ("Emulation.setPageScaleFactor", "no pinch zoom"),
    (
        "Emulation.setScriptExecutionDisabled",
        "pages run their scripts; Mars does not disable them per client",
    ),
    (
        "Emulation.setDisabledImageTypes",
        "images are governed by the page budget, not a client switch",
    ),
    (
        "Emulation.setHardwareConcurrencyOverride",
        "navigator.hardwareConcurrency is fixed",
    ),
    (
        "Emulation.setAutomationOverride",
        "navigator.webdriver stays what the page sees today",
    ),
    (
        "Emulation.setSmallViewportHeightDifferenceOverride",
        "no dynamic viewport units",
    ),
    ("Emulation.setSafeAreaInsetsOverride", "no safe-area insets"),
    ("Emulation.setDataSaverOverride", "no data saver"),
    ("Emulation.getOverriddenSensorInformation", NO_DEVICES),
    (
        "Emulation.setNavigatorOverrides",
        "deprecated; use Emulation.setUserAgentOverride",
    ),
    (
        "Emulation.setVisibleSize",
        "deprecated; use Emulation.setDeviceMetricsOverride",
    ),
    ("Emulation.canEmulate", ""),
    ("Emulation.resetPageScaleFactor", ""),
    (
        "Emulation.setDefaultBackgroundColorOverride",
        "the canvas colour is the page's",
    ),
    (
        "Emulation.setScrollbarsHidden",
        "Mars draws no scrollbars in captures",
    ),
    (
        "Emulation.setDocumentCookieDisabled",
        "document.cookie follows the jar's policy",
    ),
    ("Emulation.getScreenInfos", "one display"),
    ("Emulation.addScreen", "one display"),
    ("Emulation.removeScreen", "one display"),
    ("Input.dispatchTouchEvent", "no touch events"),
    ("Input.dispatchDragEvent", "no drag and drop"),
    ("Input.setInterceptDrags", "no drag and drop"),
    ("Input.synthesizePinchGesture", "no gestures"),
    (
        "Input.synthesizeScrollGesture",
        "no gestures; use mouseWheel",
    ),
    ("Input.synthesizeTapGesture", "no gestures"),
    ("Input.emulateTouchFromMouseEvent", "no touch events"),
    ("Input.imeSetComposition", "no IME composition"),
    (
        "Input.setIgnoreInputEvents",
        "input is admitted by ownership, not a switch",
    ),
    ("DOM.setFileInputFiles", NO_FILE_INPUT),
    ("DOM.getFileInfo", NO_FILE_INPUT),
    ("DOM.getFrameOwner", NO_FRAMES),
    ("DOM.getContainerForNode", "no container queries exposed"),
    (
        "DOM.getQueryingDescendantsForContainer",
        "no container queries exposed",
    ),
    ("DOM.getAnchorElement", "no anchor positioning exposed"),
    ("DOM.forceShowPopover", "no popover control"),
    ("DOM.getDetachedDomNodes", NO_PROFILER),
    ("DOM.getElementByRelation", "no element relations exposed"),
    ("DOM.getNodesForSubtreeByStyle", NO_CSS_MODEL),
    ("DOM.collectClassNamesFromSubtree", NO_CSS_MODEL),
    ("DOM.getTopLayerElements", "no top layer exposed"),
    (
        "DOM.setOuterHTML",
        "Mars does not replace markup in place; evaluate a script or set attributes",
    ),
    (
        "DOM.setNodeValue",
        "Mars does not edit text nodes in place; evaluate a script",
    ),
    ("DOM.setNodeName", "no in-place renames"),
    ("DOM.copyTo", "no in-place moves"),
    ("DOM.moveTo", "no in-place moves"),
    ("DOM.removeNode", "no in-place removals; evaluate a script"),
    ("DOM.markUndoableState", "no undo stack"),
    ("DOM.undo", "no undo stack"),
    ("DOM.redo", "no undo stack"),
    (
        "DOM.pushNodeByPathToFrontend",
        "node ids are the document's own identities",
    ),
    (
        "DOM.pushNodesByBackendIdsToFrontend",
        "node ids are the document's own identities",
    ),
    (
        "DOM.requestChildNodes",
        "children arrive with getDocument(depth)",
    ),
    ("DOM.highlightNode", NO_OVERLAY),
    ("DOM.highlightRect", NO_OVERLAY),
    ("DOM.hideHighlight", NO_OVERLAY),
    ("DOM.setInspectedNode", NO_OVERLAY),
    ("DOM.getRelayoutBoundary", "no relayout boundaries exposed"),
    ("DOM.discardSearchResults", ""),
    ("DOM.getSearchResults", ""),
    ("DOM.performSearch", ""),
    (
        "DOM.getFlattenedDocument",
        "deprecated; use DOM.getDocument",
    ),
    ("DOM.describeNode", ""),
    ("Accessibility.queryAXTree", ""),
    ("Accessibility.getPartialAXTree", ""),
    ("Accessibility.getChildAXNodes", ""),
    ("Accessibility.getRootAXNode", ""),
    ("Accessibility.getAXNodeAndAncestors", ""),
    (
        "DOMSnapshot.getSnapshot",
        "deprecated; use DOMSnapshot.captureSnapshot",
    ),
    ("Storage.getStorageKeyForFrame", ""),
    ("Storage.getStorageKey", ""),
    ("Storage.clearDataForOrigin", ""),
    ("Storage.clearDataForStorageKey", ""),
    ("Storage.getUsageAndQuota", "no quota accounting"),
    ("Storage.overrideQuotaForOrigin", "no quota accounting"),
    ("Storage.trackCacheStorageForOrigin", NO_SERVICE_WORKERS),
    ("Storage.trackCacheStorageForStorageKey", NO_SERVICE_WORKERS),
    ("Storage.untrackCacheStorageForOrigin", NO_SERVICE_WORKERS),
    (
        "Storage.untrackCacheStorageForStorageKey",
        NO_SERVICE_WORKERS,
    ),
    ("Storage.trackIndexedDBForOrigin", NO_STORAGE_INSPECTION),
    ("Storage.trackIndexedDBForStorageKey", NO_STORAGE_INSPECTION),
    ("Storage.untrackIndexedDBForOrigin", NO_STORAGE_INSPECTION),
    (
        "Storage.untrackIndexedDBForStorageKey",
        NO_STORAGE_INSPECTION,
    ),
    ("Storage.getTrustTokens", "no Private State Tokens"),
    ("Storage.clearTrustTokens", "no Private State Tokens"),
    ("Storage.getInterestGroupDetails", "no Protected Audience"),
    ("Storage.setInterestGroupTracking", "no Protected Audience"),
    (
        "Storage.setInterestGroupAuctionTracking",
        "no Protected Audience",
    ),
    ("Storage.getSharedStorageMetadata", "no Shared Storage"),
    ("Storage.getSharedStorageEntries", "no Shared Storage"),
    ("Storage.setSharedStorageEntry", "no Shared Storage"),
    ("Storage.deleteSharedStorageEntry", "no Shared Storage"),
    ("Storage.clearSharedStorageEntries", "no Shared Storage"),
    ("Storage.resetSharedStorageBudget", "no Shared Storage"),
    ("Storage.setSharedStorageTracking", "no Shared Storage"),
    ("Storage.setStorageBucketTracking", "no storage buckets"),
    ("Storage.deleteStorageBucket", "no storage buckets"),
    (
        "Storage.runBounceTrackingMitigations",
        "no bounce tracking mitigations",
    ),
    (
        "Storage.setAttributionReportingLocalTestingMode",
        "no Attribution Reporting",
    ),
    (
        "Storage.setAttributionReportingTracking",
        "no Attribution Reporting",
    ),
    (
        "Storage.sendPendingAttributionReports",
        "no Attribution Reporting",
    ),
    ("Storage.getRelatedWebsiteSets", "no Related Website Sets"),
    (
        "Storage.getAffectedUrlsForThirdPartyCookieMetadata",
        "no third-party cookie metadata",
    ),
    (
        "Storage.setProtectedAudienceKAnonymity",
        "no Protected Audience",
    ),
    (
        "Storage.getInterestGroupAuctionEventTracking",
        "no Protected Audience",
    ),
    ("Storage.getUsageAndQuotaForOrigin", "no quota accounting"),
    ("Log.startViolationsReport", "no violation reports"),
    ("Log.stopViolationsReport", "no violation reports"),
    ("Performance.setTimeDomain", "deprecated"),
    ("Inspector.disable", ""),
    ("Inspector.enable", ""),
    ("Console.clearMessages", ""),
    ("Console.disable", ""),
    ("Console.enable", ""),
    ("Schema.getDomains", ""),
];

/** The status of one command, given the dispatch table's implemented and accepted sets. */
pub fn status(method: &str, implemented: &[&str], accepted: &[&str]) -> Status {
    if implemented.contains(&method) {
        return Status::Implemented;
    }
    if accepted.contains(&method) {
        return Status::Accepted;
    }
    if let Some((_, reason)) = COMMANDS.iter().find(|(name, _)| *name == method)
        && !reason.is_empty()
    {
        return Status::NotApplicable(reason);
    }
    let domain = method.split('.').next().unwrap_or_default();
    if let Some((_, reason)) = DOMAINS.iter().find(|(name, _)| *name == domain) {
        return Status::NotApplicable(reason);
    }
    Status::NotYet
}

/** Counts per status, for the report and the document's summary line. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub implemented: usize,
    pub accepted: usize,
    pub not_applicable: usize,
    pub not_yet: usize,
}

impl Counts {
    /** Count every command of the catalog. */
    pub fn of(catalog: &Catalog, implemented: &[&str], accepted: &[&str]) -> Self {
        let mut counts = Self::default();
        for (method, _, _) in catalog.methods() {
            match status(&method, implemented, accepted) {
                Status::Implemented => counts.implemented += 1,
                Status::Accepted => counts.accepted += 1,
                Status::NotApplicable(_) => counts.not_applicable += 1,
                Status::NotYet => counts.not_yet += 1,
            }
        }
        counts
    }
    /** Every command has one of the four statuses. */
    pub fn total(self) -> usize {
        self.implemented + self.accepted + self.not_applicable + self.not_yet
    }
}

/** The whole matrix as Markdown, one table per domain. */
pub fn markdown(
    catalog: &Catalog,
    implemented: &[&str],
    accepted: &[&str],
    events: &[&str],
) -> String {
    let counts = Counts::of(catalog, implemented, accepted);
    let mut out = String::new();
    out.push_str("# Chrome DevTools Protocol on Mars\n\n");
    out.push_str("Generated by `mars-mdp --matrix` from the pinned protocol definitions ");
    out.push_str("(Chromium `2c592105bbcd9490a9894df48d0fe59b2c512651`, V8 `fedd021b538607860f9c56e5c193ad8dcd4f2bb1`, ");
    out.push_str(&format!(
        "protocol {}). Do not edit by hand; `cargo test -p mars-mdp` checks it.\n\n",
        catalog.version
    ));
    out.push_str("Every method is answered natively: a Mars native operation (`page.*`, `tab.*`, `control.*`) ");
    out.push_str(
        "under the same ownership, epoch and redaction rules as MCP. Mars has no debugger and ",
    );
    out.push_str(
        "proxies nothing to an engine; a method it does not translate gets an explicit error.\n\n",
    );
    out.push_str(&format!(
        "| Status | Commands |\n|:---|---:|\n| Implemented natively | {} |\n| Accepted (no effect) | {} |\n| Not applicable | {} |\n| Not yet | {} |\n| Total | {} |\n\n",
        counts.implemented, counts.accepted, counts.not_applicable, counts.not_yet, counts.total()
    ));
    out.push_str(LIMITS);
    out.push_str("## Events Mars sends\n\n");
    for event in events {
        out.push_str(&format!("- `{event}`\n"));
    }
    out.push('\n');
    let mut reasons: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for domain in &catalog.domains {
        if domain.commands.is_empty() {
            continue;
        }
        let flags = match (domain.experimental, domain.deprecated) {
            (true, _) => " (experimental)",
            (_, true) => " (deprecated)",
            _ => "",
        };
        out.push_str(&format!(
            "## {}{flags}\n\n| Command | Status | Note |\n|:---|:---|:---|\n",
            domain.name
        ));
        for item in &domain.commands {
            let method = format!("{}.{}", domain.name, item.name);
            let status = status(&method, implemented, accepted);
            let note = match status {
                Status::NotApplicable(reason) => {
                    reasons.entry(reason).or_default().push(method.clone());
                    reason.to_string()
                }
                _ => String::new(),
            };
            let flag = if item.deprecated {
                " (deprecated)"
            } else if item.experimental {
                " (experimental)"
            } else {
                ""
            };
            out.push_str(&format!(
                "| `{method}`{flag} | {} | {note} |\n",
                status.label()
            ));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Status, status};

    /** The table ranks implemented over accepted, names reasons, and leaves the rest open. */
    #[test]
    fn status_follows_the_tables() {
        assert_eq!(
            status("Page.navigate", &["Page.navigate"], &[]),
            Status::Implemented
        );
        assert_eq!(
            status("Page.enable", &[], &["Page.enable"]),
            Status::Accepted
        );
        assert!(matches!(
            status("Debugger.enable", &[], &[]),
            Status::NotApplicable(_)
        ));
        assert!(matches!(
            status("Page.printToPDF", &[], &[]),
            Status::NotApplicable(_)
        ));
        assert_eq!(status("Page.getFrameTree", &[], &[]), Status::NotYet);
    }
}
