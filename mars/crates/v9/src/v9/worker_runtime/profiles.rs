/** Completed profiles cross the worker boundary as bounded inert JSON, never VM objects or file authority. */
use super::*;

impl ConformanceWorkerHost {
    /** A real isolate may export only into the explicit session that created its endpoint. */
    pub(in crate::v9) fn record_cpu_profile(
        &self,
        endpoint: Option<u64>,
        json: String,
    ) -> EngineResult<()> {
        if endpoint.is_some_and(|id| !self.workers.lock().unwrap().contains_key(&id)) {
            return Err(EngineError::invalid(
                "CPU profile endpoint is outside its worker session.",
            ));
        }
        let mut profiles = self.profiles.lock().unwrap();
        let used = profiles
            .iter()
            .map(|(_, json)| json.capacity())
            .sum::<usize>();
        if profiles.len() >= crate::constants::VM_CONSOLE_ENTRIES
            || used.saturating_add(json.capacity()) > crate::constants::VM_STATE_BYTES
        {
            return Err(EngineError::limit(
                "V9 CPU profile artifacts",
                crate::constants::VM_STATE_BYTES,
            ));
        }
        profiles.push((endpoint, json));
        Ok(())
    }
}

impl ConformanceWorkerSession {
    /** Only the session owner receives artifacts; native profiling never chooses an output directory. */
    pub fn take_cpu_profiles(&self) -> Vec<(Option<u64>, String)> {
        std::mem::take(&mut *self.host.profiles.lock().unwrap())
    }
}
