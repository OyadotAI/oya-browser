/**
 * structuredClone: the HTML structured clone of ordinary data (primitives, plain objects, arrays,
 * Date, RegExp, Map, Set, primitive wrappers and errors), keeping shared and circular references.
 * Functions, symbols, DOM nodes and other host objects throw DataCloneError, as in a browser.
 */
use super::*;
use crate::v9::vm::data::Class;

/** structuredClone on the global object. */
pub(super) fn install(b: &mut Builder) {
    let function = b.function("structuredClone", 1, structured_clone);
    b.global("structuredClone", function);
}

/** structuredClone(value). */
fn structured_clone(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let mut memo = Vec::new();
    clone(vm, &call.arg(0), &mut memo, 0)
}

/** The error a value that cannot be cloned throws. */
fn uncloneable(vm: &mut Vm, what: &str) -> crate::v9::vm::Thrown {
    vm.dom_exception(
        "DataCloneError",
        &format!("Failed to execute 'structuredClone' on 'Window': {what} could not be cloned."),
    )
}

/** Clone one value; `memo` maps originals to their clones so cycles and sharing survive. */
fn clone(
    vm: &mut Vm,
    value: &JsValue,
    memo: &mut Vec<(ObjectId, JsValue)>,
    depth: usize,
) -> Res<JsValue> {
    let id = match value {
        JsValue::Symbol(_) => return Err(uncloneable(vm, "A symbol")),
        JsValue::Object(id) => *id,
        other => return Ok(other.clone()),
    };
    if let Some((_, copy)) = memo.iter().find(|(original, _)| *original == id) {
        return Ok(copy.clone());
    }
    if depth >= vm.limits.frames {
        return Err(vm.range_error("Maximum call stack size exceeded"));
    }
    if vm.is_callable(value) {
        return Err(uncloneable(vm, "A function"));
    }
    if super::is_raw_json(vm, value) {
        return Err(uncloneable(vm, "A raw JSON object"));
    }
    #[cfg(feature = "conformance")]
    if super::conformance_host_object(vm, value) {
        return Err(uncloneable(vm, "A native test object"));
    }
    let class = vm
        .string_data(id)
        .map(Class::String)
        .or_else(|| vm.heap.get(id).map(|object| object.class.clone()));
    let copy = match &class {
        Some(Class::Array(_)) => JsValue::Object(vm.new_array(Vec::new())?),
        Some(Class::Plain | Class::Error) => JsValue::Object(vm.new_object()?),
        Some(Class::SharedArrayBuffer(buffer)) => {
            JsValue::Object(super::import_shared_buffer(vm, buffer.clone())?)
        }
        Some(Class::ArrayBuffer(bytes)) => {
            if super::typed::buffer_state_detached(vm, id) {
                return Err(uncloneable(vm, "A detached ArrayBuffer"));
            }
            let copy = vm.alloc(crate::v9::vm::heap::Object::new(
                Class::ArrayBuffer(bytes.clone()),
                Some(crate::v9::vm::realm::realm().ids.array_buffer_prototype),
            ))?;
            if let Some(maximum) = super::buffer_maximum(vm, id) {
                super::set_buffer_maximum(vm, copy, maximum);
            }
            if super::buffer_immutable(vm, id) {
                super::make_buffer_immutable(vm, copy);
            }
            JsValue::Object(copy)
        }
        Some(Class::View(_)) => return clone_view(vm, id, memo, depth),
        Some(Class::WasmShared(root)) => super::wasm::restore_shared_gc(vm, &root.clone())?,
        Some(Class::Boolean(flag)) => JsValue::Object(vm.to_object(&JsValue::Boolean(*flag))?),
        Some(Class::Number(number)) => JsValue::Object(vm.to_object(&JsValue::Number(*number))?),
        Some(Class::String(text)) => JsValue::Object(vm.to_object(&JsValue::String(text.clone()))?),
        Some(Class::BigInt(big)) => JsValue::Object(vm.to_object(&JsValue::BigInt(*big))?),
        Some(Class::BigIntWide(big)) => {
            JsValue::Object(vm.to_object(&JsValue::BigIntWide(big.clone()))?)
        }
        Some(Class::Date(_) | Class::RegExp(_)) => {
            let constructor = vm.get(value, &key("constructor"))?;
            let copy = vm.construct(constructor, vec![value.clone()], None)?;
            memo.push((id, copy.clone()));
            return Ok(copy);
        }
        Some(Class::Map(table) | Class::Set(table)) => {
            return clone_table(vm, id, table, memo, depth);
        }
        _ => return Err(uncloneable(vm, "This object")),
    };
    memo.push((id, copy.clone()));
    vm.pin(copy.clone());
    if matches!(class, Some(Class::Error)) {
        clone_error(vm, value, &copy)?;
    }
    if matches!(class, Some(Class::Array(_) | Class::Plain | Class::Error)) {
        for name in vm.enumerable_keys(id)? {
            let field = vm.get(value, &name)?;
            let field = clone(vm, &field, memo, depth + 1)?;
            vm.set(&copy, name, field, true)?;
        }
    }
    Ok(copy)
}

/** Views clone their native buffer through the same memo table, retaining overlapping aliases. */
fn clone_view(
    vm: &mut Vm,
    id: ObjectId,
    memo: &mut Vec<(ObjectId, JsValue)>,
    depth: usize,
) -> Res<JsValue> {
    let view = super::view_of(vm, id)
        .filter(|view| !view.invalid)
        .ok_or_else(|| uncloneable(vm, "An out-of-bounds view"))?;
    let buffer = clone(vm, &JsValue::Object(view.buffer), memo, depth + 1)?
        .object()
        .expect("cloned buffer");
    let mut cloned = view;
    cloned.buffer = buffer;
    let prototype = crate::v9::vm::realm::realm().ids.view_prototypes[cloned.kind as usize];
    let copy = vm.alloc(crate::v9::vm::heap::Object::new(
        Class::View(Box::new(cloned)),
        Some(prototype),
    ))?;
    let copy = JsValue::Object(copy);
    memo.push((id, copy.clone()));
    vm.pin(copy.clone());
    Ok(copy)
}

/** An error clone keeps its prototype, name and message. */
fn clone_error(vm: &mut Vm, value: &JsValue, copy: &JsValue) -> Res<()> {
    let (Some(original), Some(target)) = (value.object(), copy.object()) else {
        return Ok(());
    };
    let proto = vm.proto_of(original);
    vm.set_proto(target, proto)?;
    if let Some(object) = vm.heap.get_mut(target) {
        object.class = Class::Error;
    }
    for name in ["name", "message"] {
        if vm.own(original, &key(name))?.is_some() {
            let field = vm.get(value, &key(name))?;
            vm.put(target, name, field)?;
        }
    }
    Ok(())
}

/** A Map or Set clone, its keys and values cloned in insertion order. */
fn clone_table(
    vm: &mut Vm,
    id: ObjectId,
    table: &crate::v9::vm::data::Table,
    memo: &mut Vec<(ObjectId, JsValue)>,
    depth: usize,
) -> Res<JsValue> {
    let set = matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Set(_)));
    let name = if set { "Set" } else { "Map" };
    let constructor = vm.get(&JsValue::Object(vm.global_object()), &key(name))?;
    let copy = vm.construct(constructor, Vec::new(), None)?;
    memo.push((id, copy.clone()));
    vm.pin(copy.clone());
    let adder = vm.get(&copy, &key(if set { "add" } else { "set" }))?;
    for (entry_key, entry_value) in table.entries.iter().flatten() {
        let entry_key = clone(vm, entry_key, memo, depth + 1)?;
        let args = if set {
            vec![entry_key]
        } else {
            vec![entry_key, clone(vm, entry_value, memo, depth + 1)?]
        };
        vm.call(adder.clone(), copy.clone(), args)?;
    }
    Ok(copy)
}
