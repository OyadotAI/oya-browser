/**
 * The coordinator side of V9's platform bridge (`host_bridge` and the `platform_apis` scripts):
 * one "platform" HostRequest per API call, dispatched by operation name through a command map to
 * the module that owns the work (locks, cookies, clipboard, Cache Storage, service workers and
 * the devices). A handler answers at once or parks the request; parked requests are retried
 * whenever platform state changes, so a lock grant, a queued event or a service worker becoming
 * ready reaches the page on its next script step.
 */
mod access;
mod cache_storage;
mod clearkey;
mod clipboard;
mod cookies;
mod credentials;
mod events;
mod gamepad;
mod locks;
mod midi;
mod records;
mod requests;
mod service_workers;
mod wake_lock;

use super::supervisor::Runtime;
use crate::protocol::{EngineError, EngineResult};
pub use clearkey::DecryptError;
pub(super) use requests::Parked;
use serde_json::{Value, json};
use url::Url;

/** Which kind of global made a request. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /** A top-level document. */
    Window,
    /** A document in a frame. */
    Frame,
    /** A dedicated worker. */
    Worker,
    /** A service worker; the version it runs. */
    Service(u64),
}

/** Who asked: the context, where it is, and what the request carried. */
#[derive(Clone, Debug)]
pub(super) struct Ctx {
    /** The context's lease identity, stable for its lifetime. */
    pub identity: String,
    pub kind: Kind,
    /** The document's or worker's current URL. */
    pub url: Url,
    /** The ASCII serialization of the context's origin. */
    pub origin: String,
    /** The top-level document's URL, whose site partitions storage. */
    pub top_level: Url,
    /** Transient user activation, as V9 recorded it (never the page's claim). */
    pub active: bool,
    /** The context's tab is the selected tab, so its document is visible and focused. */
    pub selected: bool,
    /** The tab the context belongs to. */
    pub tab: String,
}

impl Ctx {
    /** The storage key: origin plus the top-level schemeful site. */
    pub fn storage_key(&self) -> String {
        format!("{} {}", self.origin, self.partition())
    }
    /** The top-level schemeful site, or the origin when the URL has none. */
    pub fn partition(&self) -> String {
        crate::network::partition_site(&self.top_level).unwrap_or_else(|| self.origin.clone())
    }
    /** Whether this context's site differs from its top-level site (a third-party frame). */
    pub fn cross_site(&self) -> bool {
        crate::network::partition_site(&self.url) != crate::network::partition_site(&self.top_level)
    }
}

/** A handler's answer: a JSON body (`{ok}` or `{error}`) with optional bytes, or park. */
pub(super) enum Reply {
    Answer(Value, Vec<u8>),
    Park,
}

/** `{ok: value}`. */
pub(super) fn ok(value: Value) -> EngineResult<Reply> {
    Ok(Reply::Answer(json!({ "ok": value }), Vec::new()))
}

/** `{ok: value}` with bytes. */
pub(super) fn ok_bytes(value: Value, bytes: Vec<u8>) -> EngineResult<Reply> {
    Ok(Reply::Answer(json!({ "ok": value }), bytes))
}

/** `{error: {name, message}}`: the DOMException (or TypeError) the page's promise rejects with. */
pub(super) fn fail(name: &str, message: &str) -> EngineResult<Reply> {
    Ok(Reply::Answer(
        json!({ "error": { "name": name, "message": message } }),
        Vec::new(),
    ))
}

/** One operation's handler. */
pub(super) type Handler = fn(&mut Runtime, &Ctx, &Value, &[u8]) -> EngineResult<Reply>;

/** Every module's operations, looked up by name. */
const TABLES: &[&[(&str, Handler)]] = &[
    access::HANDLERS,
    cache_storage::HANDLERS,
    clearkey::HANDLERS,
    clipboard::HANDLERS,
    cookies::HANDLERS,
    credentials::HANDLERS,
    locks_handlers::HANDLERS,
    midi::HANDLERS,
    wake_lock::HANDLERS,
    service_workers::HANDLERS,
    EVENT_HANDLERS,
];

/** The handler for `op`, or None for an operation no module owns. */
pub(super) fn handler(op: &str) -> Option<Handler> {
    TABLES
        .iter()
        .flat_map(|table| table.iter())
        .find(|(name, _)| *name == op)
        .map(|(_, handler)| *handler)
}

/** The event channel every API shares. */
const EVENT_HANDLERS: &[(&str, Handler)] = &[
    ("events.subscribe", subscribe),
    ("events.next", next_events),
];

/** Subscribe the context to one topic, starting whatever feeds it. */
fn subscribe(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let topic = args["topic"].as_str().unwrap_or_default();
    if !TOPICS.contains(&topic) {
        return fail("TypeError", "Unknown event topic.");
    }
    runtime.platform.events.subscribe(&ctx.identity, topic);
    match topic {
        "gamepad" => runtime.platform.gamepads.start(),
        "cookies" => runtime.platform.cookie_watch.start(ctx),
        "sw" => runtime.note_client(ctx),
        _ => {}
    }
    ok(Value::Null)
}

/** Topics a context may subscribe to. */
const TOPICS: &[&str] = &["cookies", "sw", "locks", "wakelock", "gamepad", "midi"];

/** Answer with queued events, or park until some arrive. */
fn next_events(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    match runtime.platform.events.take(&ctx.identity) {
        Some(events) => ok(Value::Array(events)),
        None => Ok(Reply::Park),
    }
}

/** Lock operations live beside the manager; the table is here so the manager stays pure. */
mod locks_handlers {
    use super::*;

    /** Web Locks operations. */
    pub(in crate::runtime) const HANDLERS: &[(&str, Handler)] = &[
        ("locks.request", request),
        ("locks.release", release),
        ("locks.abort", abort),
        ("locks.query", query),
    ];

    /** The request these arguments describe for this context. */
    fn lock_request(ctx: &Ctx, args: &Value) -> locks::LockRequest {
        locks::LockRequest {
            owner: ctx.identity.clone(),
            id: args["id"].as_u64().unwrap_or_default(),
            key: ctx.storage_key(),
            name: args["name"].as_str().unwrap_or_default().into(),
            exclusive: args["mode"].as_str() != Some("shared"),
        }
    }

    /** Request a lock: steal, try once (ifAvailable) or queue and park until granted. */
    fn request(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
        let request = lock_request(ctx, args);
        let platform = &mut runtime.platform;
        if platform.locks.holds(&ctx.identity, request.id) {
            return ok(json!({ "granted": true }));
        }
        if args["steal"].as_bool() == Some(true) {
            for robbed in platform.locks.steal(request) {
                platform
                    .events
                    .push(&robbed.owner, json!({"topic":"locks","stolen":robbed.id}));
            }
            platform.changed();
            return ok(json!({ "granted": true }));
        }
        let waiting = platform.locks.waiting(&ctx.identity, request.id);
        if args["ifAvailable"].as_bool() == Some(true)
            && !waiting
            && !platform.locks.available(&request)
        {
            return ok(json!({ "granted": false }));
        }
        if !waiting
            && platform
                .aborted_locks
                .remove(&(ctx.identity.clone(), request.id))
        {
            return ok(json!({ "aborted": true }));
        }
        let id = request.id;
        platform.locks.enqueue(request);
        platform.locks.process();
        if platform.locks.holds(&ctx.identity, id) {
            return ok(json!({ "granted": true }));
        }
        Ok(Reply::Park)
    }

    /** Release a held lock; whoever waited behind it may now be granted. */
    fn release(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
        let id = args["id"].as_u64().unwrap_or_default();
        runtime.platform.locks.release(&ctx.identity, id);
        runtime.platform.locks.process();
        runtime.platform.changed();
        ok(Value::Null)
    }

    /** A signal aborted a waiting request: drop it so its parked request answers "aborted". */
    fn abort(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
        let id = args["id"].as_u64().unwrap_or_default();
        if runtime.platform.locks.waiting(&ctx.identity, id) {
            runtime.platform.locks.release(&ctx.identity, id);
            runtime
                .platform
                .aborted_locks
                .insert((ctx.identity.clone(), id));
            runtime.platform.changed();
        }
        ok(Value::Null)
    }

    /** The held and pending locks of this context's storage key. */
    fn query(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
        ok(runtime.platform.locks.query(&ctx.storage_key()))
    }
}

/** Everything the platform APIs keep in the coordinator. */
#[derive(Default)]
pub(super) struct Platform {
    pub events: events::Events,
    pub locks: locks::Locks,
    /** Waiting lock requests a signal aborted, until their parked request answers. */
    pub aborted_locks: std::collections::BTreeSet<(String, u64)>,
    pub access: access::Grants,
    pub cookie_watch: cookies::Watch,
    pub credentials: credentials::Store,
    pub wake: wake_lock::WakeLocks,
    pub gamepads: gamepad::Gamepads,
    pub midi: midi::Midi,
    pub keys: clearkey::Keys,
    pub caches: cache_storage::CacheStorage,
    pub workers: service_workers::ServiceWorkers,
    pub records: records::Records,
    /** Bumped on every change a parked request may be waiting for. */
    pub version: u64,
}

impl Platform {
    /** Record a change, so parked requests are retried. */
    pub fn changed(&mut self) {
        self.version = self.version.wrapping_add(1);
    }
}

/** The request body V9's bridge sends: the JSON arguments and whether the page was activated. */
pub(super) fn parse_body(body: Option<&str>) -> EngineResult<(Value, bool)> {
    let body: Value = serde_json::from_str(body.unwrap_or("{}"))
        .map_err(|_| EngineError::invalid("Invalid platform request."))?;
    let args = serde_json::from_str(body["args"].as_str().unwrap_or("null"))
        .map_err(|_| EngineError::invalid("Invalid platform arguments."))?;
    Ok((args, body["active"].as_bool().unwrap_or(false)))
}
