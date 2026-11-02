/**
 * Module graphs load in rounds: V9 names the module URLs a graph needs, the coordinator fetches
 * them the way a browser fetches module scripts (CORS mode, same-origin credentials, a
 * JavaScript media type, the page's Content-Security-Policy and any integrity metadata), and the
 * next V9 step continues the graph. A module that cannot be fetched, or would pass MODULE_SOURCES
 * or MODULE_BYTES, is delivered as failed, which fails only the graphs that import it.
 */
use super::page::Page;
use super::resources::{Credentials, Progress, ScriptAnswer, ScriptLoad, ScriptRequest};
use super::scripting::Step;
use super::supervisor::Runtime;
use crate::constants::{MODULE_BYTES, MODULE_SOURCES, SCRIPT_BYTES};
use crate::document::{ScriptKind, ScriptMode, ScriptResource};
use crate::network::{Method, cors};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{
    EntryKind, FrameRequest, ModuleEntry, ModuleInput, ModuleRequest, ModuleSource,
    ScriptLoadRequest,
};
use serde_json::json;
use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, Instant};
use url::Url;

/// Real time a script load may take on the loader, and the longest the `load` event waits for
/// the scripts and images the page's scripts started, inside the navigation's own deadline.
const MODULE_WALL: Duration = Duration::from_millis(15_000);
/// How long one wait for a landing hop lasts before the pre-load settle looks again.
const LOAD_WAIT_SLICE: Duration = Duration::from_millis(250);

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
    /// A load landed (or a frame arrived) since the last `modules` step, which delivers it.
    pub landed: bool,
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
     * Start what a step asked for: every module and classic script goes to the loader, as a
     * browser fetches an inserted `<script>` (CORS mode and same-origin credentials for a
     * module; no-cors with cookies for a classic script, its `crossorigin` attribute honoured),
     * under the page's Content-Security-Policy. The page keeps running meanwhile; `land` hands
     * each answer to V9 and the next `modules` step continues the graph. Fetching each script
     * on the coordinator held YouTube's page, and every other tab, about 150 ms per script.
     * Frames still load here, in one round.
     */
    pub(super) fn follow_modules(&mut self, page: &mut Page, step: &mut Step) -> EngineResult<()> {
        self.start_module_loads(page)?;
        self.start_script_loads(page)?;
        let frames = !page.scripting.modules.frame_requests.is_empty();
        let deadline = Instant::now() + MODULE_WALL;
        step.changed |= self.fetch_frames(page, deadline, false)?;
        if frames {
            page.scripting.modules.landed = true;
        }
        Ok(())
    }
    /** Every module V9 asked for, started; one with no admissible hop lands at once as failed. */
    fn start_module_loads(&mut self, page: &mut Page) -> EngineResult<()> {
        let requests = std::mem::take(&mut page.scripting.modules.pending);
        let base = page_base(page)?;
        for request in requests {
            if page.scripting.modules.known(&request.url) {
                continue;
            }
            let integrity = request
                .integrity
                .clone()
                .or_else(|| page.scripting.modules.integrity.get(&request.url).cloned());
            let load = ScriptLoad {
                url: request.url.clone(),
                classic: false,
                integrity,
            };
            match module_hop(page, &base, &request.url) {
                Hop::Fetch(script) => self.submit_load(page, &base, *script, load)?,
                Hop::Source(source) => land(page, load, source),
            }
        }
        Ok(())
    }
    /** Every classic script a page element asked for, started the same way. */
    fn start_script_loads(&mut self, page: &mut Page) -> EngineResult<()> {
        let requests = std::mem::take(&mut page.scripting.modules.pending_scripts);
        let base = page_base(page)?;
        for request in requests {
            if page.scripting.modules.known(&request.url) {
                continue;
            }
            let load = ScriptLoad {
                url: request.url.clone(),
                classic: true,
                integrity: request.integrity.clone(),
            };
            match classic_hop(page, &base, &request) {
                Hop::Fetch(script) => self.submit_load(page, &base, *script, load)?,
                Hop::Source(source) => land(page, load, source),
            }
        }
        Ok(())
    }
    /** One load's first hop to the loader; an answer without the network lands at once. */
    fn submit_load(
        &mut self,
        page: &mut Page,
        base: &Url,
        request: ScriptRequest,
        load: ScriptLoad,
    ) -> EngineResult<()> {
        let until = Instant::now() + MODULE_WALL;
        let mut preflights = std::mem::take(&mut page.scripting.preflights);
        let progress = self.start_script_load(base, request, load.clone(), &mut preflights, until);
        page.scripting.preflights = preflights;
        match progress? {
            Progress::Sent(state) => page.scripting.in_flight.push(*state),
            Progress::Answered(answer) => {
                let source = loaded_source(&load, answer);
                land(page, load, source);
            }
        }
        Ok(())
    }
    /**
     * What the `load` event waits for, as a browser's does: the images the scripts started and
     * the scripts they inserted, each delivered as it lands, within MODULE_WALL. Returns
     * whether the document changed.
     */
    pub(super) fn settle_resources_before_load(
        &mut self,
        page: &mut Page,
        deadline: Instant,
    ) -> EngineResult<bool> {
        let until = deadline.min(Instant::now() + MODULE_WALL);
        let mut changed = false;
        while Instant::now() < until {
            if page.scripting.modules.landed {
                page.scripting.modules.landed = false;
                changed |= self.script_step(page, json!({"kind":"modules"}))?.changed;
            } else if !page.scripting.image_loading.pending.is_empty() {
                changed |= self.settle_images_before_load(page, until)?;
            } else if loading(page) {
                changed |= self.deliver_landed(page, until)?;
            } else {
                break;
            }
        }
        Ok(changed)
    }
    /** Wait one slice for a hop to land, then deliver what did: script loads to V9 through
    `drain_landed`, and any fetch answers that landed with them to the page. */
    fn deliver_landed(&mut self, page: &mut Page, until: Instant) -> EngineResult<bool> {
        let slice = until.min(Instant::now() + LOAD_WAIT_SLICE);
        if !self.wait_for_network(page, slice) {
            return Ok(false);
        }
        let responses = self.perform_requests(page, Vec::new(), slice)?;
        if responses.is_empty() {
            return Ok(false);
        }
        let step = self.script_step(page, json!({"kind":"resume","responses":responses}))?;
        Ok(step.changed)
    }
}

/** Whether a script the page asked for is still on the loader. */
pub(super) fn loading(page: &Page) -> bool {
    page.scripting
        .in_flight
        .iter()
        .any(|hop| hop.load.is_some())
}

/** A landed load: its source (or failure) goes to V9, and the next `modules` step runs it. */
pub(super) fn land(page: &mut Page, load: ScriptLoad, source: Option<String>) {
    hand_over(page, load.url, source, load.classic);
    page.scripting.modules.landed = true;
}

/** The source a load's answer delivers: a 200 with a JavaScript media type whose bytes match
the integrity metadata. Anything else (opaque, failed, refused) is None, so the element fires
`error`, as a browser's does. */
pub(super) fn loaded_source(load: &ScriptLoad, answer: ScriptAnswer) -> Option<String> {
    let ScriptAnswer::Readable(response, _) = answer else {
        return None;
    };
    let verified = load
        .integrity
        .as_deref()
        .is_none_or(|metadata| super::integrity::matches(metadata, response.source.as_bytes()));
    let javascript = mars_core::http::is_javascript_mime_essence(&response.mime);
    (response.status == 200 && javascript && verified).then_some(response.source)
}

/** A load's first hop, or the source it has without the network (a `data:` URL, a refusal).
The request is boxed so the small variant stays small. */
enum Hop {
    Fetch(Box<ScriptRequest>),
    Source(Option<String>),
}

/** A module's hop: CORS mode, same-origin credentials, admitted by every policy. */
fn module_hop(page: &Page, base: &Url, url: &str) -> Hop {
    let Ok(url) = Url::parse(url) else {
        return Hop::Source(None);
    };
    if !csp_allows(&page.document.csp, base, &url) {
        return Hop::Source(None);
    }
    if url.scheme() == "data" {
        // A data: module is same-origin-clean and needs no network (old.reddit's bootstraps).
        return Hop::Source(super::navigation::data_script(url.as_str()));
    }
    if !["http", "https"].contains(&url.scheme()) {
        return Hop::Source(None);
    }
    Hop::Fetch(Box::new(script_request(
        url,
        cors::Mode::Cors,
        Credentials::SameOrigin,
    )))
}

/** A classic script's hop: no-cors with cookies unless `crossorigin` asks for CORS. */
fn classic_hop(page: &Page, base: &Url, request: &ScriptLoadRequest) -> Hop {
    let Some(url) = classic_script_url(page, base, request) else {
        return Hop::Source(None);
    };
    if url.scheme() == "data" {
        return Hop::Source(super::navigation::data_script(url.as_str()));
    }
    let (mode, credentials) = match request.cross_origin.as_deref() {
        Some("use-credentials") => (cors::Mode::Cors, Credentials::Include),
        Some(_) => (cors::Mode::Cors, Credentials::SameOrigin),
        None => (cors::Mode::NoCors, Credentials::Include),
    };
    Hop::Fetch(Box::new(script_request(url, mode, credentials)))
}

/** A GET for a script, as the sync module fetch built it. */
fn script_request(url: Url, mode: cors::Mode, credentials: Credentials) -> ScriptRequest {
    ScriptRequest {
        request_id: None,
        raw_response: false,
        worker_policy: None,
        url,
        method: Method::Get,
        body: Vec::new(),
        headers: Vec::new(),
        mode,
        credentials,
    }
}

/** The page URL every load resolves against. */
fn page_base(page: &Page) -> EngineResult<Url> {
    Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))
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
