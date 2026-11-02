/**
 * One CDP connection: flattened sessions over Mars tabs, the per-domain command maps, the event
 * queue, and the bookkeeping Chrome keeps per target (execution contexts, remote object ids,
 * loader ids, focus). Every command becomes native Mars operations on this connection's lane;
 * a method with no translation answers with an explicit error naming the matrix.
 */
mod dom;
mod emulation;
mod input;
mod network;
mod page;
mod runtime;
mod target;

use crate::Native;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
pub use target::user_agent;

/// JSON-RPC style error code CDP servers use for a failed method.
const SERVER_ERROR: i64 = -32000;
/// Chrome's code for malformed parameters.
const INVALID_PARAMS: i64 = -32602;
/// Chrome's code for a method it does not have.
const METHOD_NOT_FOUND: i64 = -32601;
/// A command whose answer waits for a client decision (a paused navigation).
const DEFERRED: i64 = i64::MIN;
/// Node ids of elements' synthetic text children start here; Eye keeps an element's own text on
/// the element, Chrome shows a `#text` child.
const TEXT_NODE_BASE: u64 = 1 << 30;
/// The document node's id; element ids start after it.
const DOCUMENT_NODE: u64 = 1;
/// Projected Mars node ids are offset past the document node.
const NODE_OFFSET: u64 = 2;

/** Handler for one method; the session ID routes page methods to a tab. */
type Handler = fn(&mut Session, Option<&str>, &Value) -> Result<Value, Fault>;

/** Methods that only switch on events or settings Mars already has; they change nothing. */
pub const ACCEPTED: &[&str] = &[
    "Page.enable",
    "Page.disable",
    "Page.setLifecycleEventsEnabled",
    "Page.bringToFront",
    "Page.removeScriptToEvaluateOnLoad",
    "Page.resetNavigationHistory",
    "Page.getAnnotatedPageContent",
    "Runtime.disable",
    "Runtime.runIfWaitingForDebugger",
    "Runtime.discardConsoleEntries",
    "Runtime.setAsyncCallStackDepth",
    "Network.enable",
    "Network.disable",
    "Network.setCacheDisabled",
    "Network.clearBrowserCache",
    "Network.canClearBrowserCache",
    "Network.canClearBrowserCookies",
    "Network.overrideNetworkState",
    "Network.configureDurableMessages",
    "Network.enableDeviceBoundSessions",
    "Network.deleteDeviceBoundSession",
    "Network.fetchSchemefulSite",
    "Log.enable",
    "Log.disable",
    "Log.clear",
    "Console.enable",
    "Console.disable",
    "Console.clearMessages",
    "DOM.enable",
    "DOM.disable",
    "DOM.setNodeStackTracesEnabled",
    "DOM.getNodeStackTraces",
    "DOM.performSearch",
    "DOM.getSearchResults",
    "DOM.discardSearchResults",
    "Performance.enable",
    "Performance.disable",
    "Accessibility.enable",
    "Accessibility.disable",
    "DOMSnapshot.enable",
    "DOMSnapshot.disable",
    "Security.enable",
    "Security.disable",
    "Inspector.enable",
    "Inspector.disable",
    "Emulation.setFocusEmulationEnabled",
    "Emulation.setAutoDarkModeOverride",
    "Emulation.canEmulate",
    "Emulation.resetPageScaleFactor",
    "Emulation.setDefaultBackgroundColorOverride",
    "Emulation.setScrollbarsHidden",
    "Emulation.setDocumentCookieDisabled",
    "Emulation.getScreenInfos",
    "Emulation.updateScreen",
    "Emulation.setPrimaryScreen",
    "Target.activateTarget",
    "Audits.enable",
    "Audits.disable",
    "Browser.getWindowBounds",
    "Browser.setDownloadBehavior",
    "Input.cancelDragging",
    "Storage.getStorageKeyForFrame",
    "Storage.getStorageKey",
    "Storage.clearDataForOrigin",
    "Storage.clearDataForStorageKey",
];

/** Events this front door sends, for the matrix. */
pub const EVENTS: &[&str] = &[
    "Target.targetCreated",
    "Target.targetDestroyed",
    "Target.attachedToTarget",
    "Target.detachedFromTarget",
    "Page.frameStartedLoading",
    "Page.frameNavigated",
    "Page.frameStoppedLoading",
    "Page.lifecycleEvent",
    "Page.domContentEventFired",
    "Page.loadEventFired",
    "Runtime.executionContextCreated",
    "Runtime.executionContextsCleared",
    "Runtime.consoleAPICalled",
    "Runtime.exceptionThrown",
    "Network.requestWillBeSent",
    "Network.responseReceived",
    "Network.loadingFinished",
    "Network.loadingFailed",
    "Fetch.requestPaused",
    "Log.entryAdded",
];

/** A CDP failure: the message carries the native code so clients can tell ownership from bugs. */
#[derive(Debug)]
pub struct Fault {
    code: i64,
    message: String,
}

impl Fault {
    /** A method this front door does not translate. */
    fn unsupported(method: &str) -> Self {
        Self {
            code: METHOD_NOT_FOUND,
            message: format!(
                "'{method}' wasn't found: Mars does not implement it. See docs/cdp.md or GET /json/protocol."
            ),
        }
    }
    /** A native error, kept recognizable: `control_denied: ...`. */
    pub(crate) fn native(error: &Value) -> Self {
        Self {
            code: SERVER_ERROR,
            message: format!(
                "{}: {}",
                error["code"].as_str().unwrap_or("error"),
                error["message"]
                    .as_str()
                    .unwrap_or("The native operation failed.")
            ),
        }
    }
    /** Malformed parameters. */
    fn invalid(message: &str) -> Self {
        Self {
            code: INVALID_PARAMS,
            message: message.into(),
        }
    }
    /** A server-side refusal with Chrome's wording. */
    fn server(message: &str) -> Self {
        Self {
            code: SERVER_ERROR,
            message: message.into(),
        }
    }
    /** The answer comes later, once the client decides a paused request. */
    fn deferred() -> Self {
        Self {
            code: DEFERRED,
            message: String::new(),
        }
    }
    /** Whether a native error means the lane does not hold the tab. */
    fn denied(&self) -> bool {
        self.message.starts_with("control_denied")
    }
}

/** What this connection knows about one tab it has seen. */
#[derive(Default)]
struct Target {
    /// Control epoch observed when this connection opened or attached the tab; never refreshed,
    /// so a person taking the tab over stops this client.
    epoch: Option<u64>,
    /// The node DOM.focus or a click focused, for key events and insertText.
    focused: Option<usize>,
    /// The last element a client passed to callFunctionOn: Playwright focuses through script,
    /// which V9 does not track, so insertText falls back to the element it just worked on.
    touched: Option<usize>,
    /// Main-world execution context of the current document.
    context: u64,
    /// Isolated worlds created on this tab: context id and name.
    worlds: Vec<(u64, String)>,
    /// Loader of the current document.
    loader: u64,
    /// Page identity of the current document, to notice navigations.
    page_id: String,
    /// Page console entries already reported as events.
    console_seen: usize,
    /// Nodes behind remote object ids this connection handed out.
    nodes: BTreeMap<String, usize>,
    /// The request id announced for the next navigation, when it was announced early.
    announced: Option<String>,
    /// Whether this lane opened the tab (Target.createTarget), so it holds it.
    owned: bool,
}

/** A command answered later: a navigation paused for a Fetch decision. */
struct Deferred {
    id: Value,
    session_id: Option<String>,
    tab: String,
    request_id: String,
    url: String,
}

/** One CDP client connection and its native lane. */
pub struct Session {
    native: Box<dyn Native + Send>,
    targets: BTreeMap<String, Target>,
    /// Flattened session ID to tab ID.
    sessions: BTreeMap<String, String>,
    /// Sessions attached to the browser target itself (Target.attachToBrowserTarget); their
    /// commands are browser-level, answered under their own session id.
    browser_sessions: Vec<String>,
    /// Execution context id to tab.
    contexts: BTreeMap<u64, String>,
    /// The tab a page-level WebSocket endpoint is bound to.
    default_tab: Option<String>,
    auto_attach: bool,
    wait_for_debugger: bool,
    next_context: u64,
    next_session: u64,
    next_request: u64,
    next_script: u64,
    events: Vec<Value>,
    /// Browser.close asks the server to end this connection (never the browser).
    pub closing: bool,
    /// The command being dispatched, so a handler can defer its answer.
    current: Option<(Value, Option<String>)>,
    deferred: Vec<Deferred>,
    /// Fetch.enable patterns; None when the domain is off.
    fetch: Option<Vec<network::Pattern>>,
    /// Page.addScriptToEvaluateOnNewDocument sources: identifier, source, world name.
    new_document_scripts: Vec<(String, String, String)>,
    /// Extra request headers and user agent the client set; recorded, see docs/cdp.md.
    extra_headers: BTreeMap<String, String>,
    started: Instant,
}

impl Session {
    /** A browser-level connection, or a page-level one bound to `tab`. */
    pub fn new(native: Box<dyn Native + Send>, tab: Option<String>) -> Self {
        let mut session = Self {
            native,
            targets: BTreeMap::new(),
            sessions: BTreeMap::new(),
            browser_sessions: Vec::new(),
            contexts: BTreeMap::new(),
            default_tab: None,
            auto_attach: false,
            wait_for_debugger: false,
            next_context: 0,
            next_session: 0,
            next_request: 0,
            next_script: 0,
            events: Vec::new(),
            closing: false,
            current: None,
            deferred: Vec::new(),
            fetch: None,
            new_document_scripts: Vec::new(),
            extra_headers: BTreeMap::new(),
            started: Instant::now(),
        };
        if let Some(tab) = tab {
            session.adopt(&tab);
            session.default_tab = Some(tab);
        }
        session
    }

    /** Handle one client message; returns the events it caused followed by its response. */
    pub fn handle(&mut self, message: &str) -> Vec<Value> {
        let Ok(message) = serde_json::from_str::<Value>(message) else {
            return vec![json!({"error":{"code":-32700,"message":"Invalid JSON message."}})];
        };
        let id = message.get("id").cloned().unwrap_or(Value::Null);
        let method = message["method"].as_str().unwrap_or_default().to_owned();
        let session_id = message["sessionId"].as_str().map(str::to_owned);
        let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
        self.current = Some((id.clone(), session_id.clone()));
        let routed = session_id
            .clone()
            .filter(|session| !self.browser_sessions.contains(session));
        let result = self.dispatch(&method, routed.as_deref(), &params);
        self.current = None;
        let mut out = std::mem::take(&mut self.events);
        match result {
            Err(fault) if fault.code == DEFERRED => {}
            result => out.push(response(id, session_id, result)),
        }
        out
    }

    /** Find the method in the command maps; accepted methods answer empty; the rest is unknown. */
    fn dispatch(
        &mut self,
        method: &str,
        session: Option<&str>,
        params: &Value,
    ) -> Result<Value, Fault> {
        if let Some((_, handler)) = handlers().iter().find(|(name, _)| *name == method) {
            return handler(self, session, params);
        }
        if ACCEPTED.contains(&method) {
            return Ok(json!({}));
        }
        if matches!(method, "Mdp.getCapabilities" | "Mars.getCapabilities") {
            return Ok(
                json!({"methods":implemented(),"accepted_without_effect":ACCEPTED,"events":EVENTS,"lane":"each CDP connection is its own agent lane","matrix":"docs/cdp.md"}),
            );
        }
        Err(Fault::unsupported(method))
    }

    /** One native call; errors keep their native code. */
    fn call(&mut self, op: &str, params: Value) -> Result<Value, Fault> {
        self.native
            .call(op, params)
            .map_err(|error| Fault::native(&error))
    }

    /** A native call on one tab. */
    fn call_tab(&mut self, tab: &str, op: &str, mut params: Value) -> Result<Value, Fault> {
        params["tab_id"] = json!(tab);
        self.call(op, params)
    }

    /** A native mutation on one tab, carrying the observed control epoch. */
    fn mutate(&mut self, tab: &str, op: &str, mut params: Value) -> Result<Value, Fault> {
        params["tab_id"] = json!(tab);
        params["control_epoch"] = json!(self.epoch(tab));
        let result = self.call(op, params)?;
        self.after_mutation(tab, &result);
        Ok(result)
    }

    /** A revision-scoped mutation: the page identity and revision are read just before. */
    fn mutate_revision(&mut self, tab: &str, op: &str, mut params: Value) -> Result<Value, Fault> {
        // A tab without a page still answers with its ownership refusal, as Chrome's would.
        let (page_id, revision) = self.head(tab).unwrap_or((json!(""), json!(0)));
        params["page_id"] = page_id;
        params["expected_revision"] = revision;
        self.mutate(tab, op, params)
    }

    /** The tab's current page identity and revision. */
    fn head(&mut self, tab: &str) -> Result<(Value, Value), Fault> {
        let page = self.call_tab(tab, "page.wait", json!({"after_revision":0}))?;
        Ok((page["page_id"].clone(), page["revision"].clone()))
    }

    /** The control epoch this connection observed for a tab. */
    fn epoch(&self, tab: &str) -> u64 {
        self.targets
            .get(tab)
            .and_then(|target| target.epoch)
            .unwrap_or_default()
    }

    /** Queue an event, routed to every flattened session on `tab` (or the page endpoint). */
    fn emit(&mut self, tab: Option<&str>, method: &str, params: Value) {
        let mut sessions: Vec<Option<String>> = Vec::new();
        if let Some(tab) = tab {
            sessions.extend(
                self.sessions
                    .iter()
                    .filter(|(_, t)| *t == tab)
                    .map(|(s, _)| Some(s.clone())),
            );
            if self.default_tab.as_deref() == Some(tab) {
                sessions.push(None);
            }
        } else {
            sessions.push(None);
        }
        for session in sessions {
            let mut event = json!({"method":method,"params":params});
            if let Some(session) = session {
                event["sessionId"] = json!(session);
            }
            self.events.push(event);
        }
    }

    /** Remember a tab and the control epoch it has right now. */
    fn adopt(&mut self, tab: &str) {
        let epoch = self
            .call("control.status", json!({"tab_id":tab}))
            .ok()
            .and_then(|status| status["epoch"].as_u64());
        let page_id = self
            .call_tab(tab, "page.wait", json!({"after_revision":0}))
            .ok()
            .and_then(|page| page["page_id"].as_str().map(str::to_owned))
            .unwrap_or_default();
        let context = self.new_context(tab);
        let target = self.targets.entry(tab.to_owned()).or_default();
        target.epoch = target.epoch.or(epoch);
        target.context = context;
        target.page_id = page_id;
    }

    /** A fresh execution context id on a tab. */
    fn new_context(&mut self, tab: &str) -> u64 {
        self.next_context += 1;
        self.contexts.insert(self.next_context, tab.to_owned());
        self.next_context
    }

    /** The tab a page method addresses. */
    fn tab(&self, session: Option<&str>) -> Result<String, Fault> {
        session
            .and_then(|session| self.sessions.get(session).cloned())
            .or_else(|| self.default_tab.clone())
            .ok_or_else(|| {
                Fault::server(
                    "No target is attached. Call Target.attachToTarget with flatten: true.",
                )
            })
    }

    /** Seconds since this connection started, Chrome's monotonic timestamp. */
    fn timestamp(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /** Seconds since the Unix epoch. */
    fn wall_time() -> f64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0.0, |since| since.as_secs_f64())
    }

    /** After an input or script that may navigate: report the navigation and new console lines. */
    fn after_mutation(&mut self, tab: &str, result: &Value) {
        let page_id = result["page_id"].as_str().unwrap_or_default();
        let known = self
            .targets
            .get(tab)
            .is_some_and(|target| target.page_id == page_id);
        if !page_id.is_empty() && !known && result.get("url").is_some() {
            self.navigated(tab, result);
        } else if !page_id.is_empty() && !known {
            if let Ok(page) = self.call_tab(tab, "page.wait", json!({"after_revision":0})) {
                self.navigated(tab, &page);
            }
        } else {
            self.report_console(tab);
        }
    }
}

/** Every handler of every domain. */
fn handlers() -> Vec<(&'static str, Handler)> {
    let mut all: Vec<(&'static str, Handler)> = Vec::new();
    all.extend_from_slice(target::HANDLERS);
    all.extend_from_slice(page::HANDLERS);
    all.extend_from_slice(runtime::HANDLERS);
    all.extend_from_slice(dom::HANDLERS);
    all.extend_from_slice(input::HANDLERS);
    all.extend_from_slice(network::HANDLERS);
    all.extend_from_slice(emulation::HANDLERS);
    all
}

/** Every implemented method name, for the matrix and Mars.getCapabilities. */
pub fn implemented() -> Vec<&'static str> {
    handlers().into_iter().map(|(name, _)| name).collect()
}

/** A response envelope. */
fn response(id: Value, session_id: Option<String>, result: Result<Value, Fault>) -> Value {
    let mut response = match result {
        Ok(result) => json!({"id":id,"result":result}),
        Err(fault) => json!({"id":id,"error":{"code":fault.code,"message":fault.message}}),
    };
    if let Some(session_id) = session_id {
        response["sessionId"] = json!(session_id);
    }
    response
}

/** Glob match with `*` and `?`, as Chrome's URL pattern matching (`\` escapes). */
pub fn glob(pattern: &str, text: &str) -> bool {
    fn matches(pattern: &[char], text: &[char]) -> bool {
        match pattern.split_first() {
            None => text.is_empty(),
            Some(('*', rest)) => (0..=text.len()).any(|skip| matches(rest, &text[skip..])),
            Some(('?', rest)) => !text.is_empty() && matches(rest, &text[1..]),
            Some(('\\', rest)) => match (rest.split_first(), text.split_first()) {
                (Some((escaped, after)), Some((first, more))) if escaped == first => {
                    matches(after, more)
                }
                _ => false,
            },
            Some((literal, rest)) => match text.split_first() {
                Some((first, more)) if first == literal => matches(rest, more),
                _ => false,
            },
        }
    }
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    matches(&pattern, &text)
}

#[cfg(test)]
mod tests {
    use super::glob;

    /** URL patterns match the way Chrome's interception patterns do. */
    #[test]
    fn url_patterns_glob() {
        assert!(glob("*", "https://a.example/x"));
        assert!(glob("*/api/*", "https://a.example/api/users"));
        assert!(!glob("*/api/*", "https://a.example/web/users"));
        assert!(glob("https://a.example/?", "https://a.example/x"));
        assert!(glob("https://a.example/\\*", "https://a.example/*"));
        assert!(!glob("https://a.example/\\*", "https://a.example/x"));
    }
}
