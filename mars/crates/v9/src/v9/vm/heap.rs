/**
 * The page heap: objects with ordered properties and prototypes, plus captured environments.
 * Live counts are tracked as the heap changes and recomputed exactly by every collection.
 */
use super::super::value::{JsString, JsValue, ObjectId};
use super::data::{Class, Function};
use crate::protocol::{EngineError, EngineResult};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{Error as _, SeqAccess, Visitor},
    ser::{SerializeMap, SerializeSeq},
};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};

/** Environment identities. */
pub(in crate::v9) type EnvId = u32;

/** First identity of intrinsic objects; heap identities stay below it. */
pub(in crate::v9) const INTRINSIC_BASE: ObjectId = 0x4000_0000;
/** First identity of native functions. */
pub(in crate::v9) const NATIVE_BASE: ObjectId = 0x6000_0000;

/** A small multiplicative hasher for integer and short keys; no dependency is needed for it. */
#[derive(Default, Clone, Copy)]
pub(in crate::v9) struct FastHasher(u64);

/** The multiplier from the Fx hash family. */
const FAST_SEED: u64 = 0x51_7c_c1_b7_27_22_0a_95;
/** Bits the final hash rotates by, moving the multiply's strong high bits down. */
const FINISH_ROTATION: u32 = 26;

impl Hasher for FastHasher {
    /** The final hash, rotated: the multiply leaves the low bits weak, and hashbrown picks
    buckets by the low bits, so keys like `$0`..`$zzz` (a page's many globals) collided in long
    probe chains. rustc-hash 2 finishes the same way. */
    fn finish(&self) -> u64 {
        self.0.rotate_left(FINISH_ROTATION)
    }
    /** Mix bytes eight at a time. */
    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            self.add(u64::from_le_bytes(word));
        }
    }
    /** Mix one integer. */
    fn write_u64(&mut self, value: u64) {
        self.add(value);
    }
    /** Mix one integer. */
    fn write_u32(&mut self, value: u32) {
        self.add(u64::from(value));
    }
    /** Mix one integer. */
    fn write_u16(&mut self, value: u16) {
        self.add(u64::from(value));
    }
    /** Mix one integer. */
    fn write_usize(&mut self, value: usize) {
        self.add(value as u64);
    }
}

impl FastHasher {
    /** One mixing round. */
    fn add(&mut self, value: u64) {
        self.0 = (self.0.rotate_left(5) ^ value).wrapping_mul(FAST_SEED);
    }
}

/** Hash maps keyed by small identities. */
pub(in crate::v9) type FastMap<K, V> = HashMap<K, V, BuildHasherDefault<FastHasher>>;
/** A set keyed the same way, for the collector's marks and distinct-string counts. */
pub(in crate::v9) type FastSet<T> = std::collections::HashSet<T, BuildHasherDefault<FastHasher>>;

/** A property key: a string, a symbol, or an engine-private name. */
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub(in crate::v9) enum Key {
    Str(JsString),
    Sym(u32),
    /// Class `#names` and platform-internal slots. No script value converts to this variant
    /// (`to_key` only yields `Str` and `Sym`), so a page can never spell one: any string,
    /// including one that starts with U+FFFF, stays an ordinary `Str` key.
    Private(JsString),
}

impl Key {
    /** A key from ASCII text known to fit. */
    pub fn from(text: &str) -> Self {
        Self::Str(JsString::from_utf8(text).expect("short key"))
    }
    /** Array index keys are canonical numbers below 2^32 - 1. */
    pub fn index(&self) -> Option<u32> {
        let Self::Str(text) = self else {
            return None;
        };
        if text.is_empty() || text.len() > MAX_INDEX_DIGITS {
            return None;
        }
        let mut units = text.code_units();
        let first = units.next()?;
        if first == u16::from(b'0') && text.len() > 1 {
            return None;
        }
        let mut value: u64 = 0;
        for unit in std::iter::once(first).chain(units) {
            let digit = unit.checked_sub(u16::from(b'0')).filter(|d| *d < 10)?;
            value = value * 10 + u64::from(digit);
        }
        (value < u64::from(u32::MAX)).then_some(value as u32)
    }
    /** `"prototype"`, built once per process: `new` reads it on every construction, and
    building the string each time was a quarter of the cost of `new` (C3 note). */
    pub fn prototype() -> Self {
        static KEY: std::sync::OnceLock<Key> = std::sync::OnceLock::new();
        KEY.get_or_init(|| Self::from("prototype")).clone()
    }
    /** `"constructor"`, built once per process for the same reason. */
    pub fn constructor() -> Self {
        static KEY: std::sync::OnceLock<Key> = std::sync::OnceLock::new();
        KEY.get_or_init(|| Self::from("constructor")).clone()
    }
    /** `"length"`, built once per process: array and string paths read it constantly. */
    pub fn length() -> Self {
        static KEY: std::sync::OnceLock<Key> = std::sync::OnceLock::new();
        KEY.get_or_init(|| Self::from("length")).clone()
    }
    /** The one constructor of engine-private keys, for class `#names` and platform slots. */
    pub fn private(name: &str) -> Self {
        Self::Private(JsString::from_utf8(name).expect("short key"))
    }
    /** Private names and internal keys are invisible to reflection. */
    pub fn hidden(&self) -> bool {
        matches!(self, Self::Private(_))
    }
    /** The value form of the key; a private key has none a script may hold, so undefined. */
    pub fn value(&self) -> JsValue {
        match self {
            Self::Str(text) => JsValue::String(text.clone()),
            Self::Sym(id) => JsValue::Symbol(*id),
            Self::Private(_) => JsValue::Undefined,
        }
    }
}

/** Decimal digits of the largest array index. */
const MAX_INDEX_DIGITS: usize = 10;

/** Property attribute bits. */
pub(in crate::v9) const WRITABLE: u8 = 1;
/** Shown by enumeration. */
pub(in crate::v9) const ENUMERABLE: u8 = 2;
/** Deletable and redefinable. */
pub(in crate::v9) const CONFIGURABLE: u8 = 4;
/** Ordinary assignment creates properties with every attribute. */
pub(in crate::v9) const DEFAULT_ATTRS: u8 = WRITABLE | ENUMERABLE | CONFIGURABLE;
/** Builtin methods: writable and configurable but not enumerable. */
pub(in crate::v9) const HIDDEN_ATTRS: u8 = WRITABLE | CONFIGURABLE;

/** A property's storage. */
#[derive(Clone, Debug, PartialEq)]
pub(in crate::v9) enum Slot {
    Data(JsValue),
    Accessor(Option<ObjectId>, Option<ObjectId>),
}

/** One own property. */
#[derive(Clone, Debug, PartialEq)]
pub(in crate::v9) struct Prop {
    pub slot: Slot,
    pub attrs: u8,
}

impl Prop {
    /** A data property with ordinary attributes. */
    pub fn data(value: JsValue) -> Self {
        Self {
            slot: Slot::Data(value),
            attrs: DEFAULT_ATTRS,
        }
    }
    /** String units a data value retains. */
    pub fn units(&self) -> usize {
        match &self.slot {
            Slot::Data(value) => value.string_units(),
            Slot::Accessor(..) => 0,
        }
    }
}

pub(in crate::v9) use super::shape::Props;

/** A captured environment record. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Env {
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<EnvId>,
    #[serde(rename = "s")]
    pub slots: Vec<JsValue>,
    /// The source offset of the class whose scope this is, so each evaluation of that class
    /// mints its own private names (spec 15.7.14 step 6: a fresh Private Name per evaluation).
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub class_site: Option<u32>,
}

/** One heap object. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Object {
    /** Nursery collections promote survivors after a bounded number of actual young sweeps. */
    #[serde(rename = "a", default, skip_serializing_if = "young_age")]
    pub age: u8,
    /** Object creation context owns its intrinsics and keeps detached realm references valid. */
    #[cfg(feature = "conformance")]
    #[serde(rename = "r", default, skip_serializing_if = "realm_zero")]
    pub realm: u32,
    /** Foreign global property tables use the existing aggregate binding quota. */
    #[cfg(feature = "conformance")]
    #[serde(rename = "g", default, skip_serializing_if = "is_false")]
    pub global: bool,
    #[serde(rename = "c")]
    pub class: Class,
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub proto: Option<ObjectId>,
    #[serde(rename = "k", default, skip_serializing_if = "Props::is_empty")]
    pub props: Props,
    #[serde(rename = "x", default = "yes", skip_serializing_if = "is_true")]
    pub extensible: bool,
}

/** Root-owned objects retain the old compact wire form. */
#[cfg(feature = "conformance")]
fn realm_zero(realm: &u32) -> bool {
    *realm == 0
}
/** Ordinary objects do not carry global binding-table metadata. */
#[cfg(feature = "conformance")]
fn is_false(value: &bool) -> bool {
    !*value
}

/** Serde default for extensibility. */
fn yes() -> bool {
    true
}

/** Serde skip test for extensibility. */
fn is_true(value: &bool) -> bool {
    *value
}

/** Newly allocated objects keep the previous compact serialized shape. */
fn young_age(age: &u8) -> bool {
    *age == 0
}

impl Object {
    /** A new ordinary object. */
    pub fn new(class: Class, proto: Option<ObjectId>) -> Self {
        Self {
            age: 0,
            #[cfg(feature = "conformance")]
            realm: super::realm::banks::current(),
            #[cfg(feature = "conformance")]
            global: false,
            class,
            proto,
            props: Props::default(),
            extensible: true,
        }
    }
    /** Properties plus array elements, for the aggregate property budget. */
    pub fn property_count(&self) -> usize {
        self.props.len() + self.class.element_count()
    }
    /** String units retained by the object. */
    pub fn units(&self) -> usize {
        self.props.units() + self.class.units()
    }
    /** Whether the object is callable. */
    pub fn function(&self) -> Option<&Function> {
        match &self.class {
            Class::Function(function) => Some(function),
            _ => None,
        }
    }
}

/** Whether an object's own properties are variable bindings: the global object's are the page's
global vars and functions (an object environment record), and an eval-var object's are the vars a
sloppy direct eval declared. Without this a single `var a,b,...` or repeated eval could declare
bindings past VM_BINDINGS, bounded only by the much larger property budget. */
fn holds_bindings(id: ObjectId, object: &Object) -> bool {
    #[cfg(feature = "conformance")]
    if object.global {
        return true;
    }
    id == crate::constants::VM_GLOBAL_OBJECT_ID || matches!(object.class, Class::EvalVars)
}

/** The bindings an object holds: its own property count when it holds bindings, else none. */
fn binding_props(id: ObjectId, object: &Object) -> usize {
    if holds_bindings(id, object) {
        object.props.len()
    } else {
        0
    }
}

/** Live counts the bounds apply to. */
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::v9) struct Counts {
    pub objects: usize,
    pub functions: usize,
    pub properties: usize,
    pub units: usize,
    pub environments: usize,
    /** Environment slots plus the own properties of binding holders (see `holds_bindings`). */
    pub bindings: usize,
}

/** The heap of one page. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct Heap {
    /** Persistent, so a step's working copy of the heap shares every object it does not write. */
    pub objects: super::trie::Trie<Object>,
    pub envs: super::trie::Trie<Env>,
    pub next_object: ObjectId,
    pub next_env: EnvId,
    pub counts: Counts,
    /** Deduplicated resident string units found at the last GC, including active VM roots. */
    pub last_marked_string_units: usize,
    /** Conservative weakly registered string units, refreshed after cache publication and GC. */
    pub last_registered_string_units: usize,
    /** Non-owning registrations are derived state and are reset for each VM. */
    pub(super) string_budget: super::string_budget::StringBudget,
    /** Wrapper aliases charge native shared allocation once; this derived cache is never serialized. */
    shared_stores: FastMap<(u8, u64), usize>,
    /** Mutable shared graph bytes charged at the last native observation, independently of wrapper count. */
    shared_gc_units: FastMap<u64, usize>,
    /** Objects and environments alive right after the last collection (0: never collected, as
    for a state decoded from a frame); derived, never serialized. */
    pub collected: usize,
    /** The objects a snapshot check must see (shared buffers, shared Wasm roots, atomics poll
    callbacks), kept as objects come and go so the end-of-request checks visit them alone
    instead of walking every object; derived, never serialized. */
    shared: FastSet<ObjectId>,
}

/** Whether an object belongs in the shared index. */
fn shared_candidate(object: &Object) -> bool {
    match &object.class {
        Class::SharedArrayBuffer(_) | Class::WasmShared(_) => true,
        #[cfg(feature = "conformance")]
        Class::SharedContainerRoot(_) => true,
        _ => object
            .function()
            .is_some_and(|function| crate::v9::builtins::is_shared_poll(&function.kind)),
    }
}

impl Heap {
    /** Register a string stored by native code without retaining its allocation. */
    pub(super) fn track_string(&mut self, text: &JsString) {
        self.string_budget.register(text);
        self.last_registered_string_units = self.string_budget.upper_units();
    }
    /** Restore weak registrations for strings already present in a decoded or copied heap. */
    pub(super) fn register_existing_strings(&mut self) {
        let mut strings = Vec::new();
        for object in self.objects.values() {
            for (key, prop) in object.props.iter() {
                if let Key::Str(text) | Key::Private(text) = key {
                    strings.push(text.clone());
                }
                if let Slot::Data(JsValue::String(text)) = &prop.slot {
                    strings.push(text.clone());
                }
            }
            object.class.strings(&mut |value| {
                if let JsValue::String(text) = value {
                    strings.push(text.clone());
                }
            });
            match &object.class {
                Class::String(text) => strings.push(text.clone()),
                Class::RegExp(data) => strings.push(data.source.clone()),
                _ => {}
            }
        }
        for env in self.envs.values() {
            for value in &env.slots {
                if let JsValue::String(text) = value {
                    strings.push(text.clone());
                }
            }
        }
        for text in strings {
            self.track_string(&text);
        }
    }
    /** Primitive strings need registration even when native code writes directly to the heap. */
    pub(in crate::v9) fn track_value(&mut self, value: &JsValue) {
        if let JsValue::String(text) = value {
            self.track_string(text);
        }
    }
    /** Whether the heap grew enough since its last collection to be worth collecting again:
    GC_GROWTH_MIN entries, or a GC_GROWTH_DIVISOR-th of what was alive. Collecting after every
    request of a large app (YouTube's half a million objects) costs a tenth of its time. */
    pub fn grew_since_collection(&self) -> bool {
        use crate::constants::{GC_GROWTH_DIVISOR, GC_GROWTH_MIN};
        let live = self.objects.len() + self.envs.len();
        self.collected == 0
            || live > self.collected + GC_GROWTH_MIN.max(self.collected / GC_GROWTH_DIVISOR)
    }
    /** Store a new object and count it. */
    pub fn insert(&mut self, object: Object) -> EngineResult<ObjectId> {
        let id = self.next_object;
        if id >= INTRINSIC_BASE {
            return Err(EngineError::limit(
                "V9 heap identity",
                INTRINSIC_BASE as usize,
            ));
        }
        self.next_object += 1;
        self.count_object(id, &object, true);
        self.objects.insert(id, object);
        Ok(id)
    }
    /** Store an object under a fixed identity (the global object or a materialized intrinsic). */
    pub fn insert_at(&mut self, id: ObjectId, object: Object) {
        self.count_object(id, &object, true);
        if let Some(old) = self.objects.insert(id, object) {
            self.count_object(id, &old, false);
        }
    }
    /** The indexed shared objects, in no order. */
    pub fn shared_objects(&self) -> impl Iterator<Item = ObjectId> + '_ {
        self.shared.iter().copied()
    }
    /** Drop swept objects from the shared index. */
    pub fn retain_shared(&mut self, live: &FastSet<ObjectId>) {
        self.shared.retain(|id| live.contains(id));
    }
    /** Add or remove an object's contribution to the counts and the shared index. */
    fn count_object(&mut self, id: ObjectId, object: &Object, add: bool) {
        if add {
            for (key, prop) in object.props.iter() {
                if let Key::Str(text) | Key::Private(text) = key {
                    self.track_string(text);
                }
                if let Slot::Data(value) = &prop.slot {
                    self.track_value(value);
                }
            }
            object.class.strings(&mut |value| self.track_value(value));
            match &object.class {
                Class::String(text) => self.track_string(text),
                Class::RegExp(data) => self.track_string(&data.source),
                _ => {}
            }
        }
        if shared_candidate(object) {
            if add {
                self.shared.insert(id);
            } else {
                self.shared.remove(&id);
            }
        }
        let bindings = binding_props(id, object);
        let units = self.counted_units(object, add);
        let (objects, functions) = if object.function().is_some() {
            (0, 1)
        } else {
            (1, 0)
        };
        let counts = &mut self.counts;
        if add {
            counts.objects += objects;
            counts.functions += functions;
            counts.properties += object.property_count();
            counts.units += units;
            counts.bindings += bindings;
        } else {
            counts.objects = counts.objects.saturating_sub(objects);
            counts.functions = counts.functions.saturating_sub(functions);
            counts.properties = counts.properties.saturating_sub(object.property_count());
            counts.units = counts.units.saturating_sub(units);
            counts.bindings = counts.bindings.saturating_sub(bindings);
        }
    }
    /** Strong wrapper counts track backing allocation without multiplying it for fixed Wasm windows. */
    fn counted_units(&mut self, object: &Object, add: bool) -> usize {
        let (identity, units) = match &object.class {
            Class::SharedArrayBuffer(buffer) => {
                ((0, buffer.identity()), buffer.allocated().div_ceil(2))
            }
            Class::WasmShared(value) => {
                ((1, value.arena().identity()), value.allocated().div_ceil(2))
            }
            #[cfg(feature = "conformance")]
            Class::SharedContainerRoot(value) => {
                ((1, value.arena().identity()), value.allocated().div_ceil(2))
            }
            _ => return object.units(),
        };
        let references = self.shared_stores.entry(identity).or_default();
        let charged = if add {
            *references += 1;
            *references == 1
        } else {
            *references = references.saturating_sub(1);
            *references == 0
        };
        let units = if charged && identity.0 == 1 {
            if add {
                self.shared_gc_units.insert(identity.1, units);
                units
            } else {
                self.shared_gc_units.remove(&identity.1).unwrap_or(units)
            }
        } else {
            units
        };
        object.props.units() + if charged { units } else { 0 }
    }
    /** Observe concurrent graph growth once per arena without rescanning every ordinary heap object. */
    pub fn refresh_shared_gc(&mut self, arena: &super::data::SharedGcArena) {
        if let Some(previous) = self.shared_gc_units.get_mut(&arena.identity()) {
            let current = arena.allocated().div_ceil(2);
            self.counts.units = self.counts.units.saturating_sub(*previous) + current;
            *previous = current;
        }
    }
    /** A new environment record. */
    pub fn new_env(&mut self, parent: Option<EnvId>, slots: Vec<JsValue>) -> EngineResult<EnvId> {
        for value in &slots {
            self.track_value(value);
        }
        let id = self.next_env;
        self.next_env = id
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("V9 environment identity", u32::MAX as usize))?;
        self.counts.environments += 1;
        self.counts.bindings += slots.len();
        self.counts.units += slots.iter().map(JsValue::string_units).sum::<usize>();
        self.envs.insert(
            id,
            Env {
                parent,
                slots,
                class_site: None,
            },
        );
        Ok(id)
    }
    /** Read an object. */
    pub fn get(&self, id: ObjectId) -> Option<&Object> {
        self.objects.get(&id)
    }
    /** Write an object. */
    pub fn get_mut(&mut self, id: ObjectId) -> Option<&mut Object> {
        self.objects.get_mut(&id)
    }
    /** Read an environment. */
    pub fn env(&self, id: EnvId) -> Option<&Env> {
        self.envs.get(&id)
    }
    /** Mark an environment as the scope of the class at `site`. */
    pub fn mark_class_env(&mut self, env: EnvId, site: u32) {
        if let Some(record) = self.envs.get_mut(&env) {
            record.class_site = Some(site);
        }
    }
    /** Replace one environment slot, keeping counts. */
    pub fn set_slot(&mut self, env: EnvId, slot: usize, value: JsValue) -> bool {
        self.track_value(&value);
        let Some(record) = self.envs.get_mut(&env) else {
            return false;
        };
        let Some(place) = record.slots.get_mut(slot) else {
            return false;
        };
        let added = value.string_units();
        let removed = place.string_units();
        *place = value;
        self.counts.units = (self.counts.units + added).saturating_sub(removed);
        true
    }
    /** Set an own property, keeping counts; returns the previous property. */
    pub fn put(&mut self, id: ObjectId, key: Key, prop: Prop) -> Option<Prop> {
        if let Key::Str(text) | Key::Private(text) = &key {
            self.track_string(text);
        }
        if let Slot::Data(value) = &prop.slot {
            self.track_value(value);
        }
        let object = self.objects.get_mut(&id)?;
        let binding = usize::from(holds_bindings(id, object));
        let added = prop.units();
        let old = object.props.insert(key, prop);
        let counts = &mut self.counts;
        match &old {
            Some(old) => counts.units = (counts.units + added).saturating_sub(old.units()),
            None => {
                counts.units += added;
                counts.properties += 1;
                counts.bindings += binding;
            }
        }
        old
    }
    /** Remove an own property, keeping counts. */
    pub fn remove(&mut self, id: ObjectId, key: &Key) -> Option<Prop> {
        let object = self.objects.get_mut(&id)?;
        let binding = usize::from(holds_bindings(id, object));
        let old = object.props.remove(key)?;
        self.counts.units = self.counts.units.saturating_sub(old.units());
        self.counts.properties = self.counts.properties.saturating_sub(1);
        self.counts.bindings = self.counts.bindings.saturating_sub(binding);
        Some(old)
    }
    /** Adjust counts after changing an object's internal class data in place. */
    pub fn adjust(&mut self, properties: isize, units: isize) {
        let counts = &mut self.counts;
        counts.properties = counts.properties.saturating_add_signed(properties);
        counts.units = counts.units.saturating_add_signed(units);
    }
    /** Recount everything; collections call this so counts never drift. Units count each
    distinct string content once: references (and the copies a state round trip makes of one
    shared string) are not new text, so runtime, collection and commit agree. The incremental
    counts between collections count every reference and so only ever overestimate. */
    pub fn recount(&mut self) {
        let mut counts = Counts::default();
        self.shared_stores.clear();
        self.shared_gc_units.clear();
        self.shared.clear();
        for (id, object) in &self.objects {
            if shared_candidate(object) {
                self.shared.insert(*id);
            }
            counts.bindings += binding_props(*id, object);
            if object.function().is_some() {
                counts.functions += 1;
            } else {
                counts.objects += 1;
            }
            counts.properties += object.property_count();
            counts.units += match &object.class {
                Class::String(string) => string.len(),
                Class::RegExp(data) => data.source.len(),
                // Buffer bytes count against the same budget, two bytes per UTF-16 unit.
                Class::ArrayBuffer(bytes) => bytes.0.len().div_ceil(2),
                Class::SparseWasmBuffer(bytes) => bytes.resident().div_ceil(2),
                Class::SharedArrayBuffer(bytes) => {
                    let count = self.shared_stores.entry((0, bytes.identity())).or_default();
                    *count += 1;
                    if *count == 1 {
                        bytes.allocated().div_ceil(2)
                    } else {
                        0
                    }
                }
                Class::WasmShared(root) => {
                    let identity = root.arena().identity();
                    let count = self.shared_stores.entry((1, identity)).or_default();
                    *count += 1;
                    if *count == 1 {
                        let units = root.allocated().div_ceil(2);
                        self.shared_gc_units.insert(identity, units);
                        units
                    } else {
                        0
                    }
                }
                #[cfg(feature = "conformance")]
                Class::SharedContainerRoot(root) => {
                    let identity = root.arena().identity();
                    let count = self.shared_stores.entry((1, identity)).or_default();
                    *count += 1;
                    if *count == 1 {
                        let units = root.allocated().div_ceil(2);
                        self.shared_gc_units.insert(identity, units);
                        units
                    } else {
                        0
                    }
                }
                _ => 0,
            };
        }
        counts.environments = self.envs.len();
        counts.bindings += self.envs.values().map(|env| env.slots.len()).sum::<usize>();
        counts.units += self.distinct_units();
        self.counts = counts;
    }
    /** Units of the distinct string contents held in properties, internal data and slots. */
    fn distinct_units<'a>(&'a self) -> usize {
        let mut distinct: FastMap<u64, Vec<&'a JsString>> = FastMap::default();
        let mut bigints: FastSet<&'a [u32]> = FastSet::default();
        let mut text = |value: &'a JsValue| match value {
            JsValue::String(string) => {
                let bucket = distinct.entry(string_content_hash(string)).or_default();
                if !bucket.contains(&string) {
                    bucket.push(string);
                }
            }
            JsValue::BigIntWide(integer) => {
                bigints.insert(integer.words());
            }
            _ => {}
        };
        for object in self.objects.values() {
            for (_, prop) in object.props.iter() {
                if let Slot::Data(value) = &prop.slot {
                    text(value);
                }
            }
            object.class.strings(&mut text);
        }
        self.envs
            .values()
            .flat_map(|env| &env.slots)
            .for_each(&mut text);
        for object in self.objects.values() {
            if let Class::BigIntWide(integer) = &object.class {
                bigints.insert(integer.words());
            }
        }
        distinct
            .values()
            .flatten()
            .map(|string| string.len())
            .sum::<usize>()
            + bigints.iter().map(|words| words.len() * 2).sum::<usize>()
    }
}

/** Hash content without flattening ropes; exact comparisons resolve collisions in the accounting set. */
fn string_content_hash(string: &JsString) -> u64 {
    // Four units per word, and a rope piece by piece: the same content hashes the same
    // whatever its pieces, since the words are cut from the whole string, not from each piece.
    let mut hash = FastHasher::default();
    let mut word = 0u64;
    let mut held = 0u32;
    string.for_each_chunk(|chunk| {
        for unit in chunk {
            word |= u64::from(*unit) << (held * 16);
            held += 1;
            if held == 4 {
                hash.write_u64(word);
                word = 0;
                held = 0;
            }
        }
        true
    });
    if held > 0 {
        hash.write_u64(word | (u64::from(held) << 62));
    }
    hash.finish()
}

impl Serialize for Props {
    /** Entries as `[key, value]`, `[key, value, attrs]` or `[key, [get, set], attrs]`. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len()))?;
        for (key, prop) in self.iter() {
            match &prop.slot {
                Slot::Data(value) if prop.attrs == DEFAULT_ATTRS => {
                    seq.serialize_element(&(key, value))?
                }
                Slot::Data(value) => seq.serialize_element(&(key, value, prop.attrs))?,
                Slot::Accessor(get, set) => {
                    seq.serialize_element(&(key, (get, set), prop.attrs | ACCESSOR_FLAG))?
                }
            }
        }
        seq.end()
    }
}

/** Marks an accessor entry in the wire form. */
const ACCESSOR_FLAG: u8 = 0x80;

impl<'de> Deserialize<'de> for Props {
    /** The inverse of the entry form; duplicate keys fail. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PropsVisitor;
        impl<'de> Visitor<'de> for PropsVisitor {
            type Value = Props;
            /** Error text. */
            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("property entries")
            }
            /** Each entry is a two- or three-element array. */
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Props, A::Error> {
                let mut props = Props::default();
                while let Some(entry) = seq.next_element::<serde_json::Value>()? {
                    let (key, prop) = entry_from(entry).map_err(A::Error::custom)?;
                    if props.insert(key, prop).is_some() {
                        return Err(A::Error::custom("duplicate property"));
                    }
                }
                Ok(props)
            }
        }
        deserializer.deserialize_seq(PropsVisitor)
    }
}

/** Decode one property entry. */
fn entry_from(entry: serde_json::Value) -> Result<(Key, Prop), String> {
    let serde_json::Value::Array(parts) = entry else {
        return Err("property entry must be an array".into());
    };
    let mut parts = parts.into_iter();
    let key: Key =
        serde_json::from_value(parts.next().ok_or("missing key")?).map_err(|e| e.to_string())?;
    let value = parts.next().ok_or("missing value")?;
    let attrs = match parts.next() {
        Some(attrs) => serde_json::from_value::<u8>(attrs).map_err(|e| e.to_string())?,
        None => DEFAULT_ATTRS,
    };
    if parts.next().is_some() {
        return Err("extra property field".into());
    }
    let slot = if attrs & ACCESSOR_FLAG != 0 {
        let (get, set): (Option<ObjectId>, Option<ObjectId>) =
            serde_json::from_value(value).map_err(|e| e.to_string())?;
        Slot::Accessor(get, set)
    } else {
        Slot::Data(serde_json::from_value(value).map_err(|e| e.to_string())?)
    };
    Ok((
        key,
        Prop {
            slot,
            attrs: attrs & !ACCESSOR_FLAG,
        },
    ))
}

impl Serialize for Key {
    /** Keys serialize as their value form; a private key as a one-key `p` map, which no value
    form uses, so a restored string never turns into a private key. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Private(name) => {
                let mut map = serializer.serialize_map(Some(1))?;
                map.serialize_entry("p", name.as_units())?;
                map.end()
            }
            _ => self.value().serialize(serializer),
        }
    }
}

/** The wire form of a key: a private `p` map or a value. */
#[derive(Deserialize)]
#[serde(untagged)]
enum KeyWire {
    Private { p: Vec<u16> },
    Value(JsValue),
}

impl<'de> Deserialize<'de> for Key {
    /** Strings, symbols and private names only. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match KeyWire::deserialize(deserializer)? {
            KeyWire::Private { p } => JsString::from_units(p)
                .map(Key::Private)
                .map_err(|error| D::Error::custom(error.message)),
            KeyWire::Value(JsValue::String(text)) => Ok(Key::Str(text)),
            KeyWire::Value(JsValue::Symbol(id)) => Ok(Key::Sym(id)),
            KeyWire::Value(_) => Err(D::Error::custom("invalid property key")),
        }
    }
}

impl Serialize for Heap {
    /** Objects and environments as sorted lists so snapshots are deterministic. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut objects: Vec<(&ObjectId, &Object)> = self.objects.iter().collect();
        objects.sort_by_key(|(id, _)| **id);
        let mut envs: Vec<(&EnvId, &Env)> = self.envs.iter().collect();
        envs.sort_by_key(|(id, _)| **id);
        (objects, envs, self.next_object, self.next_env).serialize(serializer)
    }
}

/** The heap's wire shape: objects, environments and the two identity counters. */
type HeapWire = (Vec<(ObjectId, Object)>, Vec<(EnvId, Env)>, ObjectId, EnvId);

impl<'de> Deserialize<'de> for Heap {
    /** Rebuild maps and counts. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (objects, envs, next_object, next_env): HeapWire =
            Deserialize::deserialize(deserializer)?;
        let mut heap = Heap {
            objects: objects.into_iter().collect(),
            envs: envs.into_iter().collect(),
            next_object,
            next_env,
            counts: Counts::default(),
            last_marked_string_units: 0,
            last_registered_string_units: 0,
            string_budget: super::string_budget::StringBudget::default(),
            shared_stores: FastMap::default(),
            shared_gc_units: FastMap::default(),
            collected: 0,
            shared: FastSet::default(),
        };
        heap.recount();
        Ok(heap)
    }
}
