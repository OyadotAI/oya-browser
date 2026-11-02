/** ShadowRealm contexts expose primitives and wrapped callables through an actual realm membrane. */
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, FunctionKind};
use crate::v9::vm::heap::{CONFIGURABLE, Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, Static, realm_for, symbol};
use crate::v9::vm::{Call, Res, Thrown, Vm};
use std::sync::OnceLock;
mod imports;

/** Native wrapper bodies are immutable; captured target objects are ordinary traced heap edges. */
static WRAPPED_CALL: OnceLock<u32> = OnceLock::new();
/** Private slots cannot be named or forged by evaluated JavaScript. */
const GLOBAL_SLOT: &str = "shadowRealmGlobal";

/** The constructor creates an independent context; no browser or filesystem authority is added. */
pub(super) fn install(builder: &mut Builder) {
    let proto = builder.object(Some(builder.ids.object_prototype));
    let ctor = builder.constructor("ShadowRealm", 0, construct, proto, None);
    builder.global("ShadowRealm", ctor);
    builder.method(proto, "evaluate", 1, evaluate);
    imports::install(builder, proto);
    builder.put(
        proto,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(crate::v9::vm::realm::string("ShadowRealm")),
        CONFIGURABLE,
    );
    let _ = WRAPPED_CALL.set(builder.native("", 0, wrapped_call));
}

/** Only construction creates the unforgeable internal context slot. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(new_target) = call.new_target else {
        return Err(vm.type_error("Constructor ShadowRealm requires 'new'"));
    };
    let fallback = vm
        .get(&JsValue::Object(call.callee), &Key::prototype())?
        .object()
        .expect("intrinsic prototype");
    let proto = vm.prototype_for(&new_target, fallback)?;
    let token = vm.realms.contexts[vm.active_realm as usize].token;
    let bank = super::realms::create_bank(vm, Some(token))?;
    vm.realms.contexts[bank as usize].shadow = true;
    let global = vm.realms.contexts[bank as usize]
        .global
        .expect("new context");
    let id = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    vm.heap.put(
        id,
        Key::private(GLOBAL_SLOT),
        Prop {
            slot: Slot::Data(JsValue::Object(global)),
            attrs: 0,
        },
    );
    Ok(JsValue::Object(id))
}

/** Brand checks read only the actual object's internal slot, never a proxy or inherited property. */
fn context(vm: &mut Vm, this: &JsValue) -> Res<u32> {
    context_for(vm, this, "ShadowRealm.prototype.evaluate")
}
/** The brand refusal identifies the actual method and describes the receiver without calling script. */
fn context_for(vm: &mut Vm, this: &JsValue, method: &str) -> Res<u32> {
    let global = this
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| object.props.get(&Key::private(GLOBAL_SLOT)))
        .and_then(|prop| match &prop.slot {
            Slot::Data(value) => value.object(),
            _ => None,
        });
    global.map(|global| vm.object_realm(global)).ok_or_else(|| {
        vm.type_error(&format!(
            "Method {method} called on incompatible receiver {}",
            receiver_label(this)
        ))
    })
}

/** Primitive labels use value spelling, while objects are never coerced for an error message. */
fn receiver_label(value: &JsValue) -> String {
    match value {
        #[cfg(feature = "conformance")]
        JsValue::Smi(number) => number.to_string(),
        JsValue::Number(number) => number.to_string(),
        JsValue::Undefined => "undefined".into(),
        JsValue::Null => "null".into(),
        JsValue::Boolean(value) => value.to_string(),
        JsValue::String(value) => value.lossy(),
        _ => "#<Object>".into(),
    }
}

/** Indirect eval keeps lexical declarations local while global var bindings belong to the shadow. */
fn evaluate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.with_trace_boundary(
        call.callee,
        call.this.clone(),
        "ShadowRealm.evaluate",
        |vm| evaluate_in_context(vm, call),
    )
}

/** The boundary remains active during compilation, execution and membrane error copying. */
fn evaluate_in_context(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let bank = context(vm, &call.this)?;
    let JsValue::String(source) = call.arg(0) else {
        return Err(vm.type_error("ShadowRealm evaluate expects a string"));
    };
    let code = crate::v9::builtins::compile_indirect_eval(vm, source.lossy())?;
    let result = vm.with_realm(bank, |vm| vm.run_script(code));
    match result {
        Ok(value) => wrap(vm, value),
        Err(error) => Err(copy_error(vm, error, "ShadowRealm evaluate threw")),
    }
}

/** Objects cross only as fresh callable wrappers; ordinary objects never cross the membrane. */
fn wrap(vm: &mut Vm, value: JsValue) -> Res<JsValue> {
    let value = match value
        .object()
        .and_then(|id| vm.heap.get(id))
        .and_then(|object| object.function())
        .map(|function| &function.kind)
    {
        Some(FunctionKind::Native { id, slots }) if Some(id) == WRAPPED_CALL.get() => {
            slots[0].clone()
        }
        _ => value,
    };
    let Some(target) = value.object() else {
        return Ok(value);
    };
    if !vm.is_callable(&value) {
        return Err(vm.type_error(&format!("{} is not a function", brief(vm, &value))));
    }
    let wrapper =
        vm.native_closure(*WRAPPED_CALL.get().expect("installed"), vec![value.clone()])?;
    match capture_metadata(vm, wrapper, target) {
        Ok(()) => Ok(JsValue::Object(wrapper)),
        Err(error) => Err(copy_error(vm, error, "Cannot wrap target callable")),
    }
}

/** CopyNameAndLength observes metadata once in specification order, including callable proxy traps. */
fn capture_metadata(vm: &mut Vm, wrapper: ObjectId, target: ObjectId) -> Res<()> {
    let key = Key::length();
    let length = if vm.own(target, &key)?.is_some() {
        match vm.get(&JsValue::Object(target), &key)? {
            JsValue::Number(n) if !n.is_nan() => n.trunc().max(0.0),
            _ => 0.0,
        }
    } else {
        0.0
    };
    let name = vm.capture_name_snapshot(target, true)?;
    vm.set_name_snapshot(wrapper, name)?;
    {
        let (key, value) = ("length", JsValue::Number(length));
        vm.heap.put(
            wrapper,
            Key::from(key),
            Prop {
                slot: Slot::Data(value),
                attrs: CONFIGURABLE,
            },
        );
    }
    Ok(())
}

/** Inspection reads a wrapper's captured target without calling it, so the membrane stays unobservable. */
pub(super) fn wrapped_target(vm: &Vm, id: ObjectId) -> Option<JsValue> {
    match vm.heap.get(id)?.function().map(|function| &function.kind)? {
        FunctionKind::Native { id, slots } if Some(id) == WRAPPED_CALL.get() => {
            slots.first().cloned()
        }
        _ => None,
    }
}

/** Call arguments and receivers are wrapped in the target context, and results in the caller. */
fn wrapped_call(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = vm.slots(call.callee)[0].clone();
    let bank = vm.constructor_realm(&target)?;
    let arguments = vm.with_realm(bank, |vm| {
        let receiver = wrap(vm, call.this)?;
        let args = call
            .args
            .iter()
            .cloned()
            .map(|value| wrap(vm, value))
            .collect::<Res<Vec<_>>>()?;
        Ok((receiver, args))
    });
    let (receiver, args) = arguments.map_err(|error| membrane_error(vm, error))?;
    let result = vm.call(target, receiver, args);
    match result {
        Ok(value) => wrap(vm, value),
        Err(error) => Err(copy_error(vm, error, "WrappedFunction threw")),
    }
}

/** Argument conversion errors belong to the caller even though wrapper allocation uses the target. */
fn membrane_error(vm: &mut Vm, error: Thrown) -> Thrown {
    let Thrown::Value(value) = error else {
        return error;
    };
    let message = value
        .object()
        .and_then(|id| data_string(vm, id, "message"))
        .unwrap_or_else(|| brief(vm, &value));
    vm.type_error(&message)
}

/** An exception's origin realm cannot leak through its prototype or run user message accessors. */
fn copy_error(vm: &mut Vm, error: Thrown, prefix: &str) -> Thrown {
    let Thrown::Value(value) = error else {
        return error;
    };
    let description = brief(vm, &value);
    let stack = match value.object().filter(|id| {
        vm.heap
            .get(*id)
            .is_some_and(|object| matches!(object.class, Class::Error))
    }) {
        Some(id) => match copied_stack(vm, id) {
            Ok(stack) => Some(stack),
            Err(Thrown::Value(error)) => {
                return vm.type_error(&format!(
                    "Error stack getter threw in ShadowRealm ({})",
                    brief(vm, &error)
                ));
            }
            Err(fatal) => return fatal,
        },
        None => None,
    };
    if stack.as_ref().is_some_and(|value| value.object().is_some()) {
        return vm.type_error(&format!(
            "Error stack is not a string in ShadowRealm ({})",
            brief(vm, stack.as_ref().unwrap())
        ));
    }
    let copied = vm.type_error(&format!("{prefix} ({description})"));
    if let (Thrown::Value(JsValue::Object(id)), Some(stack)) = (&copied, stack)
        && !stack.nullish()
    {
        vm.heap.put(
            *id,
            Key::from("stack"),
            Prop {
                slot: Slot::Data(stack),
                attrs: crate::v9::vm::heap::HIDDEN_ATTRS,
            },
        );
    }
    copied
}

/** The internal stack path formats captured Errors without invoking a replacement public getter. */
fn copied_stack(vm: &mut Vm, error: ObjectId) -> Res<JsValue> {
    if let Some(Slot::Data(value)) = vm
        .heap
        .get(error)
        .and_then(|object| object.props.get(&Key::from("stack")))
        .map(|prop| &prop.slot)
    {
        return Ok(value.clone());
    }
    crate::v9::builtins::error_stack(vm, error)
}

/** Error text uses inert data slots only; getters and proxy traps are never inspected. */
fn brief(vm: &Vm, value: &JsValue) -> String {
    let Some(id) = value.object() else {
        return match value {
            JsValue::String(text) => text.lossy(),
            other => format!("{other:?}"),
        };
    };
    if !vm
        .heap
        .get(id)
        .is_some_and(|object| matches!(object.class, Class::Error))
    {
        return "[object Object]".into();
    }
    let name = data_string(vm, id, "name").unwrap_or_else(|| "Error".into());
    match data_string(vm, id, "message") {
        Some(message) if !message.is_empty() => format!("{name}: {message}"),
        _ => name,
    }
}

/** A descriptor walk uses immutable definitions and direct heap fields, stopping at accessors/proxies. */
fn data_string(vm: &Vm, mut id: ObjectId, name: &str) -> Option<String> {
    let key = Key::from(name);
    for _ in 0..vm.limits.frames {
        if let Some(object) = vm.heap.get(id) {
            if matches!(object.class, Class::Proxy(_)) {
                return None;
            }
            if let Some(prop) = object.props.get(&key) {
                return match &prop.slot {
                    Slot::Data(JsValue::String(text)) => Some(text.lossy()),
                    _ => None,
                };
            }
            id = object.proto?;
        } else {
            let def = realm_for(id).object(id)?;
            if let Some((_, value, _)) = def.props.iter().find(|(name, _, _)| name == &key) {
                return match value {
                    Static::Value(JsValue::String(text)) => Some(text.lossy()),
                    _ => None,
                };
            }
            id = def.proto?;
        }
    }
    None
}
