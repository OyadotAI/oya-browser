/**
 * Media elements on the coordinator's side. A page's script sends commands (load, settings,
 * seek, captions, unload); the coordinator hands them to the native players through the hub,
 * fetches the bytes those players ask for with byte-range requests through Relay (cookies,
 * persona and egress rules apply as to any subresource), and turns what the players observe into
 * updates the page's media script fires as events. Players outlive a step but not their page.
 */
pub mod bytes;
pub mod hub;
pub mod stream;
mod support;
pub mod updates;

pub use support::support;

use super::Runtime;
use crate::network::{Destination, WebRequest};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::MediaCommand;
use hub::{PlayerKey, Shared};
use serde_json::Value;
use std::collections::BTreeSet;
use std::time::{Duration, Instant};

/// Commands a page may have waiting for the native players; a page past it loses the oldest.
const OUTBOX: usize = 4096;
/// Updates a page may have waiting for its script.
const INBOX: usize = 4096;
/// The longest one range request may hold the coordinator.
const FETCH_WALL: Duration = Duration::from_secs(20);
/// HTTP statuses a range request may answer with.
const OK: u16 = 200;
const PARTIAL: u16 = 206;
/// MediaError codes the page sees.
const NETWORK_ERROR: u16 = 2;

/// Per-page media queues: part of the page candidate, so a step that is rolled back sends nothing.
#[derive(Clone, Debug, Default)]
pub(super) struct PageMedia {
    /// Commands from committed steps, waiting for the native players.
    pub outbox: Vec<MediaCommand>,
    /// Updates for the page's media script, waiting for the next step.
    pub inbox: Vec<Value>,
}

impl PageMedia {
    /** Queue a step's commands, keeping the newest when a page floods the queue. */
    pub fn queue(&mut self, commands: Vec<MediaCommand>) {
        self.outbox.extend(commands);
        let excess = self.outbox.len().saturating_sub(OUTBOX);
        self.outbox.drain(..excess);
    }
    /** The waiting updates as the JSON the page's media script reads, emptying the inbox. */
    pub fn take_updates(&mut self) -> String {
        let updates = std::mem::take(&mut self.inbox);
        serde_json::to_string(&updates).unwrap_or_else(|_| "[]".into())
    }
    /** Add updates, keeping the newest when the page is not running to read them. */
    fn receive(&mut self, updates: Vec<Value>) {
        self.inbox.extend(updates);
        let excess = self.inbox.len().saturating_sub(INBOX);
        self.inbox.drain(..excess);
    }
}

/// The native controls' actions.
const CONTROLS: [&str; 3] = ["toggle", "seek", "mute"];

/// `page.media_control` parameters. A revision the click observed is accepted and ignored: a
/// playing video's page changes revision several times a second, and a person's click on its
/// controls acts on the element as it is now.
#[derive(serde::Deserialize)]
struct Control {
    page_id: String,
    node_id: String,
    action: String,
    #[serde(default)]
    time: f64,
}

/// One range request the coordinator owes a player.
struct Fetch {
    key: PlayerKey,
    url: String,
    initiator: String,
    audio: bool,
    start: u64,
    end: u64,
}

/// What a range request brought back.
struct Fetched {
    start: u64,
    length: Option<u64>,
    mime: String,
    bytes: Vec<u8>,
}

impl Runtime {
    /**
     * One media turn for the visible page: hand its new commands to the players, collect what
     * changed into updates for its script, keep it awake while something plays, fetch the next
     * missing range, and release players whose pages are gone.
     */
    pub(super) fn pump_media(&mut self) {
        let idle = self
            .page
            .as_ref()
            .is_none_or(|page| page.scripting.media.outbox.is_empty())
            && hub::lock().players.is_empty();
        if idle || self.healthy().is_err() {
            return;
        }
        // Fetches publish under the presentation's current authority, as page timers do.
        let Ok(permit) = self.presentation_permit(None) else {
            return;
        };
        let previous = std::mem::replace(&mut self.permit, permit);
        self.pump_media_permitted();
        self.permit = previous;
    }
    /** `pump_media` under the presentation permit. */
    fn pump_media_permitted(&mut self) {
        let Some(page) = self.page.as_mut() else {
            return;
        };
        let page_id = page.id();
        let commands = std::mem::take(&mut page.scripting.media.outbox);
        dispatch(&page_id, &page.url, commands);
        let (updates, playing) = collect(&page_id, Instant::now());
        page.scripting.media.receive(updates);
        let resized = apply_video_sizes(page, &video_sizes(&page_id));
        if playing {
            // A playing element is activity a person sees: its timeupdates keep flowing.
            page.scripting.awake_until = page
                .scripting
                .awake_until
                .max(Instant::now() + super::scripting::TICK_WINDOW);
        }
        self.fetch_media(&page_id);
        release(&self.live_page_ids());
        if resized && let Some(page) = self.page.clone() {
            // A video's own size replaces the default object size, so the page lays out again.
            let _ = self.commit_mutation(page);
        }
    }
    /** `page.media_control`: a click on an element's native controls (toggle, seek, mute). */
    pub(super) fn media_control_op(&mut self, params: Value) -> EngineResult<Value> {
        let input: Control = serde_json::from_value(params)
            .map_err(|_| EngineError::invalid("Invalid media control."))?;
        let page = self.current()?;
        let node = page
            .document
            .nodes
            .iter()
            .find(|node| page.id() == input.page_id && page.node_id(node.local_id) == input.node_id)
            .filter(|node| matches!(node.tag.as_str(), "video" | "audio"))
            .map(|node| node.local_id)
            .ok_or_else(|| EngineError::invalid("The media element is no longer on the page."))?;
        if !CONTROLS.contains(&input.action.as_str()) || !input.time.is_finite() {
            return Err(EngineError::invalid("Invalid media control."));
        }
        if !self.media_control(node, &input.action, input.time) {
            return Err(EngineError::invalid("The media element has no player."));
        }
        if let Some(page) = self.page.as_mut() {
            page.scripting.wake();
        }
        Ok(serde_json::json!({"ok":true}))
    }
    /** Deliver a click on an element's native controls to the page as a control update. */
    pub(super) fn media_control(&mut self, node: usize, action: &str, time: f64) -> bool {
        let Some(page) = self.page.as_mut() else {
            return false;
        };
        let page_id = page.id();
        let player = hub::lock()
            .players
            .iter()
            .find(|((id, _), shared)| *id == page_id && shared.node == node)
            .map(|((_, player), _)| *player);
        let Some(player) = player else {
            return false;
        };
        page.scripting.media.receive(vec![
            serde_json::json!({"player":player,"kind":"control","action":action,"time":time}),
        ]);
        true
    }
    /** The next missing range of any player of this page, fetched through Relay. */
    fn fetch_media(&mut self, page_id: &str) {
        let Some(job) = next_fetch(page_id) else {
            return;
        };
        let result = self.fetch_range(&job);
        debug(|| {
            format!(
                "fetch {} {}-{}: {:?}",
                job.url,
                job.start,
                job.end,
                result
                    .as_ref()
                    .map(|f| (f.start, f.length, f.bytes.len(), f.mime.clone()))
                    .map_err(|e| e.message.clone())
            )
        });
        let mut hub = hub::lock();
        let Some(shared) = hub.players.get_mut(&job.key) else {
            return;
        };
        match result {
            Ok(fetched) => {
                shared.bytes.length = shared.bytes.length.or(fetched.length);
                if shared.bytes.mime.is_empty() {
                    shared.bytes.mime = fetched.mime;
                }
                let reader = shared.need.unwrap_or(fetched.start);
                shared.bytes.insert(fetched.start, fetched.bytes, reader);
            }
            Err(error) => shared.bytes.failed = Some((NETWORK_ERROR, error.message)),
        }
    }
    /** One byte-range request with the element's destination, as Chrome sends it. */
    fn fetch_range(&mut self, job: &Fetch) -> EngineResult<Fetched> {
        let request = WebRequest {
            destination: if job.audio {
                Destination::Audio
            } else {
                Destination::Video
            },
            range: Some((job.start, Some(job.end))),
            languages: self.languages.clone(),
            ..WebRequest::get(&job.url)
        };
        let initiator = url::Url::parse(&job.initiator).ok();
        let acquired =
            self.acquire(request, Instant::now() + FETCH_WALL, initiator, false, true)?;
        fetched(acquired.response, job.start)
    }
    /** Pages whose players may keep running: the visible one and every tab's committed page. */
    fn live_page_ids(&self) -> BTreeSet<String> {
        self.page
            .iter()
            .chain(self.inactive_tab_pages())
            .map(|page| page.id())
            .collect()
    }
}

/** Interpret a range response: 206 with Content-Range, or 200 with the whole resource. */
fn fetched(response: crate::network::WebResponse, start: u64) -> EngineResult<Fetched> {
    let header = |name: &str| {
        response
            .headers
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
    };
    let bytes =
        super::images::decode_base64_within(&response.source, crate::constants::HTML_BYTES)?;
    let (start, length) = match response.status {
        PARTIAL => {
            let range = header("content-range").and_then(|value| bytes::content_range(&value));
            range.unwrap_or((start, None))
        }
        OK => (0, Some(bytes.len() as u64)),
        status => {
            return Err(EngineError::unsupported(format!(
                "MEDIA_ELEMENT_ERROR: the server answered HTTP {status}"
            )));
        }
    };
    Ok(Fetched {
        start,
        length,
        mime: response.mime,
        bytes,
    })
}

/** A diagnostic line on stderr when `MARS_MEDIA_DEBUG` is set; silent otherwise. */
pub fn debug(line: impl FnOnce() -> String) {
    if std::env::var_os("MARS_MEDIA_DEBUG").is_some() {
        eprintln!("[media] {}", line());
    }
}

/** Hand commands to the hub, creating a player's shared record at load and dropping it at unload. */
fn dispatch(page_id: &str, initiator: &str, commands: Vec<MediaCommand>) {
    if commands.is_empty() {
        return;
    }
    let mut hub = hub::lock();
    for mut command in commands {
        debug(|| {
            format!("command {page_id} {command:?}")
                .chars()
                .take(300)
                .collect()
        });
        if command.validate().is_err() {
            continue;
        }
        let key = (page_id.to_string(), command.player());
        match &mut command {
            MediaCommand::Load {
                url,
                blob,
                settings,
                node,
                ..
            } => {
                let mut shared = Shared {
                    url: url.clone(),
                    initiator: initiator.into(),
                    audio: !settings.video,
                    node: *node,
                    ..Shared::default()
                };
                if let Some(source) = blob.take().filter(|source| source.validate(url).is_ok()) {
                    shared.bytes = bytes::Bytes::whole(source.bytes.to_vec(), &source.mime_type);
                    shared.url.clear();
                }
                hub.players.insert(key.clone(), shared);
            }
            MediaCommand::Bind { node, .. } => {
                if let Some(shared) = hub.players.get_mut(&key) {
                    shared.node = *node;
                }
            }
            MediaCommand::Unload { .. } => {
                hub.players.remove(&key);
            }
            _ => {}
        }
        hub.commands.push_back((key, command));
    }
}

/** Updates for every player of one page, and whether any of them is playing. */
fn collect(page_id: &str, now: Instant) -> (Vec<Value>, bool) {
    let mut hub = hub::lock();
    let mut out = Vec::new();
    let mut playing = false;
    for ((id, player), shared) in hub.players.iter_mut() {
        if id != page_id {
            continue;
        }
        playing |= shared.status.playing;
        let updates = updates::diff(*player, &shared.status, &mut shared.reported, now);
        if !updates.is_empty() {
            debug(|| format!("updates {updates:?}").chars().take(400).collect());
        }
        out.extend(updates);
    }
    (out, playing)
}

/** Each of this page's video players' reported frame size, by element. */
fn video_sizes(page_id: &str) -> Vec<(usize, (u32, u32))> {
    let hub = hub::lock();
    hub.players
        .iter()
        .filter(|((id, _), shared)| id == page_id && shared.status.video)
        .filter(|(_, shared)| shared.status.width > 0 && shared.status.height > 0)
        .map(|(_, shared)| (shared.node, (shared.status.width, shared.status.height)))
        .collect()
}

/** Give each video element its media's natural size; true when one changed, so it lays out again. */
fn apply_video_sizes(page: &mut super::page::Page, sizes: &[(usize, (u32, u32))]) -> bool {
    let mut changed = false;
    for (node, size) in sizes {
        let element = std::sync::Arc::make_mut(&mut page.document)
            .nodes
            .iter_mut()
            .find(|candidate| candidate.local_id == *node && candidate.tag == "video");
        if let Some(element) = element.filter(|element| element.image_size != Some(*size)) {
            element.image_size = Some(*size);
            changed = true;
        }
    }
    changed
}

/** The first range a player of this page is waiting for, if any. */
fn next_fetch(page_id: &str) -> Option<Fetch> {
    let hub = hub::lock();
    hub.players.iter().find_map(|(key, shared)| {
        let need = shared.need?;
        if key.0 != page_id || shared.url.is_empty() || shared.bytes.failed.is_some() {
            return None;
        }
        let (start, end) = shared.bytes.plan(need)?;
        Some(Fetch {
            key: key.clone(),
            url: shared.url.clone(),
            initiator: shared.initiator.clone(),
            audio: shared.audio,
            start,
            end,
        })
    })
}

/** Unload every player whose page is gone (navigated away, or its tab closed). */
fn release(live: &BTreeSet<String>) {
    let mut hub = hub::lock();
    let dead: Vec<PlayerKey> = hub
        .players
        .keys()
        .filter(|(page, _)| !live.contains(page))
        .cloned()
        .collect();
    for key in dead {
        hub.players.remove(&key);
        let player = key.1;
        hub.commands
            .push_back((key, MediaCommand::Unload { player }));
    }
}
