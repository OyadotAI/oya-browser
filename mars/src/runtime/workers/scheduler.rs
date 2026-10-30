/** One bounded native scheduler services the complete dedicated-worker tree. */
use super::*;

impl Runtime {
    /** Context creation, delivery and execution never recursively enter the V9 service. */
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
            if let Some(task) = page.scripting.workers.pending.pop_front() {
                self.worker_command(page, task, until)?;
                changed = true;
                continue;
            }
            self.worker_ports(page)?;
            self.nested_worker_ports(page)?;
            if let Some(delivery) = page.scripting.workers.deliveries.pop_front() {
                if delivery.owner.is_empty() {
                    changed |= self
                        .script_step(page, json!({"kind":"event","event":delivery.event}))?
                        .changed;
                } else if let Some(worker) =
                    page.scripting.workers.contexts.get_mut(&delivery.owner)
                {
                    worker.inbox.push_back(delivery.event);
                }
                continue;
            }
            let workers = &page.scripting.workers;
            let next = workers
                .contexts
                .range((
                    std::ops::Bound::Excluded(workers.cursor.clone()),
                    std::ops::Bound::Unbounded,
                ))
                .chain(workers.contexts.range(..=workers.cursor.clone()))
                .find(|(_, worker)| {
                    worker.token.is_none()
                        || worker.inbox.iter().any(|event| worker.event_ready(event))
                        || !worker.requests.is_empty()
                        || !worker.modules.pending.is_empty()
                        || worker
                            .next_timer
                            .is_some_and(|due| due + worker.clock_origin <= page.scripting.clock_ms)
                })
                .map(|(owner, _)| owner.clone());
            let Some(owner) = next else {
                break;
            };
            page.scripting.workers.cursor = owner.clone();
            let mut worker = page
                .scripting
                .workers
                .contexts
                .remove(&owner)
                .expect("selected worker");
            match self.worker_turn(&mut worker, page.scripting.clock_ms, until) {
                Ok(execution) => {
                    if self.worker_effects(page, &owner, &mut worker, execution)? {
                        page.scripting.workers.remove(&owner, false)?;
                    } else {
                        page.scripting.workers.contexts.insert(owner, worker);
                    }
                }
                Err(error) if fatal(&error) => return Err(error),
                Err(error) => {
                    page.scripting.workers.remove(&owner, true)?;
                    let (id, parent) = owner.split_last().expect("worker owner");
                    self.worker_error(page, parent, *id, &error.message)?;
                }
            }
            page.scripting.workers.validate_queues()?;
            changed = true;
        }
        Ok(changed)
    }

    /** Parent-local IDs are qualified by the native sender before any load or delivery occurs. */
    fn worker_command(&mut self, page: &mut Page, task: Task, until: Instant) -> EngineResult<()> {
        let Task { owner, command } = task;
        if !owner.is_empty() && !page.scripting.workers.contexts.contains_key(&owner) {
            return Ok(());
        }
        let id = match &command {
            WorkerCommand::Create { id, .. }
            | WorkerCommand::Post { id, .. }
            | WorkerCommand::Terminate { id } => *id,
        };
        if id == 0 || owner.len() >= usize::from(crate::v9::WORKER_DEPTH) {
            return Err(EngineError::invalid("Invalid child worker identity."));
        }
        let mut child = owner.clone();
        child.push(id);
        match command {
            WorkerCommand::Create {
                url,
                name,
                worker_type,
                credentials,
                blob_source,
                ..
            } => {
                if page.scripting.workers.contexts.contains_key(&child) {
                    return Err(EngineError::invalid("Duplicate Worker identity."));
                }
                match self.create_worker(
                    page,
                    &owner,
                    &url,
                    name,
                    worker_type,
                    credentials,
                    blob_source,
                    until,
                ) {
                    Ok(worker) => {
                        page.scripting.workers.contexts.insert(child, worker);
                    }
                    Err(error) if fatal(&error) => return Err(error),
                    Err(error) => self.worker_error(page, &owner, id, &error.message)?,
                }
            }
            WorkerCommand::Post {
                data,
                ports,
                moved_ports,
                ..
            } => {
                page.scripting
                    .workers
                    .post(&owner, &child, 0, data, ports, moved_ports)?;
            }
            WorkerCommand::Terminate { .. } => page.scripting.workers.remove(&child, true)?,
        }
        Ok(())
    }

    /** Newly admitted transfers precede the channel's already-produced messages and close effects. */
    fn nested_worker_ports(&self, page: &mut Page) -> EngineResult<()> {
        let owners: Vec<_> = page.scripting.workers.contexts.keys().cloned().collect();
        for owner in owners {
            let worker = page
                .scripting
                .workers
                .contexts
                .get_mut(&owner)
                .expect("known worker");
            let messages = std::mem::take(&mut worker.port_outbox);
            let closes = std::mem::take(&mut worker.port_closes);
            for message in messages {
                page.scripting.workers.port(&owner, message)?;
            }
            for link in closes {
                page.scripting.workers.close_port(&owner, link)?;
            }
        }
        Ok(())
    }
}
