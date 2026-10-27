/** SharedArrayBuffer is a distinct synchronized backing store; Atomics never aliases ordinary buffer identity. */
use super::*;
use crate::v9::vm::data::SharedBytes;
mod atomics;
pub(in crate::v9) use atomics::{is_poll, notify_shared};
mod snapshots;
pub(in crate::v9) use crate::v9::vm::data::SharedBytes as SharedBuffer;
pub(in crate::v9) use snapshots::{initialize, validate};

/** Native Worker graph cloning keeps the backing Arc instead of converting shared bytes to JSON. */
pub(in crate::v9) fn shared_buffer(vm: &Vm, value: &JsValue) -> Option<SharedBuffer> {
    match vm.heap.get(value.object()?).map(|object| &object.class) {
        Some(Class::SharedArrayBuffer(bytes)) => Some(bytes.clone()),
        _ => None,
    }
}
/** A destination gets a new JS wrapper and an explicit broker grant for the same native backing. */
pub(in crate::v9) fn import_shared_buffer(vm: &mut Vm, buffer: SharedBuffer) -> Res<ObjectId> {
    let buffer = buffer.for_agent(agent(vm));
    #[cfg(feature = "conformance")]
    let buffer = match vm.page.environment.conformance_workers.clone() {
        Some(host) => host.shared(vm.page.environment.conformance_worker_id, buffer)?,
        None => buffer,
    };
    vm.alloc(Object::new(
        Class::SharedArrayBuffer(buffer),
        Some(realm().ids.shared_array_buffer_prototype),
    ))
}
/** Broker roots and workers have disjoint callback namespaces, even when trusted sessions exchange an Arc. */
fn agent(vm: &Vm) -> u64 {
    #[cfg(feature = "conformance")]
    if let Some(host) = &vm.page.environment.conformance_workers {
        return host.agent(vm.page.environment.conformance_worker_id);
    }
    let _ = vm;
    0
}

/** Shared accessors are branded separately, even when borrowed through Function.call. */
fn this_shared(vm: &mut Vm, call: &Call) -> Res<SharedBuffer> {
    shared_buffer(vm, &call.this)
        .ok_or_else(|| vm.type_error("Receiver is not a SharedArrayBuffer"))
}

/** Both fixed and growable shared storage use the existing byte budget and realm intrinsic banks. */
pub(super) fn install(builder: &mut Builder) {
    let proto = builder.object(Some(builder.ids.object_prototype));
    builder.ids.shared_array_buffer_prototype = proto;
    let ctor = builder.constructor("SharedArrayBuffer", 1, construct, proto, None);
    builder.ids.shared_array_buffer = ctor;
    builder.global("SharedArrayBuffer", ctor);
    for (name, getter) in [
        ("byteLength", "get byteLength"),
        ("maxByteLength", "get maxByteLength"),
        ("growable", "get growable"),
    ] {
        let get = builder.function(getter, 0, accessor);
        builder.put(
            proto,
            Key::from(name),
            Static::Accessor(Some(get), None),
            CONFIGURABLE,
        );
    }
    builder.method(proto, "slice", 2, slice);
    builder.method(proto, "grow", 1, grow);
    tag(builder, proto, "SharedArrayBuffer");
    species_getter(builder, ctor);
    atomics::install(builder);
}

/** Option coercion and newTarget prototype lookup retain their real observable order. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Constructor SharedArrayBuffer requires 'new'"));
    };
    let length = to_index(vm, &call.arg(0), "shared array buffer length")?;
    let maximum = buffer_state::options(vm, &call.arg(1))?;
    if length > BUFFER_BYTES || maximum.is_some_and(|max| max < length || max > BUFFER_BYTES) {
        return Err(vm.range_error("Invalid shared array buffer length"));
    }
    let proto = vm.prototype_for(&target, realm().ids.shared_array_buffer_prototype)?;
    let buffer = SharedBytes::new(length, maximum)?;
    let id = import_shared_buffer(vm, buffer)?;
    if let Some(object) = vm.heap.get_mut(id) {
        object.proto = Some(proto)
    }
    Ok(JsValue::Object(id))
}

/** Length and growability are internal backing metadata, independent of public property changes. */
fn accessor(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let buffer = this_shared(vm, &call)?;
    Ok(match callee_name(&call) {
        "get growable" => JsValue::Boolean(buffer.maximum().is_some()),
        "get maxByteLength" => {
            JsValue::Number(buffer.maximum().unwrap_or_else(|| buffer.len()) as f64)
        }
        _ => JsValue::Number(buffer.len() as f64),
    })
}

/** Growth cannot shrink or detach storage and leaves all existing byte offsets stable. */
fn grow(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let buffer = this_shared(vm, &call)?;
    if buffer.maximum().is_none() {
        return Err(vm.type_error("SharedArrayBuffer is not growable"));
    }
    let length = to_index(vm, &call.arg(0), "shared array buffer length")?;
    if !buffer.grow(length) {
        return Err(vm.range_error("Invalid shared array buffer grow length"));
    }
    Ok(JsValue::Undefined)
}

/** Species allocation is real, and a wrapper aliasing the original backing is rejected. */
fn slice(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let source = this_shared(vm, &call)?;
    let length = source.len();
    let start = vm.relative_index(&call.arg(0), length, 0)?;
    let end = vm.relative_index(&call.arg(1), length, length)?.max(start);
    let ctor = species_constructor(vm, &call.this)?;
    let result = vm.construct(ctor, vec![JsValue::Number((end - start) as f64)], None)?;
    let Some(target) = shared_buffer(vm, &result) else {
        return Err(vm.type_error("SharedArrayBuffer species did not create shared storage"));
    };
    if source.same_store(&target) || target.len() < end - start {
        return Err(vm.type_error("Invalid SharedArrayBuffer species result"));
    }
    let bytes = source
        .read(start, end - start)
        .expect("shared memory only grows");
    target.write(0, &bytes);
    Ok(result)
}

/** Nullish species uses the calling method's intrinsic constructor; arbitrary constructors are observed. */
fn species_constructor(vm: &mut Vm, value: &JsValue) -> Res<JsValue> {
    let fallback = JsValue::Object(realm().ids.shared_array_buffer);
    let ctor = vm.get(value, &Key::from("constructor"))?;
    if ctor == JsValue::Undefined {
        return Ok(fallback);
    }
    if ctor.object().is_none() {
        return Err(vm.type_error("SharedArrayBuffer constructor is not an object"));
    }
    let species = vm.get(&ctor, &Key::Sym(symbol::SPECIES))?;
    if species.nullish() {
        return Ok(fallback);
    }
    if !vm.is_constructor(&species) {
        return Err(vm.type_error("SharedArrayBuffer species is not a constructor"));
    }
    Ok(species)
}
