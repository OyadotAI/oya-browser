/** Dedicated worker messaging preserves candidate ownership and bounded execution. */
use super::*;

impl Runtime {
    /** Link ownership is local to this parent/worker pair and cannot be redirected by a message. */
    pub(in crate::runtime) fn worker_links(
        &self,
        page: &mut Page,
        id: u32,
        ports: &[u64],
        moved: &[u64],
    ) -> EngineResult<()> {
        super::super::frame_router::validate_ports(ports, moved)?;
        let added = ports
            .iter()
            .filter(|link| {
                !moved.contains(link) && !page.scripting.workers.links.contains_key(link)
            })
            .count();
        if added + page.scripting.workers.links.len() > WORKER_TASKS {
            return Err(EngineError::limit("Worker port links", WORKER_TASKS));
        }
        for link in ports {
            if moved.contains(link) && !page.scripting.workers.links.contains_key(link) {
                continue;
            }
            if page
                .scripting
                .workers
                .links
                .get(link)
                .is_some_and(|owner| *owner != id)
            {
                return Err(EngineError::invalid(
                    "Worker port belongs to another context.",
                ));
            }
            page.scripting.workers.links.insert(*link, id);
        }
        Ok(())
    }

    /** Consume only worker-owned port messages; frame routes retain every other message. */
    pub(in crate::runtime) fn worker_ports(&self, page: &mut Page) -> EngineResult<()> {
        let mut later = Vec::new();
        for message in std::mem::take(&mut page.scripting.port_outbox) {
            if let Some(worker) = page
                .scripting
                .workers
                .links
                .get(&message.link)
                .and_then(|id| page.scripting.workers.contexts.get_mut(id))
            {
                if worker.inbox.len() >= WORKER_TASKS {
                    return Err(EngineError::limit("Worker inbox", WORKER_TASKS));
                }
                worker.inbox.push_back(PageEvent::PortMessage {
                    link: message.link,
                    data: message.data,
                });
            } else {
                later.push(message);
            }
        }
        page.scripting.port_outbox = later;
        let mut later_closes = Vec::new();
        for link in std::mem::take(&mut page.scripting.port_closes) {
            let owner = page.scripting.workers.links.get(&link).copied();
            if page.scripting.workers.close_link(link, None) {
                if let Some(worker) =
                    owner.and_then(|id| page.scripting.workers.contexts.get_mut(&id))
                {
                    if worker.inbox.len() >= WORKER_TASKS {
                        return Err(EngineError::limit("Worker inbox", WORKER_TASKS));
                    }
                    worker.inbox.push_back(PageEvent::PortClose { link });
                }
            } else {
                later_closes.push(link);
            }
        }
        page.scripting.port_closes = later_closes;
        Ok(())
    }

    /** Failures stay asynchronous and visible without fabricated messages or successful workers. */
    pub(in crate::runtime) fn worker_error(
        &self,
        page: &mut Page,
        id: u32,
        message: &str,
    ) -> EngineResult<()> {
        enqueue(
            &mut page.scripting.workers,
            PageEvent::WorkerError {
                id,
                message: message.into(),
            },
        )
    }
}
