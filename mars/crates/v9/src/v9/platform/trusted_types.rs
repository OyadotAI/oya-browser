/**
 * Trusted Types (W3C): `window.trustedTypes`, the policies it creates, and the TrustedHTML,
 * TrustedScript and TrustedScriptURL values a policy returns. Pages (Google's among them) create
 * a policy and pass what it returns to innerHTML and eval, and a Chrome User-Agent that lacks
 * `trustedTypes` is a tell. A policy runs the page's own rule functions and wraps the result;
 * nothing is enforced, as in a page without a `require-trusted-types-for` directive.
 */
use crate::v9::value::JsValue;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Prop, Slot};
use crate::v9::vm::realm::{Builder, Static, string};
use crate::v9::vm::{Call, Res, Vm};

/** Each policy method with the interface of the value it makes. */
const CREATORS: [(&str, &str); 3] = [
    ("createHTML", "TrustedHTML"),
    ("createScript", "TrustedScript"),
    ("createScriptURL", "TrustedScriptURL"),
];
/** The private slot holding a trusted value's text. */
const VALUE_SLOT: &str = "trustedValue";
/** The private slot holding a policy's rule object. */
const RULES_SLOT: &str = "trustedRules";
/** The private slot holding a policy's name. */
const NAME_SLOT: &str = "trustedName";

/** The factory, the policy and value interfaces, and `window.trustedTypes`. */
pub(super) fn install(b: &mut Builder) {
    let factory_proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, "TrustedTypePolicyFactory", factory_proto);
    let factory = b.object(Some(factory_proto));
    let policy_proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, "TrustedTypePolicy", policy_proto);
    for (name, interface) in CREATORS {
        let proto = install_value_interface(b, policy_proto, name, interface);
        install_empty_value(b, factory_proto, factory, interface, proto);
    }
    b.getter(policy_proto, "name", policy_name);
    b.host_method(factory_proto, "createPolicy", 1, create_policy);
    for name in ["isHTML", "isScript", "isScriptURL"] {
        b.host_method(factory_proto, name, 1, is_trusted);
    }
    for name in ["getAttributeType", "getPropertyType"] {
        b.host_method(factory_proto, name, 2, |_, _| Ok(JsValue::Null));
    }
    b.getter(factory_proto, "defaultPolicy", |_, _| Ok(JsValue::Null));
    b.global("trustedTypes", factory);
}

/** Install one TrustedType value interface (`TrustedHTML`, `TrustedScript`, `TrustedScriptURL`). */
fn install_value_interface(
    b: &mut Builder,
    policy_proto: u32,
    creator: &'static str,
    interface: &'static str,
) -> u32 {
    let proto = b.object(Some(b.ids.object_prototype));
    super::chrome_env::interface(b, interface, proto);
    b.host_method(proto, "toString", 0, trusted_text);
    b.host_method(proto, "toJSON", 0, trusted_text);
    b.host_method(policy_proto, creator, 1, create_value);
    proto
}

/** Attach `emptyHTML` and `emptyScript` getters on `TrustedTypePolicyFactory.prototype`. */
fn install_empty_value(
    b: &mut Builder,
    factory_proto: u32,
    factory: u32,
    interface: &str,
    value_proto: u32,
) {
    let slot = match interface {
        "TrustedHTML" => "emptyHTML",
        "TrustedScript" => "emptyScript",
        _ => return,
    };
    let empty = b.object(Some(value_proto));
    b.put(
        empty,
        Key::private(VALUE_SLOT),
        Static::Value(string("")),
        HIDDEN_ATTRS,
    );
    b.put(
        factory,
        Key::private(slot),
        Static::Value(JsValue::Object(empty)),
        HIDDEN_ATTRS,
    );
    b.getter(factory_proto, slot, empty_value);
}

/** `trustedTypes.emptyHTML` and `trustedTypes.emptyScript`. */
fn empty_value(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let slot = super::dom::native_name(&call);
    read_slot(vm, &call.this, slot).ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** The prototype of a global interface, for objects made at run time. */
fn prototype_of(vm: &mut Vm, interface: &str) -> Res<Option<u32>> {
    let global = JsValue::Object(vm.global_object());
    let constructor = vm.get(&global, &Key::from(interface))?;
    Ok(vm.get(&constructor, &Key::from("prototype"))?.object())
}

/** An object of the interface carrying one private text slot. */
fn trusted(vm: &mut Vm, interface: &str, text: JsValue) -> Res<JsValue> {
    let id = vm.new_object()?;
    if let Some(proto) = prototype_of(vm, interface)? {
        vm.set_proto(id, Some(proto))?;
    }
    write_slot(vm, id, VALUE_SLOT, text);
    Ok(JsValue::Object(id))
}

/** Read a private slot from either a heap object or an unmaterialized static intrinsic. */
fn read_slot(vm: &Vm, value: &JsValue, name: &str) -> Option<JsValue> {
    let id = value.object()?;
    if let Some(prop) = vm
        .heap
        .get(id)
        .and_then(|o| o.props.get(&Key::private(name)))
    {
        return match &prop.slot {
            Slot::Data(value) => Some(value.clone()),
            _ => None,
        };
    }
    match &crate::v9::vm::realm::realm_for(id)
        .object(id)?
        .get(&Key::private(name))?
        .1
    {
        Static::Value(value) => Some(value.clone()),
        _ => None,
    }
}

/** Write a private slot. */
fn write_slot(vm: &mut Vm, id: u32, name: &str, value: JsValue) {
    let prop = Prop {
        slot: Slot::Data(value),
        attrs: HIDDEN_ATTRS,
    };
    vm.heap.put(id, Key::private(name), prop);
}

/** `trustedTypes.createPolicy(name, rules)`: a policy that applies `rules`' functions. */
fn create_policy(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = JsValue::String(vm.to_string(&call.arg(0))?);
    let id = vm.new_object()?;
    if let Some(proto) = prototype_of(vm, "TrustedTypePolicy")? {
        vm.set_proto(id, Some(proto))?;
    }
    write_slot(vm, id, NAME_SLOT, name);
    write_slot(vm, id, RULES_SLOT, call.arg(1));
    Ok(JsValue::Object(id))
}

/** `policy.name`. */
fn policy_name(vm: &mut Vm, call: Call) -> Res<JsValue> {
    read_slot(vm, &call.this, NAME_SLOT).ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** `policy.createHTML(input, ...)` and its siblings: the rule's result as a trusted value. */
fn create_value(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let method = super::dom::native_name(&call);
    let Some((_, interface)) = CREATORS.iter().find(|(name, _)| *name == method) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let Some(rules) = read_slot(vm, &call.this, RULES_SLOT) else {
        return Err(vm.type_error("Illegal invocation"));
    };
    let rule = match &rules {
        JsValue::Object(_) => vm.get(&rules, &Key::from(method))?,
        _ => JsValue::Undefined,
    };
    if !vm.is_callable(&rule) {
        return Err(vm.type_error(&format!(
            "Failed to execute '{method}' on 'TrustedTypePolicy': Policy's TrustedTypePolicyOptions did not specify a '{method}' member."
        )));
    }
    let result = vm.call(rule, rules, call.args.to_vec())?;
    let text = match result {
        JsValue::Undefined | JsValue::Null => string(""),
        other => JsValue::String(vm.to_string(&other)?),
    };
    trusted(vm, interface, text)
}

/** The text of a TrustedScript, for eval and the other script sinks; None for anything else. */
pub(in crate::v9) fn trusted_script_text(vm: &mut Vm, value: &JsValue) -> Res<Option<JsValue>> {
    let kind = value.object().and_then(|id| vm.proto_of(id));
    if kind.is_none() || kind != prototype_of(vm, "TrustedScript")? {
        return Ok(None);
    }
    Ok(read_slot(vm, value, VALUE_SLOT))
}

/** `TrustedHTML.prototype.toString` and `toJSON`: the wrapped text. */
fn trusted_text(vm: &mut Vm, call: Call) -> Res<JsValue> {
    read_slot(vm, &call.this, VALUE_SLOT).ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** `trustedTypes.isHTML(value)` and its siblings: a value this factory made, of that kind. */
fn is_trusted(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let interface = match super::dom::native_name(&call) {
        "isHTML" => "TrustedHTML",
        "isScript" => "TrustedScript",
        _ => "TrustedScriptURL",
    };
    let value = call.arg(0);
    let kind = value.object().and_then(|id| vm.proto_of(id));
    let expected = prototype_of(vm, interface)?;
    Ok(JsValue::Boolean(
        kind.is_some() && kind == expected && read_slot(vm, &value, VALUE_SLOT).is_some(),
    ))
}
