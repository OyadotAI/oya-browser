/**
 * The page side of loader-carried fetches: start a page's fetch or XMLHttpRequest on the loader,
 * collect the hops it finished, and turn each into the page's answer or the redirect's next hop.
 * The page runs on while its requests are on the wire, as in a browser, instead of the runtime
 * thread waiting out a slow response with every tab frozen behind it.
 */
use super::{FAILED_TO_FETCH, failed, readable, script_request_from};
use crate::network::WebResponse;
use crate::protocol::EngineResult;
use crate::runtime::page::Page;
use crate::runtime::resources::{InFlight, Progress, ScriptAnswer, ScriptLoad};
use crate::runtime::supervisor::Runtime;
use crate::v9::{HostRequest, HostResponse};
use std::time::{Duration, Instant};
use url::Url;

/// Finished hops kept for pages that have not taken them. A page that navigated away never
/// will, so past this many the oldest are dropped.
const LOADED_RESULTS: usize = 256;
/** Outbound admission gets a short turn before followup script and render work can block it. */
const REQUEST_DISPATCH_WALL: Duration = Duration::from_millis(2);

impl Runtime {
    /** Start task-created requests now; responses still execute through the ordinary network task source. */
    pub(super) fn dispatch_pending_requests(&mut self, page: &mut Page) -> EngineResult<()> {
        if page.scripting.requests.is_empty() {
            return Ok(());
        }
        let requests = std::mem::take(&mut page.scripting.requests);
        let until = Instant::now() + REQUEST_DISPATCH_WALL;
        let mut responses = self.perform_requests(page, requests, until)?;
        responses.append(&mut page.scripting.ready);
        page.scripting.ready = responses;
        Ok(())
    }

    /** Move every hop the loader finished into `loaded`, dropping the oldest past the bound. */
    pub(super) fn collect_loaded(&mut self) {
        let done = self.loader.poll();
        self.keep_loaded(done);
    }

    /** Record finished hops; true when any arrived. */
    fn keep_loaded(&mut self, done: Vec<crate::runtime::loader::Done>) -> bool {
        let arrived = !done.is_empty();
        for hop in done {
            self.loaded.insert(hop.id, hop.result);
        }
        while self.loaded.len() > LOADED_RESULTS {
            self.loaded.pop_first();
        }
        arrived
    }

    /** Whether one of this page's hops has a result waiting. */
    pub(super) fn landed(&self, page: &Page) -> bool {
        let in_flight = &page.scripting.in_flight;
        in_flight
            .iter()
            .any(|hop| self.loaded.contains_key(&hop.job))
    }

    /** Wait until `until` for a hop of this page to finish; true when one did. For a settle
    that has nothing else to do, so a page acted on settles with its data when it comes fast. */
    pub(in crate::runtime) fn wait_for_network(&mut self, page: &Page, until: Instant) -> bool {
        if page.scripting.in_flight.is_empty() {
            return false;
        }
        let done = self.loader.wait(until);
        self.keep_loaded(done) && self.landed(page)
    }

    /** Start one page request on the loader: Some answer when it ended at once, None while it
    is on the wire. */
    pub(super) fn fetch_on_loader(
        &mut self,
        page: &mut Page,
        base: &Url,
        request: &HostRequest,
        until: Instant,
    ) -> EngineResult<Option<HostResponse>> {
        let Some(parsed) = script_request_from(request) else {
            return Ok(Some(failed(request.id, FAILED_TO_FETCH)));
        };
        let identity = (request.id, request.kind == "beacon");
        let preflights = &mut page.scripting.preflights;
        let progress = self.start_script_fetch(base, parsed, identity, preflights, until)?;
        Ok(self.progressed(page, progress, identity))
    }

    /** Answers for this page's hops that finished, starting the next hop of any redirect. */
    pub(super) fn drain_landed(
        &mut self,
        page: &mut Page,
        until: Instant,
    ) -> EngineResult<Vec<HostResponse>> {
        let mut answers = Vec::new();
        for (state, result) in self.take_landed(page) {
            let identity = (state.id, state.beacon);
            let load = state.load.clone();
            let preflights = &mut page.scripting.preflights;
            let progress = self.resume_script_fetch(state, result, preflights, until)?;
            match load {
                Some(load) => self.progressed_load(page, progress, load),
                None => answers.extend(self.progressed(page, progress, identity)),
            }
        }
        Ok(answers)
    }

    /** A script load: kept while on the wire, or landed for the next `modules` step. */
    fn progressed_load(&self, page: &mut Page, progress: Progress, load: ScriptLoad) {
        match progress {
            Progress::Sent(state) => page.scripting.in_flight.push(*state),
            Progress::Answered(answer) => {
                let source = super::super::modules::loaded_source(&load, answer);
                super::super::modules::land(page, load, source);
            }
        }
    }

    /** This page's hops that have results, removed from its in-flight list. */
    fn take_landed(&mut self, page: &mut Page) -> Vec<(InFlight, EngineResult<WebResponse>)> {
        let mut landed = Vec::new();
        for hop in std::mem::take(&mut page.scripting.in_flight) {
            match self.loaded.remove(&hop.job) {
                Some(result) => landed.push((hop, result)),
                None => page.scripting.in_flight.push(hop),
            }
        }
        landed
    }

    /** Keep a request that is still on the wire; the page's answer for one that is over
    (none for a beacon). */
    fn progressed(
        &self,
        page: &mut Page,
        progress: Progress,
        (id, beacon): (u32, bool),
    ) -> Option<HostResponse> {
        match progress {
            Progress::Sent(state) => {
                page.scripting.in_flight.push(*state);
                None
            }
            Progress::Answered(_) if beacon => None,
            Progress::Answered(answer) => Some(host_answer(id, answer)),
        }
    }
}

/** What the page receives for a script request's outcome. */
pub(super) fn host_answer(id: u32, answer: ScriptAnswer) -> HostResponse {
    match answer {
        ScriptAnswer::Readable(response, headers) => readable(id, *response, headers),
        ScriptAnswer::Opaque => HostResponse {
            id,
            ..HostResponse::default()
        },
        ScriptAnswer::Failed => failed(id, FAILED_TO_FETCH),
    }
}
