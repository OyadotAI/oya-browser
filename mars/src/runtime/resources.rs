/** Governed one-hop resources keep effect admission, cookies and deadlines in the coordinator. */
use super::supervisor::Runtime;
use crate::{
    constants::REDIRECTS,
    network::{Destination, Method, Preflight, WebRequest, WebResponse, cors},
    protocol::{EngineError, EngineResult},
};
use serde_json::json;
use std::time::{Instant, SystemTime};
use url::Url;

mod in_flight;
mod request_diagnostics;
pub(super) use in_flight::{InFlight, Progress};
use request_diagnostics::{FailureStage, RequestDiagnostic};

/** What a redirect response does to an acquisition. */
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Redirects {
    /** Follow it, revalidating the destination (navigations, images, sheets). */
    Follow,
    /** Refuse it (classic scripts must not redirect). */
    Refuse,
    /** Hand it back so the caller re-decides credentials and origin for the next hop. */
    Return,
}
/** Who asked for a request: the page that initiated it, and whether it is a top-level navigation. */
struct Requester {
    /** The initiating page's URL, or `None` for a typed URL with no initiator. */
    initiator: Option<Url>,
    /** Whether the request navigates a top-level document rather than loading a subresource. */
    top_level: bool,
}

/** Response metadata keeps final method for history replay policy. */
pub(super) struct Acquired {
    pub response: WebResponse,
    pub method: Method,
    pub diagnostics: Vec<String>,
}
impl Runtime {
    /** One navigation deadline covers all redirects and required stylesheets. */
    pub(super) fn acquire(
        &mut self,
        request: WebRequest,
        deadline: Instant,
        initiator: Option<Url>,
        top_level: bool,
        allow_redirects: bool,
    ) -> EngineResult<Acquired> {
        let redirects = if allow_redirects {
            Redirects::Follow
        } else {
            Redirects::Refuse
        };
        self.acquire_with(request, deadline, initiator, top_level, redirects, true)
    }
    /** `acquire` with an explicit redirect policy, where `credentials: false` sends no cookies
    and stores none the response sets (a page's script request decides credentials hop by hop
    itself). */
    pub(super) fn acquire_with(
        &mut self,
        request: WebRequest,
        deadline: Instant,
        initiator: Option<Url>,
        top_level: bool,
        redirects_policy: Redirects,
        credentials: bool,
    ) -> EngineResult<Acquired> {
        self.acquire_diagnosed(
            request,
            deadline,
            Requester {
                initiator,
                top_level,
            },
            redirects_policy,
            credentials,
            None,
        )
    }

    /** Preserve the original native failure before unsafe-submission recovery hides its cause. */
    fn acquire_diagnosed(
        &mut self,
        mut request: WebRequest,
        deadline: Instant,
        requester: Requester,
        redirects_policy: Redirects,
        credentials: bool,
        diagnostic: Option<(RequestDiagnostic, FailureStage)>,
    ) -> EngineResult<Acquired> {
        let Requester {
            initiator,
            top_level,
        } = requester;
        let started = Instant::now();
        let mut admitted_unsafe = false;
        let mut diagnostics = Vec::new();
        // Who caused the request drives Sec-Fetch-Site, Origin and Referer; typed URLs have none.
        if request.initiator.is_none() {
            request.initiator = initiator.as_ref().map(Url::to_string);
        }
        let result = (|| {
            let grants = self
                .web_origins
                .clone()
                .ok_or_else(|| EngineError::unsupported("Forms require desktop networking."))?;
            for redirects in 0..=REDIRECTS {
                // A DevTools client that paused this navigation decides its first hop.
                let intercepted = (redirects == 0 && top_level)
                    .then(|| super::devtools::intercept(&mut request))
                    .flatten();
                let url = crate::network::policy::web_url(&request.url)?;
                self.shape_hop(
                    &mut request,
                    &url,
                    initiator.as_ref(),
                    top_level,
                    credentials,
                );
                let remaining = deadline
                    .saturating_duration_since(Instant::now())
                    .min(crate::constants::WEB_REQUEST_CEILING);
                if remaining.is_zero() {
                    return Err(EngineError::navigation_timeout());
                }
                let response = if let Some(answer) = intercepted {
                    self.publish(|_| ())?;
                    answer?
                } else if let Some(response) = self.take_prefetched(&request) {
                    // The batch already sent this exact request; the authority check still runs.
                    self.publish(|_| ())?;
                    response?
                } else {
                    let token=self.call_deadline("network","network.prepare",json!({"request":request,"fixture_origins":grants,"remaining_ms":remaining.as_millis() as u64}), deadline)?;
                    self.publish(|runtime| {
                        if !request.method.is_safe()
                            && let Some(page) = &mut runtime.page
                        {
                            page.revision = page.revision.saturating_add(1);
                        }
                    })?;
                    admitted_unsafe |= !request.method.is_safe();
                    let transmit = json!({"token":token["token"]});
                    let response: WebResponse =
                        self.call_typed("network", "network.transmit", &transmit, Some(deadline))?;
                    if !request.method.is_safe() {
                        self.loader.forget(&[cache_url(&url)]);
                    }
                    response
                };
                // Fetch stores a response's cookies only when the request included credentials.
                let stored: &[String] = if credentials {
                    &response.set_cookies
                } else {
                    &[]
                };
                let reports = self.commit_cookies(&url, stored)?;
                for report in reports {
                    if diagnostics.len() < crate::constants::DIAGNOSTICS
                        && !diagnostics.contains(&report)
                    {
                        diagnostics.push(report);
                    }
                }
                if ![301, 302, 303, 307, 308].contains(&response.status) {
                    return Ok(Acquired {
                        response,
                        method: request.method,
                        diagnostics,
                    });
                }
                match redirects_policy {
                    Redirects::Follow => {}
                    Redirects::Refuse => {
                        return Err(EngineError::unsupported(
                            "A required script resource redirected.",
                        ));
                    }
                    Redirects::Return => {
                        return Ok(Acquired {
                            response,
                            method: request.method,
                            diagnostics,
                        });
                    }
                }
                if redirects == REDIRECTS {
                    return Err(EngineError::unsupported("Redirect limit exceeded."));
                }
                let next = crate::network::redirect_url(
                    &url,
                    response
                        .location
                        .as_deref()
                        .ok_or_else(|| EngineError::unsupported("Redirect has no Location."))?,
                    &grants,
                )?;
                if redirects_to_get(&request.method, response.status) {
                    request.method = Method::Get;
                    request.body.clear();
                } else if !request.method.is_safe() && next.origin() != url.origin() {
                    return Err(EngineError::unsupported(
                        "Cross-origin POST-preserving redirects are not supported.",
                    ));
                }
                request.url = next.into();
                request.cookie.clear();
            }
            Err(EngineError::unsupported("Redirect limit exceeded."))
        })();
        result.map_err(|error| {
            if let Some((diagnostic, stage)) = diagnostic {
                diagnostic.error(started, stage, &error);
            }
            if admitted_unsafe { uncertain() } else { error }
        })
    }
}

/// Requests one `network.fetch_batch` call carries: two rounds of Relay's batch workers, so a
/// batch of the largest resources still fits one wire frame.
pub(super) const FETCH_BATCH: usize = 2 * crate::network::network::constants::BATCH_WORKERS;

/** The outcome of one batched request as the network service reports it. */
#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
enum BatchOutcome {
    /** The first hop's response, a redirect included. */
    Response(WebResponse),
    /** Why the request failed; the later acquire sees exactly this. */
    Error(EngineError),
}

impl Runtime {
    /** An unsafe request a loader worker sent changed this URL's resource: every other network
    process drops its cached copies, or a later GET there would read the stale one. */
    fn forget_cached(&mut self, url: &Url) -> EngineResult<()> {
        let urls = [cache_url(url)];
        self.loader.forget(&urls);
        self.call("network", "network.cache_forget", json!({"urls": urls}))?;
        Ok(())
    }

    /** Names the cache partition, initiator-derived cookies and the like for one hop, so a
    prefetched request and the later acquire of the same resource are byte-identical. */
    fn shape_hop(
        &mut self,
        request: &mut WebRequest,
        url: &Url,
        initiator: Option<&Url>,
        top_level: bool,
        credentials: bool,
    ) {
        // The HTTP cache is partitioned by the top-level site, so every request names it.
        request.languages = self.languages.clone();
        request.top_level = if top_level {
            Some(request.url.clone())
        } else {
            request
                .top_level
                .clone()
                .or_else(|| self.frame_partition.clone())
                .or_else(|| initiator.map(Url::to_string))
        };
        let partition = request
            .top_level
            .as_deref()
            .and_then(|url| Url::parse(url).ok());
        request.cookie = if credentials {
            self.cookies.header(
                url,
                if top_level {
                    initiator
                } else {
                    partition.as_ref().or(initiator)
                },
                top_level && request.method == Method::Get,
                SystemTime::now(),
            )
        } else {
            String::new()
        };
    }

    /** A prefetched answer to exactly this request (cookies included), used at most once. */
    fn take_prefetched(&mut self, request: &WebRequest) -> Option<EngineResult<WebResponse>> {
        if self.prefetched.is_empty() || request.method != Method::Get {
            return None;
        }
        self.prefetched.remove(&prefetch_key(request))
    }

    /**
     * Sends independent subresource GETs (`(request, initiator)`, shaped as `acquire` would with
     * credentials and no top-level navigation) concurrently, so a page's sheets, scripts or
     * images cost about one round trip instead of one each. Answers wait in `prefetched` until
     * `acquire` asks for the identical request; a request whose cookies changed meanwhile, or
     * that nobody asks for, is simply fetched or dropped as before. Only authority and service
     * failures stop the caller; any other batch failure leaves the sequential path to do the work.
     */
    pub(super) fn prefetch(
        &mut self,
        requests: Vec<(WebRequest, Url)>,
        deadline: Instant,
    ) -> EngineResult<()> {
        let Some(grants) = self.web_origins.clone() else {
            return Ok(());
        };
        let mut shaped: Vec<(String, WebRequest)> = Vec::new();
        for (mut request, initiator) in requests {
            let Ok(url) = crate::network::policy::web_url(&request.url) else {
                continue;
            };
            if request.method != Method::Get {
                continue;
            }
            if request.initiator.is_none() {
                request.initiator = Some(initiator.to_string());
            }
            self.shape_hop(&mut request, &url, Some(&initiator), false, true);
            let key = prefetch_key(&request);
            if !self.prefetched.contains_key(&key) && !shaped.iter().any(|(k, _)| *k == key) {
                shaped.push((key, request));
            }
        }
        // A lone request gains nothing from a batch.
        if shaped.len() < 2 {
            return Ok(());
        }
        for chunk in shaped.chunks(FETCH_BATCH) {
            let remaining = deadline
                .saturating_duration_since(Instant::now())
                .min(crate::constants::WEB_REQUEST_CEILING);
            if remaining.is_zero() {
                return Ok(());
            }
            self.publish(|_| ())?;
            let requests: Vec<&WebRequest> = chunk.iter().map(|(_, request)| request).collect();
            let started = Instant::now();
            let batch = json!({"requests":requests,"fixture_origins":grants,"remaining_ms":remaining.as_millis() as u64});
            let outcomes: Vec<BatchOutcome> =
                match self.call_typed("network", "network.fetch_batch", &batch, Some(deadline)) {
                    Ok(outcomes) => outcomes,
                    Err(error) if stops(&error) => return Err(error),
                    Err(_) => return Ok(()),
                };
            super::supervisor::perf::record("page.prefetch", started, || chunk.len().to_string());
            for ((key, _), outcome) in chunk.iter().zip(outcomes) {
                match outcome {
                    BatchOutcome::Response(response) => {
                        self.prefetched.insert(key.clone(), Ok(response));
                    }
                    // A service-level failure is the sequential path's to discover and recover.
                    BatchOutcome::Error(error) if error.service.is_none() => {
                        self.prefetched.insert(key.clone(), Err(error));
                    }
                    BatchOutcome::Error(_) => {}
                }
            }
        }
        Ok(())
    }
}

/** Every field of the request, so only an identical request reuses a prefetched answer. */
fn prefetch_key(request: &WebRequest) -> String {
    serde_json::to_string(request).unwrap_or_default()
}

/** Authority, ownership and engine-service failures stop a load; everything else degrades. */
fn stops(error: &EngineError) -> bool {
    error.service.is_some()
        || ["permission_denied", "control_denied"].contains(&error.code.as_str())
}

impl Runtime {
    /// Durable replacement precedes publication; stale authority rolls storage back.
    fn commit_cookies(&mut self, url: &Url, fields: &[String]) -> EngineResult<Vec<String>> {
        if fields.is_empty() {
            return Ok(Vec::new());
        }
        self.commit_jar(|jar| jar.receive(url, fields, SystemTime::now()))
    }

    /// `document.cookie` writes go through the jar's script setter, which refuses HttpOnly
    /// cookies and any write that would replace one, then persist exactly like server cookies.
    pub(super) fn commit_document_cookies(
        &mut self,
        url: &str,
        fields: &[String],
    ) -> EngineResult<()> {
        if fields.is_empty() {
            return Ok(());
        }
        let url = Url::parse(url).map_err(|_| EngineError::invalid("Invalid page URL."))?;
        self.commit_jar(|jar| {
            for field in fields {
                let _ = jar.set_document_cookie(&url, field, SystemTime::now());
            }
            Vec::new()
        })
        .map(|_| ())
    }

    /** Change a candidate jar, persist it to the profile, then publish it under authority. */
    pub(super) fn commit_jar(
        &mut self,
        change: impl FnOnce(&mut crate::network::CookieJar) -> Vec<String>,
    ) -> EngineResult<Vec<String>> {
        let previous = self
            .cookies
            .profile_record()
            .map_err(|_| crate::profile::unavailable())?;
        let mut candidate = self.cookies.clone();
        let reports = change(&mut candidate);
        let next = candidate
            .profile_record()
            .map_err(|_| crate::profile::unavailable())?;
        if previous == next {
            return Ok(reports);
        }
        if let Some(store) = self.profile_store.as_ref()
            && store.replace(&next).is_err()
        {
            let error = if self.persist_recovery_marker() {
                crate::profile::ProfileError::CommitUncertain
            } else {
                crate::profile::ProfileError::RecoveryUnpersisted
            };
            self.profile_error = Some(error);
            return Err(error.engine_error());
        }
        match self.publish(|runtime| runtime.cookies = candidate) {
            Ok(()) => Ok(reports),
            Err(error) => {
                if self.rollback_cookies(&previous) {
                    return Err(error);
                }
                let error = if self.persist_recovery_marker() {
                    crate::profile::ProfileError::CommitUncertain
                } else {
                    crate::profile::ProfileError::RecoveryUnpersisted
                };
                self.profile_error = Some(error);
                Err(error.engine_error())
            }
        }
    }

    /// A failed rollback replaces stale-authority errors with actionable profile recovery.
    fn rollback_cookies(&self, previous: &[u8]) -> bool {
        self.profile_store
            .as_ref()
            .is_none_or(|store| store.replace(previous).is_ok())
    }

    /// Best-effort marker preserves fail-closed recovery across a later process restart.
    fn persist_recovery_marker(&self) -> bool {
        self.profile_store.as_ref().is_some_and(|store| {
            store
                .replace(&crate::network::CookieJar::recovery_record())
                .is_ok()
        })
    }
}
/** A dispatched POST (or any other unsafe method) is never offered a generic retry, including
later rendering failure. */
pub(super) fn uncertain() -> EngineError {
    EngineError::new(
        "submission_uncertain",
        "We couldn’t confirm the submission result. The server may have received it. Check the result before submitting again.",
        "Dismiss this message; no submission was replayed.",
    )
}

/// Redirect statuses a script request follows hop by hop.
const REDIRECT_STATUSES: [u16; 5] = [301, 302, 303, 307, 308];

/// Fetch's request-body header names, dropped when a redirect turns a request into a GET.
const BODY_HEADERS: [&str; 4] = [
    "content-encoding",
    "content-language",
    "content-location",
    "content-type",
];

/// A page script's credentials mode.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Credentials {
    /// Never send or store cookies.
    Omit,
    /// Cookies only while the request has not left the page's origin.
    SameOrigin,
    /// Cookies on every hop, cross-origin too, and then the CORS check needs an exact origin
    /// and Access-Control-Allow-Credentials.
    Include,
}

impl Credentials {
    /// The credentials mode a page named, or None for one the Fetch standard does not have.
    pub(super) fn parse(text: &str) -> Option<Self> {
        match text {
            "omit" => Some(Self::Omit),
            "" | "same-origin" => Some(Self::SameOrigin),
            "include" => Some(Self::Include),
            _ => None,
        }
    }
}

/// One script request, as parsed from what the page asked for.
#[derive(Clone)]
pub(super) struct ScriptRequest {
    /// Native fetch/XHR identity correlates opt-in diagnostics without storing page addresses.
    pub request_id: Option<u32>,
    /// Fetch body readers consume raw bytes; modules and legacy XHR retain their decoding rules.
    pub raw_response: bool,
    /// The current hop's URL.
    pub url: Url,
    /// Any method a script may send; only GET and HEAD are safe.
    pub method: Method,
    /// The text body (empty for GET and HEAD).
    pub body: String,
    /// Page-set headers, forbidden names already dropped.
    pub headers: Vec<(String, String)>,
    /// cors, no-cors or same-origin.
    pub mode: cors::Mode,
    /// omit, same-origin or include.
    pub credentials: Credentials,
    /// Worker fetches retain their creator's partition and the worker response's connect policy.
    pub worker_policy: Option<super::worker_loading::Policy>,
}

/// What a script request delivers to the page.
pub(super) enum ScriptAnswer {
    /// A response the page may read, with only the headers it may see.
    Readable(Box<WebResponse>, Vec<(String, String)>),
    /// A cross-origin no-cors response: status 0, no headers, no body.
    Opaque,
    /// A network error; the page sees only TypeError("Failed to fetch").
    Failed,
}

impl Runtime {
    /**
     * A page's fetch or XMLHttpRequest as the Fetch standard runs it, one hop at a time under
     * the same egress policy: same-origin mode stops at another origin, no-cors turns
     * cross-origin answers opaque, and CORS mode preflights what is not simple, sends Origin
     * (`null` once a redirect taints it), and lets the page read a cross-origin response,
     * redirects included, only when the server's Access-Control headers pass the CORS check.
     * Every refusal is the same bare network error.
     */
    pub(super) fn fetch_for_script(
        &mut self,
        base: &Url,
        mut request: ScriptRequest,
        preflights: &mut cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<ScriptAnswer> {
        let include = request.credentials == Credentials::Include;
        let mut flow = cors::Flow::new(base.origin(), request.mode);
        for redirects in 0..=REDIRECTS {
            let diagnostic = RequestDiagnostic::new(base, &request);
            if request.worker_policy.as_ref().is_some_and(|policy| {
                !super::csp::allows_connect(&policy.csp, &policy.base, &request.url)
            }) {
                diagnostic.refused(FailureStage::Policy);
                return Ok(ScriptAnswer::Failed);
            }
            if !flow.enter(&request.url) {
                diagnostic.refused(FailureStage::Mode);
                return Ok(ScriptAnswer::Failed);
            }
            if !self.preflighted(base, &request, &flow, preflights, until)? {
                diagnostic.refused(FailureStage::Preflight);
                return Ok(ScriptAnswer::Failed);
            }
            let first = redirects == 0;
            let Some(response) = self.script_hop(base, &request, &flow, first, until)? else {
                return Ok(ScriptAnswer::Failed);
            };
            let tainting = flow.tainting();
            if tainting == cors::Tainting::Cors
                && !cors::allows(&response.headers, &flow.serialized(), include)
            {
                diagnostic.refused(FailureStage::Cors);
                return Ok(ScriptAnswer::Failed);
            }
            if !REDIRECT_STATUSES.contains(&response.status) {
                return Ok(answer(response, tainting, include));
            }
            let Some(next) = self.next_hop(&request.url, &response) else {
                diagnostic.refused(FailureStage::Redirect);
                return Ok(ScriptAnswer::Failed);
            };
            flow.redirect(&request.url, &next);
            if redirects_to_get(&request.method, response.status) {
                request.method = Method::Get;
                request.body.clear();
                request
                    .headers
                    .retain(|(name, _)| !BODY_HEADERS.contains(&name.as_str()));
            }
            request.url = next;
        }
        RequestDiagnostic::new(base, &request).refused(FailureStage::RedirectLimit);
        Ok(ScriptAnswer::Failed)
    }

    /** Whether a CORS hop that is not simple has a preflight allowing it, cached or fresh. */
    fn preflighted(
        &mut self,
        base: &Url,
        request: &ScriptRequest,
        flow: &cors::Flow,
        preflights: &mut cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<bool> {
        let method = request.method.name();
        if flow.tainting() != cors::Tainting::Cors
            || !cors::needs_preflight(method, &request.headers)
        {
            return Ok(true);
        }
        let include = request.credentials == Credentials::Include;
        let origin = flow.serialized();
        let names = cors::unsafe_header_names(&request.headers);
        let now = Instant::now();
        if preflights.covers(&origin, &request.url, include, method, &names, now) {
            return Ok(true);
        }
        let web = WebRequest {
            destination: Destination::Empty,
            top_level: request
                .worker_policy
                .as_ref()
                .map(|policy| policy.partition.to_string()),
            origin: Some(origin.clone()),
            mode: Some(cors::Mode::Cors),
            preflight: Some(Preflight {
                method: method.into(),
                headers: names.join(","),
            }),
            ..WebRequest::get(request.url.as_str())
        };
        let diagnostic = RequestDiagnostic::new(base, request);
        let Some(response) =
            self.script_send(base, web, false, until, diagnostic, FailureStage::Preflight)?
        else {
            return Ok(false);
        };
        let Some(allowance) = cors::preflight(response.status, &response.headers, &origin, include)
        else {
            return Ok(false);
        };
        let covered = allowance.covers(method, &names, include);
        preflights.store(&origin, &request.url, include, allowance, now);
        Ok(covered)
    }

    /** One hop of the real request, with the cookies, Origin and mode its state calls for. */
    fn script_hop(
        &mut self,
        base: &Url,
        request: &ScriptRequest,
        flow: &cors::Flow,
        first: bool,
        until: Instant,
    ) -> EngineResult<Option<WebResponse>> {
        let (mut web, cookies) = hop_request(request, flow);
        // A DevTools client that paused this page request decides its first hop, as it does
        // for navigations; a hop after a redirect is never paused.
        if first && let Some(answer) = super::devtools::intercept(&mut web) {
            return answer.map(Some);
        }
        self.script_send(
            base,
            web,
            cookies,
            until,
            RequestDiagnostic::new(base, request),
            FailureStage::Acquire,
        )
    }

    /** Send one script hop; an egress refusal or network failure is None, never a detail the
    page could read. Authority and service failures still stop the caller. */
    fn script_send(
        &mut self,
        base: &Url,
        web: WebRequest,
        cookies: bool,
        until: Instant,
        diagnostic: RequestDiagnostic,
        stage: FailureStage,
    ) -> EngineResult<Option<WebResponse>> {
        let started = Instant::now();
        match self.acquire_diagnosed(
            web,
            until,
            Requester {
                initiator: Some(base.clone()),
                top_level: false,
            },
            Redirects::Return,
            cookies,
            Some((diagnostic, stage)),
        ) {
            Ok(acquired) => {
                diagnostic.response(started, stage, acquired.response.status);
                Ok(Some(acquired.response))
            }
            Err(error) => {
                if error.service.is_some() || error.code == "control_denied" {
                    Err(error)
                } else {
                    Ok(None)
                }
            }
        }
    }

    /** The redirect's destination under the same egress policy, or None to fail the request. */
    fn next_hop(&self, url: &Url, response: &WebResponse) -> Option<Url> {
        let grants = self.web_origins.clone().unwrap_or_default();
        let location = response.location.as_deref()?;
        crate::network::redirect_url(url, location, &grants).ok()
    }
}

/** A URL as Relay's cache keys it: absolute, without its fragment. */
fn cache_url(url: &Url) -> String {
    let mut url = url.clone();
    url.set_fragment(None);
    url.into()
}

/** One hop's request as its CORS state calls for, and whether it carries cookies: credentials
by mode and tainting, Origin on CORS and unsafe requests. Shared by the blocking and the
loader paths so both send byte-identical hops. */
fn hop_request(request: &ScriptRequest, flow: &cors::Flow) -> (WebRequest, bool) {
    let tainting = flow.tainting();
    let cookies = match request.credentials {
        Credentials::Include => true,
        Credentials::SameOrigin => tainting == cors::Tainting::Basic,
        Credentials::Omit => false,
    };
    let origin =
        (tainting == cors::Tainting::Cors || !request.method.is_safe()).then(|| flow.serialized());
    let web = WebRequest {
        method: request.method.clone(),
        raw_response: request.raw_response,
        top_level: request
            .worker_policy
            .as_ref()
            .map(|policy| policy.partition.to_string()),
        body: request.body.clone(),
        destination: Destination::Empty,
        headers: request.headers.clone(),
        origin,
        mode: Some(request.mode),
        ..WebRequest::get(request.url.as_str())
    };
    (web, cookies)
}

/** The final response as the page may see it, by tainting. */
fn answer(response: WebResponse, tainting: cors::Tainting, include: bool) -> ScriptAnswer {
    if tainting == cors::Tainting::Opaque {
        return ScriptAnswer::Opaque;
    }
    let headers = cors::exposed(&response.headers, tainting, include);
    ScriptAnswer::Readable(Box::new(response), headers)
}

/** Fetch's redirect rewrite: a 301 or 302 turns a POST into a GET, and a 303 turns every method
but GET and HEAD into one. Any other unsafe method keeps its method and body. */
fn redirects_to_get(method: &Method, status: u16) -> bool {
    (status == 303 && !method.is_safe())
        || ([301, 302].contains(&status) && *method == Method::Post)
}
