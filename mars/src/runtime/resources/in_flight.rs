/**
 * A page's fetch or XMLHttpRequest whose hops run on the loader (runtime/loader.rs) while the
 * page keeps running. The rules are `fetch_for_script`'s, split at the network: what comes before
 * a hop (mode, preflight, cookies, authority) runs here when the hop starts, and what comes after
 * it (stored cookies, the CORS check, redirects) runs when its result is drained. The blocking
 * path froze the tab, and every other tab, for as long as a slow link took to answer.
 */
use super::request_diagnostics::{FailureStage, RequestDiagnostic};
use super::{
    BODY_HEADERS, Credentials, REDIRECT_STATUSES, ScriptAnswer, ScriptRequest, answer, hop_request,
    redirects_to_get,
};
use crate::constants::{REDIRECTS, WEB_REQUEST_CEILING};
use crate::network::{WebRequest, WebResponse, cors};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use std::time::Instant;
use url::Url;

/// A script request between hops: what it asks for now, where its CORS flow stands, and the
/// loader job carrying its current hop. Kept in the page's scripting state, so it is dropped
/// with the page a navigation replaces, as a browser cancels a document's fetches.
#[derive(Clone)]
pub(in crate::runtime) struct InFlight {
    /// The loader job carrying the current hop.
    pub job: u64,
    /// The page's identity for the request; its answer carries it back.
    pub id: u32,
    /// Beacons expect no answer, but their cookies and redirects still count.
    pub beacon: bool,
    /// The page URL the request was made from.
    base: Url,
    /// The request as it stands after the redirects so far.
    request: ScriptRequest,
    /// Its tainting and origin across redirects.
    flow: cors::Flow,
    /// Redirects followed so far.
    redirects: usize,
    /// Whether the current hop sent cookies, so it stores the ones its response sets.
    cookies: bool,
    /// When the current hop started, for the opt-in request diagnostics.
    started: Instant,
}

/// Where a request is after it starts or after a hop comes back.
pub(in crate::runtime) enum Progress {
    /// A hop is on the loader; keep this until its result is drained.
    Sent(Box<InFlight>),
    /// The request is over, with what the page sees.
    Answered(ScriptAnswer),
}

/// What starting a hop did.
enum Hop {
    /// It went to the loader as this job.
    Sent(u64),
    /// It was answered without the network (a DevTools override or a prefetched response).
    Now(EngineResult<WebResponse>),
    /// Policy refused it; the page sees a network error.
    Refused,
}

impl Runtime {
    /** Start a page's fetch: its first hop goes to the loader, or the request ends at once
    when its mode, preflight or egress policy refuses it. */
    pub(in crate::runtime) fn start_script_fetch(
        &mut self,
        base: &Url,
        request: ScriptRequest,
        (id, beacon): (u32, bool),
        preflights: &mut cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<Progress> {
        let state = InFlight {
            job: 0,
            id,
            beacon,
            base: base.clone(),
            flow: cors::Flow::new(base.origin(), request.mode),
            request,
            redirects: 0,
            cookies: false,
            started: Instant::now(),
        };
        self.drive(state, None, preflights, until)
    }

    /** Continue a request whose hop came back: answer the page, or send the redirect's hop. */
    pub(in crate::runtime) fn resume_script_fetch(
        &mut self,
        state: InFlight,
        result: EngineResult<WebResponse>,
        preflights: &mut cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<Progress> {
        self.drive(state, Some(result), preflights, until)
    }

    /** Alternate between handling a result and starting the next hop until one is on the
    loader or the request is over. */
    fn drive(
        &mut self,
        mut state: InFlight,
        mut result: Option<EngineResult<WebResponse>>,
        preflights: &mut cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<Progress> {
        loop {
            if let Some(result) = result.take()
                && let Some(answer) = self.after_hop(&mut state, result)?
            {
                return Ok(Progress::Answered(answer));
            }
            result = match self.start_hop(&mut state, preflights, until)? {
                Hop::Sent(job) => {
                    state.job = job;
                    return Ok(Progress::Sent(Box::new(state)));
                }
                Hop::Now(response) => Some(response),
                Hop::Refused => return Ok(Progress::Answered(ScriptAnswer::Failed)),
            };
        }
    }

    /** The checks before a hop, then the hop itself. */
    fn start_hop(
        &mut self,
        state: &mut InFlight,
        preflights: &mut cors::PreflightCache,
        until: Instant,
    ) -> EngineResult<Hop> {
        let diagnostic = RequestDiagnostic::new(&state.base, &state.request);
        if !state.flow.enter(&state.request.url) {
            diagnostic.refused(FailureStage::Mode);
            return Ok(Hop::Refused);
        }
        // ponytail: a preflight the cache does not cover still blocks; route it through the
        // loader too if cross-origin APIs on slow links show up as stalls.
        if !self.preflighted(&state.base, &state.request, &state.flow, preflights, until)? {
            diagnostic.refused(FailureStage::Preflight);
            return Ok(Hop::Refused);
        }
        let (mut web, cookies) = hop_request(&state.request, &state.flow);
        state.started = Instant::now();
        // A DevTools client that paused this request decides its first hop, as for navigations.
        if state.redirects == 0
            && let Some(answer) = crate::runtime::devtools::intercept(&mut web)
        {
            state.cookies = false;
            return Ok(Hop::Now(answer));
        }
        state.cookies = cookies;
        self.send_hop(&state.base, web, cookies, until)
    }

    /** `acquire`'s first half for one hop: shape it (cache partition, cookies), check authority,
    and hand it to the loader. */
    fn send_hop(
        &mut self,
        base: &Url,
        mut web: WebRequest,
        cookies: bool,
        until: Instant,
    ) -> EngineResult<Hop> {
        web.initiator.get_or_insert_with(|| base.to_string());
        let Ok(url) = crate::network::policy::web_url(&web.url) else {
            return Ok(Hop::Refused);
        };
        self.shape_hop(&mut web, &url, Some(base), false, cookies);
        if let Some(response) = self.take_prefetched(&web) {
            self.publish(|_| ())?;
            return Ok(Hop::Now(response));
        }
        let remaining = until
            .saturating_duration_since(Instant::now())
            .min(WEB_REQUEST_CEILING);
        let Some(grants) = self.web_origins.clone().filter(|_| !remaining.is_zero()) else {
            return Ok(Hop::Refused);
        };
        let unsafe_method = !web.method.is_safe();
        self.publish(|runtime| {
            if unsafe_method && let Some(page) = &mut runtime.page {
                page.revision = page.revision.saturating_add(1);
            }
        })?;
        let millis = u64::try_from(remaining.as_millis()).unwrap_or(u64::MAX);
        Ok(Hop::Sent(self.loader.submit(web, grants, millis)))
    }

    /** What a hop's result means: the page's answer, or None after following a redirect. */
    fn after_hop(
        &mut self,
        state: &mut InFlight,
        result: EngineResult<WebResponse>,
    ) -> EngineResult<Option<ScriptAnswer>> {
        let diagnostic = RequestDiagnostic::new(&state.base, &state.request);
        let response = match result {
            Ok(response) => response,
            Err(error) => {
                diagnostic.error(state.started, FailureStage::Acquire, &error);
                return Ok(Some(ScriptAnswer::Failed));
            }
        };
        diagnostic.response(state.started, FailureStage::Acquire, response.status);
        // Fetch stores a response's cookies only when the request included credentials.
        let stored: &[String] = if state.cookies {
            &response.set_cookies
        } else {
            &[]
        };
        self.commit_cookies(&state.request.url, stored)?;
        if !state.request.method.is_safe() {
            self.forget_cached(&state.request.url)?;
        }
        Ok(self.checked(state, response, diagnostic))
    }

    /** The CORS check and redirect handling for one response, as `fetch_for_script` does. */
    fn checked(
        &self,
        state: &mut InFlight,
        response: WebResponse,
        diagnostic: RequestDiagnostic,
    ) -> Option<ScriptAnswer> {
        let include = state.request.credentials == Credentials::Include;
        let tainting = state.flow.tainting();
        if tainting == cors::Tainting::Cors
            && !cors::allows(&response.headers, &state.flow.serialized(), include)
        {
            diagnostic.refused(FailureStage::Cors);
            return Some(ScriptAnswer::Failed);
        }
        if !REDIRECT_STATUSES.contains(&response.status) {
            return Some(answer(response, tainting, include));
        }
        if state.redirects == REDIRECTS {
            diagnostic.refused(FailureStage::RedirectLimit);
            return Some(ScriptAnswer::Failed);
        }
        let Some(next) = self.next_hop(&state.request.url, &response) else {
            diagnostic.refused(FailureStage::Redirect);
            return Some(ScriptAnswer::Failed);
        };
        follow(state, next, response.status);
        None
    }
}

/** Move a request on to its redirect's destination, rewriting a POST to GET as Fetch says. */
fn follow(state: &mut InFlight, next: Url, status: u16) {
    state.flow.redirect(&state.request.url, &next);
    let request = &mut state.request;
    if redirects_to_get(&request.method, status) {
        request.method = crate::network::Method::Get;
        request.body.clear();
        request
            .headers
            .retain(|(name, _)| !BODY_HEADERS.contains(&name.as_str()));
    }
    request.url = next;
    state.redirects += 1;
}
