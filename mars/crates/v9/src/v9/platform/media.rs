/**
 * Media elements cross the host boundary as data. The platform bootstrap (`media.js`) implements
 * HTMLMediaElement, text tracks and Media Source in script over a private host object built
 * here; this module owns what that script cannot: the inert commands a page sends the
 * coordinator, the updates it gets back, the elements whose media attributes or tree position
 * changed (handed to the script at the next stable state) and DOM identity adoption.
 */
use super::{Page, dom, media_source};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Thrown, Vm};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/** Commands one step may queue; a page posting more is refused rather than flooding the host. */
const COMMANDS: usize = 512;
/** Bytes one step may append to Media Source buffers, so a step's effects stay frame-bounded. */
const APPEND_BYTES: usize = 64 * 1024 * 1024;
/** Text one command may carry (URLs, captions, codec strings). */
const COMMAND_TEXT: usize = 64 * 1024;
/** Players one page may own at once; Chrome caps concurrent players similarly per frame. */
const PLAYERS: usize = 75;
/** Update text one host delivery may carry. */
pub const UPDATE_BYTES: usize = 1024 * 1024;
/** Tags whose attributes and tree position the media script follows. */
const TAGS: [&str; 4] = ["video", "audio", "source", "track"];
/** Attributes whose change can start or change a load (src, source selection, track loading). */
const ATTRIBUTES: [&str; 7] = [
    "src", "type", "media", "kind", "srclang", "label", "default",
];

/// The codecs and containers the host's decoders actually play, probed by the coordinator
/// (AVFoundation on macOS). `canPlayType`, `isTypeSupported` and `mediaCapabilities` answer
/// from it; an empty list (a host without decoders) answers "" everywhere.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaSupport {
    /// Playable container types, lowercase essences such as `video/mp4`.
    pub containers: Vec<String>,
    /// Playable `container|codec` pairs, the codec as its four-character family (`avc1`, `mp4a`).
    pub codecs: Vec<String>,
    /// Container types the host can also play from appended Media Source segments.
    pub streams: Vec<String>,
}

/// Playback state a page last asked for; the host applies it whole, so a lost or reordered
/// partial change cannot leave the player in a state the page never requested.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MediaSettings {
    pub paused: bool,
    pub rate: f64,
    pub volume: f64,
    pub muted: bool,
    pub looping: bool,
    pub preserves_pitch: bool,
    /// The element shows native controls, which the host paints and answers clicks on.
    pub controls: bool,
    /// `none`, `metadata` or `auto`; `none` defers fetching until play or seek.
    pub preload: String,
    /// Video draws frames into its box; audio has none.
    pub video: bool,
}

/// One instruction from a page to its media host. Every variant names the page-local player
/// the script allocated at load; the host ignores players it never saw loaded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum MediaCommand {
    /// Start a fresh resource for an element: a URL, a captured Blob, or a Media Source.
    Load {
        player: u64,
        #[serde(default)]
        node: usize,
        url: String,
        #[serde(default)]
        stream: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        blob: Option<super::BlobUrlSource>,
        settings: MediaSettings,
    },
    /// Replace the playback settings.
    Settings {
        player: u64,
        settings: MediaSettings,
    },
    /// Seek to a media time in seconds; `fast` allows the nearest keyframe.
    Seek { player: u64, time: f64, fast: bool },
    /// Media Source: append a segment to one source buffer of the given kind.
    Append {
        player: u64,
        buffer: u32,
        #[serde(default)]
        mime: String,
        #[serde(default)]
        bytes: Vec<u8>,
    },
    /// Media Source: drop buffered media of one buffer between two times.
    Remove {
        player: u64,
        buffer: u32,
        start: f64,
        end: f64,
    },
    /// Media Source: no more segments; the duration is final.
    EndOfStream { player: u64, duration: f64 },
    /// The cue text currently showing, painted over the video by the host.
    Captions { player: u64, lines: Vec<String> },
    /// The element moved to a new DOM identity; paint its frames there.
    Bind { player: u64, node: usize },
    /// Release the player and everything it fetched.
    Unload { player: u64 },
}

impl MediaCommand {
    /** The player this command addresses. */
    pub fn player(&self) -> u64 {
        match self {
            Self::Load { player, .. }
            | Self::Settings { player, .. }
            | Self::Seek { player, .. }
            | Self::Append { player, .. }
            | Self::Remove { player, .. }
            | Self::EndOfStream { player, .. }
            | Self::Captions { player, .. }
            | Self::Bind { player, .. }
            | Self::Unload { player } => *player,
        }
    }
    /** Bounded text and finite numbers, checked on both sides of the pipe. */
    pub fn validate(&self) -> EngineResult<()> {
        let finite = |value: f64| value.is_finite();
        let ok = match self {
            Self::Load { url, settings, .. } => url.len() <= COMMAND_TEXT && settings_ok(settings),
            Self::Settings { settings, .. } => settings_ok(settings),
            Self::Seek { time, .. } => finite(*time) && *time >= 0.0,
            Self::Append { mime, bytes, .. } => {
                mime.len() <= COMMAND_TEXT && bytes.len() <= APPEND_BYTES
            }
            Self::Remove { start, end, .. } => finite(*start) && !end.is_nan(),
            Self::EndOfStream { duration, .. } => !duration.is_nan(),
            Self::Captions { lines, .. } => {
                lines.iter().map(String::len).sum::<usize>() <= COMMAND_TEXT
            }
            Self::Bind { .. } | Self::Unload { .. } => true,
        };
        ok.then_some(())
            .ok_or_else(|| EngineError::invalid("Invalid media command."))
    }
}

/** Rates and volumes a page can set are already range-checked by the script; this is the wire bound. */
fn settings_ok(settings: &MediaSettings) -> bool {
    settings.rate.is_finite()
        && (0.0..=1.0).contains(&settings.volume)
        && settings.preload.len() <= 16
}

/** Per-page media bookkeeping. It holds no media data, only identities and the script callback. */
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(in crate::v9) struct State {
    /** The bootstrap's dispatcher, called with ("touched", elements) and ("updates", json). */
    pub handler: Option<ObjectId>,
    /** Media-related elements changed since the last stable state. */
    pub touched: BTreeSet<usize>,
    /** Node of each loaded player, so DOM adoption can rebind where frames paint. */
    pub players: BTreeMap<usize, u64>,
    /** A trusted click or key reached this page: unmuted autoplay is allowed from now on. */
    pub activated: bool,
    /** The script implementation was built (it is built on a page's first media use). */
    pub built: bool,
}

/** The private host object: built once per realm, never put on a global. */
pub(super) fn install(b: &mut Builder) {
    let host = b.object(Some(b.ids.object_prototype));
    b.method(host, "post", 2, post);
    b.method(host, "listen", 1, listen);
    b.method(host, "takeTouched", 0, |vm, _| take_touched(vm));
    b.method(host, "support", 0, support);
    b.method(host, "activation", 0, activation);
    b.method(host, "append", 3, append);
    b.ids.media_host = host;
}

/** `host.post(element, json)`: validate and queue one command, filling the element's node. */
fn post(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = vm.to_text(&call.arg(1))?;
    let mut command: MediaCommand = serde_json::from_str(&text)
        .map_err(|_| Thrown::Fatal(EngineError::invalid("Invalid media command.")))?;
    let node = dom::node_of(vm, &call.arg(0));
    attach(vm, &mut command, node, &call.arg(2))?;
    command.validate()?;
    if vm.page.effects.media.len() >= COMMANDS {
        return Err(EngineError::limit("media commands", COMMANDS).into());
    }
    track(&mut vm.page, &command, node)?;
    vm.page.effects.media.push(command);
    Ok(JsValue::Undefined)
}

/** Fill what only the native side knows: the node, Blob bytes and appended segment bytes. */
fn attach(vm: &mut Vm, command: &mut MediaCommand, node: Option<usize>, data: &JsValue) -> Res<()> {
    match command {
        MediaCommand::Load {
            node: target,
            url,
            blob,
            ..
        } => {
            *target = node.ok_or_else(|| vm.type_error("Illegal invocation"))?;
            *blob = super::blob_urls::resolve(&vm.page, url);
        }
        MediaCommand::Append { bytes, .. } => {
            *bytes = crate::v9::builtins::typed::source_bytes(vm, data)
                .ok_or_else(|| vm.type_error("The data is not a BufferSource."))?;
        }
        _ => {}
    }
    Ok(())
}

/** Keep the node to player map in step with loads, so adoption can rebind and removal can pause. */
fn track(page: &mut Page, command: &MediaCommand, node: Option<usize>) -> EngineResult<()> {
    match command {
        MediaCommand::Load { player, node, .. } => {
            if !page.state.media.players.contains_key(node)
                && page.state.media.players.len() >= PLAYERS
            {
                return Err(EngineError::limit("media players", PLAYERS));
            }
            page.state.media.players.insert(*node, *player);
        }
        MediaCommand::Unload { player } => {
            if let Some(node) = node.filter(|n| page.state.media.players.get(n) == Some(player)) {
                page.state.media.players.remove(&node);
            }
        }
        _ => {}
    }
    Ok(())
}

/** `host.listen(fn)`: the bootstrap's single dispatcher. */
fn listen(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let handler = call.arg(0);
    if !vm.is_callable(&handler) {
        return Err(vm.type_error("Illegal invocation"));
    }
    vm.page.state.media.handler = handler.object();
    Ok(JsValue::Undefined)
}

/** The changed media elements, once each, as wrappers; taken so a throwing handler cannot loop. */
fn take_touched(vm: &mut Vm) -> Res<JsValue> {
    let nodes = std::mem::take(&mut vm.page.state.media.touched);
    let mut elements = Vec::new();
    for node in nodes {
        let element = dom::wrap(vm, node)?;
        if !matches!(element, JsValue::Null) {
            elements.push(element);
        }
    }
    Ok(JsValue::Object(vm.new_array(elements)?))
}

/** `host.support()`: the host's decoder support as JSON. */
fn support(vm: &mut Vm, _: Call) -> Res<JsValue> {
    let text = serde_json::to_string(&vm.page.environment.media).unwrap_or_default();
    Ok(JsValue::string(&text)?)
}

/**
 * `host.append(bytes, init, request)`: the coded frame processing of one Media Source append, as JSON. The result
 * is the outcome, or `{"error": text}` when the bytes are refused, so the script can raise its append error.
 */
fn append(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bytes = crate::v9::builtins::typed::source_bytes(vm, &call.arg(0))
        .ok_or_else(|| vm.type_error("The data is not a BufferSource."))?;
    if bytes.len() > APPEND_BYTES {
        return Err(EngineError::limit("media segment bytes", APPEND_BYTES).into());
    }
    let init = crate::v9::builtins::typed::source_bytes(vm, &call.arg(1));
    let request = vm.to_text(&call.arg(2))?;
    let request: media_source::Request = serde_json::from_str(&request)
        .map_err(|_| Thrown::Fatal(EngineError::invalid("Invalid media segment request.")))?;
    let text = match media_source::append(init.as_deref(), &bytes, &request) {
        Ok(outcome) => serde_json::to_string(&outcome),
        Err(refusal) => serde_json::to_string(&serde_json::json!({ "error": refusal.message() })),
    };
    Ok(JsValue::string(&text.unwrap_or_default())?)
}

/** `host.activation()`: whether a person has clicked or typed in this page (sticky activation). */
fn activation(vm: &mut Vm, _: Call) -> Res<JsValue> {
    Ok(JsValue::Boolean(vm.page.state.media.activated))
}

/** An attribute that can change what a media element loads was written. */
pub(super) fn attribute_changed(page: &mut Page, node: usize, name: &str) {
    if ATTRIBUTES.contains(&name) && TAGS.contains(&page.tag(node).as_str()) {
        page.state.media.touched.insert(node);
    }
}

/** A subtree joined a parent: its media elements may now select a source or load a track. */
pub(super) fn inserted(page: &mut Page, root: usize) {
    if TAGS.contains(&page.tag(root).as_str()) {
        page.state.media.touched.insert(root);
    }
    for node in page.descendants(root) {
        if TAGS.contains(&page.tag(node).as_str()) {
            page.state.media.touched.insert(node);
        }
    }
}

/** Markup inserted `count` fresh nodes from `first`: its media elements may load. */
pub(super) fn inserted_range(page: &mut Page, first: usize, count: usize) {
    for node in first..first.saturating_add(count) {
        if TAGS.contains(&page.tag(node).as_str()) {
            page.state.media.touched.insert(node);
        }
    }
}

/** A subtree is about to leave its parent: playing elements inside it must pause. */
pub(super) fn removing(page: &mut Page, root: usize) {
    let players: Vec<usize> = page.state.media.players.keys().copied().collect();
    for node in players {
        if node == root || page.ancestors(node).contains(&root) {
            page.state.media.touched.insert(node);
        }
    }
}

/** DOM adoption moved an element to a new identity: frames must follow it. */
pub(super) fn adopt(page: &mut Page, old: usize, new: usize) -> EngineResult<()> {
    if page.state.media.touched.remove(&old) {
        page.state.media.touched.insert(new);
    }
    let Some(player) = page.state.media.players.remove(&old) else {
        return Ok(());
    };
    page.state.media.players.insert(new, player);
    page.effects
        .media
        .push(MediaCommand::Bind { player, node: new });
    Ok(())
}

/** A trusted click or key: from now on the page may start audible playback. */
pub(in crate::v9) fn activate(page: &mut Page) {
    page.state.media.activated = true;
}

/**
 * At a stable state (the microtask queue ran dry), hand changed elements to the script. True
 * when the script ran, so the caller drains the microtasks it queued.
 */
pub(in crate::v9) fn stable_state(vm: &mut Vm) -> Res<bool> {
    let Some(handler) = vm.page.state.media.handler else {
        vm.page.state.media.touched.clear();
        return Ok(false);
    };
    if vm.page.state.media.touched.is_empty() {
        return Ok(false);
    }
    vm.page.state.media.built = true;
    let elements = take_touched(vm)?;
    let kind = JsValue::string("touched")?;
    vm.call(
        JsValue::Object(handler),
        JsValue::Undefined,
        vec![kind, elements],
    )?;
    Ok(true)
}

/** Host updates (JSON produced by the coordinator) go to the script's dispatcher. */
pub(in crate::v9) fn deliver(vm: &mut Vm, updates: &str) -> Res<()> {
    if updates.len() > UPDATE_BYTES {
        return Err(EngineError::limit("media update bytes", UPDATE_BYTES).into());
    }
    let Some(handler) = vm.page.state.media.handler else {
        return Ok(());
    };
    // A click on native controls is a person's gesture: audible playback is allowed from now on.
    if updates.contains("\"kind\":\"control\"") {
        activate(&mut vm.page);
    }
    vm.page.state.media.built = true;
    let args = vec![JsValue::string("updates")?, JsValue::string(updates)?];
    vm.call(JsValue::Object(handler), JsValue::Undefined, args)?;
    Ok(())
}

/// Interfaces whose inert shapes stand in until the implementation is built: constructing one
/// builds it, then constructs the real interface.
const CONSTRUCTIBLE: [&str; 3] = ["VTTCue", "VTTRegion", "MediaSource"];
/// Shape members outside media elements whose first use builds the implementation.
const ENTRY_MEMBERS: [&str; 3] = ["isTypeSupported", "decodingInfo", "encodingInfo"];

/** Build the script implementation once; true when this call built it. */
fn ensure(vm: &mut Vm) -> Res<bool> {
    let Some(handler) = vm
        .page
        .state
        .media
        .handler
        .filter(|_| !vm.page.state.media.built)
    else {
        return Ok(false);
    };
    vm.page.state.media.built = true;
    vm.call(
        JsValue::Object(handler),
        JsValue::Undefined,
        vec![JsValue::string("ensure")?],
    )?;
    Ok(true)
}

/**
 * An inert shape member was used on a media element (or a media entry point such as
 * `MediaSource.isTypeSupported`) before the implementation existed: build it and answer with
 * the real member. None when the shape should answer as before.
 */
pub(in crate::v9) fn upgrade(
    vm: &mut Vm,
    call: &Call,
    name: &str,
    setter: bool,
) -> Res<Option<JsValue>> {
    let media =
        dom::node_of(vm, &call.this).is_some_and(|node| TAGS.contains(&vm.page.tag(node).as_str()));
    if !(media || ENTRY_MEMBERS.contains(&name)) || !ensure(vm)? {
        return Ok(None);
    }
    let key = crate::v9::vm::heap::Key::from(name);
    // A static member was read from the shape's interface object; the built one is the global's.
    let this = if name == "isTypeSupported" {
        let global = JsValue::Object(vm.global_object());
        vm.get(&global, &crate::v9::vm::heap::Key::from("MediaSource"))?
    } else {
        call.this.clone()
    };
    if setter {
        vm.set(&this, key, call.arg(0), true)?;
        return Ok(Some(JsValue::Undefined));
    }
    let value = vm.get(&this, &key)?;
    if method(vm, &this, &key)? {
        return vm.call(value, this, call.args.to_vec()).map(Some);
    }
    Ok(Some(value))
}

/** Whether the member the receiver now finds is a method (a data property) rather than an accessor. */
fn method(vm: &mut Vm, this: &JsValue, key: &crate::v9::vm::heap::Key) -> Res<bool> {
    let mut next = this.object();
    while let Some(id) = next {
        if let Some(prop) = vm.own(id, key)? {
            return Ok(matches!(prop.slot, crate::v9::vm::heap::Slot::Data(_)));
        }
        next = vm.heap.get(id).and_then(|object| object.proto);
    }
    Ok(false)
}

/** `new VTTCue(...)` before the implementation exists: build it and construct the real one. */
pub(in crate::v9) fn construct(vm: &mut Vm, call: &Call, name: &str) -> Res<Option<JsValue>> {
    if !CONSTRUCTIBLE.contains(&name) || call.new_target.is_none() || !ensure(vm)? {
        return Ok(None);
    }
    let global = JsValue::Object(vm.global_object());
    let real = vm.get(&global, &crate::v9::vm::heap::Key::from(name))?;
    vm.construct(real, call.args.to_vec(), None).map(Some)
}

/** The dispatcher stays alive as long as the page, and so does every element with a player:
its script state lives in a weak map keyed by its wrapper, which must not be collected while it
plays (a `new Audio()` nobody references keeps playing, as in browsers). */
pub(super) fn roots(state: &State, wrappers: &BTreeMap<usize, ObjectId>, out: &mut Vec<JsValue>) {
    out.extend(state.handler.map(JsValue::Object));
    out.extend(
        state
            .players
            .keys()
            .filter_map(|node| wrappers.get(node))
            .map(|id| JsValue::Object(*id)),
    );
}

/** Restored state must name a live callable dispatcher and stay within the player bound. */
pub(in crate::v9) fn validate(vm: &Vm) -> EngineResult<()> {
    let state = &vm.page.state.media;
    let handler_ok = state
        .handler
        .is_none_or(|id| vm.is_callable(&JsValue::Object(id)));
    if !handler_ok || state.players.len() > PLAYERS {
        return Err(EngineError::invalid("Invalid media state."));
    }
    Ok(())
}
