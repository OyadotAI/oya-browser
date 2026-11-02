/** A bounded broker captures recipients at post time and drops only closed or retired receivers. */
use super::*;
use crate::v9::WorkerData;
use std::collections::{BTreeMap, VecDeque};

/** Context incarnation plus local endpoint ID cannot be reused by navigation or another worker. */
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct Key {
    pub context: String,
    pub id: u64,
}

/** Creation order is native and stable across candidate clones. */
#[derive(Clone)]
pub(super) struct Endpoint {
    address: Address,
    name: Vec<u16>,
    creation: u64,
    bytes: usize,
}

/** Each receiver gets its own later task; queued data survives sender closure. */
#[derive(Clone)]
pub(super) struct Letter {
    pub serial: u64,
    pub recipient: Key,
    pub tab: String,
    pub data: Arc<WorkerData>,
    pub origin: String,
    bytes: usize,
}

/** One coordinator owns all channels; no process-global registry or persisted cross-launch state. */
#[derive(Clone, Default)]
pub(in crate::runtime) struct Broker {
    endpoints: BTreeMap<Key, Endpoint>,
    pending: VecDeque<Letter>,
    sequence: u64,
    bytes: usize,
}

impl Broker {
    /** A final empty snapshot still matters when GC retired the last endpoint. */
    pub(in crate::runtime) fn has_context(&self, identity: &str) -> bool {
        self.endpoints.keys().any(|key| key.context == identity)
    }
    /** Delivery peeks so an abandoned recipient candidate cannot consume committed mail. */
    pub(super) fn next(&self) -> Option<Letter> {
        self.pending.front().cloned()
    }
    /** Closing a tab retires recipients without retracting messages already sent to other tabs. */
    pub(in crate::runtime) fn remove_tab(&mut self, tab: &str) {
        self.endpoints
            .retain(|_, endpoint| endpoint.address.tab != tab);
        self.pending.retain(|letter| letter.tab != tab);
        self.recount();
    }
    /** Only documents in the committed native tree may retain endpoints or receive queued tasks. */
    pub(super) fn prune(&mut self, live: &[Address]) {
        self.endpoints
            .retain(|_, endpoint| live.contains(&endpoint.address));
        self.pending
            .retain(|letter| self.endpoints.contains_key(&letter.recipient));
        self.recount();
    }
    /** Retirement revokes reception immediately, but already-started posts still know their sender. */
    pub(super) fn retired_sources(&self, live: &[Address]) -> BTreeMap<Key, Endpoint> {
        self.endpoints
            .iter()
            .filter(|(_, endpoint)| !live.contains(&endpoint.address))
            .map(|(key, endpoint)| (key.clone(), endpoint.clone()))
            .collect()
    }
    /** Successful recipient publication consumes its task exactly once. */
    pub(super) fn acknowledge(&mut self, ids: &[u64]) {
        self.pending.retain(|letter| !ids.contains(&letter.serial));
        self.recount();
    }
    /** Commands execute in VM order before the final live snapshot reconciles garbage collection. */
    #[cfg(test)]
    pub(super) fn apply(&mut self, batch: Batch) -> EngineResult<()> {
        self.apply_with_sources(batch, &BTreeMap::new())
    }
    /** Retired contexts can finish captured sends without regaining any recipient authority. */
    pub(super) fn apply_with_sources(
        &mut self,
        batch: Batch,
        retired: &BTreeMap<Key, Endpoint>,
    ) -> EngineResult<()> {
        batch_bytes(&batch.commands, &batch.channels)?;
        let mut closed = BTreeMap::new();
        for command in batch.commands {
            let id = match &command {
                BroadcastCommand::Open { id, .. }
                | BroadcastCommand::Post { id, .. }
                | BroadcastCommand::Close { id } => *id,
            };
            let key = Key {
                context: batch.address.identity.clone(),
                id,
            };
            match command {
                BroadcastCommand::Open { name, .. } => self.open(key, &batch.address, name)?,
                BroadcastCommand::Post { data, .. } => {
                    let source = self
                        .endpoints
                        .get(&key)
                        .or_else(|| closed.get(&key))
                        .or_else(|| retired.get(&key))
                        .cloned()
                        .ok_or_else(|| {
                            EngineError::invalid("Broadcast source is not registered.")
                        })?;
                    self.post(&key, &source, data)?;
                }
                BroadcastCommand::Close { .. } => {
                    if let Some(endpoint) = self.endpoints.get(&key).cloned() {
                        closed.insert(key.clone(), endpoint);
                    }
                    self.close(&key);
                }
            }
        }
        for channel in &batch.channels {
            let key = Key {
                context: batch.address.identity.clone(),
                id: channel.id,
            };
            if !self
                .endpoints
                .get(&key)
                .or_else(|| retired.get(&key))
                .is_some_and(|endpoint| {
                    endpoint.name == channel.name && endpoint.address == batch.address
                })
            {
                return Err(EngineError::invalid(
                    "Broadcast snapshot disagrees with its commands.",
                ));
            }
        }
        let retired: Vec<_> = self
            .endpoints
            .keys()
            .filter(|key| {
                key.context == batch.address.identity
                    && !batch.channels.iter().any(|channel| channel.id == key.id)
            })
            .cloned()
            .collect();
        for key in retired {
            self.close(&key);
        }
        Ok(())
    }
    /** Admission includes names, repeated native identity metadata and fixed allocation overhead. */
    fn open(&mut self, key: Key, address: &Address, name: Vec<u16>) -> EngineResult<()> {
        if self.endpoints.contains_key(&key) {
            return Err(EngineError::invalid("Duplicate broadcast endpoint."));
        }
        let bytes = address.bytes() + name.len() * UNIT_BYTES + OVERHEAD;
        if self.endpoints.len() >= ENDPOINTS || self.bytes.saturating_add(bytes) > BYTES {
            return Err(EngineError::limit("broadcast endpoint storage", BYTES));
        }
        let creation = self.sequence()?;
        self.endpoints.insert(
            key,
            Endpoint {
                address: address.clone(),
                name,
                creation,
                bytes,
            },
        );
        self.bytes += bytes;
        Ok(())
    }
    /** Capture and charge the complete recipient set before cloning even one delivery. */
    fn post(&mut self, source_key: &Key, source: &Endpoint, data: WorkerData) -> EngineResult<()> {
        let bytes = data
            .bytes()?
            .saturating_add(source.address.origin.len())
            .saturating_add(OVERHEAD);
        let mut receivers: Vec<_> = self
            .endpoints
            .iter()
            .filter(|(key, endpoint)| {
                *key != source_key
                    && endpoint.name == source.name
                    && endpoint.address.origin == source.address.origin
                    && endpoint.address.partition == source.address.partition
            })
            .map(|(key, endpoint)| (endpoint.creation, key.clone(), endpoint.address.tab.clone()))
            .collect();
        receivers.sort_by_key(|(order, _, _)| *order);
        let total = receivers.iter().fold(0usize, |used, (_, key, tab)| {
            used.saturating_add(bytes + key.context.len() + tab.len())
        });
        if self.pending.len().saturating_add(receivers.len()) > TASKS
            || self.bytes.saturating_add(total) > BYTES
        {
            return Err(EngineError::limit("broadcast delivery storage", BYTES));
        }
        let data = Arc::new(data);
        for (_, recipient, tab) in receivers {
            let bytes = bytes + recipient.context.len() + tab.len();
            let serial = self.sequence()?;
            self.pending.push_back(Letter {
                serial,
                recipient,
                tab,
                data: data.clone(),
                origin: source.address.origin.clone(),
                bytes,
            });
            self.bytes += bytes;
        }
        Ok(())
    }
    /** Closed receivers cannot regain queued deliveries; sender closure affects no other destination. */
    fn close(&mut self, key: &Key) {
        self.endpoints.remove(key);
        self.pending.retain(|letter| &letter.recipient != key);
        self.recount();
    }
    /** Monotonic counters prevent delayed acknowledgments from consuming later work. */
    fn sequence(&mut self) -> EngineResult<u64> {
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("broadcast sequence", usize::MAX))?;
        Ok(self.sequence)
    }
    /** Reconciliation is bounded by endpoint/task counts and never scans payload graphs. */
    fn recount(&mut self) {
        self.bytes = self
            .endpoints
            .values()
            .map(|entry| entry.bytes)
            .sum::<usize>()
            + self
                .pending
                .iter()
                .map(|letter| letter.bytes)
                .sum::<usize>();
    }
}
