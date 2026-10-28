/** Iterator helper state is inert private heap data, so callbacks and suspended cursors survive transport. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::vm::heap::{Object, Prop, Slot};

/** The brand references a private state record rather than any author-controlled property. */
fn brand() -> Key {
    Key::private("iterator-helper-record")
}

/** The helper mode determines which steps run between two yielded values. */
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Mode {
    Map,
    Filter,
    Take,
    Drop,
    FlatMap,
    Concat,
    Zip,
    ZipKeyed,
}

impl Mode {
    /** Stable words keep saved records independent of Rust enum layout. */
    pub fn name(self) -> &'static str {
        match self {
            Self::Map => "map",
            Self::Filter => "filter",
            Self::Take => "take",
            Self::Drop => "drop",
            Self::FlatMap => "flatMap",
            Self::Concat => "concat",
            Self::Zip => "zip",
            Self::ZipKeyed => "zipKeyed",
        }
    }
    /** Unknown saved modes cannot acquire a different executable handler. */
    fn parse(value: &JsValue) -> Option<Self> {
        match value {
            JsValue::String(value) => match value.lossy().as_str() {
                "map" => Some(Self::Map),
                "filter" => Some(Self::Filter),
                "take" => Some(Self::Take),
                "drop" => Some(Self::Drop),
                "flatMap" => Some(Self::FlatMap),
                "concat" => Some(Self::Concat),
                "zip" => Some(Self::Zip),
                "zipKeyed" => Some(Self::ZipKeyed),
                _ => None,
            },
            _ => None,
        }
    }
}

/** Values copied while a native call runs remain rooted by the helper receiver's private record. */
pub(super) struct State {
    pub id: ObjectId,
    pub record: ObjectId,
    pub mode: Mode,
    pub callback: JsValue,
    pub counter: f64,
    pub remaining: f64,
    pub inner: Option<ObjectId>,
    pub done: bool,
    pub started: bool,
    pub executing: bool,
}

impl State {
    /** Private storage is created only after argument validation and next-method capture succeed. */
    pub fn create(
        vm: &mut Vm,
        record: ObjectId,
        mode: Mode,
        callback: JsValue,
        remaining: f64,
        prototype: ObjectId,
    ) -> Res<JsValue> {
        let id = vm.new_object()?;
        let state = Self {
            id,
            record,
            mode,
            callback,
            counter: 0.0,
            remaining,
            inner: None,
            done: false,
            started: false,
            executing: false,
        };
        state.save(vm);
        put(vm, id, "record", JsValue::Object(record));
        put(vm, id, "mode", JsValue::string(mode.name())?);
        let wrapper = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
        vm.heap
            .put(wrapper, brand(), Prop::data(JsValue::Object(id)));
        Ok(JsValue::Object(wrapper))
    }

    /** Direct slot checks reject proxies and borrowed calls without invoking any author getters. */
    pub fn load(vm: &mut Vm, value: &JsValue) -> Res<Self> {
        let Some(id) = value
            .object()
            .and_then(|id| vm.heap.get(id))
            .and_then(|object| object.props.get(&brand()))
            .and_then(|prop| prop_value(prop).object())
        else {
            return Err(vm.type_error("Iterator helper method called on incompatible receiver"));
        };
        let done = boolean(vm, id, "done")?;
        let record = match get(vm, id, "record").object() {
            Some(record) => record,
            None if done => id,
            None => return Err(invalid().into()),
        };
        let mode = Mode::parse(&get(vm, id, "mode")).ok_or_else(|| invalid())?;
        let callback = get(vm, id, "callback");
        let counter = number(vm, id, "counter")?;
        let remaining = number(vm, id, "remaining")?;
        if !counter.is_finite()
            || counter < 0.0
            || counter.fract() != 0.0
            || remaining.is_nan()
            || remaining < 0.0
        {
            return Err(invalid().into());
        }
        Ok(Self {
            id,
            record,
            mode,
            callback,
            counter,
            remaining,
            inner: get(vm, id, "inner").object(),
            done,
            started: boolean(vm, id, "started")?,
            executing: boolean(vm, id, "executing")?,
        })
    }

    /** Save before every callback or close that may re-enter this helper or trigger collection. */
    pub fn save(&self, vm: &mut Vm) {
        let completed = self.done && !self.executing;
        put(
            vm,
            self.id,
            "record",
            if completed {
                JsValue::Undefined
            } else {
                JsValue::Object(self.record)
            },
        );
        put(
            vm,
            self.id,
            "callback",
            if completed {
                JsValue::Undefined
            } else {
                self.callback.clone()
            },
        );
        put(vm, self.id, "counter", JsValue::Number(self.counter));
        put(vm, self.id, "remaining", JsValue::Number(self.remaining));
        put(
            vm,
            self.id,
            "inner",
            if completed {
                JsValue::Undefined
            } else {
                self.inner
                    .map(JsValue::Object)
                    .unwrap_or(JsValue::Undefined)
            },
        );
        put(vm, self.id, "started", JsValue::Boolean(self.started));
        put(vm, self.id, "done", JsValue::Boolean(self.done));
        put(vm, self.id, "executing", JsValue::Boolean(self.executing));
    }
}

/** Malformed private state is an engine admission error, never a fabricated helper result. */
fn invalid() -> EngineError {
    EngineError::invalid("Invalid iterator helper state.")
}
/** Private state never accepts accessor execution supplied through a forged saved descriptor. */
fn prop_value(prop: &Prop) -> JsValue {
    match &prop.slot {
        Slot::Data(value) => value.clone(),
        _ => JsValue::Undefined,
    }
}
/** Read one private field without JavaScript property semantics. */
fn get(vm: &Vm, id: ObjectId, name: &str) -> JsValue {
    vm.heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(name)))
        .map(prop_value)
        .unwrap_or(JsValue::Undefined)
}
/** Internal state changes do not depend on object extensibility or author setters. */
fn put(vm: &mut Vm, id: ObjectId, name: &str, value: JsValue) {
    vm.heap.put(id, Key::private(name), Prop::data(value));
}
/** Numbers include infinity only for the already validated take/drop limit. */
fn number(vm: &Vm, id: ObjectId, name: &str) -> Res<f64> {
    match get(vm, id, name) {
        #[cfg(feature = "conformance")]
        JsValue::Smi(value) => Ok(value),
        JsValue::Number(value) => Ok(value),
        _ => Err(invalid().into()),
    }
}
/** Completion flags must have the exact saved boolean type. */
fn boolean(vm: &Vm, id: ObjectId, name: &str) -> Res<bool> {
    match get(vm, id, name) {
        JsValue::Boolean(value) => Ok(value),
        _ => Err(invalid().into()),
    }
}
