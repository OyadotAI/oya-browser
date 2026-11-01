/**
 * The Service Workers spec's jobs: Register and Update fetch the script (same origin, a
 * JavaScript MIME type, no redirects, the scope under its maximum), skip byte-identical scripts,
 * and Install the new version; Install success makes it waiting and Try Activate promotes it when
 * nothing holds the old one; Unregister removes the registration. State changes go to every page
 * of the storage key as "sw" events, and the registry persists after each settled change.
 */
use super::super::{Ctx, Reply, fail, ok};
use super::registry::{Registration, Version, scope_refusal};
use super::{Lifecycle, Runtime};
use crate::network::{Destination, WebRequest};
use crate::protocol::EngineResult;
use crate::runtime::resources::Redirects;
use serde_json::{Value, json};
use std::time::{Duration, Instant};
use url::Url;

/** How long fetching a service worker script may take. */
const SCRIPT_FETCH: Duration = Duration::from_secs(30);

/** A failed script fetch: the exception name and message the job rejects with. */
type Refusal = (&'static str, String);

impl Runtime {
    /** Fetch a service worker script: its text and Service-Worker-Allowed header. */
    fn service_worker_script(
        &mut self,
        ctx: &Ctx,
        script: &Url,
    ) -> EngineResult<Result<(String, Option<String>), Refusal>> {
        let request = WebRequest {
            url: script.to_string(),
            destination: Destination::Worker,
            top_level: Some(ctx.top_level.to_string()),
            ..WebRequest::default()
        };
        let deadline = Instant::now() + SCRIPT_FETCH;
        let acquired = self.acquire_with(
            request,
            deadline,
            Some(ctx.url.clone()),
            false,
            Redirects::Refuse,
            true,
        );
        let response = match acquired {
            Ok(acquired) => acquired.response,
            Err(error) if crate::runtime::worker_loading::fatal(&error) => return Err(error),
            Err(_) => {
                return Ok(Err((
                    "TypeError",
                    "An unknown error occurred when fetching the script.".into(),
                )));
            }
        };
        if response.status != 200 {
            return Ok(Err((
                "TypeError",
                format!(
                    "A bad HTTP response code ({}) was received when fetching the script.",
                    response.status
                ),
            )));
        }
        if !mars_core::http::is_javascript_mime_essence(&response.mime) {
            return Ok(Err((
                "SecurityError",
                format!(
                    "The script has an unsupported MIME type ('{}').",
                    response.mime
                ),
            )));
        }
        let allowed = response
            .headers
            .iter()
            .find(|(name, _)| name == "service-worker-allowed")
            .map(|(_, value)| value.clone());
        Ok(Ok((response.source, allowed)))
    }

    /** The Update algorithm for one registration: fetch, compare, and install a new version. */
    fn update_registration(
        &mut self,
        ctx: &Ctx,
        key: &str,
        scope: &str,
        script: &Url,
    ) -> EngineResult<Reply> {
        let prefix = format!(
            "Failed to register a ServiceWorker for scope ('{scope}') with script ('{script}'): "
        );
        let (source, allowed) = match self.service_worker_script(ctx, script)? {
            Ok(fetched) => fetched,
            Err((name, message)) => return fail(name, &format!("{prefix}{message}")),
        };
        let scope_url = Url::parse(scope)
            .map_err(|_| crate::protocol::EngineError::invalid("Invalid scope."))?;
        if let Some(message) = scope_refusal(script, &scope_url, allowed.as_deref()) {
            return fail("SecurityError", &format!("{prefix}{message}"));
        }
        let registry = &mut self.platform.workers.registry;
        let unchanged = registry
            .by_scope(key, scope)
            .and_then(|r| r.newest())
            .is_some_and(|newest| newest.script_url == script.as_str() && newest.source == source);
        if unchanged {
            return ok(registry
                .by_scope(key, scope)
                .map(|r| r.record())
                .unwrap_or(Value::Null));
        }
        let version = Version {
            id: registry.id(),
            script_url: script.to_string(),
            source,
            state: "installing".into(),
            ..Version::default()
        };
        let registration = registry
            .by_scope(key, scope)
            .expect("registration exists before update");
        if let Some(old) = registration.installing.replace(version.clone()) {
            self.platform.workers.live.running.remove(&old.id);
        }
        let (id, record) = (registration.id, registration.record());
        self.broadcast_service_worker(key, json!({"kind":"updatefound","registration":record}));
        self.start_service_worker(id, version.id)?;
        if let Some(event) =
            self.queue_service_worker_event(version.id, json!({"kind":"install"}), true)
        {
            self.platform
                .workers
                .live
                .lifecycle
                .insert(event, (version.id, Lifecycle::Install));
        }
        ok(record)
    }

    /** Settle lifecycle events whose outcomes arrived. */
    pub(in crate::runtime) fn service_worker_progress(&mut self) -> EngineResult<()> {
        let pending: Vec<(u64, (u64, Lifecycle))> = self
            .platform
            .workers
            .live
            .lifecycle
            .iter()
            .map(|(id, entry)| (*id, *entry))
            .collect();
        for (event, (version, stage)) in pending {
            let Some((outcome, _)) = self.take_outcome(version, event) else {
                if !self.platform.workers.live.running.contains_key(&version) {
                    self.platform.workers.live.lifecycle.remove(&event);
                    self.finish_install(version, &json!({"ok": false}))?;
                }
                continue;
            };
            self.platform.workers.live.lifecycle.remove(&event);
            match stage {
                Lifecycle::Install => self.finish_install(version, &outcome)?,
                Lifecycle::Activate => self.finish_activate(version),
            }
        }
        Ok(())
    }

    /** Install finished: the version waits (or becomes redundant on failure), then Try Activate. */
    fn finish_install(&mut self, version: u64, outcome: &Value) -> EngineResult<()> {
        let Some(registration) = self.platform.workers.registry.holding(version) else {
            return Ok(());
        };
        if registration
            .installing
            .as_ref()
            .is_none_or(|v| v.id != version)
        {
            return Ok(());
        }
        let mut installed = registration.installing.take().expect("installing version");
        let key = registration.key.clone();
        if outcome["ok"] != true {
            installed.state = "redundant".into();
            let empty = registration.waiting.is_none() && registration.active.is_none();
            let record = registration.record();
            if empty {
                let id = registration.id;
                self.platform
                    .workers
                    .registry
                    .registrations
                    .retain(|r| r.id != id);
            }
            self.stop_service_worker(version);
            self.broadcast_service_worker(
                &key,
                json!({"kind":"state","worker":installed.record(),"registration":record}),
            );
            return Ok(());
        }
        installed.state = "installed".into();
        installed.has_fetch = outcome["fetch"] == true;
        if let Some(old) = registration.waiting.replace(installed.clone()) {
            self.platform.workers.live.running.remove(&old.id);
        }
        let (id, record) = (registration.id, registration.record());
        self.broadcast_service_worker(
            &key,
            json!({"kind":"state","worker":installed.record(),"registration":record}),
        );
        self.save_service_workers();
        self.try_activate(id)
    }

    /** Activate the waiting version when nothing is controlled by the active one, or it skips waiting. */
    pub(in crate::runtime) fn try_activate(&mut self, registration: u64) -> EngineResult<()> {
        let Some(found) = self.platform.workers.registry.by_id(registration) else {
            return Ok(());
        };
        let Some(waiting) = found.waiting.as_ref() else {
            return Ok(());
        };
        let active = found.active.as_ref().map(|v| v.id);
        let controlled = active.is_some_and(|id| {
            self.platform
                .workers
                .live
                .controllers
                .values()
                .any(|v| *v == id)
        });
        if controlled && !waiting.skip_waiting {
            return Ok(());
        }
        self.activate(registration)
    }

    /** Make the waiting version active and dispatch its activate event. */
    fn activate(&mut self, registration: u64) -> EngineResult<()> {
        let Some(found) = self.platform.workers.registry.by_id(registration) else {
            return Ok(());
        };
        let mut next = found.waiting.take().expect("waiting version");
        next.state = "activating".into();
        let old = found.active.replace(next.clone());
        let (key, record) = (found.key.clone(), found.record());
        if let Some(mut old) = old {
            old.state = "redundant".into();
            self.stop_service_worker(old.id);
            self.broadcast_service_worker(
                &key,
                json!({"kind":"state","worker":old.record(),"registration":record}),
            );
            for controller in self
                .platform
                .workers
                .live
                .controllers
                .values_mut()
                .filter(|v| **v == old.id)
            {
                *controller = next.id;
            }
        }
        self.broadcast_service_worker(
            &key,
            json!({"kind":"state","worker":next.record(),"registration":record}),
        );
        self.start_service_worker(registration, next.id)?;
        if let Some(event) =
            self.queue_service_worker_event(next.id, json!({"kind":"activate"}), true)
        {
            self.platform
                .workers
                .live
                .lifecycle
                .insert(event, (next.id, Lifecycle::Activate));
        }
        self.save_service_workers();
        Ok(())
    }

    /** Activate finished: the version is activated (an activate failure does not undo it). */
    fn finish_activate(&mut self, version: u64) {
        let Some(registration) = self.platform.workers.registry.holding(version) else {
            return;
        };
        let Some(active) = registration.active.as_mut().filter(|v| v.id == version) else {
            return;
        };
        active.state = "activated".into();
        let (key, worker, record) = (
            registration.key.clone(),
            active.record(),
            registration.record(),
        );
        self.save_service_workers();
        self.broadcast_service_worker(
            &key,
            json!({"kind":"state","worker":worker,"registration":record}),
        );
    }
}

/** The script and scope URLs a register call names, or the SecurityError for another origin. */
fn urls(ctx: &Ctx, args: &Value) -> Result<(Url, Url), String> {
    let script = args["scriptURL"]
        .as_str()
        .and_then(|u| Url::parse(u).ok())
        .ok_or("Invalid script URL.")?;
    let scope = args["scope"]
        .as_str()
        .and_then(|u| Url::parse(u).ok())
        .ok_or("Invalid scope URL.")?;
    if script.origin() != ctx.url.origin() {
        return Err(format!(
            "Failed to register a ServiceWorker: The origin of the provided scriptURL ('{}') does not match the current origin ('{}').",
            script.origin().ascii_serialization(),
            ctx.origin
        ));
    }
    if scope.origin() != ctx.url.origin() {
        return Err(format!(
            "Failed to register a ServiceWorker: The origin of the provided scope ('{}') does not match the current origin ('{}').",
            scope.origin().ascii_serialization(),
            ctx.origin
        ));
    }
    Ok((script, scope))
}

/** navigator.serviceWorker.register: the Register job. */
pub(super) fn register(
    runtime: &mut Runtime,
    ctx: &Ctx,
    args: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    runtime.note_client(ctx);
    let (script, scope) = match urls(ctx, args) {
        Ok(urls) => urls,
        Err(message) => return fail("SecurityError", &message),
    };
    let key = ctx.storage_key();
    let script_type = if args["type"].as_str() == Some("module") {
        "module"
    } else {
        "classic"
    };
    let update_via_cache = args["updateViaCache"]
        .as_str()
        .unwrap_or("imports")
        .to_string();
    let registry = &mut runtime.platform.workers.registry;
    if let Some(existing) = registry.by_scope(&key, scope.as_str()) {
        existing.update_via_cache = update_via_cache.clone();
        existing.script_type = script_type.into();
        if existing
            .newest()
            .is_some_and(|newest| newest.script_url == script.as_str())
        {
            return ok(existing.record());
        }
    } else {
        let id = registry.id();
        registry.registrations.push(Registration {
            id,
            key: key.clone(),
            top_level: ctx.top_level.to_string(),
            scope: scope.to_string(),
            update_via_cache,
            script_type: script_type.into(),
            preload_header: "true".into(),
            ..Registration::default()
        });
    }
    let reply = runtime.update_registration(ctx, &key, scope.as_str(), &script)?;
    runtime.drop_empty_registration(&key, scope.as_str());
    Ok(reply)
}

/** ServiceWorkerRegistration.update: the Update job with the newest worker's script. */
pub(super) fn update(
    runtime: &mut Runtime,
    ctx: &Ctx,
    args: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    let key = ctx.storage_key();
    let scope = args["scope"].as_str().unwrap_or_default().to_string();
    let script = runtime
        .platform
        .workers
        .registry
        .by_scope(&key, &scope)
        .and_then(|r| r.newest())
        .map(|v| v.script_url.clone());
    let Some(script) = script.and_then(|s| Url::parse(&s).ok()) else {
        return fail(
            "InvalidStateError",
            "Failed to update a ServiceWorker: The registration is not active.",
        );
    };
    runtime.update_registration(ctx, &key, &scope, &script)
}

/** ServiceWorkerRegistration.unregister: remove the registration; its workers become redundant. */
pub(super) fn unregister(
    runtime: &mut Runtime,
    ctx: &Ctx,
    args: &Value,
    _: &[u8],
) -> EngineResult<Reply> {
    let key = ctx.storage_key();
    let scope = args["scope"].as_str().unwrap_or_default();
    let registry = &mut runtime.platform.workers.registry;
    let Some(index) = registry
        .registrations
        .iter()
        .position(|r| r.key == key && r.scope == scope)
    else {
        return ok(json!(false));
    };
    let removed = registry.registrations.remove(index);
    for version in removed.versions() {
        runtime.platform.workers.live.running.remove(&version.id);
        let mut gone = version.clone();
        gone.state = "redundant".into();
        runtime.broadcast_service_worker(&key, json!({"kind":"state","worker":gone.record()}));
    }
    runtime.save_service_workers();
    ok(json!(true))
}

impl Runtime {
    /** A registration whose first install could not even start holds nothing: remove it. */
    fn drop_empty_registration(&mut self, key: &str, scope: &str) {
        self.platform
            .workers
            .registry
            .registrations
            .retain(|r| !(r.key == key && r.scope == scope && r.newest().is_none()));
    }
}
