/** Dedicated worker execution preserves candidate ownership and bounded execution. */
use super::*;

impl Runtime {
    /** A retained worker executes one load, message, response batch or bounded timer task. */
    pub(super) fn worker_turn(
        &mut self,
        worker: &mut Context,
        now: f64,
        until: Instant,
    ) -> EngineResult<Execution> {
        let action = if worker.token.is_none() {
            json!({"kind":"load"})
        } else if let Some(event) = worker.inbox.pop_front() {
            json!({"kind":"event","event":event})
        } else if !worker.requests.is_empty() {
            let mut responses = Vec::new();
            for request in std::mem::take(&mut worker.requests) {
                let deadline =
                    until.max(Instant::now() + crate::runtime::scripting::SCRIPT_REQUEST_WALL);
                responses.push(self.script_request(
                    &worker.policy.base,
                    &request,
                    &mut worker.preflights,
                    deadline,
                    Some(&worker.policy),
                )?);
            }
            json!({"kind":"resume","responses":responses})
        } else {
            json!({"kind":"timers","now_ms":(now-worker.clock_origin).max(0.0),"budget":1})
        };
        let identity = &worker.lease.identity;
        let keep = self
            .page
            .as_ref()
            .and_then(|page| page.context_token(identity));
        let params = json!({"sources":&*worker.sources,"document":{"identity":identity,"charset":"UTF-8","nodes":[]},
            "environment":worker.environment,"languages":self.languages,"resident":true,"resident_contexts":self.residency.snapshot(),
            "base":worker.token,"keep":keep,"action":action});
        let started = Instant::now();
        self.call_worker(&params, None, |runtime, url, deadline| {
            let source = runtime.worker_source(&worker.policy, url, false, deadline)?;
            if let Some(source) = &source {
                worker.muted_errors |= source.policy.base.origin() != worker.policy.base.origin();
            }
            Ok(source.map(|source| source.text))
        })
        .inspect_err(|error| {
            super::super::worker_loading::execution_failure(
                started,
                super::super::worker_loading::Failure::execution(error),
            );
        })
    }

    /** Worker effects are accepted only on their candidate and translated into later owner tasks. */
    pub(super) fn worker_effects(
        &mut self,
        page: &mut Page,
        id: u32,
        worker: &mut Context,
        execution: Execution,
    ) -> EngineResult<bool> {
        worker.token = execution.state_token;
        worker.next_timer = execution.effects.next_timer_ms;
        worker.requests.extend(execution.effects.requests);
        crate::runtime::scripting::record_console(
            &mut page.scripting,
            execution.effects.console,
            execution.effects.console_dropped,
        );
        let mut closed = false;
        for command in execution.effects.worker_commands {
            match command {
                WorkerCommand::Post {
                    id: 0,
                    data,
                    ports,
                    moved_ports,
                } => {
                    self.worker_links(page, id, &ports, &moved_ports)?;
                    let dead: Vec<_> = ports
                        .iter()
                        .filter(|link| !page.scripting.workers.links.contains_key(link))
                        .copied()
                        .collect();
                    enqueue(
                        &mut page.scripting.workers,
                        PageEvent::WorkerMessage { id, data, ports },
                    )?;
                    for link in dead {
                        enqueue(&mut page.scripting.workers, PageEvent::PortClose { link })?;
                    }
                }
                WorkerCommand::Terminate { id: 0 } => closed = true,
                _ => return Err(EngineError::invalid("Invalid nested worker effect.")),
            }
        }
        for message in execution.effects.port_messages {
            if page.scripting.workers.links.get(&message.link) == Some(&id) {
                enqueue(
                    &mut page.scripting.workers,
                    PageEvent::PortMessage {
                        link: message.link,
                        data: message.data,
                    },
                )?;
            }
        }
        for link in execution.effects.port_closes {
            if page.scripting.workers.close_link(link, Some(id)) {
                enqueue(&mut page.scripting.workers, PageEvent::PortClose { link })?;
            }
            worker.inbox.retain(|event| !matches!(event, PageEvent::PortMessage { link: value, .. } if *value == link));
        }
        if !execution.effects.worker_errors.is_empty() || !execution.failed.is_empty() {
            super::super::worker_loading::execution_failure(
                Instant::now(),
                super::super::worker_loading::Failure::Execution,
            );
        }
        for error in execution.effects.worker_errors {
            let message = if worker.muted_errors {
                "Script error."
            } else {
                &error
            };
            self.worker_error(page, id, message)?;
        }
        Ok(closed)
    }
}
