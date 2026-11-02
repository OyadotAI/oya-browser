/** Native worker endpoints preserve parent/child ports without confusing realm-local handle IDs. */
use super::*;

impl Workers {
    /** Only fresh channel transfers are supported; existing V9 retransfer refusals remain enforced. */
    fn link_ports(
        &mut self,
        from: &[u32],
        to: &[u32],
        ports: &[u64],
        moved: &[u64],
    ) -> EngineResult<()> {
        super::super::frame_router::validate_ports(ports, moved)?;
        if !moved.is_empty() {
            return Err(EngineError::invalid(
                "Worker port retransfer is unsupported.",
            ));
        }
        if ports.iter().any(|link| self.links.contains_key(link)) {
            return Err(EngineError::invalid(
                "Worker port belongs to another context.",
            ));
        }
        if self.links.len() + ports.len() > WORKER_TASKS {
            return Err(EngineError::limit("Worker port links", WORKER_TASKS));
        }
        for link in ports {
            self.links.insert(*link, [from.to_vec(), to.to_vec()]);
        }
        Ok(())
    }

    /** Refused construction still closes every endpoint transferred to its unavailable receiver. */
    pub(super) fn post(
        &mut self,
        from: &[u32],
        to: &[u32],
        id: u32,
        data: crate::v9::WorkerData,
        ports: Vec<u64>,
        moved: Vec<u64>,
    ) -> EngineResult<()> {
        self.link_ports(from, to, &ports, &moved)?;
        if to.is_empty() || self.contexts.contains_key(to) {
            enqueue_to(self, to, PageEvent::WorkerMessage { id, data, ports })?;
        } else {
            for link in ports {
                self.links.remove(&link);
                enqueue_to(self, from, PageEvent::PortClose { link })?;
            }
        }
        Ok(())
    }

    /** Port messages follow native paired endpoints, independently of each worker's local handle IDs. */
    pub(super) fn port(
        &mut self,
        from: &[u32],
        message: crate::v9::PortMessage,
    ) -> EngineResult<bool> {
        let Some(peer) = self
            .links
            .get(&message.link)
            .and_then(|ends| opposite(ends, from))
            .cloned()
        else {
            return Ok(false);
        };
        enqueue_to(
            self,
            &peer,
            PageEvent::PortMessage {
                link: message.link,
                data: message.data,
            },
        )?;
        Ok(true)
    }

    /** A closing endpoint cannot consume its own previously queued input after closure. */
    pub(super) fn close_port(&mut self, from: &[u32], link: u64) -> EngineResult<bool> {
        let Some(peer) = self.close_link(link, from) else {
            return Ok(false);
        };
        enqueue_to(self, &peer, PageEvent::PortClose { link })?;
        Ok(true)
    }
}

impl Runtime {
    /** Root worker routes consume only their own effects; frame routes retain unrelated messages. */
    pub(in crate::runtime) fn worker_ports(&self, page: &mut Page) -> EngineResult<()> {
        let mut later = Vec::new();
        for message in std::mem::take(&mut page.scripting.port_outbox) {
            if !page.scripting.workers.port(&[], message.clone())? {
                later.push(message);
            }
        }
        page.scripting.port_outbox = later;
        let mut later = Vec::new();
        for link in std::mem::take(&mut page.scripting.port_closes) {
            if !page.scripting.workers.close_port(&[], link)? {
                later.push(link);
            }
        }
        page.scripting.port_closes = later;
        Ok(())
    }

    /** Child errors target the creating realm's exact handle rather than the top-level document. */
    pub(in crate::runtime) fn worker_error(
        &self,
        page: &mut Page,
        owner: &[u32],
        id: u32,
        message: &str,
    ) -> EngineResult<()> {
        enqueue_to(
            &mut page.scripting.workers,
            owner,
            PageEvent::WorkerError {
                id,
                message: message.into(),
            },
        )
    }
}
