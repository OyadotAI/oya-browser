/**
 * How platform requests meet the page's script steps: a request is dispatched when the page's
 * request batch is performed; one a handler parks waits in the page's `Parked` list and is
 * dispatched again whenever platform state has changed since the page last looked, so lock
 * grants, events and service worker readiness reach it without the page polling.
 */
use super::{Ctx, Kind, Reply, handler, parse_body};
use crate::protocol::{EngineError, EngineResult};
use crate::runtime::page::Page;
use crate::runtime::supervisor::Runtime;
use crate::v9::{HostRequest, HostResponse};
use serde_json::json;
use url::Url;

/** Parked platform requests of one page and the platform version it last saw. */
#[derive(Clone, Default)]
pub(in crate::runtime) struct Parked {
    requests: Vec<HostRequest>,
    seen: u64,
}

impl Parked {
    /** Whether anything is parked. */
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }
    /** Whether a platform change since the page last looked may answer something parked. */
    pub fn wakes(&self, version: u64) -> bool {
        !self.requests.is_empty() && self.seen != version
    }
}

/** The answer a page sees for an operation no module owns. */
const UNKNOWN_OPERATION: &str = "Unknown platform operation.";

impl Runtime {
    /** The context a page's request came from. */
    pub(in crate::runtime) fn page_ctx(
        &self,
        page: &Page,
        url: &str,
        active: bool,
    ) -> EngineResult<Ctx> {
        let url = Url::parse(url)
            .or_else(|_| Url::parse(&page.url))
            .map_err(|_| EngineError::invalid("Invalid platform request URL."))?;
        let top_level = Url::parse(&page.top_level_url)
            .map_err(|_| EngineError::invalid("Invalid platform partition."))?;
        let frame = page.scripting.environment.embedder_origin.is_some();
        Ok(Ctx {
            identity: page.script_identity.clone(),
            kind: if frame { Kind::Frame } else { Kind::Window },
            origin: url.origin().ascii_serialization(),
            url,
            top_level,
            active,
            selected: self.active_tab_id == self.selected_tab_id(),
            tab: self.active_tab_id.clone(),
        })
    }

    /** Dispatch one request for `ctx`; None when the handler parked it. */
    pub(in crate::runtime) fn dispatch_platform(
        &mut self,
        ctx: &Ctx,
        request: &HostRequest,
        args: &serde_json::Value,
    ) -> EngineResult<Option<HostResponse>> {
        let reply = match handler(&request.method) {
            _ if opaque_storage(ctx, &request.method) => {
                super::fail("SecurityError", "Access is denied for this document.")?
            }
            Some(handler) => handler(self, ctx, args, &request.body_bytes)?,
            None => super::fail("TypeError", UNKNOWN_OPERATION)?,
        };
        Ok(match reply {
            Reply::Park => None,
            Reply::Answer(body, bytes) => Some(response(request.id, body, bytes)),
        })
    }

    /** A dedicated worker's platform request. Workers answer at once: an operation that would
    wait (a contended lock, an event subscription) is refused rather than parked.
    ponytail: park worker requests like a page's when workers need waiting operations. */
    pub(in crate::runtime) fn worker_platform_request(
        &mut self,
        identity: &str,
        policy: &crate::runtime::worker_loading::Policy,
        request: &HostRequest,
    ) -> EngineResult<HostResponse> {
        let (args, active) = parse_body(request.body.as_deref())?;
        let ctx = Ctx {
            identity: identity.into(),
            kind: Kind::Worker,
            origin: policy.base.origin().ascii_serialization(),
            url: policy.base.clone(),
            top_level: policy.partition.clone(),
            active,
            selected: false,
            tab: self.active_tab_id.clone(),
        };
        Ok(self.dispatch_platform(&ctx, request, &args)?.unwrap_or_else(|| {
            let error = json!({"error":{"name":"NotSupportedError","message":"Waiting operations are not available in dedicated workers in Mars yet."}});
            response(request.id, error, Vec::new())
        }))
    }

    /** A page's platform request: answered now, or parked on the page. */
    pub(in crate::runtime) fn platform_request(
        &mut self,
        page: &mut Page,
        request: HostRequest,
    ) -> EngineResult<Option<HostResponse>> {
        let (args, active) = parse_body(request.body.as_deref())?;
        let ctx = self.page_ctx(page, &request.url, active)?;
        let answer = self.dispatch_platform(&ctx, &request, &args)?;
        if answer.is_none() {
            page.scripting.parked.requests.push(request);
        }
        Ok(answer)
    }

    /** Retry the page's parked requests when platform state changed; returns the answers. */
    pub(in crate::runtime) fn answer_parked(
        &mut self,
        page: &mut Page,
    ) -> EngineResult<Vec<HostResponse>> {
        if page.scripting.parked.is_empty() || page.scripting.parked.seen == self.platform.version {
            return Ok(Vec::new());
        }
        page.scripting.parked.seen = self.platform.version;
        let mut answers = Vec::new();
        for request in std::mem::take(&mut page.scripting.parked.requests) {
            if let Some(answer) = self.platform_request(page, request)? {
                answers.push(answer);
            }
        }
        Ok(answers)
    }

    /** The settle step's resume batch from parked requests, or None when nothing was answered. */
    pub(in crate::runtime) fn parked_responses(
        &mut self,
        page: &mut Page,
    ) -> EngineResult<Option<Vec<HostResponse>>> {
        let answers = self.answer_parked(page)?;
        Ok((!answers.is_empty()).then_some(answers))
    }

    /** Poll platform state; whether the visible page or a service worker now has platform work. */
    pub(in crate::runtime) fn platform_wants_turn(&mut self) -> bool {
        self.poll_platform();
        let version = self.platform.version;
        self.service_workers_busy()
            || self
                .page
                .as_ref()
                .is_some_and(|page| page.scripting.loaded && page.scripting.parked.wakes(version))
    }

    /** Bring device, cookie and context state up to date, recording any change. */
    pub(in crate::runtime) fn poll_platform(&mut self) {
        let live: std::collections::BTreeSet<String> =
            self.residency.snapshot().into_iter().collect();
        let mut changed = self.platform.locks.retain(&live);
        self.platform.locks.process();
        self.platform.events.retain(&live);
        changed |= self.poll_cookies();
        changed |= self.poll_wake_locks(&live);
        changed |= self.poll_gamepads();
        changed |= self.poll_midi();
        if changed {
            self.platform.changed();
        }
    }
}

/** The HostResponse carrying one answer; an answer past the response bound becomes an error. */
fn response(id: u32, body: serde_json::Value, bytes: Vec<u8>) -> HostResponse {
    let text = body.to_string();
    if text.len().saturating_add(bytes.len()) > crate::constants::VM_RESPONSE_BYTES {
        let error = json!({"error":{"name":"QuotaExceededError","message":"The answer is larger than a script may receive."}});
        return HostResponse {
            id,
            status: 200,
            body: error.to_string(),
            ..HostResponse::default()
        };
    }
    HostResponse {
        id,
        status: 200,
        body: text,
        body_bytes: bytes,
        ..HostResponse::default()
    }
}

/// Operations keyed by the caller's storage key (origin and top-level site).
const STORAGE_KEYED: [&str; 3] = ["caches.", "locks.", "sw."];

/** Whether this is a storage-keyed operation from an opaque origin. Every sandboxed or data:
document serializes its origin as "null", so they would all share one key and read each other's
caches, locks and workers; browsers throw SecurityError there, as IndexedDB already does here. */
fn opaque_storage(ctx: &Ctx, op: &str) -> bool {
    ctx.origin == "null" && STORAGE_KEYED.iter().any(|prefix| op.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::{Ctx, Kind, opaque_storage};
    use url::Url;

    /** A context at `origin` in a frame of a.example. */
    fn ctx(origin: &str) -> Ctx {
        Ctx {
            identity: "frame-1".into(),
            kind: Kind::Frame,
            url: Url::parse("https://a.example/").unwrap(),
            origin: origin.into(),
            top_level: Url::parse("https://a.example/").unwrap(),
            active: false,
            selected: true,
            tab: "tab-1".into(),
        }
    }

    /** Opaque origins may not reach caches, locks or service workers; real origins may. */
    #[test]
    fn opaque_origins_are_refused_storage_keyed_operations() {
        for op in ["caches.open", "locks.request", "sw.register"] {
            assert!(opaque_storage(&ctx("null"), op), "{op}");
            assert!(!opaque_storage(&ctx("https://a.example"), op), "{op}");
        }
        assert!(!opaque_storage(&ctx("null"), "clipboard.read"));
    }
}
