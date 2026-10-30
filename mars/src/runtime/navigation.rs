/** Navigation publishes one candidate while history explicitly prevents POST replay. */
use super::{
    page::{Empty, Page},
    resources::{Acquired, uncertain},
    supervisor::{Runtime, perf},
};
use crate::{
    constants::{DIAGNOSTICS, HISTORY_ENTRIES, STYLE_SHEETS, STYLE_TOTAL_BYTES, WEB_TIMEOUT},
    document::{Document, StyleSource},
    network::{Method, WebRequest, WebResponse},
    protocol::{EngineError, EngineResult, parameters},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::Instant;

/** History retains the internal destination without exposing protected queries. */
#[derive(Clone)]
pub(super) struct HistoryEntry {
    pub(super) url: String,
    pub(super) sensitive: bool,
    pub(super) replayable: bool,
    /// The `Page::generation` of the document that made this entry; entries pushState added
    /// share their document's, so moving between them needs no reload.
    pub(super) document: u64,
    /// The state pushState saved as JSON, for `popstate`; None for a load or hash change.
    pub(super) state: Option<String>,
}
/** Navigation never accepts request headers or policy overrides. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Navigate {
    url: String,
}
#[path = "navigation_imports.rs"]
mod imports;
/** What a load skipped: bounded, deduplicated diagnostics and the feature names behind them. */
#[derive(Default)]
pub(super) struct Gaps {
    /// Messages added to the document's diagnostics.
    diagnostics: Vec<String>,
    /// Feature names ("scripts", "stylesheets", "images") for `page.degraded`.
    degraded: Vec<String>,
    /// A stylesheet could not reach Eye, so its hiding rules are unknown: no node may be
    /// agent-actionable on this page.
    untrusted_styles: bool,
}
impl Gaps {
    /** Record one skipped resource; both lists stay under DIAGNOSTICS and never repeat. */
    fn note(&mut self, feature: &str, message: String) {
        if self.degraded.len() < DIAGNOSTICS && !self.degraded.iter().any(|f| f == feature) {
            self.degraded.push(feature.into());
        }
        if self.diagnostics.len() < DIAGNOSTICS && !self.diagnostics.contains(&message) {
            self.diagnostics.push(message);
        }
    }
    /** Fold into the document's own diagnostics under the same bound. */
    fn record(&self, document: &mut Document) {
        let new: Vec<_> = self
            .diagnostics
            .iter()
            .filter(|message| !document.diagnostics.contains(message))
            .cloned()
            .collect();
        document
            .diagnostics
            .truncate(DIAGNOSTICS.saturating_sub(new.len()));
        document.diagnostics.extend(new);
    }
}
/** One script ready for V9, in execution order, remembering where the author put it. */
struct Resolved {
    /// Whether acquisition was refused rather than successfully returning empty source.
    refused: bool,
    /// Source text.
    source: String,
    /// Zero-based position in the document.
    index: usize,
    /// External URL with query and fragment removed, or None for an inline script.
    url: Option<String>,
    /// A module script or import map, which V9's module loader runs; None for a classic script.
    module: Option<super::modules::Entry>,
    /// The element's attributes, for the head record V9 gives the script.
    attributes: BTreeMap<String, String>,
}
/** History is changed only when its corresponding page commits. */
enum History {
    New,
    At(usize),
}

impl Runtime {
    /** A committed page's pushState, replaceState and hash changes enter history (see
    `same_document`), then the current entry follows the page's URL, never fetching. */
    pub(super) fn sync_history_url(&mut self) {
        self.adopt_history_records();
        let (Some(index), Some(page)) = (self.history_cursor, self.page.as_ref()) else {
            return;
        };
        if let Some(entry) = self.history.get_mut(index)
            && entry.url != page.url
        {
            entry.url.clone_from(&page.url);
        }
    }
    /** Restored destinations use GET and keep query data out of inspection. */
    pub(super) fn navigate_restored_request(&mut self, url: &str) -> EngineResult<()> {
        self.load_request(WebRequest::get(url), true, History::New)
            .map(|_| ())
    }

    /** Address and tool navigation use the same candidate pipeline. */
    pub(super) fn navigate(&mut self, params: Value) -> EngineResult<Value> {
        let input: Navigate = parameters(params)?;
        self.load_request(WebRequest::get(&input.url), false, History::New)
    }
    /** Revisit never changes the cursor on a failed load or replay refusal. */
    pub(super) fn revisit(&mut self, index: usize) -> EngineResult<Value> {
        if self.same_document(index) {
            return self.traverse_same_document(index);
        }
        let entry = &self.history[index];
        if !entry.replayable {
            return Err(replay());
        }
        self.load_request(
            WebRequest::get(&entry.url),
            entry.sensitive,
            History::At(index),
        )
    }
    /** Back cannot silently turn a POST response into a GET. */
    pub(super) fn back(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let index = self
            .history_cursor
            .and_then(|i| i.checked_sub(1))
            .ok_or_else(no_history)?;
        self.revisit(index)
    }
    /** Forward follows the same explicit replay policy. */
    pub(super) fn forward(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let index = self
            .history_cursor
            .map(|i| i + 1)
            .filter(|i| *i < self.history.len())
            .ok_or_else(no_history)?;
        self.revisit(index)
    }
    /** Reload never automatically repeats a server mutation. */
    pub(super) fn reload(&mut self, params: Value) -> EngineResult<Value> {
        let _: Empty = parameters(params)?;
        let index = self.history_cursor.ok_or_else(no_history)?;
        self.revisit(index)
    }
    /** Relative links retain the sensitive-chain projection of their base document. */
    pub(super) fn follow(&mut self, href: &str) -> EngineResult<Value> {
        let page = self.current()?;
        let sensitive = page.sensitive;
        let url = url::Url::parse(&page.url)
            .and_then(|url| url.join(href))
            .map_err(|_| EngineError::invalid("Invalid relative link URL."))?;
        self.load_request(WebRequest::get(url.as_str()), sensitive, History::New)
    }
    /** Form proposals acquire external-effect authority separately from page publication. */
    pub(super) fn navigate_submission(
        &mut self,
        submission: crate::document::Submission,
    ) -> EngineResult<Value> {
        let page = self.current()?;
        let sensitive =
            page.sensitive || (submission.protected && submission.method == Method::Get);
        let mut url = url::Url::parse(&page.url)
            .and_then(|base| base.join(&submission.action))
            .map_err(|_| EngineError::invalid("Invalid form action URL."))?;
        let mut request = WebRequest::get("");
        request.method = submission.method;
        if request.method == Method::Get {
            url.set_query(Some(&submission.encoded));
        } else {
            request.body = submission.encoded;
        }
        url.set_fragment(None);
        request.url = url.into();
        self.load_request(request, sensitive, History::New)
    }
    /** A script's navigation (location assignment, window.open) to an absolute URL, as a link
    from the current page: the same pipeline, sensitivity and authority as a click. */
    pub(super) fn script_navigate(&mut self, url: &str, hops: u8) -> EngineResult<Value> {
        let sensitive = self.current()?.sensitive;
        self.load_request_from(WebRequest::get(url), sensitive, History::New, hops)
    }
    /** Ordinary navigations start a script navigation chain. */
    fn load_request(
        &mut self,
        request: WebRequest,
        sensitive: bool,
        history: History,
    ) -> EngineResult<Value> {
        self.load_request_from(request, sensitive, history, 0)
    }
    /** Expensive parsing, VM and layout happen outside the brief final authority lock. A
    navigation the committed page's scripts asked for follows, `hops` deep in the chain. */
    fn load_request_from(
        &mut self,
        request: WebRequest,
        sensitive: bool,
        history: History,
        hops: u8,
    ) -> EngineResult<Value> {
        self.ensure_storage_ready()?;
        self.ready_download = None;
        let is_post = request.method == Method::Post;
        let started = Instant::now();
        perf::begin("navigation");
        let result = self.candidate(request, sensitive).and_then(|page| {
            perf::record("page.candidate", started, String::new);
            let started = Instant::now();
            let Some(page) = page else {
                return self.download_ready();
            };
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
                runtime.generation = page.generation;
                let entry = HistoryEntry {
                    url: page.url.clone(),
                    sensitive: page.sensitive,
                    replayable: page.replayable,
                    document: page.generation,
                    state: None,
                };
                // ponytail: history calls made while the page loaded fold into its one entry
                // (page.url is already their last URL); Chrome adds an entry per pushState.
                page.scripting.history_records.clear();
                match history {
                    History::New => {
                        let keep = runtime.history_cursor.map(|i| i + 1).unwrap_or_default();
                        runtime.history.truncate(keep);
                        runtime.history.push(entry);
                        if runtime.history.len() > HISTORY_ENTRIES {
                            runtime.history.remove(0);
                        }
                        runtime.history_cursor = Some(runtime.history.len() - 1);
                    }
                    History::At(index) => {
                        runtime.history[index] = entry;
                        runtime.history_cursor = Some(index);
                    }
                }
                runtime.scroll_y = 0.0;
                runtime.page = Some(page);
                runtime.adopt_script_scroll();
                runtime.inspection()
            });
            if published.is_err()
                && let Some(previous) = rollback
            {
                self.rollback_storage(previous);
            }
            let published = published?;
            perf::record("page.publish", started, String::new);
            Ok(match self.follow_script_navigation(hops) {
                Some(Ok(followed)) => followed,
                _ => published,
            })
        });
        result.map_err(|mut error| {
            if is_post
                && !["submission_uncertain", "validation_error"].contains(&error.code.as_str())
            {
                if error.code == "permission_denied" {
                    error.message =
                        "The form was not sent. The destination or current ownership refused it."
                            .into();
                } else if error.code == "unsupported_feature" {
                    error = uncertain();
                }
            }
            if !is_post
                && error.service.is_none()
                && !["permission_denied", "control_denied", "invalid_request"]
                    .contains(&error.code.as_str())
            {
                error.retryable = true;
            }
            error.previous_page_retained = self.page.is_some();
            error
        })
    }
    /** The committed page stays; the file waits for the desktop to save it, never parsed or run. */
    fn download_ready(&self) -> EngineResult<Value> {
        let ready = self
            .ready_download
            .as_ref()
            .ok_or_else(|| EngineError::service("network", false))?;
        Ok(json!({"download_ready":{
            "url":ready.url,"mime":ready.mime,"disposition":ready.disposition,"bytes":ready.bytes.len()
        }}))
    }
    /** Required CSS is acquired before hidden content can gain native geometry. A response the
    network service marked as a download is held in `ready_download` and yields no page. */
    fn candidate(&mut self, request: WebRequest, sensitive: bool) -> EngineResult<Option<Page>> {
        let context = self.residency.reserve(self.page.is_some())?;
        let deadline = Instant::now() + WEB_TIMEOUT;
        let initiator = self
            .page
            .as_ref()
            .and_then(|p| url::Url::parse(&p.url).ok());
        // HTML forms and links only navigate with GET or POST; other methods are for page scripts.
        if !matches!(request.method, Method::Get | Method::Post) {
            return Err(EngineError::unsupported(
                "Navigations use only GET and POST.",
            ));
        }
        let fetch = Instant::now();
        let acquired = if self.web_origins.is_some() {
            let acquired = self.acquire(request, deadline, initiator, true, true)?;
            perf::record("page.fetch", fetch, || {
                acquired.response.source.len().to_string()
            });
            acquired
        } else {
            if request.method != Method::Get {
                return Err(EngineError::unsupported(
                    "Headless forms require desktop networking.",
                ));
            }
            let value = self.call("network", "network.fetch", json!({"url":request.url}))?;
            Acquired {
                response: WebResponse {
                    source: value["source"]
                        .as_str()
                        .ok_or_else(|| EngineError::service("network", false))?
                        .into(),
                    image_data: false,
                    url: request.url,
                    status: 200,
                    mime: "text/html".into(),
                    location: None,
                    set_cookies: Vec::new(),
                    charset: value["charset"].as_str().map(str::to_owned),
                    download: false,
                    disposition: None,
                    headers: Vec::new(),
                },
                method: Method::Get,
                diagnostics: Vec::new(),
            }
        };
        if acquired.response.download && acquired.method == Method::Get {
            self.ready_download =
                Some(super::downloads::Fetched::from_download(acquired.response)?);
            return Ok(None);
        }
        let post = acquired.method == Method::Post;
        self.prefetched.clear();
        let built = self.build_page(acquired, deadline, sensitive, None, context);
        self.prefetched.clear();
        built
            .map(Some)
            .map_err(|error| if post { uncertain() } else { error })
    }
    /** Cookie network-state commits precede document parsing, under current authority. A page
    built for a frame (`embedding`) loads no frames of its own and paints nothing early. */
    pub(super) fn build_page(
        &mut self,
        acquired: Acquired,
        deadline: Instant,
        sensitive: bool,
        embedding: Option<&super::frames::Embedding>,
        context: std::sync::Arc<super::residency::Lease>,
    ) -> EngineResult<Page> {
        // XHTML and SVG documents take Eye's XML parser, as Blink picks XMLDocumentParser.
        let xml = crate::document::xml_mime(&acquired.response.mime);
        if acquired.response.mime != "text/html" && !xml {
            return Err(EngineError::unsupported(
                "The document requires UTF-8 text/html.",
            ));
        }
        let total = Instant::now();
        let mut phase = Instant::now();
        let parse = json!({"source":acquired.response.source,"charset":acquired.response.charset,"xml":xml});
        let mut document: Document = self.call_typed("document", "document.parse", &parse, None)?;
        document
            .csp
            .extend(super::csp::response_policies(&acquired.response.headers));
        if embedding.is_some() {
            crate::document::normalize_document_text(&mut document)?;
        }
        perf::record("page.parse", phase, || document.nodes.len().to_string());
        phase = Instant::now();
        document.diagnostics.extend(acquired.diagnostics);
        let script_resources = std::mem::take(&mut document.scripts);
        self.prefetch_head(
            &document,
            &script_resources,
            &acquired.response.url,
            deadline,
        )?;
        let mut gaps = Gaps::default();
        let sheets = self.stylesheets(&document, &acquired.response.url, deadline, &mut gaps)?;
        perf::record("page.fetch_styles", phase, || {
            sheets.iter().map(String::len).sum::<usize>().to_string()
        });
        phase = Instant::now();
        document = self.styled(&document, &sheets, self.render_context)?;
        perf::record("page.styles", phase, String::new);
        phase = Instant::now();
        let all = crate::constants::IMAGE_COUNT;
        self.prefetch_images(&document, &acquired.response.url, deadline, &|_| true, all)?;
        let fonts = document.formatting.fonts.len();
        let images =
            self.resolve_images(&mut document, &acquired.response.url, deadline, &mut gaps)?;
        // `ch` and `ic` resolved against fallback faces before the web fonts arrived; Blink
        // restyles font-relative lengths when a font loads, so the page styles once more.
        if document.formatting.font_metrics && document.formatting.fonts.len() > fonts {
            document = self.styled(&document, &sheets, self.render_context)?;
        }
        perf::record("page.images", phase, || images.len().to_string());
        phase = Instant::now();
        let mut images = images;
        let frame_tree = if embedding.is_none() {
            self.initial_frame_tree(&document)?
        } else {
            Default::default()
        };
        let (frames, scripted_frames) = match embedding {
            Some(_) => Default::default(),
            None => self.load_frames(
                &mut document,
                &acquired.response.url,
                deadline,
                &mut images,
                &frame_tree.routes,
            )?,
        };
        perf::record("page.frames", phase, || frames.len().to_string());
        phase = Instant::now();
        let resolved = self.resolve_scripts(
            script_resources,
            &acquired.response.url,
            &document.csp,
            deadline,
            &mut gaps,
        )?;
        perf::record("page.fetch_scripts", phase, || {
            let bytes: usize = resolved.iter().map(|script| script.source.len()).sum();
            format!("{}\t{bytes}", resolved.len())
        });
        let origin = canonical_origin(&acquired.response.url)?;
        let base_storage = self.storage_for(&origin)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("page generations", usize::MAX))?;
        let script_identity = context.identity.clone();
        let mut environment = self.page_environment(
            &acquired.response.url,
            self.referrer(&acquired.response.url),
            resolved
                .iter()
                .map(|script| script.url.clone().unwrap_or_default())
                .collect(),
            resolved
                .iter()
                .map(|script| script.attributes.clone())
                .collect(),
            &document.formatting,
        );
        environment.refused_scripts = resolved
            .iter()
            .enumerate()
            .filter_map(|(index, script)| script.refused.then_some(index))
            .collect();
        super::csp::apply_environment(&document.csp, &mut environment);
        environment.frame_documents = scripted_frames
            .into_iter()
            .map(|frame| (frame.address.clone(), frame))
            .collect();
        if embedding.is_none() {
            environment.scripted_frame_owners =
                self.scripted_owners(&document, &acquired.response.url);
        }
        if let Some(embedding) = embedding {
            environment.embedder_origin = Some(embedding.origin.clone());
            environment.window_name.clone_from(&embedding.name);
            environment.parent_frames.clone_from(&embedding.routes);
            environment.own_window = Some(embedding.window);
        }
        let mut page = Page {
            _context: context,
            generation,
            script_identity,
            revision: 1,
            document,
            scripts: resolved
                .iter()
                .map(|script| script.source.clone())
                .collect(),
            resolved_stylesheets: sheets,
            top_level_url: embedding
                .map(|embedding| embedding.top_level_url.clone())
                .unwrap_or_else(|| acquired.response.url.clone()),
            url: acquired.response.url,
            sensitive,
            replayable: acquired.method == Method::Get,
            status: acquired.response.status,
            state: Default::default(),
            state_token: None,
            images,
            frames: std::sync::Arc::new(frames),
            frame_tree,
            display: None,
            laid_out_before: 0,
            layout_generation: 1,
            element_scroll: Default::default(),
            scroll_x: 0.0,
            styled: true,
            inspection: None,
            origin,
            storage_dirty: false,
            storage: base_storage,
            degraded: Vec::new(),
            untrusted_styles: false,
            scripting: super::scripting::Scripting {
                attempted_frame_owners: environment.scripted_frame_owners.clone(),
                environment,
                modules: super::modules::Modules::new(
                    resolved
                        .iter()
                        .enumerate()
                        .filter_map(|(index, script)| Some((index, script.module.clone()?))),
                ),
                ..Default::default()
            },
        };
        super::animations::refresh_timelines(&mut page)?;
        phase = Instant::now();
        // First paint: people see the parsed and styled page while its scripts run, as a
        // browser paints before load. The window is the only observer; agents get the page
        // once it has settled, as before.
        if !resolved.is_empty()
            && embedding.is_none()
            && self.provisional.is_some()
            && self.web_origins.is_some()
        {
            self.prepare_display(&mut page)?;
            self.first_paint(&page);
            perf::record("page.first_paint", phase, String::new);
        }
        page.scripting.loading = true;
        let restyle = self.run_scripts(&mut page, &resolved, deadline, &mut gaps);
        page.scripting.loading = false;
        let restyle = restyle?;
        self.flush_inserted_images(&mut page)?;
        perf::record("page.scripts", phase, || restyle.to_string());
        phase = Instant::now();
        gaps.record(&mut page.document);
        // Script steps already noted what they inserted and could not apply (sheets, images);
        // the load's own gaps join them rather than replace them.
        for feature in std::mem::replace(&mut page.degraded, gaps.degraded) {
            if page.degraded.len() < DIAGNOSTICS && !page.degraded.contains(&feature) {
                page.degraded.push(feature);
            }
        }
        page.untrusted_styles |= gaps.untrusted_styles;
        page.scripting.wake();
        // `styled` alone cannot say the first paint stands: inserted images restyle the page
        // without laying it out, which left example.com's script-built link with no box.
        if !restyle && page.styled && page.display.is_some() && !page.untrusted_styles {
            // Scripts left the first paint's document as it was: its layout stands, only the
            // page's notes changed.
            page.refresh_inspection();
            page.publishable()?;
        } else {
            self.prepare_current(&mut page)?;
        }
        perf::record("page.prepare", phase, || {
            page.document.nodes.len().to_string()
        });
        perf::record("page.build", total, || page.url.clone());
        Ok(page)
    }
    /**
     * Run the page's scripts the way a browser does: V9 isolates each one, so a script that
     * fails is rolled back alone and noted. Its source stays as it was: its top-level declarations
     * still exist in a browser, so later requests must compile them, and no later step runs a
     * script that already ran. Then
     * DOMContentLoaded and load fire and the page settles (its requests answered, its first
     * timers run). If V9 or the document engine refuses the whole load, the page still loads
     * without script effects. Returns whether the document changed.
     */
    fn run_scripts(
        &mut self,
        page: &mut Page,
        resolved: &[Resolved],
        deadline: Instant,
        gaps: &mut Gaps,
    ) -> EngineResult<bool> {
        let loaded = match self.script_step(page, json!({"kind":"load"})) {
            Ok(step) => step,
            Err(error) if fatal(&error) => return Err(error),
            Err(error) => {
                for script in resolved {
                    gaps.note("scripts", script_note(script, &error.message));
                }
                page.scripts.clear();
                page.scripting.modules = Default::default();
                self.script_step(page, json!({"kind":"load"}))?
            }
        };
        for failure in &loaded.failed {
            if let Some(script) = resolved.get(failure.index) {
                let note = script_note(script, &failure.error.message);
                let entry = crate::v9::ConsoleEntry {
                    level: "error".into(),
                    text: note.clone(),
                };
                super::scripting::record_console(&mut page.scripting, vec![entry], 0);
                gaps.note("scripts", note);
            }
        }
        let mut changed = loaded.changed;
        for event in ["dom_content_loaded", "load"] {
            if event == "load" {
                page.scripting.load_pending = true;
                changed |= self.settle_images_before_load(page, deadline)?;
                if !page.scripting.image_loading.pending.is_empty() {
                    break;
                }
                page.scripting.load_pending = false;
            }
            match self.script_step(page, json!({"kind":"event","event":event})) {
                Ok(step) => changed |= step.changed,
                Err(error) if fatal(&error) => return Err(error),
                Err(error) => {
                    let reason = error.message.chars().take(REASON_CHARS).collect::<String>();
                    gaps.note(
                        "scripts",
                        format!("The page's {event} handlers did not run: {reason}"),
                    );
                    break;
                }
            }
        }
        // Where a browser fires `load`: scripts ran, DOMContentLoaded and load handlers ran.
        perf::since("navigation", "page.load_event");
        Ok(self.settle_after_load(page, deadline)? || changed)
    }
    /** What a new page's `document.referrer` says: the page it came from, origin only when the
    origins differ or that page's address was sensitive, as Chrome's default policy does. */
    fn referrer(&self, url: &str) -> String {
        let Some(previous) = self.page.as_ref() else {
            return String::new();
        };
        let (Ok(mut from), Ok(to)) = (url::Url::parse(&previous.url), url::Url::parse(url)) else {
            return String::new();
        };
        if previous.sensitive || from.origin() != to.origin() {
            return format!("{}/", from.origin().ascii_serialization());
        }
        from.set_fragment(None);
        from.into()
    }
    /**
     * Starts every linked stylesheet and every external script the page may run at once, as a
     * browser's preload scanner does, so `stylesheets` and `resolve_scripts` find their answers
     * waiting. Only what they would request is sent: the first STYLE_SHEETS sheets and the
     * first SCRIPT_COUNT scripts the page's Content-Security-Policy allows.
     */
    fn prefetch_head(
        &mut self,
        document: &Document,
        scripts: &[crate::document::ScriptResource],
        base: &str,
        deadline: Instant,
    ) -> EngineResult<()> {
        let Ok(base) = url::Url::parse(base) else {
            return Ok(());
        };
        let request = |url: url::Url, destination| {
            let request = WebRequest {
                destination,
                ..WebRequest::get(url.as_str())
            };
            (request, base.clone())
        };
        let sheets = document
            .styles
            .iter()
            .filter_map(|source| match source {
                StyleSource::Linked(href) | StyleSource::LinkedMedia { href, .. } => Some(href),
                StyleSource::Inline(_) => None,
            })
            .take(STYLE_SHEETS)
            .filter_map(|href| base.join(href).ok())
            .map(|url| request(url, crate::network::Destination::Style));
        let code = scripts
            .iter()
            .take(crate::constants::SCRIPT_COUNT)
            .filter_map(|script| {
                let url = base.join(script.src.as_deref()?).ok()?;
                let candidate = super::csp::Candidate {
                    url: Some(&url),
                    nonce: script.nonce.as_deref(),
                    source: &script.source,
                };
                super::csp::allows(&document.csp, &base, &candidate).then_some(url)
            })
            .map(|url| request(url, crate::network::Destination::Script));
        let requests = sheets.chain(code).collect();
        self.prefetch(requests, deadline)
    }
    /**
     * Starts the images `load_images` will read first for the nodes `wanted` selects, together:
     * each visible img's first candidate source and each CSS background, up to `limit` (what is left of IMAGE_COUNT). A fallback candidate, needed
     * only when the first fails, is still fetched on its own.
     */
    pub(super) fn prefetch_images(
        &mut self,
        document: &Document,
        base: &str,
        deadline: Instant,
        wanted: &dyn Fn(&crate::document::Node) -> bool,
        limit: usize,
    ) -> EngineResult<()> {
        self.prefetch_images_skipping(
            document,
            base,
            deadline,
            wanted,
            limit,
            &std::collections::BTreeSet::new(),
        )
    }

    /** Omit only lifecycle-owned image sources; ordinary background prefetch remains unchanged. */
    pub(super) fn prefetch_images_skipping(
        &mut self,
        document: &Document,
        base: &str,
        deadline: Instant,
        wanted: &dyn Fn(&crate::document::Node) -> bool,
        limit: usize,
        skip: &std::collections::BTreeSet<usize>,
    ) -> EngineResult<()> {
        let Ok(base) = url::Url::parse(base) else {
            return Ok(());
        };
        let viewport = self.render_context.width;
        let firsts = document
            .nodes
            .iter()
            .filter(|node| {
                node.tag == "img"
                    && !node.hidden()
                    && wanted(node)
                    && !skip.contains(&node.local_id)
            })
            .filter_map(|node| {
                super::images::sources(document, node, viewport)
                    .into_iter()
                    .next()
            })
            .map(|(src, _)| src);
        let backgrounds = crate::document::background_urls(document, base.as_str())
            .into_iter()
            // A later layer's key names its element; the canvas's is always wanted.
            .filter(|(id, _)| {
                crate::document::background_owner(*id).is_none_or(|owner| {
                    document
                        .nodes
                        .iter()
                        .find(|node| node.local_id == owner)
                        .is_some_and(wanted)
                })
            })
            .map(|(_, url)| url);
        let mut requests: Vec<(WebRequest, url::Url)> = firsts
            .chain(backgrounds)
            .take(limit)
            .filter(|src| {
                !src.get(..5)
                    .is_some_and(|s| s.eq_ignore_ascii_case("data:"))
            })
            .filter_map(|src| base.join(src.trim()).ok())
            .map(|url| {
                let request = WebRequest {
                    destination: crate::network::Destination::Image,
                    image: !super::images::svg_address(&url),
                    ..WebRequest::get(url.as_str())
                };
                (request, base.clone())
            })
            .collect();
        // The text's font files load beside the images, so the first paint has its faces.
        requests.extend(
            crate::document::font_requests(document)
                .into_iter()
                .filter_map(|font| base.join(&font.url).ok())
                .map(|url| {
                    let request = WebRequest {
                        destination: crate::network::Destination::Font,
                        ..WebRequest::get(url.as_str())
                    };
                    (request, base.clone())
                }),
        );
        self.prefetch(requests, deadline)
    }
    /**
     * Optional static images share governed networking, selected from the styled document so
     * hidden images are not fetched and CSS backgrounds are known. An ordinary failure, or an
     * image past IMAGE_COUNT or the page byte budgets, shows alt text.
     */
    fn resolve_images(
        &mut self,
        document: &mut Document,
        base: &str,
        deadline: Instant,
        gaps: &mut Gaps,
    ) -> EngineResult<std::collections::BTreeMap<usize, super::images::ImageAsset>> {
        let base =
            url::Url::parse(base).map_err(|_| EngineError::invalid("Invalid image base URL."))?;
        let mut notes = Vec::new();
        let images = self.load_images(
            document,
            &base,
            deadline,
            &mut super::images::Budget::default(),
            &|_| true,
            &mut notes,
        )?;
        for note in notes {
            gaps.note("images", note);
        }
        Ok(images)
    }
    /**
     * Resource destinations inherit no fixture grant beyond the configured exact origins. A
     * linked sheet that fails or is not text/css is skipped with a diagnostic. A sheet over
     * STYLE_BYTES still goes to Eye, which keeps its hiding rules as agent masks. A sheet that
     * would pass STYLE_TOTAL_BYTES (or a linked sheet past STYLE_SHEETS) cannot cross the pipe,
     * so its hiding rules are unknown and the page fails closed for agents.
     */
    pub(super) fn stylesheets(
        &mut self,
        document: &Document,
        base: &str,
        deadline: Instant,
        gaps: &mut Gaps,
    ) -> EngineResult<Vec<String>> {
        let base = url::Url::parse(base)
            .map_err(|_| EngineError::invalid("Invalid document base URL."))?;
        let mut sheets = Vec::new();
        let mut linked = 0;
        let mut total = 0usize;
        for source in &document.styles {
            let css = match source {
                StyleSource::Inline(css) => {
                    Some(self.with_imports(&base, css.clone(), deadline, (gaps, 0))?)
                }
                StyleSource::Linked(href) | StyleSource::LinkedMedia { href, .. } => {
                    linked += 1;
                    if linked > STYLE_SHEETS {
                        gaps.note("stylesheets", format!("Stylesheets after the first {STYLE_SHEETS} linked ones were skipped; agents may not act on this page."));
                        gaps.untrusted_styles = true;
                        None
                    } else {
                        self.linked_sheet(&base, href, deadline, (gaps, 0))?
                            .map(|css| match source {
                                StyleSource::LinkedMedia { media, .. } => {
                                    format!("@media {media} {{\n{css}\n}}")
                                }
                                _ => css,
                            })
                    }
                }
            };
            let Some(css) = css else { continue };
            if total + css.len() > STYLE_TOTAL_BYTES {
                gaps.note(
                    "stylesheets",
                    "Stylesheets past the page's style allowance were skipped; agents may not act on this page.".into(),
                );
                gaps.untrusted_styles = true;
                continue;
            }
            total += css.len();
            sheets.push(css);
        }
        Ok(sheets)
    }
    /** A stylesheet a page script linked after load, fetched as a linked sheet is; None when it
    cannot be loaded as text/css. */
    pub(super) fn inserted_sheet(
        &mut self,
        base: &url::Url,
        href: &str,
        deadline: Instant,
    ) -> EngineResult<Option<String>> {
        self.linked_sheet(base, href, deadline, (&mut Gaps::default(), 0))
    }
    /** One linked sheet (`depth` imports deep) with its own imports placed before it; only
    authority, service and deadline failures stop the navigation. */
    fn linked_sheet(
        &mut self,
        base: &url::Url,
        href: &str,
        deadline: Instant,
        (gaps, depth): (&mut Gaps, usize),
    ) -> EngineResult<Option<String>> {
        let skipped = |gaps: &mut Gaps| {
            gaps.note(
                "stylesheets",
                "A stylesheet could not be loaded as UTF-8 text/css and was skipped.".into(),
            );
            Ok(None)
        };
        let Ok(url) = base.join(href) else {
            return skipped(gaps);
        };
        if url.scheme() == "data" {
            return match data_text(url.as_str(), crate::constants::STYLE_TOTAL_BYTES) {
                Some((mime, css)) if mime == "text/css" => Ok(Some(css)),
                _ => skipped(gaps),
            };
        }
        match self.acquire(
            WebRequest {
                destination: crate::network::Destination::Style,
                ..WebRequest::get(url.as_str())
            },
            deadline,
            Some(base.clone()),
            false,
            true,
        ) {
            Ok(acquired)
                if acquired.response.status == 200 && acquired.response.mime == "text/css" =>
            {
                let css = crate::document::rebase_urls(&acquired.response.source, url.as_str());
                Ok(Some(self.with_imports(
                    &url,
                    css,
                    deadline,
                    (gaps, depth),
                )?))
            }
            Err(error) if fatal(&error) => Err(error),
            _ => skipped(gaps),
        }
    }
    /**
     * External scripts use the shared deadline and reject redirects before following them.
     * Fetching stays in document order; the scripts come back in execution order (blocking,
     * then defer, then async, each in document order, which the spec allows because the parse
     * has finished before any script runs). A script that cannot be fetched, is not
     * JavaScript, or would pass SCRIPT_COUNT, SCRIPT_BYTES or SCRIPT_TOTAL_BYTES is skipped
     * with a diagnostic; only authority and service failures stop the navigation.
     */
    fn resolve_scripts(
        &mut self,
        resources: Vec<crate::document::ScriptResource>,
        base: &str,
        csp: &[String],
        deadline: Instant,
        gaps: &mut Gaps,
    ) -> EngineResult<Vec<Resolved>> {
        use crate::constants::{SCRIPT_BYTES, SCRIPT_COUNT, SCRIPT_TOTAL_BYTES};
        let base = url::Url::parse(base)
            .map_err(|_| EngineError::invalid("Invalid document base URL."))?;
        let mut total = 0usize;
        let mut ready = Vec::new();
        for (index, script) in resources.into_iter().enumerate() {
            let url = script.src.as_deref().map(|src| {
                base.join(src)
                    .map(|url| super::page::safe_url(url.as_str(), true))
            });
            let resources_mode = script.mode;
            let mut skip = |gaps: &mut Gaps, url: Option<String>, reason: &str| {
                let script = Resolved {
                    refused: true,
                    source: String::new(),
                    index,
                    url,
                    module: None,
                    attributes: script.attributes.clone(),
                };
                gaps.note("scripts", script_note(&script, reason));
                if index < SCRIPT_COUNT {
                    ready.push((resources_mode, script));
                }
            };
            if index >= SCRIPT_COUNT {
                skip(
                    gaps,
                    None,
                    &format!("only the first {SCRIPT_COUNT} scripts run"),
                );
                continue;
            }
            let raw = script.src.as_deref().and_then(|src| base.join(src).ok());
            let candidate = super::csp::Candidate {
                url: raw.as_ref(),
                nonce: script.nonce.as_deref(),
                source: &script.source,
            };
            if !super::csp::allows(csp, &base, &candidate) {
                skip(
                    gaps,
                    url.and_then(Result::ok),
                    "the page's Content-Security-Policy forbids it",
                );
                continue;
            }
            if let Some(module) = super::modules::entry(&script, raw.as_ref()) {
                // Module graphs load through V9's module loader, which asks for each module.
                let source = if script.src.is_some() {
                    String::new()
                } else {
                    script.source
                };
                match url {
                    Some(Err(_)) => skip(gaps, None, "its URL is invalid"),
                    _ if total + source.len() > SCRIPT_TOTAL_BYTES => skip(
                        gaps,
                        url.and_then(Result::ok),
                        "it is over the script size budget",
                    ),
                    url => {
                        total += source.len();
                        let url = url.and_then(Result::ok);
                        let module = Some(module);
                        ready.push((
                            script.mode,
                            Resolved {
                                refused: false,
                                source,
                                index,
                                url,
                                module,
                                attributes: script.attributes,
                            },
                        ));
                    }
                }
                continue;
            }
            // Integrity only constrains fetched scripts; inline scripts ignore it, as in browsers.
            let integrity = script.src.as_ref().and(script.integrity);
            let source = match (&url, script.src) {
                (Some(Err(_)), _) => {
                    skip(gaps, None, "its URL is invalid");
                    continue;
                }
                (Some(Ok(_)), Some(src)) => match self
                    .fetch_script(&base, &src, deadline)
                    .map_err(|error| at_script(error, index))?
                {
                    Ok(source) => source,
                    Err(reason) => {
                        skip(gaps, url.and_then(Result::ok), reason);
                        continue;
                    }
                },
                _ => script.source,
            };
            let url = url.and_then(Result::ok);
            if integrity
                .as_deref()
                .is_some_and(|metadata| !super::integrity::matches(metadata, source.as_bytes()))
            {
                skip(gaps, url, "its integrity hash does not match");
                continue;
            }
            if source.len() > SCRIPT_BYTES || total + source.len() > SCRIPT_TOTAL_BYTES {
                skip(gaps, url, "it is over the script size budget");
                continue;
            }
            total += source.len();
            let module = None;
            ready.push((
                script.mode,
                Resolved {
                    refused: false,
                    source,
                    index,
                    url,
                    module,
                    attributes: script.attributes,
                },
            ));
        }
        ready.sort_by_key(|(mode, script)| (*mode, script.index));
        Ok(ready.into_iter().map(|(_, script)| script).collect())
    }
    /** The outer error stops navigation; the inner one is the reason this one script is skipped. */
    pub(super) fn fetch_script(
        &mut self,
        base: &url::Url,
        src: &str,
        deadline: Instant,
    ) -> EngineResult<Result<String, &'static str>> {
        let Ok(url) = base.join(src) else {
            return Ok(Err("its URL is invalid"));
        };
        if url.scheme() == "data" {
            return Ok(
                match data_text(url.as_str(), crate::constants::SCRIPT_BYTES) {
                    Some((mime, source)) if javascript_mime(&mime) => Ok(source),
                    Some(_) => Err("it did not return JavaScript"),
                    None => Err("it could not be loaded"),
                },
            );
        }
        let acquired = match self.acquire(
            WebRequest {
                destination: crate::network::Destination::Script,
                ..WebRequest::get(url.as_str())
            },
            deadline,
            Some(base.clone()),
            false,
            false,
        ) {
            Ok(acquired) => acquired,
            Err(error) if fatal(&error) => return Err(error),
            Err(_) => return Ok(Err("it could not be loaded")),
        };
        Ok(if acquired.response.status != 200 {
            Err("it could not be loaded")
        } else if !javascript_mime(&acquired.response.mime) {
            Err("it did not return JavaScript")
        } else {
            Ok(acquired.response.source)
        })
    }
}

/**
 * A stylesheet or script carried in a `data:` URL, decoded here as images are: Relay fetches
 * only network URLs, and facebook.com ships its sheets and scripts this way. The media type
 * essence and UTF-8 text, or None when it is malformed, past `limit` or not UTF-8.
 */
fn data_text(url: &str, limit: usize) -> Option<(String, String)> {
    let (mime, bytes) = super::images::data_body(url, limit).ok()?;
    Some((mime, String::from_utf8(bytes).ok()?))
}
/** Characters of an engine reason a diagnostic quotes. */
const REASON_CHARS: usize = 240;
/** A navigation-stopping script error names the script's document position, never its URL. */
fn at_script(mut error: EngineError, index: usize) -> EngineError {
    let location = error.location.get_or_insert_with(Default::default);
    location.script_index = Some(index + 1);
    error
}
/** Authority, ownership and engine-service failures stop a load; everything else degrades. */
fn fatal(error: &EngineError) -> bool {
    error.service.is_some()
        || ["permission_denied", "control_denied"].contains(&error.code.as_str())
}
/** A script's diagnostic names its document position and, for external scripts, its URL. */
fn script_note(script: &Resolved, reason: &str) -> String {
    let place = script.url.as_deref().unwrap_or("inline");
    let reason = reason.chars().take(REASON_CHARS).collect::<String>();
    format!(
        "Script {} ({place}) did not run: {reason}",
        script.index + 1
    )
}
/** Normalize browser storage ownership to the standards-defined serialized origin. */
fn canonical_origin(value: &str) -> EngineResult<String> {
    let url =
        url::Url::parse(value).map_err(|_| EngineError::invalid("Invalid storage origin."))?;
    Ok(url.origin().ascii_serialization())
}
/** Only common UTF-8 classic JavaScript media types are executable in this slice. */
fn javascript_mime(mime: &str) -> bool {
    mars_core::http::is_javascript_mime_essence(mime)
}
/** A prior POST requires an explicit new form action, never a generic Retry. */
fn replay() -> EngineError {
    EngineError::new(
        "resubmission_required",
        "This page came from a form submission and cannot be reloaded automatically.",
        "Return to the form and submit it deliberately; no request was replayed.",
    )
}
/** Missing history remains a recoverable operation error. */
fn no_history() -> EngineError {
    EngineError::new(
        "invalid_target",
        "No destination exists in that history direction.",
        "Navigate to another page first.",
    )
}

#[cfg(test)]
mod diagnostic_tests {
    use super::*;

    /** Recovery details retain the actual failed script when layout warnings filled the cap. */
    #[test]
    fn failed_resource_diagnostics_displace_old_warnings_without_growing() {
        let mut document = crate::document::parse("<p>kept</p>").unwrap();
        document.diagnostics = (0..DIAGNOSTICS)
            .map(|index| format!("CSS warning {index}"))
            .collect();
        let mut gaps = Gaps::default();
        gaps.note("scripts", "Script 1 did not run: download refused".into());
        gaps.record(&mut document);
        assert_eq!(document.diagnostics.len(), DIAGNOSTICS);
        assert_eq!(
            document.diagnostics.last().unwrap(),
            "Script 1 did not run: download refused"
        );
        gaps.record(&mut document);
        assert_eq!(document.diagnostics.len(), DIAGNOSTICS);
    }
}
