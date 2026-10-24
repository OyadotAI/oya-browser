/** Internal object kinds: what makes an object an array, function, promise, iterator or host wrapper. */
use super::super::value::{JsString, JsValue, ObjectId};
use super::heap::{EnvId, FastMap, Key};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/** Numbers that may be NaN or infinite, serialized through the value codec. */
mod num {
    use super::*;
    /** Serialize a number as a value. */
    pub fn serialize<S: Serializer>(value: &f64, serializer: S) -> Result<S::Ok, S::Error> {
        JsValue::Number(*value).serialize(serializer)
    }
    /** Deserialize a number value. */
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
        match JsValue::deserialize(deserializer)? {
            JsValue::Number(value) => Ok(value),
            _ => Err(serde::de::Error::custom("expected a number")),
        }
    }
}

/** BigInt wrapper data on the wire, in the BigInt value's own compact form. */
mod big {
    use super::*;
    /** Serialize a BigInt as a value. */
    pub fn serialize<S: Serializer>(value: &i128, serializer: S) -> Result<S::Ok, S::Error> {
        JsValue::BigInt(*value).serialize(serializer)
    }
    /** Deserialize a BigInt value. */
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<i128, D::Error> {
        match JsValue::deserialize(deserializer)? {
            JsValue::BigInt(value) => Ok(value),
            _ => Err(serde::de::Error::custom("expected a bigint")),
        }
    }
}

/** What kind of object this is. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum Class {
    Plain,
    /** Dense elements; `Hole` marks missing ones. */
    Array(Vec<JsValue>),
    Function(Box<Function>),
    Error,
    Boolean(bool),
    Number(#[serde(with = "num")] f64),
    String(JsString),
    Symbol(u32),
    /** A BigInt wrapper object (Object(1n)). */
    BigInt(#[serde(with = "big")] i128),
    Date(#[serde(with = "num")] f64),
    RegExp(Box<RegExpData>),
    Map(Box<Table>),
    Set(Box<Table>),
    WeakMap(Box<Table>),
    WeakSet(Box<Table>),
    Promise(Box<PromiseData>),
    Arguments,
    Generator(Box<GeneratorData>),
    Async(Box<AsyncData>),
    Iter(Box<IterState>),
    Host(Box<HostData>),
    /** An ArrayBuffer's bytes. */
    ArrayBuffer(Bytes),
    /** A typed array or DataView over an ArrayBuffer. */
    View(Box<ViewData>),
    /** A Proxy; both fields are None once revoked. */
    Proxy(Box<ProxyData>),
    /** A module namespace: its exports read the named module's bindings live. */
    Namespace(String),
}

/** A Proxy's target and handler. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct ProxyData {
    pub target: Option<ObjectId>,
    pub handler: Option<ObjectId>,
}

/** Raw ArrayBuffer bytes; hex text on the wire so committed state stays compact. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct Bytes(pub Vec<u8>);

impl Serialize for Bytes {
    /** Two lowercase hex digits per byte. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let text: String = self.0.iter().map(|byte| format!("{byte:02x}")).collect();
        serializer.serialize_str(&text)
    }
}

impl<'de> Deserialize<'de> for Bytes {
    /** The inverse of the hex form; anything else fails. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        if text.len() % 2 != 0 || !text.is_ascii() {
            return Err(serde::de::Error::custom("invalid buffer bytes"));
        }
        (0..text.len())
            .step_by(2)
            .map(|at| u8::from_str_radix(&text[at..at + 2], 16))
            .collect::<Result<Vec<u8>, _>>()
            .map(Bytes)
            .map_err(|_| serde::de::Error::custom("invalid buffer bytes"))
    }
}

/** What a view's elements are. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum ElementKind {
    Int8,
    Uint8,
    Uint8Clamped,
    Int16,
    Uint16,
    Int32,
    Uint32,
    Float32,
    Float64,
    BigInt64,
    BigUint64,
    /** A DataView: bytes read and written by explicit typed accessors. */
    DataView,
}

/** A view over part of an ArrayBuffer: `length` elements (bytes for a DataView) from `offset`. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct ViewData {
    pub kind: ElementKind,
    pub buffer: ObjectId,
    pub offset: usize,
    pub length: usize,
}

impl Class {
    /** Elements and entries count toward the property budget. */
    pub fn element_count(&self) -> usize {
        match self {
            Class::Array(elements) => elements
                .iter()
                .filter(|value| **value != JsValue::Hole)
                .count(),
            Class::Map(table)
            | Class::Set(table)
            | Class::WeakMap(table)
            | Class::WeakSet(table) => table.live,
            _ => 0,
        }
    }
    /** Every string value held inside internal data, so a recount can see shared contents. */
    pub fn strings<'a>(&'a self, visit: &mut dyn FnMut(&'a JsValue)) {
        match self {
            Class::Array(elements) => elements.iter().for_each(visit),
            Class::Map(table)
            | Class::Set(table)
            | Class::WeakMap(table)
            | Class::WeakSet(table) => {
                for (key, value) in table.entries.iter().flatten() {
                    visit(key);
                    visit(value);
                }
            }
            _ => {}
        }
    }
    /** String units held inside internal data. */
    pub fn units(&self) -> usize {
        match self {
            Class::Array(elements) => elements.iter().map(JsValue::string_units).sum(),
            Class::String(text) => text.len(),
            Class::Map(table)
            | Class::Set(table)
            | Class::WeakMap(table)
            | Class::WeakSet(table) => table
                .entries
                .iter()
                .flatten()
                .map(|(key, value)| key.string_units() + value.string_units())
                .sum(),
            Class::RegExp(data) => data.source.len(),
            Class::ArrayBuffer(bytes) => bytes.0.len().div_ceil(2),
            _ => 0,
        }
    }
}

/** A callable object's behavior. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Function {
    #[serde(rename = "f")]
    pub kind: FunctionKind,
    /** The object `super` looks up from, for methods. */
    #[serde(rename = "h", default, skip_serializing_if = "Option::is_none")]
    pub home: Option<ObjectId>,
    /** A class constructor's instance field initializer. */
    #[serde(rename = "i", default, skip_serializing_if = "Option::is_none")]
    pub fields: Option<ObjectId>,
}

/** Closures name source code; natives name a builtin; bound functions wrap a target. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum FunctionKind {
    Closure {
        site: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env: Option<EnvId>,
    },
    /** A builtin with captured values, such as a promise's resolve function. */
    Native { id: u32, slots: Vec<JsValue> },
    Bound {
        target: ObjectId,
        this: JsValue,
        args: Vec<JsValue>,
    },
}

/** A regular expression's source and flags; the compiled program is cached per request. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct RegExpData {
    pub source: JsString,
    pub flags: String,
}

/** A hashable form of a value under SameValueZero. */
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(in crate::v9) enum TableKey {
    Undefined,
    Null,
    Bool(bool),
    Number(u64),
    String(JsString),
    Object(ObjectId),
    Symbol(u32),
    /** BigInts are keyed by value, as SameValueZero compares them. */
    BigInt(i128),
}

impl TableKey {
    /** The key for a value; -0 and +0 match, as do all NaNs. */
    pub fn of(value: &JsValue) -> Self {
        match value {
            JsValue::Undefined | JsValue::Hole => Self::Undefined,
            JsValue::Null => Self::Null,
            JsValue::Boolean(value) => Self::Bool(*value),
            JsValue::Number(value) => Self::Number(if *value == 0.0 {
                0
            } else if value.is_nan() {
                f64::NAN.to_bits()
            } else {
                value.to_bits()
            }),
            JsValue::String(text) => Self::String(text.clone()),
            JsValue::BigInt(value) => Self::BigInt(*value),
            JsValue::Object(id) => Self::Object(*id),
            JsValue::Symbol(id) => Self::Symbol(*id),
        }
    }
}

/** Map and Set storage: insertion-ordered entries with stable positions for live iterators. */
#[derive(Clone, Debug, Default)]
pub(in crate::v9) struct Table {
    pub entries: Vec<Option<(JsValue, JsValue)>>,
    pub index: FastMap<TableKey, usize>,
    pub live: usize,
}

impl Table {
    /** The value for a key. */
    pub fn get(&self, key: &JsValue) -> Option<&JsValue> {
        let position = *self.index.get(&TableKey::of(key))?;
        self.entries[position].as_ref().map(|(_, value)| value)
    }
    /** Whether a key is present. */
    pub fn has(&self, key: &JsValue) -> bool {
        self.index.contains_key(&TableKey::of(key))
    }
    /** Insert or replace; returns true when a new entry was added. */
    pub fn set(&mut self, key: JsValue, value: JsValue) -> bool {
        let key = if matches!(key, JsValue::Number(n) if n == 0.0) {
            JsValue::Number(0.0)
        } else {
            key
        };
        let hash = TableKey::of(&key);
        if let Some(position) = self.index.get(&hash) {
            self.entries[*position] = Some((key, value));
            return false;
        }
        self.index.insert(hash, self.entries.len());
        self.entries.push(Some((key, value)));
        self.live += 1;
        true
    }
    /** Remove a key; returns the removed entry. */
    pub fn delete(&mut self, key: &JsValue) -> Option<(JsValue, JsValue)> {
        let position = self.index.remove(&TableKey::of(key))?;
        self.live -= 1;
        self.entries[position].take()
    }
    /** Remove every entry, leaving positions so live iterators end. */
    pub fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = None;
        }
        self.index.clear();
        self.live = 0;
    }
}

impl Serialize for Table {
    /** Only live entries cross the wire. */
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let live: Vec<&(JsValue, JsValue)> = self.entries.iter().flatten().collect();
        live.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Table {
    /** Rebuild the index; duplicates fail. */
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let entries: Vec<(JsValue, JsValue)> = Deserialize::deserialize(deserializer)?;
        let mut table = Table::default();
        for (key, value) in entries {
            if !table.set(key, value) {
                return Err(serde::de::Error::custom("duplicate table key"));
            }
        }
        Ok(table)
    }
}

/** A promise's settlement and waiting reactions. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct PromiseData {
    pub state: PromiseState,
    pub reactions: Vec<Reaction>,
    pub handled: bool,
}

/** Pending, or settled with a value. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum PromiseState {
    Pending,
    Fulfilled(JsValue),
    Rejected(JsValue),
}

/** Work to run when a promise settles. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Reaction {
    /** True for the rejection branch. */
    pub reject: bool,
    pub handler: ReactionHandler,
    /** The derived promise to settle with the handler's outcome. */
    pub target: Option<ObjectId>,
}

/** What a reaction runs. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum ReactionHandler {
    /** A script function, or undefined for pass-through. */
    Function(JsValue),
    /** Resume a suspended async function. */
    Resume(ObjectId),
}

/** A queued microtask. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum Job {
    /** Run a reaction with the settled value. */
    Reaction {
        reaction: Reaction,
        argument: JsValue,
    },
    /** Adopt a thenable's state into a promise. */
    Thenable {
        promise: ObjectId,
        thenable: JsValue,
        then: JsValue,
    },
    /** queueMicrotask. */
    Callback(JsValue),
}

/** One exception handler in a frame. */
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct Handler {
    pub catch: u32,
    pub finally: u32,
    pub slot: u16,
    pub stack: u32,
    pub env: Option<EnvId>,
    pub envs: u16,
}

/** A suspended frame of a generator or async function. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct SavedFrame {
    pub site: u32,
    pub pc: u32,
    pub stack: Vec<JsValue>,
    pub regs: Vec<JsValue>,
    pub env: Option<EnvId>,
    pub this: JsValue,
    pub callee: JsValue,
    pub args: Vec<JsValue>,
    pub new_target: JsValue,
    pub handlers: Vec<Handler>,
    pub envs: u16,
}

/** Generator lifecycle. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum GeneratorState {
    Start,
    Yielded,
    Running,
    Done,
}

/** A generator object's suspended body. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct GeneratorData {
    pub state: GeneratorState,
    pub frame: Option<SavedFrame>,
}

/** A suspended async function and the promise it settles. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct AsyncData {
    pub frame: Option<SavedFrame>,
    pub promise: ObjectId,
}

/** Which values an array, map or set iterator produces. */
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum IterKind {
    Keys,
    Values,
    Entries,
}

/** Iterator state: native iterators step without calling script code. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum IterState {
    Array {
        object: JsValue,
        index: u32,
        kind: IterKind,
        done: bool,
    },
    String {
        text: JsString,
        index: u32,
        done: bool,
    },
    Table {
        table: ObjectId,
        index: u32,
        kind: IterKind,
        done: bool,
    },
    ForIn {
        object: JsValue,
        keys: Vec<Key>,
        index: u32,
    },
    /** A script iterator and its cached next method. */
    Record {
        iterator: JsValue,
        next: JsValue,
        done: bool,
    },
    /** matchAll results. */
    Matches {
        regexp: ObjectId,
        text: JsString,
        global: bool,
        unicode: bool,
        done: bool,
    },
}

/** Script-visible wrappers of host things. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(in crate::v9) enum HostData {
    /** An element, text node or the document's virtual html/head/body. */
    Node(usize),
    Event(Box<EventData>),
    Style(usize),
    /** getComputedStyle: inline values over CSS defaults, read-only. */
    Computed(usize),
    ClassList(usize),
    Dataset(usize),
    /** A static list of nodes. */
    NodeList(Vec<usize>),
    /** A live child or element list of a node. */
    Children(usize, bool),
    Xhr(u32),
    Response(Box<ResponseData>),
    Headers(Vec<(String, String)>),
    /** A parsed URL. */
    Url(String),
    SearchParams(Vec<(String, String)>),
    /** localStorage (true) or sessionStorage. */
    Storage(bool),
    /** A virtual document made by createHTMLDocument; its body is a detached node. */
    Document(usize),
    /** CustomEvent, Event and friends created by script. */
    Observer,
    /** A Range: two boundary points over node wrappers. */
    Range(Box<RangeData>),
    /** The page's Selection: the Range objects it holds (at most one, as in browsers). */
    Selection(Vec<JsValue>),
    /** An Intl formatter: its kind, locale and the options it was built with. */
    Intl(Box<IntlData>),
    /** A CSSStyleSheet: the style or stylesheet link element it belongs to. */
    StyleSheet(usize),
    /** A DOMTokenList over an attribute other than `class` (`sandbox`, `rel`, `part`). */
    TokenList(usize, String),
    /** A fetch Request: the request `fetch(request)` sends. */
    Request(Box<crate::v9::platform::HostRequest>),
}

/** What an Intl formatter keeps: its constructor's name, the first requested locale, and the
options the page passed, as strings. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct IntlData {
    pub kind: String,
    pub locale: String,
    pub options: std::collections::BTreeMap<String, String>,
}

/** A Range's boundary points; each holds the container's wrapper, which follows the node when
insertion renumbers it, and an offset into its children or text units. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct RangeData {
    pub start: JsValue,
    pub start_offset: usize,
    pub end: JsValue,
    pub end_offset: usize,
}

/** A dispatched or script-created event. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct EventData {
    pub kind: String,
    pub target: JsValue,
    pub current: JsValue,
    pub bubbles: bool,
    pub cancelable: bool,
    pub default_prevented: bool,
    pub stopped: bool,
    pub immediate: bool,
    pub phase: u8,
    pub detail: JsValue,
    pub trusted: bool,
}

/** A fetch response body and metadata. */
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(in crate::v9) struct ResponseData {
    pub status: u16,
    pub status_text: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub used: bool,
}
