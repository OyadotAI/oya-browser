/**
 * The intrinsic realm: builtin prototypes, constructors and native functions, built once per
 * process. Intrinsics are virtual objects outside the page heap, so they cost no heap budget and
 * never serialize; a script that writes to one gets a heap copy under the same identity.
 */
use super::super::value::{JsString, JsValue, ObjectId};
use super::heap::{FastMap, HIDDEN_ATTRS, INTRINSIC_BASE, Key, NATIVE_BASE};
use super::{Call, Res, Vm};
use std::sync::OnceLock;
#[cfg(feature = "conformance")]
pub(in crate::v9) mod banks;

/** A builtin function body. */
pub(in crate::v9) type NativeFn = fn(&mut Vm, Call) -> Res<JsValue>;

/** One builtin function. */
#[derive(Clone)]
pub(in crate::v9) struct NativeDef {
    pub name: &'static str,
    /** Display metadata is separate from the stable native dispatch key. */
    pub display_name: Option<String>,
    pub length: u32,
    pub function: NativeFn,
    /** Callable with `new`. */
    pub constructor: bool,
    /** Embedder API entries record activations only under the shell flag, never as builtins. */
    pub host: bool,
}

/** An intrinsic property's static value. */
#[derive(Clone)]
pub(in crate::v9) enum Static {
    Value(JsValue),
    Accessor(Option<ObjectId>, Option<ObjectId>),
}

/** One intrinsic object. */
pub(in crate::v9) struct IntrinsicDef {
    pub proto: Option<ObjectId>,
    pub props: Vec<(Key, Static, u8)>,
    pub index: FastMap<Key, usize>,
    /** Constructors and other callable intrinsics. */
    pub call: Option<u32>,
}

impl IntrinsicDef {
    /** Look up a static property. */
    pub fn get(&self, key: &Key) -> Option<&(Key, Static, u8)> {
        self.index.get(key).map(|index| &self.props[*index])
    }
}

/** Well-known symbol identities; user symbols start after them. Some exist only so identities
stay in specification order. */
#[allow(dead_code)]
pub(in crate::v9) mod symbol {
    pub const ITERATOR: u32 = 0;
    pub const ASYNC_ITERATOR: u32 = 1;
    pub const HAS_INSTANCE: u32 = 2;
    pub const TO_PRIMITIVE: u32 = 3;
    pub const TO_STRING_TAG: u32 = 4;
    pub const SPECIES: u32 = 5;
    pub const IS_CONCAT_SPREADABLE: u32 = 6;
    pub const UNSCOPABLES: u32 = 7;
    pub const MATCH: u32 = 8;
    pub const REPLACE: u32 = 9;
    pub const SEARCH: u32 = 10;
    pub const SPLIT: u32 = 11;
    pub const MATCH_ALL: u32 = 12;
    /** Count of well-known symbols. */
    /** Resource cleanup method identity, shared across intrinsic banks. */
    pub const DISPOSE: u32 = 13;
    /** Asynchronous cleanup method identity. */
    pub const ASYNC_DISPOSE: u32 = 14;
    pub const WELL_KNOWN: u32 = 15;
    /** Their descriptions, in identity order. */
    pub const NAMES: [&str; WELL_KNOWN as usize] = [
        "Symbol.iterator",
        "Symbol.asyncIterator",
        "Symbol.hasInstance",
        "Symbol.toPrimitive",
        "Symbol.toStringTag",
        "Symbol.species",
        "Symbol.isConcatSpreadable",
        "Symbol.unscopables",
        "Symbol.match",
        "Symbol.replace",
        "Symbol.search",
        "Symbol.split",
        "Symbol.matchAll",
        "Symbol.dispose",
        "Symbol.asyncDispose",
    ];
}

/** Identities the engine refers to directly. */
#[derive(Default)]
pub(in crate::v9) struct Ids {
    pub object_prototype: ObjectId,
    pub function_prototype: ObjectId,
    pub array_prototype: ObjectId,
    /** Canonical Array constructor for cross-realm ArraySpeciesCreate, immune to property edits. */
    pub array: ObjectId,
    pub string_prototype: ObjectId,
    pub number_prototype: ObjectId,
    pub boolean_prototype: ObjectId,
    pub symbol_prototype: ObjectId,
    pub bigint_prototype: ObjectId,
    pub error_prototype: ObjectId,
    /** Canonical Error constructor, unaffected by script mutation of prototype.constructor. */
    pub error: ObjectId,
    /** Structured V8 CallSite instances share their creation context's native prototype. */
    pub call_site_prototype: ObjectId,
    pub type_error_prototype: ObjectId,
    pub range_error_prototype: ObjectId,
    pub reference_error_prototype: ObjectId,
    pub syntax_error_prototype: ObjectId,
    pub eval_error_prototype: ObjectId,
    pub uri_error_prototype: ObjectId,
    pub aggregate_error_prototype: ObjectId,
    pub dom_exception_prototype: ObjectId,
    pub date_prototype: ObjectId,
    pub regexp_prototype: ObjectId,
    pub map_prototype: ObjectId,
    pub set_prototype: ObjectId,
    pub weak_map_prototype: ObjectId,
    pub weak_set_prototype: ObjectId,
    pub promise_prototype: ObjectId,
    pub promise: ObjectId,
    pub generator_prototype: ObjectId,
    pub generator_function_prototype: ObjectId,
    pub async_function_prototype: ObjectId,
    pub async_generator_prototype: ObjectId,
    pub async_generator_function_prototype: ObjectId,
    pub async_iterator_prototype: ObjectId,
    pub async_from_sync_iterator_prototype: ObjectId,
    pub array_iterator_prototype: ObjectId,
    pub string_iterator_prototype: ObjectId,
    pub map_iterator_prototype: ObjectId,
    pub set_iterator_prototype: ObjectId,
    pub regexp_iterator_prototype: ObjectId,
    pub iterator_prototype: ObjectId,
    pub window_prototype: ObjectId,
    pub array_values: ObjectId,
    /** %ThrowTypeError%: the frozen thrower behind restricted `caller`, `arguments` and `callee`. */
    pub throw_type_error: ObjectId,
    pub event_target_prototype: ObjectId,
    pub node_prototype: ObjectId,
    pub element_prototype: ObjectId,
    pub html_element_prototype: ObjectId,
    pub text_prototype: ObjectId,
    pub document_prototype: ObjectId,
    /** XMLDocument.prototype: the prototype of documents createDocument makes, an XML document. */
    pub xml_document_prototype: ObjectId,
    pub document: ObjectId,
    /** The page's `document.implementation`, and the prototype script-made ones share. */
    pub implementation: ObjectId,
    pub implementation_prototype: ObjectId,
    /** `document.all`: the one object with [[IsHTMLDDA]], falsy and typeof "undefined". */
    pub document_all: ObjectId,
    /** The prototype of every frame window: it holds the page's interface constructors and
    DOMException, so a frame's window reaches them without a copy per frame. */
    pub window_interfaces: ObjectId,
    pub event_prototype: ObjectId,
    pub custom_event_prototype: ObjectId,
    pub style_prototype: ObjectId,
    pub class_list_prototype: ObjectId,
    pub dataset_prototype: ObjectId,
    pub node_list_prototype: ObjectId,
    pub xhr_prototype: ObjectId,
    pub response_prototype: ObjectId,
    pub headers_prototype: ObjectId,
    pub url_prototype: ObjectId,
    pub search_params_prototype: ObjectId,
    pub storage_prototype: ObjectId,
    pub location: ObjectId,
    pub fields_dummy: ObjectId,
    pub array_buffer_prototype: ObjectId,
    /** SAB has its own brand, prototype and constructor in each intrinsic bank. */
    pub shared_array_buffer_prototype: ObjectId,
    pub shared_array_buffer: ObjectId,
    pub abort_signal_prototype: ObjectId,
    pub idle_deadline_prototype: ObjectId,
    pub range_prototype: ObjectId,
    pub static_range_prototype: ObjectId,
    pub named_node_map_prototype: ObjectId,
    /** The Attr prototype: an Attr is a Node whose members are its name, value and owner. */
    pub attr_prototype: ObjectId,
    pub style_sheet_prototype: ObjectId,
    pub request_prototype: ObjectId,
    pub selection_prototype: ObjectId,
    /** Each typed array kind's prototype, in `ElementKind` order (DataView last). */
    pub view_prototypes: [ObjectId; 13],
}

/** Every intrinsic, built once. */
pub(in crate::v9) struct Realm {
    pub objects: Vec<IntrinsicDef>,
    pub natives: Vec<NativeDef>,
    pub ids: Ids,
    /** A virtual identity bank, zero for the production intrinsic realm. */
    pub bank: u32,
    /** The language globals are installed before browser interfaces and remain ordinary shell properties. */
    #[cfg(feature = "conformance")]
    pub shell_globals: Vec<(&'static str, JsValue)>,
}

impl Realm {
    /** A local native dispatcher has a distinct observable identity in every context. */
    pub fn native_identity(&self, index: u32) -> ObjectId {
        NATIVE_BASE + self.bank * bank_stride() + index
    }
    /** The intrinsic object with this identity. */
    pub fn object(&self, id: ObjectId) -> Option<&IntrinsicDef> {
        id.checked_sub(INTRINSIC_BASE + self.bank * bank_stride())
            .filter(|_| id < NATIVE_BASE)
            .and_then(|index| self.objects.get(index as usize))
    }
    /** The native function with this identity. */
    pub fn native(&self, id: ObjectId) -> Option<&NativeDef> {
        id.checked_sub(NATIVE_BASE + self.bank * bank_stride())
            .and_then(|index| self.natives.get(index as usize))
    }
}

/** The process-wide realm. */
pub(in crate::v9) fn realm() -> &'static Realm {
    #[cfg(feature = "conformance")]
    {
        return banks::bank(banks::current());
    }
    #[cfg(not(feature = "conformance"))]
    root_realm()
}
/** The root definitions are the immutable template for independently identified contexts. */
fn root_realm() -> &'static Realm {
    static REALM: OnceLock<Realm> = OnceLock::new();
    REALM.get_or_init(build)
}

/** Production builds retain their original single-bank identity space. */
fn bank_stride() -> u32 {
    #[cfg(feature = "conformance")]
    {
        return banks::BANK_STRIDE;
    }
    #[cfg(not(feature = "conformance"))]
    0
}
/** Resolve an intrinsic in its owning context rather than the currently executing one. */
pub(in crate::v9) fn realm_for(id: ObjectId) -> &'static Realm {
    #[cfg(feature = "conformance")]
    {
        return banks::bank(banks::owner(id));
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = id;
        realm()
    }
}
/** The local dispatcher index is stable across banks. */
pub(in crate::v9) fn native_index(id: ObjectId) -> u32 {
    #[cfg(feature = "conformance")]
    {
        return banks::native_index(id);
    }
    #[cfg(not(feature = "conformance"))]
    {
        id - NATIVE_BASE
    }
}

/** Builds the realm with a small fluent API. */
pub(in crate::v9) struct Builder {
    pub objects: Vec<IntrinsicDef>,
    pub natives: Vec<NativeDef>,
    pub ids: Ids,
    /** Names the global object exposes, gathered while installing. */
    pub globals: Vec<(&'static str, JsValue)>,
}

impl Builder {
    /** A new intrinsic object. */
    pub fn object(&mut self, proto: Option<ObjectId>) -> ObjectId {
        self.objects.push(IntrinsicDef {
            proto,
            props: Vec::new(),
            index: FastMap::default(),
            call: None,
        });
        INTRINSIC_BASE + (self.objects.len() as u32 - 1)
    }
    /** A native function object. */
    pub fn function(&mut self, name: &'static str, length: u32, function: NativeFn) -> ObjectId {
        self.natives.push(NativeDef {
            name,
            display_name: None,
            length,
            function,
            constructor: false,
            host: false,
        });
        NATIVE_BASE + (self.natives.len() as u32 - 1)
    }
    /** A native index without an object, for functions created with captured slots. */
    pub fn native(&mut self, name: &'static str, length: u32, function: NativeFn) -> u32 {
        self.function(name, length, function) - NATIVE_BASE
    }
    /** A constructor: a callable intrinsic with `prototype`, linked both ways. */
    pub fn constructor(
        &mut self,
        name: &'static str,
        length: u32,
        function: NativeFn,
        prototype: ObjectId,
        parent: Option<ObjectId>,
    ) -> ObjectId {
        let proto = parent.unwrap_or(self.ids.function_prototype);
        let id = self.object(Some(proto));
        self.natives.push(NativeDef {
            name,
            display_name: None,
            length,
            function,
            constructor: true,
            host: false,
        });
        let native = self.natives.len() as u32 - 1;
        self.def(id).call = Some(native);
        self.value(id, "prototype", JsValue::Object(prototype), 0);
        self.value(id, "name", string(name), super::heap::CONFIGURABLE);
        self.value(
            id,
            "length",
            JsValue::Number(f64::from(length)),
            super::heap::CONFIGURABLE,
        );
        self.value(prototype, "constructor", JsValue::Object(id), HIDDEN_ATTRS);
        id
    }
    /** The definition behind an identity. */
    fn def(&mut self, id: ObjectId) -> &mut IntrinsicDef {
        &mut self.objects[(id - INTRINSIC_BASE) as usize]
    }
    /** Add or replace a static property. */
    pub fn put(&mut self, target: ObjectId, key: Key, value: Static, attrs: u8) {
        let def = self.def(target);
        if let Some(index) = def.index.get(&key) {
            def.props[*index] = (key, value, attrs);
            return;
        }
        def.index.insert(key.clone(), def.props.len());
        def.props.push((key, value, attrs));
    }
    /** Optional intrinsic properties can be absent without exposing a placeholder through reflection. */
    pub fn remove(&mut self, target: ObjectId, name: &str) {
        let object = self.def(target);
        object.props.retain(|(key, ..)| *key != Key::from(name));
        object.index = object
            .props
            .iter()
            .enumerate()
            .map(|(index, (key, ..))| (key.clone(), index))
            .collect();
    }
    /** A data property. */
    pub fn value(&mut self, target: ObjectId, name: &str, value: JsValue, attrs: u8) {
        self.put(target, Key::from(name), Static::Value(value), attrs);
    }
    /** A method. */
    pub fn method(
        &mut self,
        target: ObjectId,
        name: &'static str,
        length: u32,
        function: NativeFn,
    ) -> ObjectId {
        let id = self.function(name, length, function);
        self.value(target, name, JsValue::Object(id), HIDDEN_ATTRS);
        id
    }
    /** A symbol-keyed method. */
    pub fn symbol_method(
        &mut self,
        target: ObjectId,
        symbol: u32,
        name: &'static str,
        length: u32,
        function: NativeFn,
    ) -> ObjectId {
        let id = self.function(name, length, function);
        self.put(
            target,
            Key::Sym(symbol),
            Static::Value(JsValue::Object(id)),
            HIDDEN_ATTRS,
        );
        id
    }
    /** An accessor property; enumerable accessors model host attributes. */
    pub fn accessor(
        &mut self,
        target: ObjectId,
        name: &'static str,
        getter: Option<NativeFn>,
        setter: Option<NativeFn>,
    ) {
        let get = getter.map(|function| self.function(name, 0, function));
        let set = setter.map(|function| self.function(name, 1, function));
        self.put(
            target,
            Key::from(name),
            Static::Accessor(get, set),
            super::heap::CONFIGURABLE | super::heap::ENUMERABLE,
        );
    }
    /** A getter-only accessor. */
    pub fn getter(&mut self, target: ObjectId, name: &'static str, getter: NativeFn) {
        self.accessor(target, name, Some(getter), None);
    }
    /** An ECMAScript builtin accessor: configurable and, unlike DOM attributes, not enumerable. */
    pub fn builtin_accessor(
        &mut self,
        target: ObjectId,
        name: &'static str,
        getter: Option<NativeFn>,
        setter: Option<NativeFn>,
    ) {
        let get = getter.map(|function| self.accessor_function(name, 0, "get", function));
        let set = setter.map(|function| self.accessor_function(name, 1, "set", function));
        self.put(
            target,
            Key::from(name),
            Static::Accessor(get, set),
            super::heap::CONFIGURABLE,
        );
    }
    /** Display labels retain the accessor prefix without changing existing dispatch names or leaking static strings. */
    fn accessor_function(
        &mut self,
        name: &'static str,
        length: u32,
        prefix: &str,
        function: NativeFn,
    ) -> ObjectId {
        let id = self.function(name, length, function);
        self.natives.last_mut().expect("new native").display_name =
            Some(format!("{prefix} {name}"));
        id
    }
    /** Expose a value as a global binding. */
    pub fn global(&mut self, name: &'static str, value: ObjectId) {
        self.globals.push((name, JsValue::Object(value)));
    }
    /** Expose a primitive constant, such as a host-declared length, as a global binding. */
    #[cfg(feature = "conformance")]
    pub fn global_value(&mut self, name: &'static str, value: JsValue) {
        self.globals.push((name, value));
    }
    /** Alias an existing value under another name. */
    pub fn alias(&mut self, target: ObjectId, name: &str, value: ObjectId) {
        self.value(target, name, JsValue::Object(value), HIDDEN_ATTRS);
    }
}

/** A string value from builder text. */
pub(in crate::v9) fn string(text: &str) -> JsValue {
    JsValue::String(JsString::from_utf8(text).expect("builtin text fits"))
}

/** Gathers every global the builder exposed onto one object, the prototype of frame windows. It runs
after all installs, so it sees every interface; each frame window then reaches the constructors
through its prototype chain in O(1), and nothing is copied per frame. */
fn install_window_interfaces(builder: &mut Builder) {
    let interfaces = builder.object(Some(builder.ids.object_prototype));
    for (name, value) in builder.globals.clone() {
        builder.value(interfaces, name, value, HIDDEN_ATTRS);
    }
    builder.ids.window_interfaces = interfaces;
}

/** Build every intrinsic. */
fn build() -> Realm {
    let mut builder = Builder {
        objects: Vec::new(),
        natives: Vec::new(),
        ids: Ids::default(),
        globals: Vec::new(),
    };
    super::super::builtins::install(&mut builder);
    #[cfg(feature = "conformance")]
    let shell_globals = builder.globals.clone();
    super::super::platform::install(&mut builder);
    super::super::modules::install(&mut builder);
    install_window_interfaces(&mut builder);
    Realm {
        objects: builder.objects,
        natives: builder.natives,
        ids: builder.ids,
        bank: 0,
        #[cfg(feature = "conformance")]
        shell_globals,
    }
}
