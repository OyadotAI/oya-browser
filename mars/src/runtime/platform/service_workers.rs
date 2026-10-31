/**
 * Service workers in the coordinator: the persisted registry, the jobs that change it (Register,
 * Update, Unregister with Install and Activate), the running workers, controller assignment for
 * documents, and fetch interception for controlled pages' subresources and for navigations.
 * Pages reach it through the `sw.*` operations below; a running worker's own requests arrive as
 * the ServiceWorkerGlobalScope operations in `scope`.
 */
mod fetch;
mod jobs;
mod registry;
mod runner;
mod scope;

use super::{Ctx, Handler, Kind, Reply, fail, ok};
use crate::profile::Record;
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/** Which lifecycle event a dispatched event id belongs to. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::runtime) enum Lifecycle {
    Install,
    Activate,
}

/** What exists only while Mars runs: running workers, controllers and event bookkeeping. */
#[derive(Default)]
pub(in crate::runtime) struct Live {
    pub running: BTreeMap<u64, runner::Running>,
    /** Each controlled client's controller version. */
    pub controllers: BTreeMap<String, u64>,
    /** Each page context that used the API, and its storage key. */
    pub clients: BTreeMap<String, ClientInfo>,
    /** Lifecycle events in flight: event id to (version, which). */
    pub lifecycle: BTreeMap<u64, (u64, Lifecycle)>,
    /** Event ids whose outcome someone waits for. */
    pub waited: std::collections::BTreeSet<u64>,
    pub next_event: u64,
    pub console: Vec<String>,
}

/** A page context the service worker side knows: its storage key, URL and tab. A document
that registered during its own load is a client before its tab commits it. */
#[derive(Clone, Debug)]
pub(in crate::runtime) struct ClientInfo {
    pub key: String,
    pub url: String,
    pub tab: String,
}

/** The registry (persisted) and the live state (not). */
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub(in crate::runtime) struct ServiceWorkers {
    pub registry: registry::Registry,
    #[serde(skip)]
    pub live: Live,
}

/** Page-side service worker operations, then the worker scope's. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("sw.register", jobs::register),
    ("sw.update", jobs::update),
    ("sw.unregister", jobs::unregister),
    ("sw.getRegistration", get_registration),
    ("sw.getRegistrations", get_registrations),
    ("sw.ready", ready),
    ("sw.preload", preload),
    ("sw.postMessage", post_message),
    ("sw.skipWaiting", scope::skip_waiting),
    ("sw.clients", scope::clients),
    ("sw.claim", scope::claim),
    ("sw.clientPost", scope::client_post),
    ("sw.navigate", scope::navigate),
];

impl Runtime {
    /** Remember a page context that uses service workers, so state events reach it. */
    pub(in crate::runtime) fn note_client(&mut self, ctx: &Ctx) {
        if matches!(ctx.kind, Kind::Window | Kind::Frame) {
            let info = ClientInfo {
                key: ctx.storage_key(),
                url: ctx.url.to_string(),
                tab: ctx.tab.clone(),
            };
            self.platform
                .workers
                .live
                .clients
                .insert(ctx.identity.clone(), info);
        }
    }

    /** Send an "sw" event to every page context of a storage key. */
    pub(in crate::runtime) fn broadcast_service_worker(&mut self, key: &str, mut event: Value) {
        event["topic"] = json!("sw");
        let identities: Vec<String> = self
            .platform
            .workers
            .live
            .clients
            .iter()
            .filter(|(_, info)| info.key == key)
            .map(|(id, _)| id.clone())
            .collect();
        for identity in identities {
            self.platform.events.push(&identity, event.clone());
        }
        self.platform.changed();
    }

    /** Persist the registry (waiting and active versions only). */
    pub(in crate::runtime) fn save_service_workers(&mut self) {
        self.platform
            .records
            .save(Record::ServiceWorkers, &self.platform.workers);
    }
}

/** getRegistration(url): the registration matching it in this key. */
fn get_registration(
    runtime: &mut Runtime,
    ctx: &Ctx,
    args: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    runtime.note_client(ctx);
    let url = args["url"].as_str().unwrap_or(ctx.url.as_str()).to_string();
    let found = runtime
        .platform
        .workers
        .registry
        .matching(&ctx.storage_key(), &url);
    ok(found.map_or(Value::Null, registry::Registration::record))
}

/** getRegistrations(): every registration of this key. */
fn get_registrations(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    runtime.note_client(ctx);
    let key = ctx.storage_key();
    let list: Vec<Value> = runtime
        .platform
        .workers
        .registry
        .registrations
        .iter()
        .filter(|r| r.key == key)
        .map(registry::Registration::record)
        .collect();
    ok(Value::Array(list))
}

/** ready: the matching registration once it has an active worker; parked until then. */
fn ready(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    runtime.note_client(ctx);
    match runtime
        .platform
        .workers
        .registry
        .matching(&ctx.storage_key(), ctx.url.as_str())
    {
        Some(registration) if registration.active.is_some() => ok(registration.record()),
        _ => Ok(Reply::Park),
    }
}

/** NavigationPreloadManager changes. */
fn preload(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let scope = args["scope"].as_str().unwrap_or_default();
    let Some(registration) = runtime
        .platform
        .workers
        .registry
        .by_scope(&ctx.storage_key(), scope)
    else {
        return fail("InvalidStateError", "The registration is no longer valid.");
    };
    if let Some(enabled) = args["enabled"].as_bool() {
        registration.preload_enabled = enabled;
    }
    if let Some(value) = args["headerValue"].as_str() {
        registration.preload_header = value.into();
    }
    let record = registration.record();
    runtime.save_service_workers();
    ok(record)
}

/** ServiceWorker.postMessage from a page: a message event at that worker. */
fn post_message(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let version = args["worker"].as_u64().unwrap_or_default();
    let Some(registration) = runtime
        .platform
        .workers
        .registry
        .holding(version)
        .map(|r| r.id)
    else {
        return ok(Value::Null);
    };
    runtime.start_service_worker(registration, version)?;
    let source = json!({"id":ctx.identity,"url":ctx.url.as_str(),"type":"window","frameType":if ctx.kind == Kind::Frame {"nested"} else {"top-level"},"focused":ctx.selected,"visibilityState":"visible"});
    runtime.queue_service_worker_event(
        version,
        json!({"kind":"message","data":args["data"],"origin":ctx.origin,"source":source}),
        false,
    );
    ok(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::test_support::MemoryRecordStore;
    use std::sync::Arc;

    /** An activated registration survives a restart through the record store; live state does not. */
    #[test]
    fn registrations_persist_across_restarts() {
        let store = Arc::new(MemoryRecordStore::default());
        let mut records = super::super::records::Records::default();
        records.attach(store);
        let mut workers = ServiceWorkers::default();
        let id = workers.registry.id();
        let active = registry::Version {
            id: workers.registry.id(),
            script_url: "https://a.test/sw.js".into(),
            source: "self.x=1".into(),
            state: "activated".into(),
            has_fetch: true,
            skip_waiting: false,
        };
        let installing = registry::Version {
            id: workers.registry.id(),
            state: "installing".into(),
            ..registry::Version::default()
        };
        workers.registry.registrations.push(registry::Registration {
            id,
            key: "https://a.test https://a.test".into(),
            scope: "https://a.test/".into(),
            active: Some(active.clone()),
            installing: Some(installing),
            ..registry::Registration::default()
        });
        workers
            .live
            .controllers
            .insert("context-1".into(), active.id);
        assert!(records.save(Record::ServiceWorkers, &workers));
        let restored: ServiceWorkers = records.load(Record::ServiceWorkers);
        let registration = restored
            .registry
            .matching("https://a.test https://a.test", "https://a.test/page")
            .unwrap();
        assert_eq!(registration.active.as_ref(), Some(&active));
        assert!(registration.installing.is_none());
        assert!(restored.live.controllers.is_empty());
        assert_eq!(restored.registry.next_id, workers.registry.next_id);
    }
}
