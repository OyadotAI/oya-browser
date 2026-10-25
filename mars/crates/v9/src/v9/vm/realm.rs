/**
 * The intrinsic realm: builtin prototypes, constructors and native functions, built once per
 * process. Intrinsics are virtual objects outside the page heap, so they cost no heap budget and
 * never serialize; a script that writes to one gets a heap copy under the same identity.
 */
use super::super::value::{JsString, JsValue, ObjectId};
use super::heap::{FastMap, HIDDEN_ATTRS, INTRINSIC_BASE, Key, NATIVE_BASE};
use super::{Call, Res, Vm};
use std::sync::OnceLock;

/** A builtin function body. */
pub(in crate::v9) type NativeFn = fn(&mut Vm, Call) -> Res<JsValue>;

/** One builtin function. */
pub(in crate::v9) struct NativeDef {
    pub name: &'static str,
    pub length: u32,
    pub function: NativeFn,
    /** Callable with `new`. */
    pub constructor: bool,
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
    pub const WELL_KNOWN: u32 = 13;
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
    ];
}

/** Identities the engine refers to directly. */
#[derive(Default)]
pub(in crate::v9) struct Ids {
    pub object_prototype: ObjectId,
    pub function_prototype: ObjectId,
    pub array_prototype: ObjectId,
    pub string_prototype: ObjectId,
    pub number_prototype: ObjectId,
    pub boolean_prototype: ObjectId,
    pub symbol_prototype: ObjectId,
    pub bigint_prototype: ObjectId,
    pub error_prototype: ObjectId,
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
    pub document: ObjectId,
    /** `document.all`: the one object with [[IsHTMLDDA]], falsy and typeof "undefined". */
    pub document_all: ObjectId,
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
    pub abort_signal_prototype: ObjectId,
    pub idle_deadline_prototype: ObjectId,
    pub range_prototype: ObjectId,
    pub named_node_map_prototype: ObjectId,
    pub style_sheet_prototype: ObjectId,
    pub request_prototype: ObjectId,
    pub selection_prototype: ObjectId,
    /** Each typed array kind's prototype, in `ElementKind` order (DataView last). */
    pub view_prototypes: [ObjectId; 12],
}

/** Every intrinsic, built once. */
pub(in crate::v9) struct Realm {
    pub objects: Vec<IntrinsicDef>,
    pub natives: Vec<NativeDef>,
    pub ids: Ids,
}

impl Realm {
    /** The intrinsic object with this identity. */
    pub fn object(&self, id: ObjectId) -> Option<&IntrinsicDef> {
        id.checked_sub(INTRINSIC_BASE)
            .filter(|_| id < NATIVE_BASE)
            .and_then(|index| self.objects.get(index as usize))
    }
    /** The native function with this identity. */
    pub fn native(&self, id: ObjectId) -> Option<&NativeDef> {
        id.checked_sub(NATIVE_BASE)
            .and_then(|index| self.natives.get(index as usize))
    }
}

/** The process-wide realm. */
pub(in crate::v9) fn realm() -> &'static Realm {
    static REALM: OnceLock<Realm> = OnceLock::new();
    REALM.get_or_init(build)
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
            length,
            function,
            constructor: false,
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
            length,
            function,
            constructor: true,
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
        let get = getter.map(|function| self.function(name, 0, function));
        let set = setter.map(|function| self.function(name, 1, function));
        self.put(
            target,
            Key::from(name),
            Static::Accessor(get, set),
            super::heap::CONFIGURABLE,
        );
    }
    /** Expose a value as a global binding. */
    pub fn global(&mut self, name: &'static str, value: ObjectId) {
        self.globals.push((name, JsValue::Object(value)));
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

/** Build every intrinsic. */
fn build() -> Realm {
    let mut builder = Builder {
        objects: Vec::new(),
        natives: Vec::new(),
        ids: Ids::default(),
        globals: Vec::new(),
    };
    super::super::builtins::install(&mut builder);
    super::super::platform::install(&mut builder);
    super::super::modules::install(&mut builder);
    Realm {
        objects: builder.objects,
        natives: builder.natives,
        ids: builder.ids,
    }
}
