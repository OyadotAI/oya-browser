/** Bounded shared snapshots preserve alias revisions without recovering native authority from IDs. */
use super::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
/** The wire contains inert bounded bytes and identity; no global map is consulted by decoding it. */
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wire {
    id: u64,
    revision: u64,
    bytes: Bytes,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sparse: Option<super::super::SparseBuffer>,
    maximum: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    visible: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logical_maximum: Option<usize>,
    #[serde(default)]
    agent: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    waiters: Vec<asynchronous::AsyncSnapshot>,
}

impl SharedBytes {
    /** New backing storage reserves its bounded maximum so growth cannot exceed accounted allocation. */
    pub(in crate::v9) fn new(length: usize, maximum: Option<usize>) -> EngineResult<Self> {
        if length > MAX_BYTES || maximum.is_some_and(|max| max < length || max > MAX_BYTES) {
            return Err(EngineError::limit("V9 shared buffer bytes", MAX_BYTES));
        }
        use ring::rand::SecureRandom;
        let mut id = [0; std::mem::size_of::<u64>()];
        ring::rand::SystemRandom::new()
            .fill(&mut id)
            .map_err(|_| EngineError::invalid("Shared backing identity is unavailable."))?;
        Self::from_wire(
            Wire {
                id: u64::from_le_bytes(id).max(1),
                revision: 0,
                bytes: Bytes(vec![0; length]),
                sparse: None,
                maximum,
                visible: None,
                logical_maximum: None,
                agent: 0,
                waiters: Vec::new(),
            },
            false,
        )
    }
    /** Sparse Wasm stores reserve the same fixed resident budget in every broker ledger. */
    pub(in crate::v9) fn new_sparse_wasm(length: usize, maximum: usize) -> EngineResult<Self> {
        let sparse = super::super::SparseBuffer::new(length);
        if !sparse.valid()
            || maximum < length
            || maximum as u64 > super::super::sparse_buffer::MAX_LOGICAL
            || !maximum.is_multiple_of(WASM_PAGE_BYTES)
        {
            return Err(EngineError::invalid(
                "Invalid sparse shared WebAssembly memory.",
            ));
        }
        let shared = Self::new(0, Some(MAX_BYTES))?;
        {
            let mut state = shared.lock();
            state.bytes = Vec::new();
            state.sparse = Some(sparse);
            state.maximum = Some(maximum);
        }
        Ok(shared)
    }
    /** Every decoded backing is independently bounded before it can become a live buffer. */
    fn from_wire(wire: Wire, restored: bool) -> EngineResult<Self> {
        let length = wire
            .sparse
            .as_ref()
            .map_or(wire.bytes.0.len(), |s| s.length);
        let sparse_valid = wire.sparse.as_ref().is_none_or(|s| {
            s.valid()
                && wire.bytes.0.is_empty()
                && wire.maximum.is_some_and(|max| {
                    max as u64 <= super::super::sparse_buffer::MAX_LOGICAL
                        && max % WASM_PAGE_BYTES == 0
                })
        });
        if wire.id == 0
            || !sparse_valid
            || wire.revision > MAX_SNAPSHOT_REVISION
            || wire.visible.is_some_and(|visible| visible > length)
            || wire.logical_maximum.is_some_and(|maximum| {
                wire.visible.is_some()
                    || maximum < length
                    || maximum % WASM_PAGE_BYTES != 0
                    || maximum > MAX_INDEX
                    || wire.maximum.is_none_or(|physical| physical > maximum)
            })
            || wire.bytes.0.len() > MAX_BYTES
            || wire
                .maximum
                .is_some_and(|max| max < length || (wire.sparse.is_none() && max > MAX_BYTES))
        {
            return Err(EngineError::invalid("Invalid shared buffer snapshot."));
        }
        let mut bytes = wire.bytes.0;
        if let Some(maximum) = wire.maximum.filter(|_| !restored && wire.sparse.is_none()) {
            bytes
                .try_reserve_exact(maximum - bytes.len())
                .map_err(|_| EngineError::limit("V9 shared buffer bytes", MAX_BYTES))?;
        }
        let (waiters, next_ticket) = asynchronous::restore(&wire.waiters, wire.agent, length)?;
        Ok(Self {
            store: Arc::new(Store {
                id: wire.id,
                state: Mutex::new(State {
                    bytes,
                    sparse: wire.sparse,
                    maximum: wire.maximum,
                    revision: wire.revision,
                    waiters,
                    next_ticket,
                }),
            }),
            restored,
            visible: wire.visible,
            logical_maximum: wire.logical_maximum,
            agent: wire.agent,
        })
    }
    /** Equal revisions must agree byte-for-byte; a newer revision may include legitimate concurrent writes. */
    pub(in crate::v9) fn compatible_snapshot(&self, other: &Self) -> bool {
        if self.same_store(other) {
            return true;
        }
        let a = self.snapshot();
        let b = other.snapshot();
        a.agent == b.agent
            && a.maximum == b.maximum
            && a.sparse.is_some() == b.sparse.is_some()
            && (a.maximum.is_some() || a.bytes.0.len() == b.bytes.0.len())
            && if a.revision == b.revision {
                a.bytes.0 == b.bytes.0 && a.sparse == b.sparse && a.waiters == b.waiters
            } else if a.revision > b.revision {
                a.sparse.as_ref().map_or(a.bytes.0.len(), |s| s.length)
                    >= b.sparse.as_ref().map_or(b.bytes.0.len(), |s| s.length)
            } else {
                a.sparse.as_ref().map_or(a.bytes.0.len(), |s| s.length)
                    <= b.sparse.as_ref().map_or(b.bytes.0.len(), |s| s.length)
            }
    }
    /** Restore without a broker chooses one coherent snapshot, retaining no cross-VM authority. */
    pub(in crate::v9) fn canonical_snapshot_for_agent(&self, agent: u64) -> EngineResult<Self> {
        let mut wire = self.snapshot();
        wire.revision = 0;
        wire.agent = agent;
        Self::from_wire(wire, false)
    }
    /** A snapshot is captured under one lock and never contains wait queues or OS handles. */
    fn snapshot(&self) -> Wire {
        let state = self.lock();
        Wire {
            id: self.identity(),
            revision: state.revision,
            bytes: Bytes(state.bytes.clone()),
            sparse: state.sparse.clone(),
            maximum: state.maximum,
            visible: self.visible,
            logical_maximum: self.logical_maximum,
            agent: self.agent,
            waiters: asynchronous::snapshots(&state, self.agent),
        }
    }
}
impl Serialize for SharedBytes {
    /** Serialization is a self-contained snapshot, independently safe to decode in another process. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.snapshot().serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for SharedBytes {
    /** Native sharing is never recreated from an ID alone. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_wire(Wire::deserialize(deserializer)?, true)
            .map_err(|error| serde::de::Error::custom(error.message))
    }
}
