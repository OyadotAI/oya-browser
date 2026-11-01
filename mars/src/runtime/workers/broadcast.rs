/** Broadcast events enter the actual dedicated worker realm, including module top-level await. */
use super::*;

impl Workers {
    /** The worker response policy supplies origin and root partition independently of its environment. */
    pub(in crate::runtime) fn broadcast_addresses(
        &self,
        tab: &str,
        output: &mut Vec<super::super::broadcast::Address>,
    ) -> EngineResult<()> {
        for worker in self
            .contexts
            .values()
            .filter(|worker| worker.token.is_some())
        {
            output.push(super::super::broadcast::Address::new(
                &worker.lease.identity,
                tab,
                &worker.policy.base.origin().ascii_serialization(),
                worker.policy.partition.as_str(),
            )?);
        }
        Ok(())
    }
}

impl Runtime {
    /** A fenced recipient gets one event task; pending owner messages retain their existing ordering. */
    pub(in crate::runtime) fn deliver_worker_broadcast(
        &mut self,
        page: &mut Page,
        identity: &str,
        event: PageEvent,
    ) -> EngineResult<Option<bool>> {
        let owner = page
            .scripting
            .workers
            .contexts
            .iter()
            .find(|(_, worker)| worker.lease.identity == identity)
            .map(|(owner, _)| owner.clone());
        let Some(owner) = owner else {
            return Ok(None);
        };
        let mut worker = page
            .scripting
            .workers
            .contexts
            .remove(&owner)
            .expect("known worker");
        match self.worker_action(&mut worker, json!({"kind":"event","event":event})) {
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
        Ok(Some(true))
    }
}
