/**
 * Runtime.evaluate, callFunctionOn, getProperties, releaseObject and awaitPromise as native
 * operations: one V9 devtools request over the page's committed realm, applied to the candidate
 * page exactly as a lifecycle step is (`scripting::script_step`, mirrored here because the
 * request's answer rides beside the Execution). With control the candidate commits like a
 * click's; `page.devtools_peek` reads the realm in place and keeps nothing.
 */
use super::super::{page::Page, scripting, supervisor::Runtime};
use crate::document::Document;
use crate::protocol::{EngineError, EngineResult, parameters};
use crate::v9::{PageEnvironment, devtools::Outcome};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::time::{Duration, Instant, SystemTime};
use url::Url;

/// Settle rounds one awaited promise may wait for its page work (requests, timers).
const AWAIT_ROUNDS: usize = 24;
/// Wall time one awaited promise may hold the runtime.
const AWAIT_WALL: Duration = Duration::from_millis(2500);
/// Virtual milliseconds each settle round advances the page's clock, so timers fire.
const AWAIT_VIRTUAL_MS: f64 = 250.0;

/** page.devtools and page.devtools_peek: the V9 request, passed through to the realm. */
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Devtools {
    request: Value,
}

/** v9.devtools parameters, written straight to the frame as `scripting::PageCall` is. */
#[derive(serde::Serialize)]
struct DevtoolsCall<'a> {
    sources: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    sources_digest: Option<&'a str>,
    document: &'a crate::document::ScriptDocument,
    storage: &'a crate::v9::StorageState,
    languages: &'a [String],
    environment: &'a PageEnvironment,
    /** Always set: V9 keeps the step's state and replies with its token and summary. */
    resident: bool,
    /** The resident state the step starts from; none for a page's first load. */
    base: Option<u64>,
    /** The committed state of the same page, which V9 keeps beside `base` for an abort. */
    keep: Option<u64>,
    #[serde(skip_serializing_if = "classic")]
    modules: &'a crate::v9::ModuleInput,
    request: &'a Value,
}

/** A page without module scripts sends no module input. */
fn classic(modules: &&crate::v9::ModuleInput) -> bool {
    modules.entries.is_empty() && modules.sources.is_empty()
}

impl Runtime {
    /** page.devtools: the request under control; its changes commit like any mutation. */
    pub(in crate::runtime) fn devtools(&mut self, params: Value) -> EngineResult<Value> {
        let input: Devtools = parameters(params)?;
        self.publish(|_| ())?;
        self.run_devtools(input.request, true)
    }

    /** page.devtools_peek: the request as a read; nothing it did survives the call. */
    pub(in crate::runtime) fn devtools_peek(&mut self, params: Value) -> EngineResult<Value> {
        let input: Devtools = parameters(params)?;
        self.run_devtools(input.request, false)
    }

    /** Run the request, await a pending promise while allowed, then commit or discard. */
    fn run_devtools(&mut self, request: Value, commit: bool) -> EngineResult<Value> {
        let mut page = self.current()?.clone();
        let started = Instant::now();
        let options = request.get("options").cloned().unwrap_or_else(|| json!({}));
        let (request, appended) = place_source(&mut page, request)?;
        let mut outcome = self.devtools_step(&mut page, &request, commit)?;
        if appended && !outcome.retain {
            page.scripts.pop();
        }
        let mut changed = outcome.changed;
        let mut rounds = 0;
        while commit && outcome.result["pending"] == true && rounds < AWAIT_ROUNDS {
            if started.elapsed() >= AWAIT_WALL {
                break;
            }
            let end = page.scripting.clock_ms + AWAIT_VIRTUAL_MS;
            changed |= self.settle_scripts(&mut page, started + AWAIT_WALL, end)?;
            let await_request =
                json!({"kind":"await","handle":outcome.result["handle"],"options":options});
            outcome = self.devtools_step(&mut page, &await_request, commit)?;
            changed |= outcome.changed;
            rounds += 1;
        }
        let mut result = outcome.result;
        result["console"] = json!(outcome.console);
        if !commit {
            result["ephemeral"] = json!(true);
            result["page_id"] = json!(page.id());
            result["revision"] = json!(page.revision);
            return Ok(result);
        }
        let navigation = page.scripting.navigation.is_some();
        // An agent's evaluation is input even when the timers it set have not fired yet.
        page.scripting.wake();
        let (page_id, revision) = if changed {
            let inspection = self.finish_mutation(page)?;
            (
                inspection["page_id"].clone(),
                inspection["revision"].clone(),
            )
        } else {
            let ids = (json!(page.id()), json!(page.revision));
            self.commit_quietly(page)?;
            ids
        };
        result["page_id"] = page_id;
        result["revision"] = revision;
        if navigation && let Some(Ok(navigated)) = self.follow_script_navigation(0) {
            result["navigated"] = json!({"page_id":navigated["page_id"],"url":navigated["url"]});
        }
        Ok(result)
    }

    /**
     * One devtools request over the candidate page: V9 runs it, Eye replays its changes, and
     * the page takes the new state, exactly as `script_step` does for a lifecycle step. With
     * `effects` false (a peek) cookies, images and pending requests are dropped, since nothing
     * will be committed.
     */
    fn devtools_step(
        &mut self,
        page: &mut Page,
        request: &Value,
        effects: bool,
    ) -> EngineResult<Step> {
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
        let mut environment = self.devtools_environment(page)?;
        if !page.scripting.loaded {
            environment.metadata.clone_from(&page.document.metadata);
        }
        let base = page.scripting.loaded.then_some(page.state_token).flatten();
        let keep = self.committed_token(page);
        let digest = (!page.scripts.is_empty())
            .then(|| super::super::service::sources_digest(&page.scripts));
        let resident = digest
            .as_ref()
            .is_some_and(|digest| self.v9_sources.contains(digest));
        let languages = self.languages.clone();
        let mut call = DevtoolsCall {
            sources: if resident { &[] } else { &page.scripts },
            sources_digest: digest.as_deref(),
            document: &projection,
            storage: &page.storage,
            languages: &languages,
            environment: &environment,
            resident: true,
            base,
            keep,
            modules: &page.scripting.modules.input,
            request,
        };
        let outcome: Outcome = match self.call_typed("v9", "v9.devtools", &call, None) {
            Err(error) if resident && error.code == super::super::service::SOURCES_MISSING => {
                call.sources = &page.scripts;
                self.call_typed("v9", "v9.devtools", &call, None)
            }
            result => result,
        }?;
        if let Some(digest) = digest {
            self.note_v9_sources(digest);
        }
        let execution = outcome.execution;
        let changed = !execution.changes.is_empty() || !execution.script_changes.is_empty();
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
        if effects {
            self.commit_document_cookies(&page.url, &execution.effects.cookies)?;
        }
        if let Some(applied) = applied {
            let before = page.document.styles.len();
            page.resolved_stylesheets
                .extend(scripting::inserted_sheets(&applied, before));
            page.document = applied;
            page.styled = false;
        }
        if effects && changed {
            // Images the request's changes brought in load like a script step's.
            page.scripting.pending_images = Some(Default::default());
            self.flush_devtools_images(page, first_new)?;
        }
        page.state = execution.state;
        page.state_token = execution.state_token;
        page.storage_dirty |= execution.storage != page.storage;
        page.storage = execution.storage;
        let scripting = &mut page.scripting;
        scripting.loaded = true;
        scripting.next_timer_ms = execution.effects.next_timer_ms;
        if effects {
            scripting.requests.extend(execution.effects.requests);
            if execution.effects.navigation.is_some() {
                scripting.navigation = execution.effects.navigation;
            }
            scripting.modules.pending.extend(execution.module_requests);
            scripting
                .modules
                .pending_scripts
                .extend(execution.script_requests);
            scripting
                .modules
                .frame_requests
                .extend(execution.frame_requests);
        }
        scripting::record_console(
            scripting,
            execution.effects.console,
            execution.effects.console_dropped,
        );
        if let Some(title) = execution.effects.title {
            page.document.title = title;
        }
        apply_history(page, &execution.effects.history);
        Ok(Step {
            result: outcome.result,
            console: outcome.console,
            changed,
            retain: outcome.retain,
        })
    }

    /** Per-request facts, as `scripting::step_environment` fills them for a lifecycle step. */
    fn devtools_environment(&mut self, page: &Page) -> EngineResult<PageEnvironment> {
        let mut environment = page.scripting.environment.clone();
        let url = Url::parse(&page.url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        environment.cookie = self.cookies.document_cookie(&url, SystemTime::now());
        environment.url = page.url.clone();
        environment.viewport_width = self.render_context.width;
        environment.viewport_height = self.render_context.height;
        environment.title = page.document.title.clone();
        environment.node_text = node_text(&page.document);
        environment.geometry = page
            .display
            .as_ref()
            .map(scripting::boxes)
            .unwrap_or_default();
        environment.scroll_y = self.scroll_y;
        environment.scroll_x = page.scroll_x;
        environment.positions = scripting::positions(&page.document);
        Ok(environment)
    }

    /** Fetch images the request inserted, through the same path a script step uses. */
    fn flush_devtools_images(&mut self, page: &mut Page, first_new: usize) -> EngineResult<()> {
        let touched: Vec<(usize, String)> = page
            .document
            .nodes
            .iter()
            .filter(|node| node.local_id >= first_new)
            .map(|node| (node.local_id, "src".to_owned()))
            .collect();
        page.scripting.pending_images = Some(scripting::PendingImages {
            first_new,
            changed: touched,
            ..Default::default()
        });
        self.flush_inserted_images(page)
    }
}

/** What one devtools step produced. */
struct Step {
    /// The protocol-shaped answer.
    result: Value,
    /// Console lines the request wrote.
    console: Vec<crate::v9::ConsoleEntry>,
    /// Whether the document changed.
    changed: bool,
    /// Whether the page must keep the source the request ran.
    retain: bool,
}

/**
 * Client source becomes a page source: an evaluate's expression, or a callFunctionOn's
 * declaration in parentheses, appended to the page's scripts (or found there, when the same
 * text ran before) and named by index. V9 compiles it at a stable page offset, so functions it
 * creates stay callable from every later step, and the page's own eval budget is untouched.
 * Returns the rewritten request and whether a source was appended.
 */
// ponytail: an appended source changes the page's source digest, so the next V9 call sends the
// whole script list again; a per-source digest would send only the new text.
fn place_source(page: &mut Page, mut request: Value) -> EngineResult<(Value, bool)> {
    let text = match request["kind"].as_str() {
        Some("evaluate") => request["expression"].as_str().map(str::to_owned),
        Some("call_function") => request["declaration"]
            .as_str()
            .map(|text| format!("({text})")),
        _ => None,
    };
    let Some(text) = text else {
        return Ok((request, false));
    };
    if text.len() > crate::constants::SCRIPT_BYTES {
        return Err(EngineError::limit(
            "script bytes",
            crate::constants::SCRIPT_BYTES,
        ));
    }
    let (index, appended) = match page.scripts.iter().position(|known| *known == text) {
        Some(index) => (index, false),
        None => {
            page.scripts.push(text);
            (page.scripts.len() - 1, true)
        }
    };
    if let Some(fields) = request.as_object_mut() {
        fields.remove("expression");
        fields.remove("declaration");
        fields.insert("source".into(), json!(index));
    }
    Ok((request, appended))
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

/** Text of text nodes and of elements holding their own text, by local id, for V9's reads. */
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
