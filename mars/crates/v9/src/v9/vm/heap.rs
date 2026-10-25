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

impl Hasher for FastHasher {
    /** The final hash. */
    fn finish(&self) -> u64 {
        self.0
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
        let units = text.as_units();
        if units.is_empty()
            || units.len() > MAX_INDEX_DIGITS
            || units[0] == u16::from(b'0') && units.len() > 1
        {
            return None;
        }
        let mut value: u64 = 0;
        for unit in units {
            let digit = unit.checked_sub(u16::from(b'0')).filter(|d| *d < 10)?;
            value = value * 10 + u64::from(digit);
        }
        (value < u64::from(u32::MAX)).then_some(value as u32)
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
}

/** One heap object. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Object {
    #[serde(rename = "c")]
    pub class: Class,
    #[serde(rename = "p", default, skip_serializing_if = "Option::is_none")]
    pub proto: Option<ObjectId>,
    #[serde(rename = "k", default, skip_serializing_if = "Props::is_empty")]
    pub props: Props,
    #[serde(rename = "x", default = "yes", skip_serializing_if = "is_true")]
    pub extensible: bool,
}

/** Serde default for extensibility. */
fn yes() -> bool {
    true
}

/** Serde skip test for extensibility. */
fn is_true(value: &bool) -> bool {
    *value
}

impl Object {
    /** A new ordinary object. */
    pub fn new(class: Class, proto: Option<ObjectId>) -> Self {
        Self {
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
    pub objects: FastMap<ObjectId, Object>,
    pub envs: FastMap<EnvId, Env>,
    pub next_object: ObjectId,
    pub next_env: EnvId,
    pub counts: Counts,
}

impl Heap {
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
    /** Add or remove an object's contribution to the counts. */
    fn count_object(&mut self, id: ObjectId, object: &Object, add: bool) {
        let bindings = binding_props(id, object);
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
            counts.units += object.units();
            counts.bindings += bindings;
        } else {
            counts.objects = counts.objects.saturating_sub(objects);
            counts.functions = counts.functions.saturating_sub(functions);
            counts.properties = counts.properties.saturating_sub(object.property_count());
            counts.units = counts.units.saturating_sub(object.units());
            counts.bindings = counts.bindings.saturating_sub(bindings);
        }
    }
    /** A new environment record. */
    pub fn new_env(&mut self, parent: Option<EnvId>, slots: Vec<JsValue>) -> EngineResult<EnvId> {
        let id = self.next_env;
        self.next_env = id
            .checked_add(1)
            .ok_or_else(|| EngineError::limit("V9 environment identity", u32::MAX as usize))?;
        self.counts.environments += 1;
        self.counts.bindings += slots.len();
        self.counts.units += slots.iter().map(JsValue::string_units).sum::<usize>();
        self.envs.insert(id, Env { parent, slots });
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
    /** Replace one environment slot, keeping counts. */
    pub fn set_slot(&mut self, env: EnvId, slot: usize, value: JsValue) -> bool {
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
        for (id, object) in &self.objects {
            counts.bindings += binding_props(*id, object);
            if object.function().is_some() {
                counts.functions += 1;
            } else {
                counts.objects += 1;
            }
            counts.properties += object.property_count();
            // Boxed strings and regexp sources are internal copies, counted as before.
            counts.units += match &object.class {
                Class::String(string) => string.len(),
                Class::RegExp(data) => data.source.len(),
                // Buffer bytes count against the same budget, two bytes per UTF-16 unit.
                Class::ArrayBuffer(bytes) => bytes.0.len().div_ceil(2),
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
        let mut distinct: FastSet<&'a [u16]> = FastSet::default();
        let mut text = |value: &'a JsValue| {
            if let JsValue::String(string) = value {
                distinct.insert(string.as_units());
            }
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
        distinct.iter().map(|units| units.len()).sum()
    }
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
        };
        heap.recount();
        Ok(heap)
    }
}
