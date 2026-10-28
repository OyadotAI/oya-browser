/** Bounded primitive shared-container storage keeps worker writes on one synchronized backing. */
use super::shared;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::{Class, SharedBytes};
use crate::v9::vm::heap::{ENUMERABLE, Key, Object, Prop, Slot, WRITABLE};
use crate::v9::vm::{Res, Vm};

/** Physical bytes are bounded independently from the logical container field count. */
const STORAGE_BYTES: usize = 65_536;
const HEADER_BYTES: usize = 4;
const STORAGE_KEY: &str = "conformance-shared-container-storage";
const REFERENCES_KEY: &str = "conformance-shared-container-references";

/** Shared slots never contain isolate-local heap IDs; references index a traced local capability list. */
#[derive(Clone, serde::Serialize, serde::Deserialize)]
enum SharedSlot {
    Primitive(JsValue),
    Reference(usize),
}

/** The worker graph alone may install this hidden reference edge. */
pub(super) fn references_key() -> Key {
    Key::private(REFERENCES_KEY)
}

/** Return the traced references array, which the worker graph transports as real edges. */
pub(super) fn references(vm: &Vm, id: ObjectId) -> Option<JsValue> {
    match &vm.heap.get(id)?.props.get(&references_key())?.slot {
        Slot::Data(value @ JsValue::Object(_)) => Some(value.clone()),
        _ => None,
    }
}

/** Resolve an index only through an actual locally rooted capability array. */
fn reference(vm: &Vm, id: ObjectId, index: usize) -> Option<JsValue> {
    let array = references(vm, id)?.object()?;
    let Class::Array(values) = &vm.heap.get(array)?.class else {
        return None;
    };
    let value = values.get(index)?.clone();
    value
        .object()
        .filter(|id| shared::is_shared_object(vm, *id))
        .map(|_| value)
}

/** A packet transfers a capability and inert fixed layout, never sender heap identities. */
#[derive(Clone, Debug)]
pub(super) struct Packet {
    pub store: SharedBytes,
    pub array: bool,
    pub keys: Vec<Key>,
}

/** Only JSON-roundtrippable primitives enter the current bounded transport. */
fn primitive(value: &JsValue) -> bool {
    !matches!(
        value,
        JsValue::Object(_) | JsValue::Symbol(_) | JsValue::Hole | JsValue::LocalNumber(_) | JsValue::SharedNumber(_)
    )
}

/** Find the backing granted to this wrapper, if transfer has activated it. */
fn backing(vm: &Vm, id: ObjectId) -> Option<SharedBytes> {
    let object = vm.heap.get(id)?;
    let Slot::Data(JsValue::Object(backing)) = &object.props.get(&Key::private(STORAGE_KEY))?.slot
    else {
        return None;
    };
    match &vm.heap.get(*backing)?.class {
        Class::SharedArrayBuffer(store) => Some(store.clone()),
        _ => None,
    }
}
/** Active worker-shared wrappers need atomic descriptor operations, not ordinary heap writes. */
pub(in crate::v9) fn active(vm: &Vm, id: ObjectId) -> bool {
    backing(vm, id).is_some()
}

/** Existing fixed fields accept compatible descriptor updates through the shared backing. */
pub(in crate::v9) fn define(vm: &mut Vm, id: ObjectId, key: &Key, descriptor: &crate::v9::vm::property::Descriptor) -> Res<Option<bool>> {
    if !active(vm, id) || field_index(vm, id, key).is_none() { return Ok(None); }
    if descriptor.accessor() || descriptor.configurable == Some(true) || descriptor.enumerable == Some(false) || descriptor.writable == Some(false) { return Ok(Some(false)); }
    match &descriptor.value { Some(value) => write(vm, id, key, value), None => Ok(Some(true)) }
}

/** Layout comes only from existing fixed own slots; private metadata is never serialized. */
fn keys(vm: &Vm, id: ObjectId) -> Vec<Key> {
    let Some(object) = vm.heap.get(id) else {
        return Vec::new();
    };
    match &object.class {
        Class::Array(elements) => (0..elements.len())
            .map(|index| Key::from(index.to_string().as_str()))
            .collect(),
        _ => object
            .props
            .iter()
            .filter_map(|(key, _)| (!key.hidden()).then(|| key.clone()))
            .collect(),
    }
}
/** Arrays address an index directly rather than enumerating their whole logical length. */
fn field_index(vm: &Vm, id: ObjectId, key: &Key) -> Option<usize> {
    let object = vm.heap.get(id)?;
    if let Class::Array(elements) = &object.class {
        return key
            .index()
            .map(|index| index as usize)
            .filter(|index| *index < elements.len());
    }
    keys(vm, id).iter().position(|candidate| candidate == key)
}

/** Shared-reference updates may use only capabilities already present in this graph. */
fn slot(vm: &mut Vm, id: ObjectId, value: &JsValue) -> Res<SharedSlot> {
    if matches!(value, JsValue::LocalNumber(_) | JsValue::SharedNumber(_)) { return Err(vm.unsupported_error("boxed number in legacy shared transport")); }
    if primitive(value) {
        return Ok(SharedSlot::Primitive(value.clone()));
    }
    let Some(target) = value
        .object()
        .filter(|target| shared::is_shared_object(vm, *target))
    else {
        return Err(vm.type_error("Value cannot enter shared storage"));
    };
    let refs = references(vm, id)
        .and_then(|value| value.object())
        .and_then(|id| vm.heap.get(id));
    let index = match refs.map(|object| &object.class) {
        Some(Class::Array(values)) => values
            .iter()
            .position(|value| value == &JsValue::Object(target)),
        _ => None,
    }
    .ok_or_else(|| vm.unsupported_error("new shared-container reference after transfer"))?;
    Ok(SharedSlot::Reference(index))
}

/** Resolve native stored values through a validated, traced capability edge. */
fn resolve(vm: &mut Vm, id: ObjectId, value: &SharedSlot) -> Res<JsValue> {
    match value {
        SharedSlot::Primitive(value) => Ok(value.clone()),
        SharedSlot::Reference(index) => reference(vm, id, *index)
            .ok_or_else(|| vm.type_error("Invalid shared container reference")),
    }
}

/** Payload is length-prefixed, validated in full and always bounded by the native store. */
fn decode(bytes: &[u8]) -> Option<Vec<SharedSlot>> {
    let length = u32::from_le_bytes(bytes.get(..HEADER_BYTES)?.try_into().ok()?) as usize;
    let values: Vec<SharedSlot> =
        serde_json::from_slice(bytes.get(HEADER_BYTES..HEADER_BYTES.checked_add(length)?)?).ok()?;
    values
        .iter()
        .all(|value| match value {
            SharedSlot::Primitive(value) => primitive(value),
            SharedSlot::Reference(_) => true,
        })
        .then_some(values)
}
fn encode(bytes: &mut [u8], values: &[SharedSlot]) -> bool {
    let Ok(data) = serde_json::to_vec(values) else {
        return false;
    };
    if data.len() > bytes.len().saturating_sub(HEADER_BYTES) {
        return false;
    }
    bytes[..HEADER_BYTES].copy_from_slice(&(data.len() as u32).to_le_bytes());
    bytes[HEADER_BYTES..HEADER_BYTES + data.len()].copy_from_slice(&data);
    true
}

/** Activating transfer snapshots actual current primitive fields before publishing the backing. */
pub(super) fn packet(vm: &mut Vm, id: ObjectId) -> Res<Packet> {
    vm.pin(JsValue::Object(id));
    let array = matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Array(_)));
    let keys = keys(vm, id);
    if let Some(store) = backing(vm, id) {
        return Ok(Packet { store, array, keys });
    }
    let values = keys
        .iter()
        .map(|key| {
            vm.own(id, key).map(|prop| match prop.map(|p| p.slot) {
                Some(Slot::Data(value)) => value,
                _ => JsValue::Undefined,
            })
        })
        .collect::<Res<Vec<_>>>()?;
    let mut refs = Vec::new();
    let mut slots = Vec::new();
    for value in values {
        if primitive(&value) {
            slots.push(SharedSlot::Primitive(value));
            continue;
        }
        let Some(target) = value
            .object()
            .filter(|target| shared::is_shared_object(vm, *target))
        else {
            return Err(vm.unsupported_error("unshareable shared-container reference"));
        };
        let index = refs
            .iter()
            .position(|value| value == &JsValue::Object(target))
            .unwrap_or_else(|| {
                refs.push(JsValue::Object(target));
                refs.len() - 1
            });
        slots.push(SharedSlot::Reference(index));
    }
    let store = SharedBytes::new(STORAGE_BYTES, None)?;
    if !store
        .atomic(0, STORAGE_BYTES, true, |bytes| encode(bytes, &slots))
        .unwrap_or(false)
    {
        return Err(
            crate::protocol::EngineError::limit("shared container bytes", STORAGE_BYTES).into(),
        );
    }
    let buffer = crate::v9::builtins::import_shared_buffer(vm, store.clone())?;
    vm.pin(JsValue::Object(buffer));
    for value in &refs {
        vm.pin(value.clone());
    }
    let refs = vm.new_array(refs)?;
    vm.heap.put(
        id,
        Key::private(STORAGE_KEY),
        Prop::data(JsValue::Object(buffer)),
    );
    vm.heap
        .put(id, references_key(), Prop::data(JsValue::Object(refs)));
    Ok(Packet { store, array, keys })
}

/** Restore a local fixed-layout wrapper, reusing an existing wrapper for the same capability. */
pub(super) fn restore(vm: &mut Vm, packet: &Packet) -> Res<ObjectId> {
    if let Some(id) = vm
        .heap
        .objects
        .keys()
        .copied()
        .find(|id| backing(vm, *id).is_some_and(|other| other.same_store(&packet.store)))
    {
        return Ok(id);
    }
    let object = if packet.array {
        vm.new_array(vec![JsValue::Undefined; packet.keys.len()])?
    } else {
        vm.alloc(Object::new(Class::Plain, None))?
    };
    vm.pin(JsValue::Object(object));
    vm.set_proto(object, None)?;
    if !packet.array {
        for key in &packet.keys {
            vm.heap.put(
                object,
                key.clone(),
                Prop {
                    slot: Slot::Data(JsValue::Undefined),
                    attrs: WRITABLE | ENUMERABLE,
                },
            );
        }
    }
    shared::mark_transport(vm, object, packet.array)?;
    let buffer = crate::v9::builtins::import_shared_buffer(vm, packet.store.clone())?;
    vm.heap.put(
        object,
        Key::private(STORAGE_KEY),
        Prop::data(JsValue::Object(buffer)),
    );
    crate::v9::builtins::integrity::prevent_extensions(vm, object)?;
    Ok(object)
}

/** Reads select the real current value after another worker has committed a write. */
pub(in crate::v9) fn read(vm: &mut Vm, id: ObjectId, key: &Key) -> Res<Option<JsValue>> {
    let Some(store) = backing(vm, id) else {
        return Ok(None);
    };
    let Some(index) = field_index(vm, id, key) else {
        return Ok(None);
    };
    let values = store
        .atomic(0, STORAGE_BYTES, false, |bytes| decode(bytes))
        .flatten()
        .ok_or_else(|| vm.type_error("Invalid shared container storage"))?;
    let value = resolve(
        vm,
        id,
        values.get(index).ok_or_else(|| {
            crate::protocol::EngineError::invalid("Invalid shared container layout")
        })?,
    )?;
    Ok(Some(value))
}

/** Writes serialize and replace a field inside one native atomic event. */
pub(in crate::v9) fn write(
    vm: &mut Vm,
    id: ObjectId,
    key: &Key,
    value: &JsValue,
) -> Res<Option<bool>> {
    let Some(store) = backing(vm, id) else {
        return Ok(None);
    };
    let Some(index) = field_index(vm, id, key) else {
        return Ok(Some(false));
    };
    let slot_value = slot(vm, id, value)?;
    let updated = store
        .atomic(0, STORAGE_BYTES, true, |bytes| {
            let Some(mut values) = decode(bytes) else {
                return false;
            };
            let Some(slot) = values.get_mut(index) else {
                return false;
            };
            *slot = slot_value;
            encode(bytes, &values)
        })
        .unwrap_or(false);
    if !updated {
        return Err(
            crate::protocol::EngineError::limit("shared container bytes", STORAGE_BYTES).into(),
        );
    }
    Ok(Some(true))
}

/** Implement primitive Atomics on one shared field inside a single synchronized event. */
pub(in crate::v9) fn atomic(vm: &mut Vm, call: &crate::v9::vm::Call, name: &str) -> Res<JsValue> {
    let id = call
        .arg(0)
        .object()
        .ok_or_else(|| vm.type_error("Invalid shared container"))?;
    let key = vm.to_key(&call.arg(1))?;
    if key == Key::length()
        && matches!(
            vm.heap.get(id).map(|object| &object.class),
            Some(Class::Array(_))
        )
    {
        if name == "load" {
            return vm.get(&JsValue::Object(id), &key);
        }
        return Err(vm.type_error("Cannot modify shared array length"));
    }
    let Some(index) = field_index(vm, id, &key) else {
        return Err(vm.range_error("Invalid shared container field"));
    };
    let store = packet(vm, id)?.store;
    let operand = if name == "load" {
        None
    } else {
        Some(slot(vm, id, &call.arg(2))?)
    };
    let replacement = if name == "compareExchange" {
        Some(slot(vm, id, &call.arg(3))?)
    } else {
        operand.clone()
    };
    let old = store
        .atomic(0, STORAGE_BYTES, name != "load", |bytes| {
            let mut values = decode(bytes)?;
            let old = values.get(index)?.clone();
            let equal = match (&old, &operand) {
                (SharedSlot::Primitive(old), Some(SharedSlot::Primitive(expected))) => {
                    old.strict_equal(expected)
                }
                (SharedSlot::Reference(old), Some(SharedSlot::Reference(expected))) => {
                    old == expected
                }
                _ => false,
            };
            if name != "load" && (name != "compareExchange" || equal) {
                values[index] = replacement.clone()?;
                if !encode(bytes, &values) {
                    return None;
                }
            }
            Some(old)
        })
        .flatten()
        .ok_or_else(|| vm.type_error("Invalid or oversized shared container storage"))?;
    if name == "store" {
        Ok(call.arg(2))
    } else {
        resolve(vm, id, &old)
    }
}
