mod blob;
/** Worker scripts use distinct origin, CSP, MIME and partition rules through governed acquisition. */
mod diagnostics;
use super::{csp, resources::Redirects, supervisor::Runtime};
use crate::network::{Destination, WebRequest};
use crate::protocol::{EngineError, EngineResult};
use crate::v9::WorkerCredentials;
pub(super) use diagnostics::{Failure, execution_failure, record};
use std::time::Instant;
use url::Url;

/** A worker's response policy and API base are immutable for its lifetime. */
#[derive(Clone)]
pub(super) struct Policy {
    pub base: Url,
    pub partition: Url,
    pub csp: Vec<String>,
}

/** Initial loading returns both actual source and the response's worker policy. */
pub(super) struct Source {
    pub text: String,
    pub policy: Policy,
}

impl Runtime {
    /** Initial workers are same-origin; imports may cross origins under the worker's own CSP. */
    pub(super) fn worker_source(
        &mut self,
        policy: &Policy,
        address: &str,
        initial: bool,
        deadline: Instant,
    ) -> EngineResult<Option<Source>> {
        self.worker_source_mode(policy, address, initial, None, deadline)
    }

    /** Modules retain credentials and require CORS for every cross-origin dependency. */
    pub(super) fn worker_module_source(
        &mut self,
        policy: &Policy,
        address: &str,
        initial: bool,
        credentials: WorkerCredentials,
        deadline: Instant,
    ) -> EngineResult<Option<Source>> {
        self.worker_source_mode(policy, address, initial, Some(credentials), deadline)
    }

    /** Both grammars share origin, policy, partition, deadlines and executable response checks. */
    fn worker_source_mode(
        &mut self,
        policy: &Policy,
        address: &str,
        initial: bool,
        module: Option<WorkerCredentials>,
        deadline: Instant,
    ) -> EngineResult<Option<Source>> {
        let started = Instant::now();
        self.publish(|_| ())
            .inspect_err(|error| record(initial, started, Failure::engine(error)))?;
        let Ok(url) = crate::network::policy::web_url(address) else {
            return refused(initial, started, Failure::Address);
        };
        if initial && url.origin() != policy.base.origin() {
            return refused(initial, started, Failure::Origin);
        }
        if !csp::allows_worker(&policy.csp, &policy.base, &url, initial) {
            return refused(initial, started, Failure::Policy);
        }
        let cross = url.origin() != policy.base.origin();
        let credentials = match module {
            Some(WorkerCredentials::Omit) => false,
            Some(WorkerCredentials::Include) => true,
            _ => !cross,
        };
        let request = WebRequest {
            url: url.to_string(),
            destination: if initial {
                Destination::Worker
            } else {
                Destination::WorkerImport
            },
            top_level: Some(policy.partition.to_string()),
            origin: (module.is_some() && cross).then(|| policy.base.origin().ascii_serialization()),
            mode: module.map(|_| crate::network::cors::Mode::Cors),
            ..WebRequest::default()
        };
        let acquired = self.acquire_with(
            request,
            deadline,
            Some(policy.base.clone()),
            false,
            Redirects::Refuse,
            credentials,
        );
        self.publish(|_| ())
            .inspect_err(|error| record(initial, started, Failure::engine(error)))?;
        let response = match acquired {
            Ok(acquired) => acquired.response,
            Err(error) => {
                record(initial, started, Failure::engine(&error));
                return if fatal(&error) { Err(error) } else { Ok(None) };
            }
        };
        if let Some(reason) = response_failure(&response) {
            return refused(initial, started, reason);
        }
        if module.is_some()
            && cross
            && !crate::network::cors::allows(
                &response.headers,
                &policy.base.origin().ascii_serialization(),
                credentials,
            )
        {
            return refused(initial, started, Failure::Policy);
        }
        record(initial, started, Failure::Loaded);
        let csp = csp::response_policies(&response.headers);
        Ok(Some(Source {
            text: response.source,
            policy: Policy {
                base: url,
                partition: policy.partition.clone(),
                csp,
            },
        }))
    }
}

/** A refusal records only a fixed category, never URLs, script text, headers or server strings. */
fn refused(initial: bool, started: Instant, failure: Failure) -> EngineResult<Option<Source>> {
    record(initial, started, failure);
    Ok(None)
}

/** Classify response shape before reading any source as JavaScript. */
fn response_failure(response: &crate::network::WebResponse) -> Option<Failure> {
    if response.status != diagnostics::HTTP_OK {
        return Some(Failure::Status(response.status));
    }
    if response.download || response.image_data {
        return Some(Failure::BodyKind);
    }
    if response.source.len() > crate::constants::SCRIPT_BYTES {
        return Some(Failure::SourceLimit);
    }
    (!javascript(&response.mime)).then_some(Failure::Mime)
}

/** Script responses require an actual JavaScript type; HTML errors never become executable. */
fn javascript(mime: &str) -> bool {
    mars_core::http::is_javascript_mime_essence(mime)
}

/** Ownership, service failure and cancellation cannot be converted into catchable load failures. */
pub(super) fn fatal(error: &EngineError) -> bool {
    error.service.is_some()
        || [
            "permission_denied",
            "control_denied",
            "state_missing",
            "navigation_cancelled",
        ]
        .contains(&error.code.as_str())
}
