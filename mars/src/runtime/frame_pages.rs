/**
 * Frames that run their own scripts. A cross-origin iframe, or a same-origin sandbox allowing
 * both scripts and its origin, showing an http(s) document gets a
 * page of its own, built as a navigation builds one (`build_page` with an `Embedding`): its own
 * V9 realm, cookies through the same jar, its own storage origin, and `parent` a proxy for the
 * embedder. Google's app launcher, sign-in widgets and gapi relays are such frames, and they
 * answer their embedder only by postMessage, so a frame that cannot run its scripts reads as a
 * failure ("Can't seem to load the app launcher").
 *
 * After every settle, the bounded frame router exchanges messages and retains pending rounds;
 * each frame page's timers advance by as much virtual time as the embedder's did.
 * A frame's document is spliced into the embedder's display as any frame's is; its own frames are
 * loaded through the same bounded coordinator; frame input remains unsupported. A frame whose scripts fail stops alone.
 */
use super::frames::{Embedding, FRAME_COUNT, Frame, HTTP_OK, same_origin};
use super::images::stops_loading;
use super::page::Page;
use super::supervisor::Runtime;
use crate::network::{Destination, WebRequest};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{FrameMessage, PortMessage};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Characters of a message's data the perf log keeps.
const MESSAGE_TRACE_CHARS: usize = 160;
/// The longest one frame page may take to load, so a slow widget never fails the embedder.
const FRAME_PAGE_WALL: Duration = Duration::from_secs(8);

impl Runtime {
    /**
     * The frame of iframe `owner` showing `address` as a page of its own, or None when the
     * address is ordinary same-origin (its document is the embedder's to reach), not http(s), sandboxed
     * without both `allow-scripts` and `allow-same-origin` (Mars gives no frame an opaque
     * origin, so such a frame runs no scripts), refuses to be framed by this embedder
     * (`frame-ancestors`, `X-Frame-Options`), or does not load as an HTML document in time.
     * Only authority and engine-service failures stop the embedder.
     */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_page(
        &mut self,
        parent: &mut Page,
        owner: usize,
        address: &str,
        attributes: &BTreeMap<String, String>,
        paint_base: usize,
        deadline: Instant,
        budget: &mut super::frames::FrameBudget,
    ) -> EngineResult<Option<Frame>> {
        let base = url::Url::parse(&parent.url)
            .map_err(|_| EngineError::invalid("Invalid frame parent URL."))?;
        let Ok(url) = base.join(address) else {
            return Ok(None);
        };
        if !self.scripted_frame_allowed(&base, &url, attributes) {
            return Ok(None);
        }
        let window = parent
            .frame_tree
            .routes
            .iter()
            .find(|route| route.owner == owner)
            .map(|route| route.window)
            .ok_or_else(|| EngineError::invalid("Missing frame context."))?;
        let replacement = self
            .page
            .as_ref()
            .is_some_and(|root| root.has_frame_window(window));
        let context = self.residency.reserve(replacement)?;
        let deadline = deadline.min(Instant::now() + FRAME_PAGE_WALL);
        let request = WebRequest {
            destination: Destination::Document,
            top_level: Some(parent.top_level_url.clone()),
            ..WebRequest::get(url.as_str())
        };
        let acquired = match self.acquire(request, deadline, Some(base.clone()), false, true) {
            Ok(acquired) => acquired,
            Err(error) if stops_loading(&error) => return Err(error),
            Err(_) => return Ok(None),
        };
        let final_url = url::Url::parse(&acquired.response.url)
            .map_err(|_| EngineError::invalid("Invalid acquired frame URL."))?;
        let mut ancestors = parent.frame_ancestors.clone();
        ancestors.push(parent.url.clone());
        if acquired.response.status != HTTP_OK
            || acquired.response.mime != "text/html"
            || !super::framing::may_frame_chain(&acquired.response.headers, &ancestors, &final_url)
        {
            return Ok(None);
        }
        budget.source(acquired.response.source.len())?;
        let embedding = Embedding {
            top_level_url: parent.top_level_url.clone(),
            origin: base.origin().ascii_serialization(),
            name: attributes.get("name").cloned().unwrap_or_default(),
            routes: parent.frame_tree.routes.clone(),
            window,
            ancestors,
            contexts: parent.ancestor_contexts(),
            paint_base,
            source_budget: super::frames::FRAME_BYTES_TOTAL.saturating_sub(budget.bytes),
        };
        if parent.display.is_none() || !parent.styled {
            let previous = self.render_context;
            let scroll = self.scroll_y;
            if !parent.frame_ancestors.is_empty() {
                self.render_context.width = parent.scripting.environment.viewport_width;
                self.render_context.height = parent.scripting.environment.viewport_height;
                self.scroll_y = parent.scripting.scroll_y.unwrap_or(0.0);
            }
            let prepared = self.prepare_current(parent);
            self.render_context = previous;
            self.scroll_y = scroll;
            prepared?;
        }
        let dimensions = frame_dimensions(parent, owner);
        let previous = self.render_context;
        let scroll = self.scroll_y;
        self.render_context.width = dimensions.0;
        self.render_context.height = dimensions.1;
        self.scroll_y = 0.0;
        let partition = self.frame_partition.replace(parent.top_level_url.clone());
        let built = self.build_page(acquired, deadline, false, Some(&embedding), context);
        self.frame_partition = partition;
        self.render_context = previous;
        self.scroll_y = scroll;
        match built {
            Ok(page) => {
                budget.source(
                    page.inert_frame_bytes
                        .saturating_add(page.scripting.modules.frame_bytes),
                )?;
                Ok(Some(Frame::of_page(
                    owner,
                    page,
                    paint_base - parent.frame_base,
                    &mut parent.images,
                )))
            }
            Err(error) if stops_loading(&error) => Err(error),
            Err(error) => {
                super::supervisor::perf::record("frame.failed", Instant::now(), || {
                    error.message.clone()
                });
                Ok(None)
            }
        }
    }

    /** One admission predicate serves markup and inserted frames without weakening sandbox policy. */
    pub(super) fn scripted_frame_allowed(
        &self,
        base: &url::Url,
        url: &url::Url,
        attributes: &BTreeMap<String, String>,
    ) -> bool {
        self.web_origins.is_some()
            && matches!(url.scheme(), "http" | "https")
            && (!same_origin(base, url) || scriptable_sandbox(attributes))
            && !sandboxed(attributes)
    }

    /** Mark each markup owner's route independently, including refused scripted loads. */
    pub(super) fn scripted_owners(
        &self,
        document: &crate::document::Document,
        base: &str,
    ) -> BTreeSet<usize> {
        let Ok(base) = url::Url::parse(base) else {
            return BTreeSet::new();
        };
        document
            .nodes
            .iter()
            .filter(|node| node.tag == "iframe" && !node.attributes.contains_key("srcdoc"))
            .filter(|node| {
                node.attributes
                    .get("src")
                    .filter(|src| !src.trim().is_empty())
                    .and_then(|src| base.join(src.trim()).ok())
                    .is_some_and(|url| self.scripted_frame_allowed(&base, &url, &node.attributes))
            })
            .map(|node| node.local_id)
            .collect()
    }

    /** Route only current requested owners; prune removed nodes before handing hints to V9. */
    pub(super) fn route_inserted_frames(&self, page: &mut Page, urls: &[String]) {
        prune_routes(page);
        let Ok(base) = url::Url::parse(&page.url) else {
            return;
        };
        let eligible = self.scripted_owners(&page.document, &page.url);
        page.scripting
            .environment
            .scripted_frame_owners
            .clone_from(&eligible);
        for (_, owner, _) in unattached(page, &base, urls) {
            if eligible.contains(&owner) {
                page.scripting
                    .environment
                    .scripted_frame_owners
                    .insert(owner);
            }
        }
    }

    /**
     * Loads admitted inserted iframes as pages of their own, selecting their route by owner.
     * The requested
     * documents V9 asked for (`urls`). Returns whether a frame was added, so the page lays out
     * again. Embedded pages defer admission to the root coordinator's shared tree pass.
     */
    pub(super) fn attach_frame_pages(
        &mut self,
        page: &mut Page,
        urls: &[String],
        deadline: Instant,
    ) -> EngineResult<bool> {
        if urls.is_empty() || page.scripting.environment.embedder_origin.is_some() {
            return Ok(false);
        }
        let mut budget = super::frames::FrameBudget::for_page(page);
        self.attach_frame_pages_budget(page, urls, deadline, &mut budget)
    }

    /** Only root reconciliation may admit descendants against the shared whole-tree allowance. */
    fn attach_frame_pages_budget(
        &mut self,
        page: &mut Page,
        urls: &[String],
        deadline: Instant,
        budget: &mut super::frames::FrameBudget,
    ) -> EngineResult<bool> {
        if urls.is_empty() || page.frame_ancestors.len() >= super::frames::FRAME_DEPTH - 1 {
            return Ok(false);
        }
        let Ok(base) = url::Url::parse(&page.url) else {
            return Ok(false);
        };
        let mut added = false;
        for (index, owner, address) in unattached(page, &base, urls) {
            if !page
                .scripting
                .environment
                .scripted_frame_owners
                .contains(&owner)
            {
                continue;
            }
            if budget.count >= FRAME_COUNT {
                break;
            }
            if !page.scripting.attempted_frame_owners.insert(owner) {
                continue;
            }
            let id_base = budget.reserve()?;
            let attributes = page.document.nodes[index].attributes.clone();
            let opened = match self.frame_page(
                page,
                owner,
                &address,
                &attributes,
                id_base,
                deadline,
                budget,
            ) {
                Err(error) if error.code == "limit_exceeded" => {
                    super::scripting::record_console(
                        &mut page.scripting,
                        vec![crate::v9::ConsoleEntry {
                            level: "warning".into(),
                            text: error.message,
                        }],
                        0,
                    );
                    continue;
                }
                result => result?,
            };
            if let Some(frame) = opened {
                page.document.nodes[index].embedded = true;
                Arc::make_mut(&mut page.frames).push(frame);
                added = true;
            }
        }
        Ok(added)
    }

    /**
     * Carries messages between `page` and its frame pages and advances the frames' timers by
     * `advance_ms` of virtual time, for at most MESSAGE_ROUNDS rounds. Returns whether the page
     * or any frame's document changed, so the page lays out again.
     */
    pub(super) fn run_frame_pages(
        &mut self,
        page: &mut Page,
        until: Instant,
        advance_ms: f64,
    ) -> EngineResult<bool> {
        if page.scripting.environment.embedder_origin.is_some() {
            return Ok(false);
        }
        self.exchange_frames(page, until, advance_ms, false)
    }

    /** Idle observation updates never run unrelated author timers in descendants. */
    pub(super) fn run_frame_observers(
        &mut self,
        page: &mut Page,
        until: Instant,
    ) -> EngineResult<bool> {
        self.exchange_frames(page, until, 0.0, true)
    }

    /** A single root pass prunes all retired descendants before allocating against shared limits. */
    pub(super) fn reconcile_frame_contexts(
        &mut self,
        page: &mut Page,
        until: Instant,
    ) -> EngineResult<bool> {
        let retired = self.prune_frame_branch(page)?;
        let mut budget = super::frames::FrameBudget::for_page(page);
        budget.validate()?;
        let changed = retired | self.admit_frame_branch(page, until, &mut budget)?;
        self.refresh_child_directories(page);
        Ok(changed)
    }

    /** Dropping one parent drops its whole descendant subtree and every associated residency lease. */
    fn prune_frame_branch(&mut self, page: &mut Page) -> EngineResult<bool> {
        let previous = page.frames.len();
        self.sync_frame_tree(page)?;
        let mut changed = previous != page.frames.len();
        self.route_inserted_frames(page, &[]);
        for frame in Arc::make_mut(&mut page.frames) {
            if let Some(child) = frame.page.as_mut() {
                changed |= self.prune_frame_branch(child)?;
                frame.follow_page(&mut page.images);
            }
        }
        Ok(changed)
    }

    /** Static and newly inserted nested owners use the same admission path and never reset budgets. */
    fn admit_frame_branch(
        &mut self,
        page: &mut Page,
        until: Instant,
        budget: &mut super::frames::FrameBudget,
    ) -> EngineResult<bool> {
        if Instant::now() >= until {
            return Ok(false);
        }
        let base = url::Url::parse(&page.url)
            .map_err(|_| EngineError::invalid("Invalid frame parent URL."))?;
        let urls: Vec<_> = page
            .document
            .nodes
            .iter()
            .filter(|node| {
                node.tag == "iframe"
                    && !page
                        .scripting
                        .attempted_frame_owners
                        .contains(&node.local_id)
            })
            .filter_map(|node| {
                node.attributes
                    .get("src")
                    .and_then(|source| base.join(source).ok())
                    .map(String::from)
            })
            .collect();
        let mut changed = self.attach_frame_pages_budget(page, &urls, until, budget)?;
        for frame in Arc::make_mut(&mut page.frames) {
            if let Some(child) = frame.page.as_mut() {
                changed |= self.admit_frame_branch(child, until, budget)?;
                frame.follow_page(&mut page.images);
            }
        }
        Ok(changed)
    }

    /** One frame page's turn: its mail, then its timers. Whether its document changed. */
    pub(super) fn run_child(
        &mut self,
        child: &mut Page,
        until: Instant,
        advance_ms: f64,
        observe_only: bool,
    ) -> EngineResult<bool> {
        let mut changed = false;
        let resized = child.scripting.environment.viewport_width != self.render_context.width
            || child.scripting.environment.viewport_height != self.render_context.height;
        if resized || !child.styled || child.display.is_none() {
            child.styled = false;
            self.prepare_current(child)?;
            child.scripting.environment.viewport_width = self.render_context.width;
            child.scripting.environment.viewport_height = self.render_context.height;
            changed = true;
        }
        if child.scripting.loaded {
            let end = child.scripting.clock_ms + advance_ms;
            let result = if observe_only || Instant::now() >= child.scripting.awake_until {
                if child.scripting.intersections.pending {
                    self.observe_intersections(child)
                } else {
                    Ok(false)
                }
            } else {
                self.settle_scripts(child, until, end)
            };
            match result {
                Ok(settled) => changed |= settled,
                Err(error) if stops_loading(&error) => return Err(error),
                // A frame whose state the V9 service lost stops alone; the embedder stays.
                Err(error) => stop_frame_scripts(child, &error),
            }
        }
        if changed {
            self.prepare_current(child)?;
        }
        Ok(changed)
    }

    /**
     * Fires one message at `to` when its origin is the one the sender named, with the ports it
     * carries. Returns whether `to`'s document changed.
     */
    pub(super) fn deliver(
        &mut self,
        to: &mut Page,
        message: FrameMessage,
        origin: Option<&str>,
        source: Option<i64>,
        queued: Vec<crate::v9::QueuedPortMessage>,
    ) -> EngineResult<bool> {
        let wanted = message.target_origin.as_str();
        let delivered = to.scripting.loaded && (wanted == "*" || wanted == to.origin);
        trace(&to.origin, wanted, delivered, &message.data);
        if !delivered {
            return Ok(false);
        }
        let event = json!({"message":{
            "data": message.data,
            "origin": origin.unwrap_or("null"),
            "source": source,
            "ports": message.ports,
            "queued_port_messages": queued,
        }});
        self.step_in(to, event)
    }

    /** Fires a message that crossed a port link at the port bound to it in `to`. */
    pub(super) fn deliver_on_port(
        &mut self,
        to: &mut Page,
        message: PortMessage,
    ) -> EngineResult<bool> {
        if !to.scripting.loaded {
            return Ok(false);
        }
        trace(&to.origin, "port", true, &message.data);
        let event = json!({"port_message":{"link": message.link, "data": message.data}});
        self.step_in(to, event)
    }

    /** Native disentanglement releases the far endpoint's retained route without running a callback. */
    pub(super) fn close_on_port(&mut self, to: &mut Page, link: u64) -> EngineResult<bool> {
        if !to.scripting.loaded {
            return Ok(false);
        }
        self.step_in(to, json!({"port_close":{"link":link}}))
    }

    /**
     * One event step in `to`. A step the page refuses stops that page's scripts, not the
     * caller's; authority and service failures still stop everything.
     */
    fn step_in(&mut self, to: &mut Page, event: serde_json::Value) -> EngineResult<bool> {
        to.scripting.wake();
        let partition = self.frame_partition.replace(to.top_level_url.clone());
        let result = self.script_step(to, json!({"kind":"event","event":event}));
        self.frame_partition = partition;
        match result {
            Ok(step) => Ok(step.changed),
            Err(error) if stops_loading(&error) => Err(error),
            Err(error) => {
                stop_frame_scripts(to, &error);
                Ok(false)
            }
        }
    }
}

/** Removed owners cannot leave route hints attached to a later document projection. */
pub(super) fn prune_routes(page: &mut Page) {
    page.scripting.environment.scripted_frame_owners = current_routes(page);
    page.scripting.attempted_frame_owners.retain(|owner| {
        page.scripting
            .environment
            .scripted_frame_owners
            .contains(owner)
    });
    let owners: BTreeSet<_> = page
        .document
        .nodes
        .iter()
        .map(|node| node.local_id)
        .collect();
    let removed: BTreeSet<_> = page
        .frames
        .iter()
        .filter(|frame| !owners.contains(&frame.owner()))
        .map(Frame::owner)
        .collect();
    if removed.is_empty() {
        return;
    }
    Arc::make_mut(&mut page.frames).retain(|frame| {
        if !removed.contains(&frame.owner()) {
            return true;
        }
        frame.remove_images(&mut page.images);
        false
    });
    // The router rejects stale destinations and retires transferred peers; dropping
    // their pending effects here would bypass that ownership cleanup.
}

/** Read-only inspector calls use the same current-owner filter as ordinary lifecycle steps. */
pub(super) fn current_routes(page: &Page) -> BTreeSet<usize> {
    page.document
        .nodes
        .iter()
        .filter(|node| {
            node.tag == "iframe"
                && page
                    .scripting
                    .environment
                    .scripted_frame_owners
                    .contains(&node.local_id)
        })
        .map(|node| node.local_id)
        .collect()
}

/** A shared URL still needs its inert answer if any current owner uses the local-document route. */
pub(super) fn needs_inert_answer(page: &Page, address: &str) -> bool {
    let Ok(base) = url::Url::parse(&page.url) else {
        return false;
    };
    let owners: Vec<_> = page
        .document
        .nodes
        .iter()
        .filter(|node| node.tag == "iframe" && !node.attributes.contains_key("srcdoc"))
        .filter(|node| {
            node.attributes
                .get("src")
                .and_then(|src| base.join(src.trim()).ok())
                .is_some_and(|url| url.as_str() == address)
        })
        .collect();
    owners.iter().any(|node| {
        !page
            .scripting
            .environment
            .scripted_frame_owners
            .contains(&node.local_id)
    })
}

/** Records one delivery in the perf log, so a frame handshake can be followed. */
fn trace(to: &str, wanted: &str, delivered: bool, data: &crate::v9::MessageData) {
    super::supervisor::perf::record("frame.message", Instant::now(), || {
        let (crate::v9::MessageData::Text(text) | crate::v9::MessageData::Json(text)) = data;
        let text: String = text.chars().take(MESSAGE_TRACE_CHARS).collect();
        format!("{to}\t{wanted}\t{delivered}\t{text}")
    });
}

impl Page {
    /** Native observer work remains schedulable after the ordinary author timer wake window closes. */
    pub(super) fn frame_observations_pending(&self) -> bool {
        self.frames
            .iter()
            .filter_map(|frame| frame.page.as_ref())
            .any(|child| {
                child.scripting.loaded && (!child.styled || child.scripting.intersections.pending)
                    || child.frame_observations_pending()
            })
    }
    /** Whether a frame page has script work waiting or messages are waiting to cross, so the
    embedder's background run keeps going. */
    pub(super) fn frames_pending(&self) -> bool {
        self.frame_tree.mail.pending()
            || !self.scripting.outbox.is_empty()
            || !self.scripting.port_outbox.is_empty()
            || !self.scripting.port_closes.is_empty()
            || self
                .frames
                .iter()
                .filter_map(|f| f.page.as_ref())
                .any(|child| {
                    child.scripting.pending()
                        || child.scripting.intersections.pending
                        || child.frames_pending()
                })
    }
}

/** Stops a page's scripts after a step it refused, so it does not fail the same way forever. */
fn stop_frame_scripts(page: &mut Page, error: &EngineError) {
    page.scripting.loaded = false;
    page.scripting.next_timer_ms = None;
    page.scripting.requests.clear();
    let entry = crate::v9::ConsoleEntry {
        level: "error".into(),
        text: format!("Frame scripts stopped: {}", error.message),
    };
    super::scripting::record_console(&mut page.scripting, vec![entry], 0);
}

/** Whether the iframe's `sandbox` keeps its scripts from running as Mars runs them. */
fn sandboxed(attributes: &BTreeMap<String, String>) -> bool {
    attributes
        .get("sandbox")
        .is_some_and(|tokens| !allows_sandbox(tokens))
}

/** Whether a sandboxed same-origin iframe may execute its own scripts. */
fn scriptable_sandbox(attributes: &BTreeMap<String, String>) -> bool {
    attributes
        .get("sandbox")
        .is_some_and(|tokens| allows_sandbox(tokens))
}

/** Whether sandbox tokens preserve both script execution and the frame's origin. */
fn allows_sandbox(tokens: &str) -> bool {
    ["allow-scripts", "allow-same-origin"].iter().all(|flag| {
        tokens
            .split_ascii_whitespace()
            .any(|token| token.eq_ignore_ascii_case(flag))
    })
}

/** The iframes of `page` whose `src` resolves to one of `urls` and that show no frame yet: their
node index, identity and address. */
fn unattached(page: &Page, base: &url::Url, urls: &[String]) -> Vec<(usize, usize, String)> {
    let shown: std::collections::BTreeSet<usize> = page.frames.iter().map(Frame::owner).collect();
    page.document
        .nodes
        .iter()
        .enumerate()
        .filter(|(_, node)| node.tag == "iframe" && !shown.contains(&node.local_id))
        .filter(|(_, node)| !node.attributes.contains_key("srcdoc"))
        .filter_map(|(index, node)| {
            let src = node.attributes.get("src")?.trim();
            let resolved = base.join(src).ok()?;
            (urls.iter().any(|url| url == resolved.as_str()))
                .then(|| (index, node.local_id, src.to_string()))
        })
        .collect()
}

/** Child geometry is measured in its parent, then interpreted as a local viewport. */
pub(super) fn frame_dimensions(page: &Page, owner: usize) -> (f64, f64) {
    let dimensions = page
        .display
        .as_ref()
        .and_then(|display| display.items.iter().find(|item| item.local_id == owner))
        .map(crate::document::frame_viewport)
        .map(|rect| (rect[2], rect[3]))
        .unwrap_or(super::frames::DEFAULT_VIEWPORT);
    (
        dimensions.0.clamp(1.0, super::frames::MAX_FRAME_VIEWPORT),
        dimensions.1.clamp(1.0, super::frames::MAX_FRAME_VIEWPORT),
    )
}

impl Page {
    /** A native layout trigger schedules resized descendants without polling their timers. */
    pub(super) fn invalidate_resized_frames(&mut self) {
        let resized: Vec<_> = self
            .frames
            .iter()
            .filter_map(|frame| {
                let child = frame.page.as_ref()?;
                let dimensions = frame_dimensions(self, frame.owner());
                (dimensions
                    != (
                        child.scripting.environment.viewport_width,
                        child.scripting.environment.viewport_height,
                    ))
                    .then_some(frame.owner())
            })
            .collect();
        for frame in Arc::make_mut(&mut self.frames) {
            if resized.contains(&frame.owner())
                && let Some(child) = frame.page.as_mut()
            {
                child.styled = false;
                child.scripting.intersections.invalidate_layout();
            }
        }
    }
}
