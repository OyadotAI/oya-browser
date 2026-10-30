/** Dedicated worker scheduler preserves candidate ownership and bounded execution. */
use super::*;

impl Runtime {
    /** One bounded task loop delivers real messages without recursively entering the V9 service. */
    pub(in crate::runtime) fn run_workers(
        &mut self,
        page: &mut Page,
        until: Instant,
    ) -> EngineResult<bool> {
        let mut changed = false;
        for _ in 0..WORKER_TASKS {
            page.scripting.workers.validate_queues()?;
            if Instant::now() >= until {
                break;
            }
            self.publish(|_| ())?;
            self.worker_ports(page)?;
            if let Some(command) = page.scripting.workers.pending.pop_front() {
                self.worker_command(page, command, until)?;
                changed = true;
                continue;
            }
            if let Some(event) = page.scripting.workers.deliveries.pop_front() {
                changed |= self
                    .script_step(page, json!({"kind":"event","event":event}))?
                    .changed;
                continue;
            }
            let next = page
                .scripting
                .workers
                .contexts
                .range((
                    std::ops::Bound::Excluded(page.scripting.workers.cursor),
                    std::ops::Bound::Unbounded,
                ))
                .chain(
                    page.scripting
                        .workers
                        .contexts
                        .range(..=page.scripting.workers.cursor),
                )
                .find(|(_, worker)| {
                    worker.token.is_none()
                        || (worker.ready && !worker.inbox.is_empty())
                        || !worker.requests.is_empty()
                        || !worker.modules.pending.is_empty()
                        || worker
                            .next_timer
                            .is_some_and(|due| due + worker.clock_origin <= page.scripting.clock_ms)
                })
                .map(|(id, _)| *id);
            let Some(id) = next else {
                break;
            };
            page.scripting.workers.cursor = id;
            let mut worker = page
                .scripting
                .workers
                .contexts
                .remove(&id)
                .expect("selected worker");
            let result = self.worker_turn(&mut worker, page.scripting.clock_ms, until);
            match result {
                Ok(execution) => {
                    let closed = self.worker_effects(page, id, &mut worker, execution)?;
                    if !closed {
                        page.scripting.workers.contexts.insert(id, worker);
                    } else {
                        page.scripting.workers.close_links(id)?;
                    }
                }
                Err(error) if fatal(&error) => return Err(error),
                Err(error) => {
                    page.scripting.workers.remove(id)?;
                    self.worker_error(page, id, &error.message)?;
                }
            }
            changed = true;
        }
        Ok(changed)
    }

    /** Parent tasks preserve construction/message order; no task can address a different page. */
    pub(in crate::runtime) fn worker_command(
        &mut self,
        page: &mut Page,
        command: WorkerCommand,
        until: Instant,
    ) -> EngineResult<()> {
        match command {
            WorkerCommand::Create {
                id,
                url,
                name,
                worker_type,
                credentials,
            } => match self.create_worker(page, &url, name, worker_type, credentials, until) {
                Ok(worker) => {
                    page.scripting.workers.contexts.insert(id, worker);
                }
                Err(error) if fatal(&error) => return Err(error),
                Err(error) => self.worker_error(page, id, &error.message)?,
            },
            WorkerCommand::Post {
                id,
                data,
                ports,
                moved_ports,
            } => {
                if page.scripting.workers.contexts.contains_key(&id) {
                    self.worker_links(page, id, &ports, &moved_ports)?;
                    let dead: Vec<_> = ports
                        .iter()
                        .filter(|link| !page.scripting.workers.links.contains_key(link))
                        .copied()
                        .collect();
                    let worker = page
                        .scripting
                        .workers
                        .contexts
                        .get_mut(&id)
                        .expect("checked worker");
                    if worker.inbox.len() + dead.len() >= WORKER_TASKS {
                        return Err(EngineError::limit("Worker inbox", WORKER_TASKS));
                    }
                    worker
                        .inbox
                        .push_back(PageEvent::WorkerMessage { id: 0, data, ports });
                    for link in dead {
                        worker.inbox.push_back(PageEvent::PortClose { link });
                    }
                } else {
                    // Source acquisition may fail before the already-transferred port is admitted.
                    self.worker_links(page, id, &ports, &moved_ports)?;
                    for link in ports {
                        page.scripting.workers.close_link(link, Some(id));
                        enqueue(&mut page.scripting.workers, PageEvent::PortClose { link })?;
                    }
                    page.scripting.workers.validate_queues()?;
                }
            }
            WorkerCommand::Terminate { id } => page.scripting.workers.remove(id)?,
        }
        Ok(())
    }
}
