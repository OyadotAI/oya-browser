/** Temporal's named time zones: IANA rules compiled from the engine's embedded zone source, parsed once and cached per zone. */
use super::zone_rules::{self, RuleTable};
use super::zone_source::{self, Era};
use std::collections::BTreeMap;
use std::sync::OnceLock;

/** The engine's zone source: tzdata's zic rules, zones and links, offsets only, embedded at build time. */
const ZONE_SOURCE: &str = include_str!("zones.zi");
/** Epoch nanoseconds per second, to split an instant into whole seconds. */
const NANOS_PER_SECOND: i128 = 1_000_000_000;

/** The parsed source, built on first use; None only if the embedded source is malformed. */
static INDEX: OnceLock<Option<Index>> = OnceLock::new();

/** Every zone with its lazily computed transitions, and every name (zone or link) in sorted order. */
struct Index {
    /** Zones in source order. */
    zones: Vec<Entry>,
    /** Rule sets shared by all zones. */
    rules: RuleTable,
    /** Lowercase names with their zone slot, sorted for binary search. */
    names: Vec<(String, usize)>,
}

/** A zone's source eras and its transitions, computed on the first lookup that needs them. */
struct Entry {
    /** The canonical IANA name. */
    id: String,
    /** The eras the transitions are built from. */
    eras: Vec<Era>,
    /** (epoch seconds, offset seconds) pairs, computed once. */
    transitions: OnceLock<Vec<(i64, i32)>>,
}

/** Parses the embedded source and indexes zones and links; a link to an unknown zone fails the whole index. */
fn build() -> Option<Index> {
    let source = zone_source::parse(ZONE_SOURCE)?;
    let mut slots = BTreeMap::new();
    let mut zones = Vec::with_capacity(source.zones.len());
    for line in source.zones {
        slots.insert(line.name.clone(), zones.len());
        zones.push(Entry {
            id: line.name,
            eras: line.eras,
            transitions: OnceLock::new(),
        });
    }
    let mut names: Vec<(String, usize)> = zones
        .iter()
        .enumerate()
        .map(|(slot, entry)| (entry.id.to_ascii_lowercase(), slot))
        .collect();
    for (target, alias) in &source.links {
        let slot = *slots.get(target)?;
        names.push((alias.to_ascii_lowercase(), slot));
    }
    names.sort();
    Some(Index {
        zones,
        rules: source.rules,
        names,
    })
}

/** A resolved named zone: the shared index and the zone's slot in it. */
#[derive(Clone, Copy)]
pub(super) struct Zone {
    /** The index that owns the zone's data. */
    index: &'static Index,
    /** The zone's position in the index. */
    slot: usize,
}

/** The shipped zone a name refers to; names match ignoring ASCII case, and links resolve to their zone. */
pub(super) fn lookup(name: &str) -> Option<Zone> {
    let index = INDEX.get_or_init(build).as_ref()?;
    let key = name.to_ascii_lowercase();
    let at = index
        .names
        .binary_search_by(|(candidate, _)| candidate.as_str().cmp(&key))
        .ok()?;
    Some(Zone {
        index,
        slot: index.names[at].1,
    })
}

impl Zone {
    /** The canonical IANA identifier, which Temporal reports even when a link was asked for. */
    pub fn id(&self) -> &'static str {
        &self.index.zones[self.slot].id
    }

    /** The offset in epoch nanoseconds in force at an exact instant. */
    pub fn offset_ns(&self, epoch_ns: i128) -> i128 {
        let seconds = epoch_ns.div_euclid(NANOS_PER_SECOND) as i64;
        i128::from(self.offset_seconds(seconds)) * NANOS_PER_SECOND
    }

    /** The offset in seconds at an epoch second: a binary search over the zone's transitions. */
    fn offset_seconds(&self, epoch_seconds: i64) -> i32 {
        let entry = &self.index.zones[self.slot];
        let transitions = entry
            .transitions
            .get_or_init(|| zone_rules::transitions(&entry.eras, &self.index.rules));
        let at = zone_rules::fold(epoch_seconds);
        let next = transitions.partition_point(|(switch, _)| *switch <= at);
        transitions[next.saturating_sub(1)].1
    }
}
