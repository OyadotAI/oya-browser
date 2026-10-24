/** docs/cdp.md is the rendered matrix; a stale copy fails here so the document stays true. */
use mars_mdp::{catalog::Catalog, matrix, session};

/** The committed document equals what `mars-mdp --matrix` prints. */
#[test]
fn committed_matrix_is_current() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/cdp.md");
    let committed = std::fs::read_to_string(path).expect("docs/cdp.md exists");
    let rendered = matrix::markdown(
        &Catalog::pinned(),
        &session::implemented(),
        session::ACCEPTED,
        session::EVENTS,
    );
    assert!(
        committed == rendered,
        "docs/cdp.md is stale; regenerate it with `mars-mdp --matrix > docs/cdp.md`"
    );
}

/** Everything Playwright, Puppeteer, browser-use and Stagehand call is implemented or accepted. */
#[test]
fn client_methods_are_covered() {
    let implemented = session::implemented();
    let covered =
        |method: &str| implemented.contains(&method) || session::ACCEPTED.contains(&method);
    let clients = [
        "Browser.getVersion",
        "Target.setAutoAttach",
        "Target.getTargetInfo",
        "Target.attachToTarget",
        "Target.createTarget",
        "Target.closeTarget",
        "Target.setDiscoverTargets",
        "Target.getBrowserContexts",
        "Page.enable",
        "Page.getFrameTree",
        "Page.setLifecycleEventsEnabled",
        "Page.createIsolatedWorld",
        "Page.addScriptToEvaluateOnNewDocument",
        "Page.navigate",
        "Page.reload",
        "Page.captureScreenshot",
        "Page.getNavigationHistory",
        "Page.navigateToHistoryEntry",
        "Page.close",
        "Runtime.enable",
        "Runtime.runIfWaitingForDebugger",
        "Runtime.evaluate",
        "Runtime.callFunctionOn",
        "Runtime.getProperties",
        "Runtime.releaseObject",
        "Runtime.releaseObjectGroup",
        "Runtime.awaitPromise",
        "DOM.describeNode",
        "DOM.getBoxModel",
        "DOM.getContentQuads",
        "DOM.resolveNode",
        "DOM.scrollIntoViewIfNeeded",
        "DOM.getDocument",
        "DOM.querySelector",
        "Input.dispatchMouseEvent",
        "Input.dispatchKeyEvent",
        "Input.insertText",
        "Network.enable",
        "Network.getCookies",
        "Network.setCookies",
        "Network.deleteCookies",
        "Network.setExtraHTTPHeaders",
        "Network.setUserAgentOverride",
        "Network.getResponseBody",
        "Fetch.enable",
        "Fetch.disable",
        "Fetch.continueRequest",
        "Fetch.fulfillRequest",
        "Fetch.failRequest",
        "Emulation.setDeviceMetricsOverride",
        "Emulation.setEmulatedMedia",
        "Emulation.setFocusEmulationEnabled",
        "Emulation.setTimezoneOverride",
        "Emulation.setLocaleOverride",
        "Emulation.setUserAgentOverride",
        "Log.enable",
        "Performance.enable",
        "Performance.getMetrics",
        "Accessibility.getFullAXTree",
        "Accessibility.queryAXTree",
        "Storage.getCookies",
        "Storage.setCookies",
        "Storage.clearCookies",
        "DOMSnapshot.captureSnapshot",
    ];
    let missing: Vec<&str> = clients
        .iter()
        .copied()
        .filter(|method| !covered(method))
        .collect();
    assert!(missing.is_empty(), "not covered: {missing:?}");
    let counts = matrix::Counts::of(&Catalog::pinned(), &implemented, session::ACCEPTED);
    assert_eq!(counts.not_yet, 0, "{counts:?}");
}
