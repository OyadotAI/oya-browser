/** Branded BroadcastChannel WebIDL operations record ordered native work without delivering inline. */
use super::*;
use crate::v9::value::JsString;
use crate::v9::vm::Call;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{CONFIGURABLE, HIDDEN_ATTRS, Object};
use crate::v9::vm::realm::{Builder, Static, string, symbol};
use std::sync::OnceLock;

static PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();
pub(super) static MESSAGE_PROTOTYPE: OnceLock<ObjectId> = OnceLock::new();

/** Capture the intrinsic rather than consulting an author-replaced MessageEvent constructor. */
pub(in crate::v9) fn message_prototype(id: ObjectId) {
    let _ = MESSAGE_PROTOTYPE.set(id);
}

/** Install actual operations before generic interface shape completion runs. */
pub(in crate::v9) fn install(builder: &mut Builder) {
    let prototype = builder.object(Some(builder.ids.event_target_prototype));
    builder.getter(prototype, "name", name);
    builder.host_method(prototype, "postMessage", 1, post);
    builder.host_method(prototype, "close", 0, close);
    for name in ["onmessage", "onmessageerror"] {
        builder.accessor(prototype, name, Some(get_handler), Some(set_handler));
    }
    builder.put(
        prototype,
        Key::Sym(symbol::TO_STRING_TAG),
        Static::Value(string("BroadcastChannel")),
        CONFIGURABLE,
    );
    let constructor = builder.constructor("BroadcastChannel", 1, construct, prototype, None);
    builder.global("BroadcastChannel", constructor);
    let _ = PROTOTYPE.set(prototype);
}

/** Conversion and subclass prototype getters run before final admission because either may reenter. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(
            vm.type_error("Failed to construct 'BroadcastChannel': Please use the 'new' operator.")
        );
    };
    if call.args.is_empty() {
        return Err(vm.type_error("BroadcastChannel requires a name"));
    }
    let name = vm.to_string(&call.arg(0))?;
    if name.len() > BROADCAST_NAME_UNITS {
        return Err(
            EngineError::limit("BroadcastChannel name UTF-16 units", BROADCAST_NAME_UNITS).into(),
        );
    }
    let name: Vec<u16> = name.code_units().collect();
    let prototype = vm.prototype_for(
        &target,
        *PROTOTYPE.get().expect("installed BroadcastChannel"),
    )?;
    vm.pin(JsValue::Object(prototype));
    if vm.page.state.broadcast.channels.len() >= BROADCAST_CHANNELS {
        vm.collect_now()?;
        if vm.page.state.broadcast.channels.len() >= BROADCAST_CHANNELS {
            return Err(EngineError::limit("BroadcastChannel handles", BROADCAST_CHANNELS).into());
        }
    }
    let id = vm
        .page
        .state
        .broadcast
        .next
        .checked_add(1)
        .ok_or_else(|| EngineError::invalid("BroadcastChannel identities exhausted."))?;
    let command = BroadcastCommand::Open {
        id,
        name: name.clone(),
    };
    admit(vm, &command)?;
    let object = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.heap.put(
        object,
        brand(),
        Prop {
            slot: Slot::Data(JsValue::Boolean(true)),
            attrs: HIDDEN_ATTRS,
        },
    );
    vm.page.state.broadcast.next = id;
    vm.page.state.broadcast.channels.insert(
        id,
        state::Handle {
            object,
            name,
            closed: false,
        },
    );
    vm.page.effects.broadcast_commands.push(command);
    Ok(JsValue::Object(object))
}

/** Names remain readable after close and preserve unmatched UTF-16 surrogates. */
fn name(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    Ok(JsValue::String(JsString::from_slice(
        &vm.page.state.broadcast.channels[&id].name,
    )?))
}

/** Closed-state refusal precedes clone getters; clone effects happen before the final ordered post. */
fn post(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    if call.args.is_empty() {
        return Err(vm.type_error("BroadcastChannel.postMessage requires data"));
    }
    if vm.page.state.worker_closed {
        return Ok(JsValue::Undefined);
    }
    if closed(vm, id) {
        return Err(vm.dom_exception("InvalidStateError", "BroadcastChannel is closed"));
    }
    let data = crate::v9::worker_clone::encode(vm, &call.arg(0))?;
    let command = BroadcastCommand::Post { id, data };
    admit(vm, &command)?;
    vm.page.effects.broadcast_commands.push(command);
    Ok(JsValue::Undefined)
}

/** Repeated close is harmless and never emits duplicate routing commands. */
fn close(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    if !closed(vm, id) {
        let command = BroadcastCommand::Close { id };
        admit(vm, &command)?;
        vm.page
            .state
            .broadcast
            .channels
            .get_mut(&id)
            .expect("branded channel")
            .closed = true;
        vm.page.effects.broadcast_commands.push(command);
    }
    Ok(JsValue::Undefined)
}

/** EventHandler getters read the dedicated native slot, not an ordinary overridable property. */
fn get_handler(vm: &mut Vm, call: Call) -> Res<JsValue> {
    identity(vm, &call.this)?;
    Ok(handler(vm, &call.this, super::super::dom::native_name(&call)).unwrap_or(JsValue::Null))
}

/** Non-callable handler assignments clear the handler, as EventHandler conversion requires. */
fn set_handler(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    let object = vm.page.state.broadcast.channels[&id].object;
    let value = call.arg(0);
    let value = if vm.is_callable(&value) {
        value
    } else {
        JsValue::Null
    };
    vm.heap.put(
        object,
        handler_key(super::super::dom::native_name(&call)),
        Prop {
            slot: Slot::Data(value),
            attrs: HIDDEN_ATTRS,
        },
    );
    Ok(JsValue::Undefined)
}

/** The event dispatcher reads this native handler without invoking author property accessors. */
pub(in crate::v9) fn handler(vm: &Vm, value: &JsValue, name: &str) -> Option<JsValue> {
    if !["onmessage", "onmessageerror"].contains(&name) {
        return None;
    }
    let object = vm.heap.get(value.object()?)?;
    match &object.props.get(&handler_key(name))?.slot {
        Slot::Data(value) => Some(value.clone()),
        _ => None,
    }
}
