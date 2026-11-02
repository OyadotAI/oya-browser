/**
 * ServiceWorkerGlobalScope operations a running worker sends: skipWaiting, the Clients
 * interface (get, matchAll, claim, Client.postMessage) and WindowClient.navigate. Clients are
 * the tabs' committed top-level pages of the worker's storage key.
 */
use super::super::{Ctx, Kind, Reply, fail, ok};
use super::Runtime;
use crate::protocol::EngineResult;
use serde_json::{Value, json};

/** One window client: identity, URL and whether its tab is selected. */
struct Client {
    identity: String,
    url: String,
    selected: bool,
}

impl Client {
    /** The Client record the worker's Clients API exposes. */
    fn record(&self) -> Value {
        json!({"id":self.identity,"url":self.url,"type":"window","frameType":"top-level",
            "focused":self.selected,"visibilityState":if self.selected {"visible"} else {"hidden"}})
    }
}

impl Runtime {
    /** The registration id, key and active version id of the worker `version`. */
    fn worker_registration(&mut self, version: u64) -> Option<(u64, String, Option<u64>)> {
        let registration = self.platform.workers.registry.holding(version)?;
        Some((
            registration.id,
            registration.key.clone(),
            registration.active.as_ref().map(|v| v.id),
        ))
    }

    /** Window clients of a storage key: every tab's top-level page, and live documents that used
    the API before their tab committed them. */
    fn window_clients(&mut self, key: &str) -> Vec<Client> {
        let selected = self.selected_tab_id().to_string();
        let mut clients: Vec<Client> = self
            .tab_pages()
            .into_iter()
            .filter_map(|(tab, page)| {
                let url = url::Url::parse(&page.url).ok()?;
                let site = crate::network::partition_site(&url)?;
                let client_key = format!("{} {site}", url.origin().ascii_serialization());
                (client_key == key).then(|| Client {
                    identity: page.script_identity.clone(),
                    url: page.url.clone(),
                    selected: tab == selected,
                })
            })
            .collect();
        let live: std::collections::BTreeSet<String> =
            self.residency.snapshot().into_iter().collect();
        for (identity, info) in &self.platform.workers.live.clients {
            if info.key == key
                && live.contains(identity)
                && !clients.iter().any(|c| &c.identity == identity)
            {
                clients.push(Client {
                    identity: identity.clone(),
                    url: info.url.clone(),
                    selected: info.tab == selected,
                });
            }
        }
        clients
    }
}

/** The worker version a scope request came from, or None for a page. */
fn version_of(ctx: &Ctx) -> Option<u64> {
    match ctx.kind {
        Kind::Service(version) => Some(version),
        _ => None,
    }
}

/** skipWaiting(): mark the version, and activate it now if it is waiting. */
pub(super) fn skip_waiting(
    runtime: &mut Runtime,
    ctx: &Ctx,
    _: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    let Some(version) = version_of(ctx) else {
        return fail(
            "InvalidStateError",
            "Only a service worker can skip waiting.",
        );
    };
    let Some(registration) = runtime.platform.workers.registry.holding(version) else {
        return ok(Value::Null);
    };
    let id = registration.id;
    if let Some(found) = registration.version_mut(version) {
        found.skip_waiting = true;
    }
    if registration
        .waiting
        .as_ref()
        .is_some_and(|v| v.id == version)
    {
        runtime.try_activate(id)?;
    }
    ok(Value::Null)
}

/** clients.get(id) and clients.matchAll(options). */
pub(super) fn clients(
    runtime: &mut Runtime,
    ctx: &Ctx,
    args: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    let Some((_, key, active)) = version_of(ctx).and_then(|v| runtime.worker_registration(v))
    else {
        return ok(json!([]));
    };
    let windows = matches!(args["type"].as_str(), None | Some("window" | "all"));
    let uncontrolled =
        args["includeUncontrolled"].as_bool() == Some(true) || args["id"].is_string();
    let controllers = runtime.platform.workers.live.controllers.clone();
    let list: Vec<Value> = runtime
        .window_clients(&key)
        .into_iter()
        .filter(|_| windows || args["id"].is_string())
        .filter(|client| args["id"].as_str().is_none_or(|id| id == client.identity))
        .filter(|client| uncontrolled || controllers.get(&client.identity) == active.as_ref())
        .map(|client| client.record())
        .collect();
    ok(Value::Array(list))
}

/** clients.claim(): the active worker controls every in-scope client of its key now. */
pub(super) fn claim(runtime: &mut Runtime, ctx: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    let version = version_of(ctx).unwrap_or_default();
    let Some((registration, key, active)) = runtime.worker_registration(version) else {
        return ok(Value::Null);
    };
    if active != Some(version) {
        return fail(
            "InvalidStateError",
            "Only the active worker can claim clients.",
        );
    }
    let worker = runtime
        .platform
        .workers
        .registry
        .by_id(registration)
        .and_then(|r| r.active.as_ref())
        .map(super::registry::Version::record);
    for client in runtime.window_clients(&key) {
        let matched = runtime
            .platform
            .workers
            .registry
            .matching(&key, &client.url)
            .map(|r| r.id);
        let current = runtime
            .platform
            .workers
            .live
            .controllers
            .get(&client.identity)
            .copied();
        if matched == Some(registration) && current != Some(version) {
            runtime
                .platform
                .workers
                .live
                .controllers
                .insert(client.identity.clone(), version);
            runtime.platform.events.subscribe(&client.identity, "sw");
            runtime.platform.events.push(
                &client.identity,
                json!({"topic":"sw","kind":"controller","worker":worker}),
            );
        }
    }
    runtime.platform.changed();
    ok(Value::Null)
}

/** Client.postMessage(): a message event at the client's navigator.serviceWorker. */
pub(super) fn client_post(
    runtime: &mut Runtime,
    ctx: &Ctx,
    args: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    let version = version_of(ctx).unwrap_or_default();
    let Some(worker) = runtime
        .platform
        .workers
        .registry
        .holding(version)
        .and_then(|r| {
            r.versions()
                .find(|v| v.id == version)
                .map(super::registry::Version::record)
        })
    else {
        return ok(Value::Null);
    };
    let client = args["client"].as_str().unwrap_or_default().to_string();
    runtime.platform.events.subscribe(&client, "sw");
    runtime.platform.events.push(&client, json!({"topic":"sw","kind":"message","data":args["data"],"origin":ctx.origin,"source":worker}));
    runtime.platform.changed();
    ok(Value::Null)
}

/** WindowClient.navigate(): Mars cannot yet navigate another tab from a worker. */
pub(super) fn navigate(_: &mut Runtime, _: &Ctx, _: &Value, _: &[u8]) -> EngineResult<Reply> {
    fail(
        "TypeError",
        "Failed to navigate the client: navigating a client from a service worker is not supported in Mars yet.",
    )
}
