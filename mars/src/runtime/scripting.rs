/**
 * Page scripts run the way a browser runs them: V9 loads the page, lifecycle events fire, the
 * page's fetch and XMLHttpRequest calls go out through the governed network path, and timers
 * run on a virtual clock the coordinator advances. Every step lands in the candidate page
 * atomically, and nothing a script asks for (a navigation, a cookie, a request) happens without
 * the coordinator's own policy and the current owner's authority.
 */
use super::page::Page;
use super::resources::{Credentials, ScriptAnswer, ScriptRequest};
use super::supervisor::Runtime;
use crate::constants::{DIAGNOSTICS, SCRIPT_REQUESTS_PER_STEP, VM_RESPONSE_BYTES};
use crate::document::{Document, StyleSource};
use crate::network::Method;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{
    ConsoleEntry, DomLevel, Execution, HostRequest, HostResponse, NavigationRequest,
    PageEnvironment, ScriptFailure,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use url::Url;

/// Virtual milliseconds of timers and animation frames a page runs before it first commits, so
/// `setTimeout(0)` and requestAnimationFrame rendering is part of the first paint.
const SETTLE_VIRTUAL_MS: f64 = 200.0;
/// Real time one step may spend fetching the images its changes inserted.
const INSERTED_IMAGE_WALL: Duration = Duration::from_millis(1500);
/// Network time a started script request always gets, however much of the slice is left.
const SCRIPT_REQUEST_WALL: Duration = Duration::from_secs(3);
/// Real time the first commit may spend on that settling, inside the navigation deadline.
const SETTLE_WALL: Duration = Duration::from_millis(1500);
/// V9 round trips one settle may make (requests and timer runs together).
const SETTLE_STEPS: usize = 12;
/// Timer callbacks one V9 timer run may make.
const TIMER_BUDGET: usize = 64;
/// The only failure message a page sees, as in browsers: no status, headers or body.
const FAILED_TO_FETCH: &str = "Failed to fetch";
/// Fetch labels a string body with this when the page set no Content-Type.
const TEXT_BODY_TYPE: &str = "text/plain;charset=UTF-8";
/// Wall time between background timer runs while a page is open.
const TICK_INTERVAL: Duration = Duration::from_millis(100);
/// Real time one background run may spend, so the coordinator keeps answering people promptly.
const TICK_WALL: Duration = Duration::from_millis(400);
/// Background timers keep running this long after the page's last commit or interaction, then
/// wait for the next one, so an animation loop cannot churn revisions under an agent forever.
/// An app such as YouTube boots over many background runs, each a bounded slice of virtual
/// time, so the window spans what a slow boot needs.
const TICK_WINDOW: Duration = Duration::from_secs(20);
/// Virtual milliseconds one background run may advance, however long the page sat idle.
const TICK_MAX_VIRTUAL_MS: f64 = 1000.0;
/// Console entries the page keeps for page.console; older ones are dropped first.
pub(super) const CONSOLE_ENTRIES: usize = 4 * DIAGNOSTICS;
/// Navigations a script may chain (page A's script sends to B, whose script sends to C).
pub(super) const SCRIPT_NAVIGATIONS: u8 = 3;
/// Milliseconds per second, for the time origin and the virtual clock.
const MS_PER_SECOND: f64 = 1000.0;
/// Seconds per minute, for the timezone offset.
const SECONDS_PER_MINUTE: f64 = 60.0;

/// What the coordinator keeps for a page's scripts between V9 requests.
#[derive(Clone)]
pub(super) struct Scripting {
    /// Whether V9 has loaded this page (its state is then committed engine state).
    pub loaded: bool,
    /// Facts the page sees; the per-request fields are refreshed before each call.
    pub environment: PageEnvironment,
    /// Virtual milliseconds since the page's time origin.
    pub clock_ms: f64,
    /// When V9 next has a timer or animation frame due, in virtual milliseconds.
    pub next_timer_ms: Option<f64>,
    /// Requests the page made that have not been answered yet.
    pub requests: Vec<HostRequest>,
    /// The last navigation a script asked for, performed after the page commits.
    pub navigation: Option<NavigationRequest>,
    /// Bounded console output for page.console.
    pub console: Vec<ConsoleEntry>,
    /// Console entries dropped past the bound.
    pub console_dropped: usize,
    /// Background timers run until this instant; interactions move it forward.
    pub awake_until: Instant,
    /// The last background run, which sets how far the virtual clock moves next.
    pub last_tick: Instant,
    /// CORS preflights this page's requests already passed, so they are not repeated.
    pub preflights: crate::network::cors::PreflightCache,
    /// Wall time the last background run took. The next waits at least this long, so a page
    /// whose timer steps are slow keeps the coordinator free half the time for people and agents.
    pub tick_cost: Duration,
    /// Module scripts, import maps and the modules fetched for them.
    pub modules: super::modules::Modules,
    /// True while a navigation runs the page's load, lifecycle events and first settle: image
    /// work the steps bring in waits for `flush_inserted_images`, one restyle and one fetch
    /// batch for the whole phase instead of one per step.
    pub loading: bool,
    /// Image work steps brought in and have not fetched yet.
    pub pending_images: Option<PendingImages>,
    /// The viewport scrolled since the page last heard: the next run fires `scroll` first.
    pub scroll_pending: bool,
}

/// What a step changed that may need images: the first node identity its insertions took, and
/// the attributes it set. Steps of one load phase merge into one.
#[derive(Clone, Default)]
pub(super) struct PendingImages {
    /// Nodes with this identity or higher were inserted by scripts.
    pub first_new: usize,
    /// Attributes scripts set, as (node, attribute name).
    pub changed: Vec<(usize, String)>,
    /// A script set an attribute of <html> or <body>, which can restyle any element.
    pub root: bool,
}

impl PendingImages {
    /** The facts of one step: the first identity after the nodes it started with, and the
    attributes its changes set. */
    fn of(
        first_new: usize,
        changes: &[crate::document::DomChange],
        script_changes: &[crate::document::ScriptChange],
    ) -> Self {
        use crate::document::{DomChange, ScriptChange};
        let attribute = |change: &DomChange| match change {
            DomChange::SetAttribute { node, name, .. } => Some((*node, name.clone())),
            _ => None,
        };
        let changed = changes
            .iter()
            .filter_map(attribute)
            .chain(script_changes.iter().filter_map(|change| match change {
                ScriptChange::Attribute { node, name, .. } => Some((*node, name.clone())),
                ScriptChange::Dom { change } => attribute(change),
                _ => None,
            }))
            .collect();
        let root = script_changes
            .iter()
            .any(|change| matches!(change, ScriptChange::RootAttribute { .. }));
        Self {
            first_new,
            changed,
            root,
        }
    }
    /** Both steps' facts: identities are monotonic, so the earlier first identity covers both. */
    fn merge(self, later: Self) -> Self {
        let mut changed = self.changed;
        changed.extend(later.changed);
        Self {
            first_new: self.first_new.min(later.first_new),
            changed,
            root: self.root || later.root,
        }
    }
}

impl Default for Scripting {
    /** A page before V9 has loaded it. */
    fn default() -> Self {
        let now = Instant::now();
        Self {
            loaded: false,
            environment: PageEnvironment::default(),
            clock_ms: 0.0,
            next_timer_ms: None,
            requests: Vec::new(),
            navigation: None,
            console: Vec::new(),
            console_dropped: 0,
            awake_until: now + TICK_WINDOW,
            last_tick: now,
            preflights: crate::network::cors::PreflightCache::default(),
            tick_cost: Duration::ZERO,
            modules: super::modules::Modules::default(),
            loading: false,
            pending_images: None,
            scroll_pending: false,
        }
    }
}

impl Scripting {
    /** Keep background timers running for another window after a commit or interaction. */
    pub fn wake(&mut self) {
        let now = Instant::now();
        self.awake_until = now + TICK_WINDOW;
        self.last_tick = now;
    }
    /** Whether the page has script work waiting. */
    fn pending(&self) -> bool {
        self.loaded
            && (self.scroll_pending || self.next_timer_ms.is_some() || !self.requests.is_empty())
    }
}

/** v9.page parameters, written straight to the frame. */
#[derive(serde::Serialize)]
struct PageCall<'a> {
    sources: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    sources_digest: Option<&'a str>,
    document: &'a crate::document::ScriptDocument,
    storage: &'a crate::v9::StorageState,
    languages: &'a [String],
    environment: &'a PageEnvironment,
    state: Option<&'a crate::v9::VmState>,
    #[serde(skip_serializing_if = "is_classic")]
    modules: &'a crate::v9::ModuleInput,
    action: &'a Value,
}

/** A page without module scripts sends no module input. */
fn is_classic(modules: &&crate::v9::ModuleInput) -> bool {
    modules.entries.is_empty() && modules.sources.is_empty()
}

/// One V9 step's outcome once its changes are in the candidate page.
pub(super) struct Step {
    /// Whether the document changed (it then needs restyling and layout).
    pub changed: bool,
    /// Scripts the load rolled back, for the load's diagnostics.
    pub failed: Vec<ScriptFailure>,
}

impl Runtime {
    /// The environment a new page starts with: its URL and referrer, the presented browser, the
    /// viewport, a fresh time origin, the local timezone and a random seed of its own.
    pub(super) fn page_environment(
        &self,
        url: &str,
        referrer: String,
        scripts: Vec<String>,
        script_attributes: Vec<BTreeMap<String, String>>,
        formatting: &crate::document::DocumentFormatting,
    ) -> PageEnvironment {
        PageEnvironment {
            url: url.into(),
            referrer,
            user_agent: format!(
                "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) {} Safari/537.36",
                crate::network::CHROME_ENGINE
            ),
            viewport_width: self.render_context.width,
            viewport_height: self.render_context.height,
            time_origin_ms: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0.0, |elapsed| elapsed.as_secs_f64() * MS_PER_SECOND),
            timezone_offset_minutes: timezone_offset_minutes(),
            random_seed: random_seed(),
            dom_level: DomLevel::Extended,
            scripts,
            script_attributes,
            html_attributes: formatting.html.attributes.clone(),
            body_attributes: formatting.body.attributes.clone(),
            ..PageEnvironment::default()
        }
    }

    /// One V9 lifecycle step on the candidate page. The page changes only when the step, Eye's
    /// replay and the structure check all succeed; effects are recorded for the caller to act on.
    pub(super) fn script_step(&mut self, page: &mut Page, action: Value) -> EngineResult<Step> {
        let load = action["kind"] == "load";
        let started = Instant::now();
        let kind = action["kind"].as_str().unwrap_or_default().to_string();
        if !page.storage_dirty {
            page.storage = self.storage_for(&page.origin)?;
        }
        let projection = crate::document::script_document(&page.document, &page.script_identity)?;
        let first_new = projection
            .nodes
            .iter()
            .map(|node| node.local_id + 1)
            .max()
            .unwrap_or(0);
        page.inspection = None;
        let mut environment = self.step_environment(page)?;
        if load {
            // V9 builds the head records once, at load; later steps would only resend them.
            environment.metadata.clone_from(&page.document.metadata);
        }
        let mut execution = self.execute(page, &projection, &environment, &action)?;
        if execution.effects.stale_geometry
            && let Ok(geometry) = self.forced_geometry(page, &execution)
        {
            // V9 steps are pure, so the first run is dropped whole and the second commits.
            environment.geometry = geometry;
            environment.laid_out_before = None;
            execution = self.execute(page, &projection, &environment, &action)?;
        }
        let mut changed = !execution.changes.is_empty() || !execution.script_changes.is_empty();
        // A step that changed nothing keeps the document and the projection it was given: Eye
        // would return the same document, and at NODES that copy and its projections cost as
        // much as styling the page. The structure check still runs.
        let applied = changed
            .then(|| {
                self.apply_all(
                    &page.document,
                    &execution.changes,
                    &execution.script_changes,
                )
            })
            .transpose()?;
        let after = match &applied {
            Some(applied) => crate::document::script_document(applied, &page.script_identity)?,
            None => projection,
        };
        if execution.structure != after.digest() {
            return Err(EngineError::invalid(
                "The script and document engines disagree on the page structure.",
            ));
        }
        execution.state.validate_document(&after)?;
        let Execution {
            state,
            storage,
            failed,
            effects,
            module_requests,
            script_requests,
            changes,
            script_changes,
            ..
        } = execution;
        self.commit_document_cookies(&page.url, &effects.cookies)?;
        if let Some(applied) = applied {
            let before = page.document.styles.len();
            page.resolved_stylesheets
                .extend(inserted_sheets(&applied, before));
            page.document = applied;
            page.styled = false;
        }
        changed |= self.load_inserted_sheets(page, &effects.stylesheets)?;
        let pending = PendingImages::of(first_new, &changes, &script_changes);
        page.scripting.pending_images = Some(match page.scripting.pending_images.take() {
            Some(earlier) => earlier.merge(pending),
            None => pending,
        });
        if !page.scripting.loading {
            self.flush_inserted_images(page)?;
        }
        page.state = state;
        page.storage_dirty |= storage != page.storage;
        page.storage = storage;
        let scripting = &mut page.scripting;
        scripting.loaded = true;
        scripting.next_timer_ms = effects.next_timer_ms;
        scripting.requests.extend(effects.requests);
        if effects.navigation.is_some() {
            scripting.navigation = effects.navigation;
        }
        record_console(scripting, effects.console, effects.console_dropped);
        if let Some(title) = effects.title {
            page.document.title = title;
        }
        apply_history(page, &effects.history);
        super::supervisor::perf::record("script.step", started, || format!("{kind}\t{changed}"));
        page.scripting.modules.pending.extend(module_requests);
        page.scripting
            .modules
            .pending_scripts
            .extend(script_requests);
        let mut step = Step { changed, failed };
        if kind != "modules" {
            self.follow_modules(page, &mut step)?;
        }
        Ok(step)
    }

    /// Fetch the images the pending steps' changes brought in, as a browser starts loading an
    /// image when a script inserts it or sets its source: inserted img, picture and srcset
    /// images, images whose source attributes changed, and new CSS backgrounds of inserted or
    /// changed elements. The document is restyled first so hidden images are skipped and
    /// backgrounds are known; the page's existing images count against the same page budget.
    /// Steps that touched no node cost nothing. Ordinary failures show alt text; only authority
    /// and engine-service failures stop the step.
    pub(super) fn flush_inserted_images(&mut self, page: &mut Page) -> EngineResult<()> {
        let Some(PendingImages {
            first_new,
            changed,
            root,
        }) = page.scripting.pending_images.take()
        else {
            return Ok(());
        };
        let mut budget = super::images::Budget::spent(&page.images);
        if budget.exhausted() {
            return Ok(());
        }
        if changed.is_empty()
            && !root
            && !page.document.nodes.iter().any(|n| n.local_id >= first_new)
        {
            return Ok(());
        }
        let parent = |id: usize| {
            page.document
                .nodes
                .iter()
                .find(|n| n.local_id == id)
                .and_then(|n| n.parent)
        };
        let source_like =
            |name: &str| ["src", "srcset", "sizes"].contains(&name) || name.starts_with("data-");
        // A source's change reaches its picture's img through their shared parent.
        let sourced: std::collections::BTreeSet<usize> = changed
            .iter()
            .filter(|(_, name)| source_like(name))
            .flat_map(|(node, _)| [Some(*node), parent(*node)])
            .flatten()
            .collect();
        let touched: std::collections::BTreeSet<usize> =
            changed.iter().map(|(node, _)| *node).collect();
        let loaded: std::collections::BTreeSet<usize> = page.images.keys().copied().collect();
        // Backgrounds before the restyle: a class change on an ancestor (Wikipedia's client-js
        // switch on <html>) can give an untouched element a background or mask it lacked, and a
        // style recalc starts that image loading in Blink.
        let backgrounds_before: std::collections::BTreeSet<(usize, String)> =
            crate::document::background_urls(&page.document, &page.url)
                .into_iter()
                .collect();
        page.document = self.styled(
            &page.document,
            &page.resolved_stylesheets,
            self.render_context,
        )?;
        page.styled = true;
        let restyled: std::collections::BTreeSet<usize> =
            crate::document::background_urls(&page.document, &page.url)
                .into_iter()
                .filter(|pair| !backgrounds_before.contains(pair) && !loaded.contains(&pair.0))
                .map(|(id, _)| id)
                .collect();
        let wanted = |node: &crate::document::Node| {
            let id = node.local_id;
            id >= first_new
                || restyled.contains(&id)
                || (node.tag == "img"
                    && (sourced.contains(&id) || node.parent.is_some_and(|p| sourced.contains(&p))))
                || (touched.contains(&id) && !loaded.contains(&id))
        };
        let base = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        let mut notes = Vec::new();
        let deadline = Instant::now() + INSERTED_IMAGE_WALL;
        // The inserted images go out together, as in `build_page`.
        let left = crate::constants::IMAGE_COUNT.saturating_sub(page.images.len());
        self.prefetched.clear();
        let loaded = self
            .prefetch_images(&page.document, &page.url, deadline, &wanted, left)
            .and_then(|()| {
                self.load_images(
                    &mut page.document,
                    &base,
                    deadline,
                    &mut budget,
                    &wanted,
                    &mut notes,
                )
            });
        self.prefetched.clear();
        let images = match loaded {
            Ok(images) => images,
            Err(error) if super::images::stops_loading(&error) => return Err(error),
            Err(_) => {
                notes.push("Some images scripts inserted did not load in time.".into());
                BTreeMap::new()
            }
        };
        for node in page
            .document
            .nodes
            .iter()
            .filter(|node| node.tag == "img" && wanted(node))
        {
            page.images.remove(&node.local_id);
        }
        page.images.extend(images);
        for note in notes {
            if page.document.diagnostics.len() < DIAGNOSTICS
                && !page.document.diagnostics.contains(&note)
            {
                page.document.diagnostics.push(note);
            }
            if page.degraded.len() < DIAGNOSTICS && !page.degraded.iter().any(|f| f == "images") {
                page.degraded.push("images".into());
            }
        }
        Ok(())
    }

    /// Fetch the stylesheets scripts linked in this step and add them to the page's sheets, as a
    /// browser applies an inserted `<link rel=stylesheet>` (craigslist links its page CSS from
    /// its boot script). Each gets SCRIPT_REQUEST_WALL; the page's style allowance still holds.
    /// Returns whether a sheet was added, so the page restyles.
    fn load_inserted_sheets(&mut self, page: &mut Page, hrefs: &[String]) -> EngineResult<bool> {
        let base = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        let mut added = false;
        for href in hrefs {
            let deadline = Instant::now() + SCRIPT_REQUEST_WALL;
            let total: usize = page.resolved_stylesheets.iter().map(String::len).sum();
            let Some(css) = self.inserted_sheet(&base, href, deadline)? else {
                note_degraded(page);
                continue;
            };
            if total + css.len() > crate::constants::STYLE_TOTAL_BYTES {
                note_degraded(page);
                page.untrusted_styles = true;
                continue;
            }
            page.resolved_stylesheets.push(css);
            page.styled = false;
            added = true;
        }
        Ok(added)
    }

    /** One V9 page step over `projection`, sending the sources only when V9 does not hold them. */
    fn execute(
        &mut self,
        page: &Page,
        projection: &crate::document::ScriptDocument,
        environment: &PageEnvironment,
        action: &Value,
    ) -> EngineResult<Execution> {
        let load = action["kind"] == "load";
        let digest =
            (!page.scripts.is_empty()).then(|| super::service::sources_digest(&page.scripts));
        let resident = digest
            .as_ref()
            .is_some_and(|digest| self.v9_sources.contains(digest));
        let languages = self.languages.clone();
        let mut call = PageCall {
            sources: if resident { &[] } else { &page.scripts },
            sources_digest: digest.as_deref(),
            document: projection,
            storage: &page.storage,
            languages: &languages,
            environment,
            state: (!load).then_some(&page.state),
            modules: &page.scripting.modules.input,
            action,
        };
        let execution = match self.call_typed("v9", "v9.page", &call, None) {
            Err(error) if resident && error.code == super::service::SOURCES_MISSING => {
                call.sources = &page.scripts;
                self.call_typed("v9", "v9.page", &call, None)
            }
            result => result,
        }?;
        if let Some(digest) = digest {
            self.note_v9_sources(digest);
        }
        Ok(execution)
    }

    /**
     * The boxes of the page as `execution` left it, for a step whose script measured a node
     * inserted since the last layout. A browser lays out synchronously when script reads
     * geometry; this is the bounded equivalent: one cascade and layout of the candidate, only
     * for such a step, never for every step, and only once per step.
     */
    fn forced_geometry(
        &mut self,
        page: &Page,
        execution: &Execution,
    ) -> EngineResult<Vec<crate::v9::NodeBox>> {
        let applied = self.apply_all(
            &page.document,
            &execution.changes,
            &execution.script_changes,
        )?;
        let mut sheets = page.resolved_stylesheets.clone();
        sheets.extend(inserted_sheets(&applied, page.document.styles.len()));
        let styled = self.styled(&applied, &sheets, self.render_context)?;
        let display = self.render(&styled)?;
        Ok(boxes(&display))
    }

    /** Per-request facts: current URL, viewport, title, node text and the jar's script cookies. */
    fn step_environment(&mut self, page: &Page) -> EngineResult<PageEnvironment> {
        let mut environment = page.scripting.environment.clone();
        let url = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        environment.cookie = self.cookies.document_cookie(&url, SystemTime::now());
        environment.url = page.url.clone();
        environment.viewport_width = self.render_context.width;
        environment.viewport_height = self.render_context.height;
        environment.title = page.document.title.clone();
        environment.node_text = node_text(&page.document);
        // The last layout's boxes, in paint order. A node inserted since has none, and a step
        // that measures one is laid out and run again (`forced_geometry`).
        environment.geometry = page.display.as_ref().map(boxes).unwrap_or_default();
        environment.laid_out_before = page.display.is_some().then_some(page.laid_out_before);
        environment.positions = positions(&page.document);
        environment.scroll_y = self.scroll_y;
        Ok(environment)
    }

    /// Answer requests and run due timers until the page is quiet, `virtual_end` is reached,
    /// `until` passes or SETTLE_STEPS round trips are spent. Returns whether the document
    /// changed. A step V9 or Eye refuses stops the page's scripts rather than retrying forever.
    pub(super) fn settle_scripts(
        &mut self,
        page: &mut Page,
        until: Instant,
        virtual_end: f64,
    ) -> EngineResult<bool> {
        let mut changed = false;
        for _ in 0..SETTLE_STEPS {
            if Instant::now() >= until || !page.scripting.loaded {
                break;
            }
            let action = if page.scripting.scroll_pending {
                page.scripting.scroll_pending = false;
                json!({"kind":"event","event":"scroll"})
            } else if !page.scripting.requests.is_empty() {
                let requests = std::mem::take(&mut page.scripting.requests);
                let responses = self.perform_requests(page, requests, until)?;
                if responses.is_empty() {
                    continue;
                }
                json!({"kind":"resume","responses":responses})
            } else if page
                .scripting
                .next_timer_ms
                .is_some_and(|due| due <= virtual_end)
            {
                page.scripting.clock_ms = page.scripting.clock_ms.max(virtual_end);
                json!({"kind":"timers","now_ms":page.scripting.clock_ms,"budget":TIMER_BUDGET})
            } else {
                break;
            };
            match self.script_step(page, action) {
                Ok(step) => changed |= step.changed,
                Err(error) if stops_navigation(&error) => return Err(error),
                Err(error) => {
                    stop_scripts(page, &error);
                    break;
                }
            }
        }
        page.scripting.clock_ms = page.scripting.clock_ms.max(virtual_end);
        Ok(changed)
    }

    /// The first settle after load: SETTLE_VIRTUAL_MS of virtual time within SETTLE_WALL.
    pub(super) fn settle_after_load(
        &mut self,
        page: &mut Page,
        deadline: Instant,
    ) -> EngineResult<bool> {
        let until = deadline.min(Instant::now() + SETTLE_WALL);
        let end = page.scripting.clock_ms + SETTLE_VIRTUAL_MS;
        self.settle_scripts(page, until, end)
    }

    /// Perform a batch of script requests; beacons are sent and expect no answer. Requests start
    /// only while the slice has time and at most SCRIPT_REQUESTS_PER_STEP per batch; the rest
    /// stay pending for the next slice, as a browser's fetch waits for a busy main thread rather
    /// than failing. Each started request gets at least SCRIPT_REQUEST_WALL on the network, so
    /// a slice that slow script steps used up does not turn every answer into a network error.
    fn perform_requests(
        &mut self,
        page: &mut Page,
        requests: Vec<HostRequest>,
        until: Instant,
    ) -> EngineResult<Vec<HostResponse>> {
        let base = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        let mut responses = Vec::new();
        let mut requests = requests.into_iter();
        for sent in 0..SCRIPT_REQUESTS_PER_STEP {
            let started = Instant::now();
            // At least one request per slice, so a page always makes progress.
            if sent > 0 && started >= until {
                break;
            }
            let Some(request) = requests.next() else {
                break;
            };
            let deadline = until.max(started + SCRIPT_REQUEST_WALL);
            let preflights = &mut page.scripting.preflights;
            let response = self.script_request(&base, &request, preflights, deadline)?;
            if request.kind != "beacon" {
                responses.push(response);
            }
        }
        let later = std::mem::take(&mut page.scripting.requests);
        page.scripting.requests = requests.chain(later).collect();
        Ok(responses)
    }

    /**
     * One fetch or XMLHttpRequest under the Fetch standard's CORS rules (`fetch_for_script`):
     * forbidden headers are dropped, no-cors keeps only safelisted ones, a text body without a
     * Content-Type is labeled text/plain, and only GET and POST are sent. Whatever goes wrong,
     * the page sees one bare "Failed to fetch", never a status, header or body.
     */
    fn script_request(
        &mut self,
        base: &Url,
        request: &HostRequest,
        preflights: &mut crate::network::cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<HostResponse> {
        if request
            .url
            .get(..5)
            .is_some_and(|s| s.eq_ignore_ascii_case("data:"))
        {
            return Ok(data_response(request));
        }
        let Some(parsed) = script_request_from(request) else {
            return Ok(failed(request.id, FAILED_TO_FETCH));
        };
        let id = request.id;
        let answer = self.fetch_for_script(base, parsed, preflights, until)?;
        Ok(match answer {
            ScriptAnswer::Readable(response, headers) => readable(id, *response, headers),
            ScriptAnswer::Opaque => HostResponse {
                id,
                ..HostResponse::default()
            },
            ScriptAnswer::Failed => failed(id, FAILED_TO_FETCH),
        })
    }

    /// Run the visible page's due timers and answer its requests on the coordinator's idle
    /// schedule, under whoever holds control of the tab now. Returns whether a new revision was
    /// published, so the window repaints.
    pub fn run_page_scripts(&mut self) -> bool {
        let now = Instant::now();
        let due = self.page.as_ref().is_some_and(|page| {
            page.scripting.pending()
                && now < page.scripting.awake_until
                && now >= page.scripting.last_tick + TICK_INTERVAL.max(page.scripting.tick_cost)
        });
        if !due || self.healthy().is_err() {
            return false;
        }
        let Ok(permit) = self.presentation_permit(None) else {
            return false;
        };
        let previous = std::mem::replace(&mut self.permit, permit);
        let published = self.tick(now).unwrap_or(false);
        self.permit = previous;
        published
    }

    /** One background run: settle, then commit (a new revision only when the document changed). */
    fn tick(&mut self, now: Instant) -> EngineResult<bool> {
        let mut page = self.current()?.clone();
        let elapsed = now
            .saturating_duration_since(page.scripting.last_tick)
            .as_secs_f64()
            * MS_PER_SECOND;
        page.scripting.last_tick = now;
        let end = page.scripting.clock_ms + elapsed.min(TICK_MAX_VIRTUAL_MS);
        let changed = self.settle_scripts(&mut page, now + TICK_WALL, end)?;
        page.scripting.tick_cost = now.elapsed();
        if changed {
            self.finish_mutation(page)?;
            self.follow_script_navigation(0);
            return Ok(true);
        }
        let navigation = page.scripting.navigation.is_some();
        self.commit_quietly(page)?;
        if navigation {
            self.follow_script_navigation(0);
            return Ok(true);
        }
        Ok(false)
    }

    /** Commit engine state that changed no document: same revision, same layout. */
    pub(super) fn commit_quietly(&mut self, page: Page) -> EngineResult<()> {
        let rollback = if page.storage_dirty {
            self.write_storage(&page.origin, &page.storage)?
        } else {
            None
        };
        let origin = page.origin.clone();
        let storage = page.storage.clone();
        let published = self.publish(move |runtime| {
            let mut page = page;
            if page.storage_dirty {
                runtime.commit_storage(&origin, &storage);
            }
            page.storage_dirty = false;
            runtime.page = Some(page);
        });
        if published.is_err()
            && let Some(previous) = rollback
        {
            self.rollback_storage(previous);
        }
        published
    }

    /// Perform the navigation a committed page's script asked for, through the ordinary
    /// navigation path and the current owner's authority. Script navigations chain at most
    /// SCRIPT_NAVIGATIONS deep; a refused or failed one leaves the committed page in place.
    pub(super) fn follow_script_navigation(&mut self, hops: u8) -> Option<EngineResult<Value>> {
        let navigation = self.page.as_mut()?.scripting.navigation.take()?;
        if hops >= SCRIPT_NAVIGATIONS {
            return None;
        }
        let result = match navigation.kind.as_str() {
            "reload" => self.reload(json!({})),
            "back" | "forward" | "go" if navigation.delta < 0 => self.back(json!({})),
            "back" | "forward" | "go" if navigation.delta > 0 => self.forward(json!({})),
            "back" | "forward" | "go" => self.reload(json!({})),
            _ => self.script_navigate(&navigation.url, hops + 1),
        };
        Some(result)
    }
}

/** What the page asked for, parsed: None for a URL, method, mode or credentials it cannot use. */
fn script_request_from(request: &HostRequest) -> Option<ScriptRequest> {
    use crate::network::cors;
    let method = Method::parse(&request.method)?;
    let mode = cors::Mode::parse(&request.mode)?;
    // Fetch refuses a no-cors request whose method is not CORS-safelisted.
    if mode == cors::Mode::NoCors && !cors::safelisted_method(method.name()) {
        return None;
    }
    let mut headers: Vec<(String, String)> = request
        .headers
        .iter()
        .map(|(name, value)| (name.to_ascii_lowercase(), value.trim().to_owned()))
        .filter(|(name, value)| {
            !cors::forbidden_request_header(name)
                && (mode != cors::Mode::NoCors || cors::safelisted_request_header(name, value))
        })
        .take(crate::network::network::constants::SCRIPT_REQUEST_HEADERS)
        .collect();
    let body = if method.is_safe() {
        String::new()
    } else if request.body_bytes.is_empty() {
        request.body.clone().unwrap_or_default()
    } else {
        identity_body(&request.body_bytes, &mut headers)?
    };
    if request.body.is_some() && !headers.iter().any(|(name, _)| name == "content-type") {
        headers.push(("content-type".into(), TEXT_BODY_TYPE.into()));
    }
    Some(ScriptRequest {
        url: Url::parse(&request.url).ok()?,
        method,
        body,
        headers,
        mode,
        credentials: Credentials::parse(&request.credentials)?,
    })
}

/**
 * A binary body as the text Relay sends. Relay carries text bodies only, so a gzip or deflate
 * encoded body (YouTube gzips its Innertube JSON, the guide among it) goes out decoded, without
 * its Content-Encoding: the server reads the same JSON. Any other binary body, or one that does
 * not decode to UTF-8 within the body bound, cannot be sent and fails as a network error.
 * ponytail: decode-and-send until Relay's WebRequest body is bytes; then send them unchanged.
 */
fn identity_body(bytes: &[u8], headers: &mut Vec<(String, String)>) -> Option<String> {
    use std::io::Read;
    let encoding = headers
        .iter()
        .find(|(name, _)| name == "content-encoding")
        .map(|(_, value)| value.to_ascii_lowercase())?;
    let limit = crate::constants::VM_REQUEST_BODY_BYTES as u64 + 1;
    let mut decoded = Vec::new();
    match encoding.as_str() {
        "gzip" => flate2::read::GzDecoder::new(bytes)
            .take(limit)
            .read_to_end(&mut decoded),
        "deflate" => flate2::read::ZlibDecoder::new(bytes)
            .take(limit)
            .read_to_end(&mut decoded),
        _ => return None,
    }
    .ok()?;
    if decoded.len() as u64 >= limit {
        return None;
    }
    headers.retain(|(name, _)| name != "content-encoding");
    String::from_utf8(decoded).ok()
}

/** A readable final response, handed to the page if it is text within VM_RESPONSE_BYTES, with
only the headers the CORS rules expose. */
fn readable(
    id: u32,
    response: crate::network::WebResponse,
    headers: Vec<(String, String)>,
) -> HostResponse {
    if response.image_data || response.download || response.source.len() > VM_RESPONSE_BYTES {
        return failed(id, FAILED_TO_FETCH);
    }
    HostResponse {
        id,
        status: response.status,
        status_text: String::new(),
        url: response.url,
        headers,
        body: response.source,
        error: None,
    }
}

/** A `data:` URL fetched by script: decoded locally, as Fetch's scheme fetch does, never sent to
Relay. GET only, UTF-8 text within VM_RESPONSE_BYTES, with its media type as Content-Type;
anything else is a network error. YouTube reads its player configuration this way. */
fn data_response(request: &HostRequest) -> HostResponse {
    let decoded = super::images::data_body(&request.url, VM_RESPONSE_BYTES)
        .ok()
        .filter(|_| request.method == "GET")
        .and_then(|(mime, bytes)| Some((mime, String::from_utf8(bytes).ok()?)));
    let Some((mime, body)) = decoded else {
        return failed(request.id, FAILED_TO_FETCH);
    };
    HostResponse {
        id: request.id,
        status: 200,
        status_text: "OK".into(),
        url: request.url.clone(),
        headers: vec![("content-type".into(), mime)],
        body,
        error: None,
    }
}

/** A failed answer the page sees as a network error. */
fn failed(id: u32, message: &str) -> HostResponse {
    HostResponse {
        id,
        error: Some(message.into()),
        ..HostResponse::default()
    }
}

/** Authority, ownership and engine-service failures stop the operation; others stop scripts. */
fn stops_navigation(error: &EngineError) -> bool {
    error.service.is_some()
        || ["permission_denied", "control_denied"].contains(&error.code.as_str())
}

/** A refused step ends this page's script work; the page itself stays as committed. */
fn stop_scripts(page: &mut Page, error: &EngineError) {
    const REASON_CHARS: usize = 240;
    page.scripting.next_timer_ms = None;
    page.scripting.requests.clear();
    let entry = ConsoleEntry {
        level: "error".into(),
        text: format!("Page scripts stopped: {}", error.message),
    };
    record_console(&mut page.scripting, vec![entry], 0);
    let message = format!(
        "Page scripts stopped: {}",
        error.message.chars().take(REASON_CHARS).collect::<String>()
    );
    if page.document.diagnostics.len() < DIAGNOSTICS
        && !page.document.diagnostics.contains(&message)
    {
        page.document.diagnostics.push(message);
    }
    if page.degraded.len() < DIAGNOSTICS
        && !page.degraded.iter().any(|feature| feature == "scripts")
    {
        page.degraded.push("scripts".into());
    }
}

/** Keep the newest CONSOLE_ENTRIES entries. */
pub(super) fn record_console(
    scripting: &mut Scripting,
    entries: Vec<ConsoleEntry>,
    dropped: usize,
) {
    scripting.console.extend(entries);
    scripting.console_dropped += dropped;
    if scripting.console.len() > CONSOLE_ENTRIES {
        let excess = scripting.console.len() - CONSOLE_ENTRIES;
        scripting.console.drain(..excess);
        scripting.console_dropped += excess;
    }
}

/** pushState, replaceState and hash changes move the page's URL within its own origin only. */
fn apply_history(page: &mut Page, records: &[crate::v9::HistoryRecord]) {
    for record in records {
        let same_origin = Url::parse(&page.url)
            .ok()
            .zip(Url::parse(&record.url).ok())
            .is_some_and(|(current, next)| current.origin() == next.origin());
        if same_origin {
            page.url.clone_from(&record.url);
        }
    }
}

/** Text of text nodes and of elements holding their own text, by local id, for V9's reads. A
comment's data is its value, kept out of `text` so nothing paints or labels it, until a script
writes it as text. */
fn node_text(document: &Document) -> BTreeMap<usize, String> {
    document
        .nodes
        .iter()
        .map(|node| match node.tag.as_str() {
            "#comment" if node.text.is_empty() => (node.local_id, &node.value),
            _ => (node.local_id, &node.text),
        })
        .filter(|(_, text)| !text.is_empty())
        .map(|(id, text)| (id, text.clone()))
        .collect()
}

/** Local time minus UTC, in minutes, from the operating system's timezone rules. */
fn timezone_offset_minutes() -> f64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs()) as libc::time_t;
    // SAFETY: localtime_r writes only into the zeroed tm we own and reads the time we pass.
    let mut local: libc::tm = unsafe { std::mem::zeroed() };
    let converted = unsafe { libc::localtime_r(&now, &mut local) };
    if converted.is_null() {
        return 0.0;
    }
    local.tm_gmtoff as f64 / SECONDS_PER_MINUTE
}

/** A page's Math.random seed comes from the operating system's CSPRNG. */
fn random_seed() -> u64 {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0u8; 8];
    if SystemRandom::new().fill(&mut bytes).is_err() {
        return SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.subsec_nanos().into());
    }
    u64::from_le_bytes(bytes)
}

/** Inline style sheets scripts inserted join the page's sheets, as before. */
/** Each element's cascaded `position` where it is not static, for `getComputedStyle`. */
pub(super) fn positions(document: &Document) -> std::collections::BTreeMap<usize, String> {
    use crate::document::Position;
    document
        .nodes
        .iter()
        .filter_map(|node| {
            let name = match node.style.position {
                Position::Static => return None,
                Position::Relative => "relative",
                Position::Absolute => "absolute",
                Position::Fixed => "fixed",
                Position::Sticky => "sticky",
            };
            Some((node.local_id, name.to_owned()))
        })
        .collect()
}
/** A display list's boxes as V9 reads them, in paint order. */
pub(super) fn boxes(display: &crate::document::DisplayList) -> Vec<crate::v9::NodeBox> {
    display
        .items
        .iter()
        .map(|item| crate::v9::NodeBox {
            node: item.local_id,
            x: item.x,
            y: item.y,
            width: item.width,
            height: item.height,
        })
        .collect()
}
pub(super) fn inserted_sheets(
    applied: &Document,
    before: usize,
) -> impl Iterator<Item = String> + '_ {
    applied
        .styles
        .iter()
        .skip(before)
        .filter_map(|source| match source {
            StyleSource::Inline(css) => Some(css.clone()),
            StyleSource::Linked(_) | StyleSource::LinkedMedia { .. } => None,
        })
}

/** A script-inserted sheet that never applied is reported as navigation reports a linked one. A
 * failed load is only degraded, as a browser drops it too; one skipped for the style allowance
 * leaves its hiding rules unknown, so the caller also marks the page's styles untrusted and
 * agents fail closed. */
fn note_degraded(page: &mut Page) {
    if page.degraded.len() < DIAGNOSTICS && !page.degraded.iter().any(|f| f == "stylesheets") {
        page.degraded.push("stylesheets".into());
    }
}
