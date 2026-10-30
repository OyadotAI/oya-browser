/** Dedicated worker lifecycle preserves candidate ownership and bounded execution. */
use super::*;

impl Runtime {
    /** Capacity is reserved before source acquisition, and every unsuccessful exit releases it. */
    pub(super) fn create_worker(
        &mut self,
        page: &Page,
        url: &str,
        name: String,
        until: Instant,
    ) -> EngineResult<Context> {
        let started = Instant::now();
        let lease = self.residency.reserve(false).inspect_err(|error| {
            super::super::worker_loading::record(
                true,
                started,
                super::super::worker_loading::Failure::engine(error),
            );
        })?;
        let base = url::Url::parse(&page.url)
            .map_err(|_| EngineError::invalid("Invalid worker creator URL."))?;
        let partition = url::Url::parse(&page.top_level_url)
            .map_err(|_| EngineError::invalid("Invalid worker partition."))?;
        let creator = Policy {
            base,
            partition,
            csp: page.document.csp.clone(),
        };
        let deadline = until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
        let source = self
            .worker_source(&creator, url, true, deadline)?
            .ok_or_else(|| EngineError::unsupported("The worker script could not be loaded."))?;
        let environment = environment(page, &source.policy, name);
        Ok(Context {
            lease,
            sources: Arc::new(vec![source.text]),
            environment,
            policy: source.policy,
            token: None,
            inbox: VecDeque::new(),
            requests: Vec::new(),
            preflights: Default::default(),
            next_timer: None,
            clock_origin: page.scripting.clock_ms,
            muted_errors: false,
        })
    }
}
