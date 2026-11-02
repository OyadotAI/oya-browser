/** Symbol: unique keys, the global registry and the well-known symbols. */
use super::*;
use crate::constants::VM_SYMBOLS;
use crate::protocol::EngineError;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::{realm, symbol};

/** Symbol and Symbol.prototype. */
pub(super) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.object_prototype));
    b.ids.symbol_prototype = proto;
    let ctor = b.constructor("Symbol", 0, construct, proto, None);
    b.global("Symbol", ctor);
    for (index, name) in symbol::NAMES.iter().enumerate() {
        let short = name.trim_start_matches("Symbol.");
        b.value(ctor, short, JsValue::Symbol(index as u32), 0);
    }
    b.method(ctor, "for", 1, for_key);
    b.method(ctor, "keyFor", 1, key_for);
    b.method(proto, "toString", 0, to_string);
    b.method(proto, "valueOf", 0, value_of);
    b.builtin_accessor(proto, "description", Some(description), None);
    b.symbol_method(
        proto,
        symbol::TO_PRIMITIVE,
        "[Symbol.toPrimitive]",
        1,
        value_of,
    );
    b.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        crate::v9::vm::realm::Static::Value(crate::v9::vm::realm::string("Symbol")),
        crate::v9::vm::heap::CONFIGURABLE,
    );
}

/** A new symbol with an optional description. */
pub(in crate::v9) fn new_symbol(vm: &mut Vm, description: Option<JsString>) -> Res<u32> {
    if vm.symbols.descriptions.len() >= VM_SYMBOLS {
        return Err(EngineError::limit("V9 symbols", VM_SYMBOLS).into());
    }
    vm.symbols.descriptions.push(description);
    Ok(symbol::WELL_KNOWN + vm.symbols.descriptions.len() as u32 - 1)
}

/** `Symbol(description)`; `new Symbol` throws. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_some() {
        return Err(vm.type_error("Symbol is not a constructor"));
    }
    let description = match call.arg(0) {
        JsValue::Undefined => None,
        value => Some(vm.to_string(&value)?),
    };
    Ok(JsValue::Symbol(new_symbol(vm, description)?))
}

/** Symbol.for. */
fn for_key(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let name = arg_string(vm, &call, 0)?;
    let text = name.lossy();
    if let Some(id) = vm.symbols.registry.get(&text) {
        return Ok(JsValue::Symbol(*id));
    }
    let id = new_symbol(vm, Some(name))?;
    vm.symbols.registry.insert(text, id);
    Ok(JsValue::Symbol(id))
}

/** Symbol.keyFor. */
fn key_for(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let JsValue::Symbol(id) = call.arg(0) else {
        return Err(vm.type_error("Symbol.keyFor requires a symbol"));
    };
    let found = vm
        .symbols
        .registry
        .iter()
        .find(|(_, value)| **value == id)
        .map(|(name, _)| name.clone());
    match found {
        Some(name) => text(vm, &name),
        None => Ok(JsValue::Undefined),
    }
}

/** The symbol a receiver holds. */
fn this_symbol(vm: &mut Vm, call: &Call) -> Res<u32> {
    match &call.this {
        JsValue::Symbol(id) => Ok(*id),
        JsValue::Object(id) => match vm.heap.get(*id).map(|o| &o.class) {
            Some(Class::Symbol(symbol)) => Ok(*symbol),
            _ => Err(vm.type_error("Symbol.prototype method called on incompatible receiver")),
        },
        _ => Err(vm.type_error("Symbol.prototype method called on incompatible receiver")),
    }
}

/** A symbol's description. */
pub(in crate::v9) fn symbol_description(vm: &Vm, id: u32) -> Option<String> {
    if id < symbol::WELL_KNOWN {
        return Some(symbol::NAMES[id as usize].into());
    }
    vm.symbols
        .descriptions
        .get((id - symbol::WELL_KNOWN) as usize)
        .cloned()
        .flatten()
        .map(|text| text.lossy())
}

/** Symbol.prototype.toString. */
fn to_string(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_symbol(vm, &call)?;
    let description = symbol_description(vm, id).unwrap_or_default();
    text(vm, &format!("Symbol({description})"))
}

/** Symbol.prototype.valueOf. */
fn value_of(vm: &mut Vm, call: Call) -> Res<JsValue> {
    Ok(JsValue::Symbol(this_symbol(vm, &call)?))
}

/** Symbol.prototype.description. */
fn description(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = this_symbol(vm, &call)?;
    match symbol_description(vm, id) {
        Some(description) => text(vm, &description),
        None => Ok(JsValue::Undefined),
    }
}

/** Box a symbol (rarely needed). */
#[allow(dead_code)]
fn boxed(vm: &mut Vm, id: u32) -> Res<ObjectId> {
    vm.alloc(Object::new(
        Class::Symbol(id),
        Some(realm().ids.symbol_prototype),
    ))
}
