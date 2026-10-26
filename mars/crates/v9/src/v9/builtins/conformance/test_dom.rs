/** The d8 shell's API test objects model native receiver signatures, independently of browser DOM nodes. */
use crate::v9::builtins::key;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{
    CONFIGURABLE, DEFAULT_ATTRS, ENUMERABLE, Key, Object, Prop, Slot, WRITABLE,
};
use crate::v9::vm::realm::{Builder, NativeFn, Static, banks};
use crate::v9::vm::{Call, Res, Vm};

/** A hidden immutable type belongs to the native allocation, never its mutable public prototype. */
pub(super) const BRAND_KEY: &str = "d8-dom-native-type";
/** The pinned NodeTypeCallback reports the DOM element-node discriminator. */
const NODE_TYPE_VALUE: f64 = 1.0;
/** Only the five FunctionTemplate types created by the pinned shell are represented. */
const TEMPLATE_COUNT: usize = 5;

/** The exact FunctionTemplate inheritance order in pinned d8.cc. */
#[derive(Clone, Copy, PartialEq, PartialOrd)]
#[repr(u8)]
enum NativeType {
    EventTarget = 1,
    Node,
    Element,
    HtmlElement,
    Div,
}

/** The namespace exposes only EventTarget and Div; intermediate constructors remain reachable from their prototypes. */
pub(super) fn install(builder: &mut Builder, d8: ObjectId) {
    let namespace = builder.object(Some(builder.ids.object_prototype));
    let mut parent = builder.ids.object_prototype;
    for (name, kind, callback) in definitions() {
        let prototype = builder.object(Some(parent));
        if kind == NativeType::Node {
            install_node_type(builder, prototype);
        }
        let constructor = builder.constructor(name, 0, callback, prototype, None);
        builder.value(
            constructor,
            "prototype",
            JsValue::Object(prototype),
            WRITABLE,
        );
        if !name.is_empty() {
            builder.value(namespace, name, JsValue::Object(constructor), DEFAULT_ATTRS);
        }
        parent = prototype;
    }
    builder.value(d8, "dom", JsValue::Object(namespace), DEFAULT_ATTRS);
}

/** Native callback identities fix each allocated type without consulting writable function names or constructors. */
fn definitions() -> [(&'static str, NativeType, NativeFn); TEMPLATE_COUNT] {
    [
        ("EventTarget", NativeType::EventTarget, |vm, call| {
            construct(vm, call, NativeType::EventTarget)
        }),
        ("", NativeType::Node, |vm, call| {
            construct(vm, call, NativeType::Node)
        }),
        ("", NativeType::Element, |vm, call| {
            construct(vm, call, NativeType::Element)
        }),
        ("", NativeType::HtmlElement, |vm, call| {
            construct(vm, call, NativeType::HtmlElement)
        }),
        ("Div", NativeType::Div, |vm, call| {
            construct(vm, call, NativeType::Div)
        }),
    ]
}

/** FunctionTemplate accessor properties are enumerable/configurable with an anonymous nonconstructible callback. */
fn install_node_type(builder: &mut Builder, prototype: ObjectId) {
    let getter = builder.function("", 0, node_type);
    builder.put(
        prototype,
        key("nodeType"),
        Static::Accessor(Some(getter), None),
        ENUMERABLE | CONFIGURABLE,
    );
}

/** A null native constructor callback returns its receiver on ordinary calls and creates branded state only under new. */
fn construct(vm: &mut Vm, call: Call, kind: NativeType) -> Res<JsValue> {
    let Some(target) = &call.new_target else {
        return converted_receiver(vm, call.this);
    };
    let prototype = prototype(vm, target)?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap.put(
        object,
        Key::private(BRAND_KEY),
        Prop {
            slot: Slot::Data(JsValue::Number(f64::from(kind as u8))),
            attrs: 0,
        },
    );
    vm.check_heap_bounds()?;
    Ok(JsValue::Object(object))
}

/** API callbacks without a signature use non-strict receiver conversion and never brand an existing receiver. */
fn converted_receiver(vm: &mut Vm, receiver: JsValue) -> Res<JsValue> {
    if receiver.nullish() {
        return Ok(JsValue::Object(vm.global_object()));
    }
    vm.to_object(&receiver).map(JsValue::Object)
}

/** GetPrototypeFromConstructor observes prototype once before deriving a fallback from newTarget's own creation realm. */
fn prototype(vm: &mut Vm, target: &JsValue) -> Res<ObjectId> {
    match vm.get(target, &key("prototype"))? {
        JsValue::Object(prototype) => Ok(prototype),
        _ => {
            let bank = vm.constructor_realm(target)?;
            Ok(banks::bank(bank).ids.object_prototype)
        }
    }
}

/** Direct immutable allocation state supplies the brand; inherited properties and proxies cannot impersonate API objects. */
fn kind(vm: &Vm, id: ObjectId) -> Option<NativeType> {
    let object = vm
        .heap
        .get(id)
        .filter(|object| matches!(object.class, Class::Plain))?;
    let prop = object
        .props
        .get(&Key::private(BRAND_KEY))
        .filter(|prop| prop.attrs == 0)?;
    let Slot::Data(JsValue::Number(value)) = &prop.slot else {
        return None;
    };
    [
        NativeType::EventTarget,
        NativeType::Node,
        NativeType::Element,
        NativeType::HtmlElement,
        NativeType::Div,
    ]
    .into_iter()
    .find(|kind| f64::from(*kind as u8) == *value)
}

/** The actual d8 callback returns the element-node value only after its Node FunctionTemplate signature has matched. */
fn node_type(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let valid = call.this.object().is_some_and(|id| {
        vm.object_realm(id) == vm.active_realm
            && kind(vm, id).is_some_and(|kind| kind >= NativeType::Node)
    });
    if !valid {
        return Err(vm.type_error("Illegal invocation"));
    }
    Ok(JsValue::Number(NODE_TYPE_VALUE))
}

/** Saved native brands are exact known type numbers on ordinary API-object records, with immutable attributes. */
pub(in crate::v9) fn valid(vm: &Vm, id: ObjectId) -> bool {
    vm.heap.get(id).is_none_or(|object| {
        object.props.get(&Key::private(BRAND_KEY)).is_none() || kind(vm, id).is_some()
    })
}
