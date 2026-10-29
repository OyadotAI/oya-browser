/**
 * Frames that run their own scripts. An iframe showing another origin's http(s) document gets a
 * page of its own, built as a navigation builds one (`build_page` with an `Embedding`): its own
 * V9 realm, cookies through the same jar, its own storage origin, and `parent` a proxy for the
 * embedder. Google's app launcher, sign-in widgets and gapi relays are such frames, and they
 * answer their embedder only by postMessage, so a frame that cannot run its scripts reads as a
 * failure ("Can't seem to load the app launcher").
 *
 * After every settle of the embedding page, messages cross both ways for at most MESSAGE_ROUNDS
 * rounds, and each frame page's timers advance by as much virtual time as the embedder's did.
 * A frame's document is spliced into the embedder's display as any frame's is; its own frames are
 * not loaded (one level), and it takes no input yet. A frame whose scripts fail stops alone.
 */
use super::frames::{Embedding, FRAME_COUNT, Frame, HTTP_OK, same_origin};
use super::images::{ImageAsset, stops_loading};
use super::page::Page;
use super::supervisor::Runtime;
use crate::network::{Destination, WebRequest};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::{FrameMessage, MESSAGE_PARENT, PortMessage};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Rounds of messages one settle carries between a page and its frames: a request, its answer
/// and the follow-ups a handshake needs, without letting two documents ping-pong forever.
const MESSAGE_ROUNDS: usize = 8;
/// Characters of a message's data the perf log keeps.
const MESSAGE_TRACE_CHARS: usize = 160;
/// The longest one frame page may take to load, so a slow widget never fails the embedder.
const FRAME_PAGE_WALL: Duration = Duration::from_secs(8);
/// Where the identities of frames added after load start when the page had none: the first
/// range above every page node, as FRAME_ID_SHIFT places the first loaded frame.
const FIRST_FRAME_BASE: usize = 1 << 32;

impl Runtime {
    /**
     * The frame of iframe `owner` showing `address` as a page of its own, or None when the
     * address is same-origin (its document is the embedder's to reach), not http(s), sandboxed
     * without both `allow-scripts` and `allow-same-origin` (Mars gives no frame an opaque
     * origin, so such a frame runs no scripts), refuses to be framed by this embedder
     * (`frame-ancestors`, `X-Frame-Options`), or does not load as an HTML document in time.
     * Only authority and engine-service failures stop the embedder.
     */
    #[allow(clippy::too_many_arguments)]
    pub(super) fn frame_page(
        &mut self,
        owner: usize,
        address: &str,
        attributes: &BTreeMap<String, String>,
        base: &url::Url,
        id_base: usize,
        deadline: Instant,
        images: &mut BTreeMap<usize, ImageAsset>,
    ) -> EngineResult<Option<Frame>> {
        let Ok(url) = base.join(address) else {
            return Ok(None);
        };
        let web = matches!(url.scheme(), "http" | "https");
        let same_origin_sandbox = same_origin(base, &url) && scriptable_sandbox(attributes);
        if !web
            || same_origin(base, &url) && !same_origin_sandbox
            || self.web_origins.is_none()
            || sandboxed(attributes)
        {
            return Ok(None);
        }
        let deadline = deadline.min(Instant::now() + FRAME_PAGE_WALL);
        let request = WebRequest {
            destination: Destination::Document,
            ..WebRequest::get(url.as_str())
        };
        let acquired = match self.acquire(request, deadline, Some(base.clone()), false, true) {
            Ok(acquired) => acquired,
            Err(error) if stops_loading(&error) => return Err(error),
            Err(error) => {
                super::supervisor::perf::record("frame.failed", Instant::now(), || {
                    error.message.clone()
                });
                return Ok(None);
            }
        };
        if acquired.response.status != HTTP_OK
            || acquired.response.mime != "text/html"
            || !super::framing::may_frame(&acquired.response.headers, base, &url)
        {
            return Ok(None);
        }
        self.frame_pages += 1;
        let embedding = Embedding {
            origin: base.origin().ascii_serialization(),
            name: attributes.get("name").cloned().unwrap_or_default(),
            identity: format!("{}:frame-{}", self.active_tab_id, self.frame_pages),
        };
        match self.build_page(acquired, deadline, false, Some(&embedding)) {
            Ok(page) => {
                for entry in page.scripting.console.iter().filter(|e| e.level == "error") {
                    super::supervisor::perf::record("frame.console", Instant::now(), || {
                        entry.text.chars().take(MESSAGE_TRACE_CHARS).collect()
                    });
                }
                Ok(Some(Frame::of_page(owner, page, id_base, images)))
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

    /**
     * Loads, as pages of their own, the cross-origin iframes the page's scripts inserted whose
     * documents V9 asked for (`urls`). Returns whether a frame was added, so the page lays out
     * again. A frame page loads no frames itself.
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
        let Ok(base) = url::Url::parse(&page.url) else {
            return Ok(false);
        };
        let mut added = false;
        for (index, owner, address) in unattached(page, &base, urls) {
            if page.frames.len() >= FRAME_COUNT {
                break;
            }
            let id_base = page
                .frames
                .iter()
                .map(Frame::id_end)
                .max()
                .unwrap_or(FIRST_FRAME_BASE);
            let attributes = page.document.nodes[index].attributes.clone();
            let opened = self.frame_page(
                owner,
                &address,
                &attributes,
                &base,
                id_base,
                deadline,
                &mut page.images,
            )?;
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
        if !page.frames.iter().any(|frame| frame.page.is_some()) {
            // A frame page keeps what it posted to its parent for the embedder to collect; a
            // message to a frame that runs no scripts goes nowhere.
            page.scripting
                .outbox
                .retain(|message| message.target == MESSAGE_PARENT);
            if page.scripting.environment.embedder_origin.is_none() {
                page.scripting.outbox.clear();
                page.scripting.port_outbox.clear();
            }
            return Ok(false);
        }
        let mut frames = std::mem::take(&mut page.frames);
        let result = self.exchange(
            page,
            Arc::make_mut(&mut frames).as_mut_slice(),
            until,
            advance_ms,
        );
        page.frames = frames;
        result
    }

    /** The rounds of `run_frame_pages` over the frames taken out of `page`. */
    fn exchange(
        &mut self,
        page: &mut Page,
        frames: &mut [Frame],
        until: Instant,
        advance_ms: f64,
    ) -> EngineResult<bool> {
        let mut changed = false;
        for round in 0..MESSAGE_ROUNDS {
            let outgoing = std::mem::take(&mut page.scripting.outbox);
            let port_outgoing = std::mem::take(&mut page.scripting.port_outbox);
            let mut moved = !outgoing.is_empty() || !port_outgoing.is_empty();
            let advance = if round == 0 { advance_ms } else { 0.0 };
            for frame in frames.iter_mut() {
                let owner = frame.owner();
                let Some(mut child) = frame.page.take() else {
                    continue;
                };
                let mail = Mail {
                    messages: outgoing
                        .iter()
                        .filter(|m| m.target == owner as i64)
                        .collect(),
                    ports: (port_outgoing.iter())
                        .filter(|m| page.scripting.port_links.get(&m.link) == Some(&owner))
                        .collect(),
                };
                for link in mail.messages.iter().flat_map(|m| &m.ports) {
                    page.scripting.port_links.insert(*link, owner);
                }
                let ran = self.run_child(&mut child, mail, &page.origin, until, advance);
                let returned = std::mem::take(&mut child.scripting.outbox);
                let returned_ports = std::mem::take(&mut child.scripting.port_outbox);
                let origin = child.origin.clone();
                frame.page = Some(child);
                if ran? {
                    frame.follow_page(&mut page.images);
                    changed = true;
                }
                for message in returned.into_iter().filter(|m| m.target == MESSAGE_PARENT) {
                    moved = true;
                    for link in &message.ports {
                        page.scripting.port_links.insert(*link, owner);
                    }
                    changed |= self.deliver(page, message, Some(&origin), Some(owner as i64))?;
                }
                for message in returned_ports {
                    moved = true;
                    changed |= self.deliver_on_port(page, message)?;
                }
            }
            if !moved || Instant::now() >= until {
                break;
            }
        }
        Ok(changed)
    }

    /** One frame page's turn: its mail, then its timers. Whether its document changed. */
    fn run_child(
        &mut self,
        child: &mut Page,
        mail: Mail<'_>,
        embedder: &str,
        until: Instant,
        advance_ms: f64,
    ) -> EngineResult<bool> {
        let mut changed = false;
        for message in mail.messages {
            changed |=
                self.deliver(child, message.clone(), Some(embedder), Some(MESSAGE_PARENT))?;
        }
        for message in mail.ports {
            changed |= self.deliver_on_port(child, message.clone())?;
        }
        if child.scripting.loaded {
            let end = child.scripting.clock_ms + advance_ms;
            match self.settle_scripts(child, until, end) {
                Ok(settled) => changed |= settled,
                Err(error) if stops_loading(&error) => return Err(error),
                // A frame whose state the V9 service lost stops alone; the embedder stays.
                Err(error) => stop_frame_scripts(child, &error),
            }
        }
        Ok(changed)
    }

    /**
     * Fires one message at `to` when its origin is the one the sender named, with the ports it
     * carries. Returns whether `to`'s document changed.
     */
    fn deliver(
        &mut self,
        to: &mut Page,
        message: FrameMessage,
        origin: Option<&str>,
        source: Option<i64>,
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
        }});
        self.step_in(to, event)
    }

    /** Fires a message that crossed a port link at the port bound to it in `to`. */
    fn deliver_on_port(&mut self, to: &mut Page, message: PortMessage) -> EngineResult<bool> {
        if !to.scripting.loaded {
            return Ok(false);
        }
        trace(&to.origin, "port", true, &message.data);
        let event = json!({"port_message":{"link": message.link, "data": message.data}});
        self.step_in(to, event)
    }

    /**
     * One event step in `to`. A step the page refuses stops that page's scripts, not the
     * caller's; authority and service failures still stop everything.
     */
    fn step_in(&mut self, to: &mut Page, event: serde_json::Value) -> EngineResult<bool> {
        match self.script_step(to, json!({"kind":"event","event":event})) {
            Ok(step) => Ok(step.changed),
            Err(error) if stops_loading(&error) => Err(error),
            Err(error) => {
                stop_frame_scripts(to, &error);
                Ok(false)
            }
        }
    }
}

/** What one round carries to a frame page. */
struct Mail<'m> {
    /// Window messages for it.
    messages: Vec<&'m FrameMessage>,
    /// Messages on ports linked into it.
    ports: Vec<&'m PortMessage>,
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
    /** Whether a frame page has script work waiting or messages are waiting to cross, so the
    embedder's background run keeps going. */
    pub(super) fn frames_pending(&self) -> bool {
        !self.scripting.outbox.is_empty()
            || !self.scripting.port_outbox.is_empty()
            || self
                .frames
                .iter()
                .filter_map(|f| f.page.as_ref())
                .any(|child| {
                    child.scripting.pending()
                        || !child.scripting.outbox.is_empty()
                        || !child.scripting.port_outbox.is_empty()
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
