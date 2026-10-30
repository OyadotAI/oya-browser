/**
 * Module graphs load in rounds: V9 names the module URLs a graph needs, the coordinator fetches
 * them the way a browser fetches module scripts (CORS mode, same-origin credentials, a
 * JavaScript media type, the page's Content-Security-Policy and any integrity metadata), and the
 * next V9 step continues the graph. A module that cannot be fetched, or would pass MODULES or
 * MODULE_BYTES, is delivered as failed, which fails only the graphs that import it.
 */
use super::page::Page;
use super::resources::{Credentials, ScriptAnswer, ScriptRequest};
use super::scripting::Step;
use super::supervisor::Runtime;
use crate::constants::{MODULE_BYTES, MODULES, SCRIPT_BYTES};
use crate::document::{ScriptKind, ScriptMode, ScriptResource};
use crate::network::{Destination, Method, WebRequest, cors};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{
    EntryKind, FrameRequest, ModuleEntry, ModuleInput, ModuleRequest, ModuleSource,
    ScriptLoadRequest,
};
use serde_json::json;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use url::Url;

/// Fetch rounds one step may make; github.com's graph is three deep, and each round fetches
/// every module the last one found.
const MODULE_ROUNDS: usize = 8;
/// Real time one step may spend loading module graphs, inside the navigation's own deadline.
const MODULE_WALL: Duration = Duration::from_millis(4000);

/// A page's module scripts and the modules fetched for them, kept between V9 requests.
#[derive(Clone, Default)]
pub(super) struct Modules {
    /// What every V9 request carries: which sources are modules, and the fetched modules.
    pub input: ModuleInput,
    /// Integrity metadata of external module scripts, by URL.
    integrity: BTreeMap<String, String>,
    /// Modules V9 asked for that the next round fetches.
    pub pending: Vec<ModuleRequest>,
    /// Classic scripts page elements asked for (`<script src>` connected by script) that the
    /// next round fetches.
    pub pending_scripts: Vec<ScriptLoadRequest>,
    /// Bytes of delivered module sources, bounded by MODULE_BYTES.
    bytes: usize,
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
            modules.input.entries.push(ModuleEntry {
                index,
                kind: entry.kind,
                url: entry.url,
            });
        }
        modules
    }
    /** Hand one module or classic script (or its failure) to V9, refusing it past the page's
    module bounds, which fetched classic scripts share. */
    fn deliver(&mut self, url: String, source: Option<String>, classic: bool) {
        if self.input.sources.len() >= MODULES {
            return;
        }
        let fits = |source: &String| {
            source.len() <= SCRIPT_BYTES && self.bytes + source.len() + url.len() <= MODULE_BYTES
        };
        let source = source.filter(fits);
        self.bytes += url.len() + source.as_ref().map_or(0, String::len);
        self.input.sources.push(ModuleSource {
            url,
            source,
            classic,
        });
    }
    /** Whether a URL was delivered already, as a module or a classic script. */
    fn known(&self, url: &str) -> bool {
        self.input.sources.iter().any(|module| module.url == url)
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
            let full = page.scripting.modules.input.sources.len() >= MODULES;
            let source = if give_up || full {
                None
            } else {
                self.fetch_module(page, &base, &request, deadline)?
            };
            page.scripting.modules.deliver(request.url, source, false);
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
        for request in requests {
            if page.scripting.modules.known(&request.url) {
                continue;
            }
            let full = page.scripting.modules.input.sources.len() >= MODULES;
            let source = if give_up || full {
                None
            } else {
                self.fetch_script_load(page, &base, &request, deadline)?
            };
            page.scripting.modules.deliver(request.url, source, true);
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
        let Ok(url) = Url::parse(&request.url) else {
            return Ok(None);
        };
        let candidate = super::csp::Candidate {
            url: Some(&url),
            nonce: request.nonce.as_deref(),
            source: "",
        };
        if !["http", "https"].contains(&url.scheme())
            || !super::csp::allows(&page.document.csp, base, &candidate)
        {
            return Ok(None);
        }
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
        if !["http", "https"].contains(&url.scheme()) || !csp_allows(&page.document.csp, base, &url)
        {
            return Ok(None);
        }
        let script = ScriptRequest {
            worker_policy: None,
            url,
            method: Method::Get,
            body: String::new(),
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
