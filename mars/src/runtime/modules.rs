/**
 * Module graphs load in rounds: V9 names the module URLs a graph needs, the coordinator fetches
 * them the way a browser fetches module scripts (CORS mode, same-origin credentials, a
 * JavaScript media type, the page's Content-Security-Policy and any integrity metadata), and the
 * next V9 step continues the graph. A module that cannot be fetched, or would pass MODULE_SOURCES
 * or MODULE_BYTES, is delivered as failed, which fails only the graphs that import it.
 */
use super::page::Page;
use super::resources::{Credentials, ScriptAnswer, ScriptRequest};
use super::scripting::Step;
use super::supervisor::Runtime;
use crate::constants::{MODULE_BYTES, MODULE_SOURCES, SCRIPT_BYTES, SCRIPT_LOADS};
use crate::document::{ScriptKind, ScriptMode, ScriptResource};
use crate::network::{Destination, Method, WebRequest, cors};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{
    EntryKind, FrameRequest, ModuleEntry, ModuleInput, ModuleRequest, ModuleSource,
    ScriptLoadRequest,
};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};
use url::Url;

/// Fetch rounds one step may make; each round fetches every module the last one found. A chain
/// of dynamic scripts, each inserted by the last one's load, needs one round per link, so the
/// rounds match SCRIPT_LOADS, the most a page may start. MODULE_WALL still bounds the time; what
/// is still pending when rounds run out fails its whole graph.
const MODULE_ROUNDS: usize = SCRIPT_LOADS;
/// Real time one step may spend loading module graphs, inside the navigation's own deadline.
const MODULE_WALL: Duration = Duration::from_millis(15_000);

/// A page's module scripts and the modules fetched for them, kept between V9 requests.
#[derive(Clone, Default)]
pub(super) struct Modules {
    /// What every V9 request carries: which sources are modules, and the fetched modules. Shared
    /// by Arc so a step or a tick copies the page without copying its megabytes of module text;
    /// `deliver` appends through `Arc::make_mut`, which copies only while a step still holds it.
    pub input: std::sync::Arc<ModuleInput>,
    /// The V9 key of each delivered source (`module_source_key`), in `input.sources` order, so a
    /// step hands V9 keys instead of making it hash the page's module text again.
    pub keys: std::sync::Arc<Vec<crate::v9::SourceKey>>,
    /// Integrity metadata of external module scripts, by URL.
    integrity: BTreeMap<String, String>,
    /// Modules V9 asked for that the next round fetches.
    pub pending: Vec<ModuleRequest>,
    /// Classic scripts page elements asked for (`<script src>` connected by script) that the
    /// next round fetches.
    pub pending_scripts: Vec<ScriptLoadRequest>,
    /// Bytes of delivered module sources, bounded by MODULE_BYTES.
    bytes: usize,
    /// Every URL delivered to V9, as a module or a classic script: a set, so the check before
    /// each fetch costs the same whether the page has ten scripts or a thousand.
    delivered: HashSet<String>,
    /// Frame documents V9 asked for that the next round fetches.
    pub frame_requests: Vec<FrameRequest>,
    /// Bytes of frame documents delivered to the page, bounded by FRAME_BYTES_TOTAL.
    pub frame_bytes: usize,
}

/// What a page source that is not a classic script is, for V9.
#[derive(Clone)]
pub(super) struct Entry {
    kind: EntryKind,
    url: Option<String>,
    integrity: Option<String>,
}

/** A module script or import map as V9 sees it; None for a classic script. `url` is the
script's resolved address (with its query, which is the module's identity). */
pub(super) fn entry(script: &ScriptResource, url: Option<&Url>) -> Option<Entry> {
    let kind = match (script.kind, script.mode) {
        (ScriptKind::Classic, _) => return None,
        (ScriptKind::ImportMap, _) => EntryKind::ImportMap,
        (ScriptKind::Module, ScriptMode::Async) => EntryKind::AsyncModule,
        (ScriptKind::Module, _) => EntryKind::Module,
    };
    let url = script.src.as_ref().and(url).map(Url::to_string);
    let integrity = url.as_ref().and(script.integrity.clone());
    Some(Entry {
        kind,
        url,
        integrity,
    })
}

impl Modules {
    /** The module input for a page's sources in execution order. */
    pub fn new(entries: impl Iterator<Item = (usize, Entry)>) -> Self {
        let mut modules = Self::default();
        for (index, entry) in entries {
            if let (Some(url), Some(integrity)) = (&entry.url, entry.integrity) {
                modules.integrity.insert(url.clone(), integrity);
            }
            std::sync::Arc::make_mut(&mut modules.input)
                .entries
                .push(ModuleEntry {
                    index,
                    kind: entry.kind,
                    url: entry.url,
                });
        }
        modules
    }
    /**
     * Hand one module or classic script to V9, or its failure when it has no source: a source
     * over the byte bounds is delivered as failed, so its element still fires `error`. Returns
     * false only when the source count is full, which V9's own caps make unreachable.
     */
    fn deliver(&mut self, url: String, source: Option<String>, classic: bool) -> bool {
        if self.input.sources.len() >= MODULE_SOURCES {
            return false;
        }
        let fits = |source: &String| {
            source.len() <= SCRIPT_BYTES && self.bytes + source.len() + url.len() <= MODULE_BYTES
        };
        let source = source.filter(fits);
        self.bytes += url.len() + source.as_ref().map_or(0, String::len);
        self.delivered.insert(url.clone());
        let key = crate::v9::module_source_key(source.as_deref().unwrap_or(""), classic);
        std::sync::Arc::make_mut(&mut self.keys).push(key);
        std::sync::Arc::make_mut(&mut self.input)
            .sources
            .push(ModuleSource {
                url,
                source,
                classic,
            });
        true
    }
    /** Whether a URL was delivered already, as a module or a classic script. */
    fn known(&self, url: &str) -> bool {
        self.delivered.contains(url)
    }
}

impl Runtime {
    /**
     * Load the module graphs a step asked for: fetch every requested module, run a `modules`
     * step, and repeat while it finds more, within MODULE_ROUNDS and MODULE_WALL. Past either,
     * what is still missing is delivered as failed so no module script waits forever. A modules
     * step V9 refuses stops module loading for the page; the page itself carries on.
     */
    pub(super) fn follow_modules(&mut self, page: &mut Page, step: &mut Step) -> EngineResult<()> {
        let deadline = Instant::now() + MODULE_WALL;
        for round in 0..=MODULE_ROUNDS {
            if page.scripting.modules.pending.is_empty()
                && page.scripting.modules.pending_scripts.is_empty()
                && page.scripting.modules.frame_requests.is_empty()
            {
                return Ok(());
            }
            let give_up = round == MODULE_ROUNDS || Instant::now() >= deadline;
            self.fetch_modules(page, deadline, give_up)?;
            self.fetch_script_loads(page, deadline, give_up)?;
            step.changed |= self.fetch_frames(page, deadline, give_up)?;
            match self.script_step(page, json!({"kind":"modules"})) {
                Ok(next) => {
                    step.changed |= next.changed;
                    step.failed.extend(next.failed);
                }
                Err(error) if stops(&error) => return Err(error),
                Err(error) => {
                    page.scripting.modules.pending.clear();
                    let entry = crate::v9::ConsoleEntry {
                        level: "error".into(),
                        text: format!("Module scripts stopped: {}", error.message),
                    };
                    super::scripting::record_console(&mut page.scripting, vec![entry], 0);
                    return Ok(());
                }
            }
        }
        Ok(())
    }
    /** Fetch this round's modules, batched when there are several. */
    fn fetch_modules(
        &mut self,
        page: &mut Page,
        deadline: Instant,
        give_up: bool,
    ) -> EngineResult<()> {
        let requests = std::mem::take(&mut page.scripting.modules.pending);
        let base = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        let known = |url: &str, page: &Page| page.scripting.modules.known(url);
        let requests: Vec<ModuleRequest> = requests
            .into_iter()
            .filter(|request| !known(&request.url, page))
            .collect();
        if !give_up {
            let batch = requests
                .iter()
                .filter_map(|request| Url::parse(&request.url).ok())
                .map(|url| (module_request(&base, &url), base.clone()))
                .collect();
            self.prefetch(batch, deadline)?;
        }
        for request in requests {
            if known(&request.url, page) {
                continue;
            }
            let source = if give_up {
                None
            } else {
                self.fetch_module(page, &base, &request, deadline)?
            };
            hand_over(page, request.url, source, false);
        }
        Ok(())
    }
    /**
     * Fetch this round's classic script loads the way a browser fetches `<script src>`: with
     * the page's Content-Security-Policy (the element's nonce counts), a JavaScript media type
     * and the element's integrity metadata. A refusal or failure is delivered as failed, so the
     * element fires `error` instead of waiting forever.
     */
    fn fetch_script_loads(
        &mut self,
        page: &mut Page,
        deadline: Instant,
        give_up: bool,
    ) -> EngineResult<()> {
        let requests = std::mem::take(&mut page.scripting.modules.pending_scripts);
        let base = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        if !give_up {
            let batch = requests
                .iter()
                .filter(|request| !page.scripting.modules.known(&request.url))
                .filter_map(|request| classic_script_url(page, &base, request))
                .map(|url| {
                    (
                        WebRequest {
                            destination: Destination::Script,
                            ..WebRequest::get(url.as_str())
                        },
                        base.clone(),
                    )
                })
                .collect();
            self.prefetch(batch, deadline)?;
        }
        for request in requests {
            if page.scripting.modules.known(&request.url) {
                continue;
            }
            let source = if give_up {
                None
            } else {
                self.fetch_script_load(page, &base, &request, deadline)?
            };
            hand_over(page, request.url, source, true);
        }
        Ok(())
    }
    /** One classic script a page element asked for; None for any refusal or failure. */
    fn fetch_script_load(
        &mut self,
        page: &mut Page,
        base: &Url,
        request: &ScriptLoadRequest,
        deadline: Instant,
    ) -> EngineResult<Option<String>> {
        let Some(url) = classic_script_url(page, base, request) else {
            return Ok(None);
        };
        let Ok(source) = self.fetch_script(base, url.as_str(), deadline)? else {
            return Ok(None);
        };
        let verified = request
            .integrity
            .as_deref()
            .is_none_or(|metadata| super::integrity::matches(metadata, source.as_bytes()));
        Ok(verified.then_some(source))
    }
    /** One module, as a browser fetches a module script; None for any refusal or failure. */
    fn fetch_module(
        &mut self,
        page: &mut Page,
        base: &Url,
        request: &ModuleRequest,
        deadline: Instant,
    ) -> EngineResult<Option<String>> {
        let Ok(url) = Url::parse(&request.url) else {
            return Ok(None);
        };
        if !csp_allows(&page.document.csp, base, &url) {
            return Ok(None);
        }
        if url.scheme() == "data" {
            // A data: module is same-origin-clean and needs no network (old.reddit's bootstraps).
            return Ok(super::navigation::data_script(url.as_str()));
        }
        if !["http", "https"].contains(&url.scheme()) {
            return Ok(None);
        }
        let script = ScriptRequest {
            request_id: None,
            raw_response: false,
            worker_policy: None,
            url,
            method: Method::Get,
            body: Vec::new(),
            headers: Vec::new(),
            mode: cors::Mode::Cors,
            credentials: Credentials::SameOrigin,
        };
        let mut preflights = std::mem::take(&mut page.scripting.preflights);
        let answer = self.fetch_for_script(base, script, &mut preflights, deadline);
        page.scripting.preflights = preflights;
        let ScriptAnswer::Readable(response, _) = answer? else {
            return Ok(None);
        };
        let integrity = request
            .integrity
            .as_ref()
            .or_else(|| page.scripting.modules.integrity.get(&request.url));
        let verified = integrity
            .is_none_or(|metadata| super::integrity::matches(metadata, response.source.as_bytes()));
        let javascript = mars_core::http::is_javascript_mime_essence(&response.mime);
        Ok((response.status == 200 && javascript && verified).then_some(response.source))
    }
}

/**
 * Deliver one fetched source to V9 and, should V9's caps ever refuse it, say so in the console so
 * the refusal is never silent.
 */
fn hand_over(page: &mut Page, url: String, source: Option<String>, classic: bool) {
    if page.scripting.modules.deliver(url.clone(), source, classic) {
        return;
    }
    let entry = crate::v9::ConsoleEntry {
        level: "error".into(),
        text: format!("Script {url} was not delivered: the page has too many sources."),
    };
    super::scripting::record_console(&mut page.scripting, vec![entry], 0);
}

/** Prefetch and final delivery share admission, so blocked scripts never start a speculative hop. */
fn classic_script_url(page: &Page, base: &Url, request: &ScriptLoadRequest) -> Option<Url> {
    let url = Url::parse(&request.url).ok()?;
    let candidate = super::csp::Candidate {
        url: Some(&url),
        nonce: request.nonce.as_deref(),
        source: "",
    };
    (["http", "https", "data"].contains(&url.scheme())
        && super::csp::allows(&page.document.csp, base, &candidate))
    .then_some(url)
}

/** The first hop `fetch_for_script` sends for a module, so a batch prefetch can answer it. */
fn module_request(base: &Url, url: &Url) -> WebRequest {
    let cross = url.origin() != base.origin();
    WebRequest {
        destination: Destination::Empty,
        origin: cross.then(|| base.origin().ascii_serialization()),
        mode: Some(cors::Mode::Cors),
        ..WebRequest::get(url.as_str())
    }
}

/** Every policy admits the module: by its URL, or because the policy trusts what trusted
scripts load (`'strict-dynamic'`), which is how a module graph's imports are admitted. */
fn csp_allows(policies: &[String], base: &Url, url: &Url) -> bool {
    let candidate = super::csp::Candidate {
        url: Some(url),
        nonce: None,
        source: "",
    };
    policies.iter().all(|policy| {
        policy.to_ascii_lowercase().contains("'strict-dynamic'")
            || super::csp::allows(std::slice::from_ref(policy), base, &candidate)
    })
}

/** Authority, ownership and engine-service failures stop the operation. */
fn stops(error: &EngineError) -> bool {
    error.service.is_some()
        || ["permission_denied", "control_denied"].contains(&error.code.as_str())
}
