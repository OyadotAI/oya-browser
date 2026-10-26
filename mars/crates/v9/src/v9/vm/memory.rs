/** Post-GC memory estimates count live language data, excluding process caches, allocator metadata and RSS. */
mod storage;
use super::{
    Vm,
    heap::{Env, EnvId, Key, Object, Slot},
};
use crate::v9::value::{JsString, JsValue};
use std::collections::{BTreeMap, BTreeSet};
use std::mem::size_of;

/** Ranges retain exact resident capacities where available and bound opaque B-tree allocation slack. */
#[derive(Clone, Copy, Default, Debug)]
pub(in crate::v9) struct BytesRange {
    pub lower: usize,
    pub upper: usize,
}
impl BytesRange {
    /** Fixed resident allocations contribute equally to both ends of the estimate. */
    pub(super) fn exact(bytes: usize) -> Self {
        Self {
            lower: bytes,
            upper: bytes,
        }
    }
    /** Capacity uncertainty never changes the underlying byte accounting units. */
    pub(super) fn add(&mut self, other: Self) {
        self.lower = self.lower.saturating_add(other.lower);
        self.upper = self.upper.saturating_add(other.upper);
    }
}
/** Result contexts share the caller's security token and each retain at least one reachable context root. */
pub(in crate::v9) struct MemoryEstimate {
    pub total: BytesRange,
    pub current: BytesRange,
    pub other: Vec<BytesRange>,
}
/** Native shared allocations have opaque identities; no pointer is ever exposed to JavaScript. */
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Allocation {
    CpuProfile,
    String(usize),
    StringNode(usize),
    BigInt(usize),
    Shared(u64),
    Environment(EnvId),
}
/** Multiple contexts may retain one allocation; current/other ranges reflect that attribution uncertainty. */
struct Shared {
    bytes: BytesRange,
    owners: BTreeSet<u32>,
}
/** Only selected contexts enter the tracker, so hidden-token allocations cannot affect its output. */
struct Tracker {
    realms: BTreeMap<u32, BytesRange>,
    shared: BTreeMap<Allocation, Shared>,
    environments: BTreeMap<EnvId, BTreeSet<u32>>,
}
impl Tracker {
    /** A fixed object allocation is owned by its creation context. */
    fn owned(&mut self, bank: u32, bytes: BytesRange) {
        self.realms.get_mut(&bank).unwrap().add(bytes);
    }
    /** Shared payloads are counted once in total and attributed only when one measured context owns them. */
    fn allocation(&mut self, bank: u32, id: Allocation, bytes: BytesRange) {
        self.shared
            .entry(id)
            .or_insert_with(|| Shared {
                bytes,
                owners: BTreeSet::new(),
            })
            .owners
            .insert(bank);
    }
    /** Flat buffers and concat nodes retain actual allocation sizes without forcing materialization. */
    fn string(&mut self, bank: u32, text: &JsString) {
        text.storage_allocations(|node, id, lower, upper| {
            let id = if node {
                Allocation::StringNode(id)
            } else {
                Allocation::String(id)
            };
            self.allocation(bank, id, BytesRange { lower, upper });
        });
    }
    /** Symbol descriptions are language strings too; object edges are already accounted by the live heap walk. */
    fn value(&mut self, vm: &Vm, bank: u32, value: &JsValue) {
        match value {
            JsValue::String(text) => self.string(bank, text),
            JsValue::BigIntWide(value) => self.bigint(bank, value),
            JsValue::Symbol(id) => {
                if let Some(text) = id
                    .checked_sub(super::realm::symbol::WELL_KNOWN)
                    .and_then(|id| vm.symbols.descriptions.get(id as usize))
                    .and_then(Option::as_ref)
                {
                    self.string(bank, text);
                }
            }
            _ => {}
        }
    }
    /** Wide BigInts share immutable limb storage, counted once regardless of wrapper and primitive aliases. */
    fn bigint(&mut self, bank: u32, value: &crate::v9::value::BigInteger) {
        let words = value.words();
        if !words.is_empty() {
            self.allocation(
                bank,
                Allocation::BigInt(words.as_ptr() as usize),
                BytesRange::exact(words.len() * size_of::<u32>() + 2 * size_of::<usize>()),
            );
        }
    }
    /** One captured environment may be shared by several closures or contexts. */
    fn environment(&mut self, vm: &Vm, bank: u32, mut id: Option<EnvId>) {
        while let Some(current) = id {
            if !self.environments.entry(current).or_default().insert(bank) {
                break;
            }
            let Some(environment) = vm.heap.env(current) else {
                break;
            };
            id = environment.parent;
        }
    }
    /** Payload sharing contributes once to total and as a bounded uncertainty to each possible owner. */
    fn finish(mut self, current: u32) -> MemoryEstimate {
        let mut total = BytesRange::default();
        for shared in self.shared.values() {
            if shared.owners.len() == 1 {
                self.realms
                    .get_mut(shared.owners.first().unwrap())
                    .unwrap()
                    .add(shared.bytes);
            } else {
                total.add(shared.bytes);
                for owner in &shared.owners {
                    self.realms.get_mut(owner).unwrap().upper =
                        self.realms[owner].upper.saturating_add(shared.bytes.upper);
                }
            }
        }
        for (bank, bytes) in &self.realms {
            let mut exclusive = *bytes;
            for shared in self.shared.values().filter(|allocation| {
                allocation.owners.len() > 1 && allocation.owners.contains(bank)
            }) {
                exclusive.upper = exclusive.upper.saturating_sub(shared.bytes.upper);
            }
            total.add(exclusive);
        }
        let current = self.realms.remove(&current).unwrap_or_default();
        MemoryEstimate {
            total,
            current,
            other: self.realms.into_values().collect(),
        }
    }
}

impl Vm<'_> {
    /** Measure only post-collection live objects and context-owned bindings, without running author getters. */
    pub(in crate::v9) fn memory_estimate(&self) -> MemoryEstimate {
        let token = self.realms.contexts[self.active_realm as usize].token;
        let realms = self
            .realms
            .contexts
            .iter()
            .enumerate()
            .filter(|(_, context)| context.global.is_some() && context.token == token)
            .map(|(bank, _)| (bank as u32, BytesRange::default()))
            .collect();
        let mut tracker = Tracker {
            realms,
            shared: BTreeMap::new(),
            environments: BTreeMap::new(),
        };
        // One isolate profile is shared native data, not separate fabricated allocations per realm.
        let profile_bytes = self.page.state.cpu_profile.units() * size_of::<u16>();
        if profile_bytes != 0 {
            for bank in tracker.realms.keys().copied().collect::<Vec<_>>() {
                tracker.allocation(
                    bank,
                    Allocation::CpuProfile,
                    BytesRange {
                        lower: profile_bytes.saturating_sub(1),
                        upper: profile_bytes,
                    },
                );
            }
        }
        for (id, object) in &self.heap.objects {
            let bank = self.object_realm(*id);
            if !tracker.realms.contains_key(&bank) {
                continue;
            }
            let (lower, upper) = object.props.memory_bounds();
            let mut bytes = BytesRange { lower, upper };
            bytes.add(BytesRange::exact(size_of::<Object>()));
            bytes.add(storage::class(self, &object.class, bank, &mut tracker));
            tracker.owned(bank, bytes);
            for (key, property) in object.props.iter() {
                if let Key::Str(name) | Key::Private(name) = key {
                    tracker.string(bank, name);
                }
                if let Slot::Data(value) = &property.slot {
                    tracker.value(self, bank, value);
                }
            }
            storage::environments(self, &object.class, bank, &mut tracker);
        }
        let environments = std::mem::take(&mut tracker.environments);
        for (id, owners) in environments {
            if let Some(environment) = self.heap.env(id) {
                for owner in owners {
                    tracker.allocation(
                        owner,
                        Allocation::Environment(id),
                        BytesRange::exact(
                            size_of::<Env>() + environment.slots.capacity() * size_of::<JsValue>(),
                        ),
                    );
                    for value in &environment.slots {
                        tracker.value(self, owner, value);
                    }
                }
            }
        }
        for bank in tracker.realms.keys().copied().collect::<Vec<_>>() {
            let globals = if bank == self.active_realm {
                &self.globals
            } else {
                &self.realms.contexts[bank as usize].globals
            };
            tracker.owned(
                bank,
                storage::tree_bytes::<(String, crate::v9::binding::Binding)>(globals.len()),
            );
            for (name, binding) in globals {
                tracker.owned(bank, BytesRange::exact(name.capacity()));
                if let Some(value) = &binding.value {
                    tracker.value(self, bank, value);
                }
            }
        }
        tracker.finish(self.active_realm)
    }
}

/** Property dictionaries reuse the same bounded bucket estimate as language Map indexes. */
pub(super) fn hash_index_bounds<T>(capacity: usize) -> (usize, usize) {
    let bytes = storage::hash_bytes::<T>(capacity);
    (bytes.lower, bytes.upper)
}
