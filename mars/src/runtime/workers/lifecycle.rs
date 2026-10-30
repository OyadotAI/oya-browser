/** Dedicated worker lifecycle preserves candidate ownership and bounded execution. */
use super::*;

impl Runtime {
    /** Capacity is reserved before source acquisition, and every unsuccessful exit releases it. */
    pub(super) fn create_worker(
        &mut self,
        page: &Page,
        owner: &[u32],
        url: &str,
        name: String,
        worker_type: WorkerType,
        credentials: WorkerCredentials,
        blob_source: Option<crate::v9::BlobUrlSource>,
        until: Instant,
    ) -> EngineResult<Context> {
        if owner.len() >= usize::from(crate::v9::WORKER_DEPTH)
            || page.scripting.workers.contexts.len() >= WORKER_CONTEXTS
        {
            return Err(EngineError::limit("Worker tree contexts", WORKER_CONTEXTS));
        }
        let (creator, parent, clock) = if owner.is_empty() {
            (
                Policy {
                    base: url::Url::parse(&page.url)
                        .map_err(|_| EngineError::invalid("Invalid worker creator URL."))?,
                    partition: url::Url::parse(&page.top_level_url)
                        .map_err(|_| EngineError::invalid("Invalid worker partition."))?,
                    csp: page.document.csp.clone(),
                },
                &page.scripting.environment,
                page.scripting.clock_ms,
            )
        } else {
            let parent = page
                .scripting
                .workers
                .contexts
                .get(owner)
                .ok_or_else(|| EngineError::invalid("Worker creator is no longer live."))?;
            (
                parent.policy.clone(),
                &parent.environment,
                (page.scripting.clock_ms - parent.clock_origin).max(0.0),
            )
        };
        let started = Instant::now();
        let lease = self.residency.reserve(false).inspect_err(|error| {
            super::super::worker_loading::record(
                true,
                started,
                super::super::worker_loading::Failure::engine(error),
            );
        })?;
        let deadline = until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
        let source = if url.starts_with("blob:") {
            self.worker_blob_source(&creator, url, blob_source.as_ref(), worker_type)
        } else if blob_source.is_some() {
            return Err(EngineError::invalid(
                "Blob source requires a Blob worker URL.",
            ));
        } else {
            match worker_type {
                WorkerType::Classic => self.worker_source(&creator, url, true, deadline),
                WorkerType::Module => {
                    self.worker_module_source(&creator, url, true, credentials, deadline)
                }
            }
        }?
        .ok_or_else(|| EngineError::unsupported("The worker script could not be loaded."))?;
        let mut environment =
            environment(parent, clock, (owner.len() + 1) as u8, &source.policy, name);
        environment.worker_module = worker_type == WorkerType::Module;
        let modules = modules::Graph::new(worker_type, credentials, url, &source.text);
        Ok(Context {
            lease,
            sources: Arc::new(vec![if worker_type == WorkerType::Module {
                String::new()
            } else {
                source.text
            }]),
            environment,
            policy: source.policy,
            token: None,
            inbox: VecDeque::new(),
            requests: Vec::new(),
            port_outbox: Vec::new(),
            port_closes: Vec::new(),
            preflights: Default::default(),
            next_timer: None,
            clock_origin: page.scripting.clock_ms,
            muted_errors: false,
            modules,
            ready: worker_type == WorkerType::Classic,
        })
    }
}
