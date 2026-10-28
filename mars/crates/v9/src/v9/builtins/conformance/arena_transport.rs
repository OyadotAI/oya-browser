/** Traced shared-container graphs store values in a session-owned synchronized arena. */
use super::shared;
use crate::protocol::EngineError;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::{Class, SharedGc, SharedGcArena, SharedGcValue};
use crate::v9::vm::heap::{Key, Object, Prop, Slot, ENUMERABLE, WRITABLE};
use crate::v9::vm::{Res, Vm};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

const ROOT_KEY: &str = "conformance-shared-container-arena";
const LEGACY_KEY: &str = "conformance-shared-container-storage";
const META_PREFIX: &[u8] = b"d8-shared-container-v1:";
pub(super) const MUTEX_META: &[u8] = b"d8-shared-mutex-v1";
pub(super) const CONDITION_META: &[u8] = b"d8-shared-condition-v1";
pub(super) const NUMBER_META: &[u8] = b"d8-shared-number-v1";

/** Shared boxed numbers use real traced nodes and local hidden roots. */
pub(in crate::v9) fn new_shared_number(vm: &mut Vm, value: f64) -> Res<JsValue> {
    let arena = arena(vm)?;
    let mark = vm.pins.len();
    let result = allocate_shared_number(vm, &arena, value).and_then(|root| restore_shared_number(vm, root));
    vm.pins.truncate(mark);
    result
}
/** Allocate a real number in the target container arena, keeping its grant alive. */
fn allocate_shared_number(vm: &mut Vm, arena: &SharedGcArena, value: f64) -> Res<SharedGc> {
    let allocate = || arena.allocate(NUMBER_META, 0, false, vec![SharedGcValue::Number(value.to_bits())]);
    let root = match allocate() {
        Ok(root) => root,
        Err(error) if error.code == "limit_exceeded" => { vm.collect_now()?; allocate()? },
        Err(error) => return Err(error.into()),
    };
    match grant(vm, root.clone()) {
        Ok(root) => Ok(root),
        Err(error) if error.code == "limit_exceeded" => { vm.collect_now()?; Ok(grant(vm, root)?) },
        Err(error) => Err(error.into()),
    }
}
/** Reuse one local hidden root per shared node, retaining identity across messages. */
pub(super) fn restore_shared_number(vm: &mut Vm, root: SharedGc) -> Res<JsValue> {
    let root = grant(vm, root)?;
    let (module, ty, array, count) = root.metadata()?;
    let (SharedGcValue::Number(bits), _) = root.read(0)? else { return Err(EngineError::invalid("Invalid shared number backing").into()) };
    if module.as_ref() != NUMBER_META || ty != 0 || array || count != 1 { return Err(EngineError::invalid("Invalid shared number backing").into()); }
    let existing = vm.heap.objects.iter().find_map(|(id, object)| match &object.class {
        Class::SharedContainerRoot(other) if other.node() == root.node() && other.arena().same_store(&root.arena()) => Some(*id),
        _ => None,
    });
    let id = if let Some(id) = existing { id } else {
        let mut native = Object::new(Class::SharedContainerRoot(root), None);
        native.extensible = false;
        vm.alloc(native)?
    };
    vm.pin(JsValue::Object(id));
    Ok(JsValue::SharedNumber(Arc::new(crate::v9::value::SharedNumberData { value: f64::from_bits(bits), root: id })))
}

/** This native packet conveys a granted root, never a recoverable numeric capability. */
#[derive(Clone, Debug)]
pub(super) struct Packet { pub root: SharedGc }

/** A pending source graph is validated completely before any original object is activated. */
struct Plan { id: ObjectId, array: bool, keys: Vec<Key>, values: Vec<JsValue> }

fn root(vm: &Vm, id: ObjectId) -> Option<SharedGc> {
    let Slot::Data(JsValue::Object(backing)) = &vm.heap.get(id)?.props.get(&Key::private(ROOT_KEY))?.slot else { return None };
    match &vm.heap.get(*backing)?.class { Class::SharedContainerRoot(root) => Some(root.clone()), _ => None }
}
pub(in crate::v9) fn active(vm: &Vm, id: ObjectId) -> bool { root(vm, id).is_some() }

/** Fixed layout names are inert UTF-16 keys; array indices are canonical and sequential. */
fn keys(vm: &Vm, id: ObjectId) -> Vec<Key> {
    let Some(object) = vm.heap.get(id) else { return Vec::new() };
    match &object.class {
        Class::Array(elements) => (0..elements.len()).map(|index| Key::from(index.to_string().as_str())).collect(),
        _ => object.props.iter().filter_map(|(key, _)| (!key.hidden()).then(|| key.clone())).collect(),
    }
}
fn index(vm: &Vm, id: ObjectId, key: &Key) -> Option<usize> {
    let object = vm.heap.get(id)?;
    if let Class::Array(elements) = &object.class { return key.index().map(|v| v as usize).filter(|v| *v < elements.len()); }
    keys(vm, id).iter().position(|candidate| candidate == key)
}
fn metadata(keys: &[Key]) -> Result<Vec<u8>, EngineError> {
    let mut bytes = META_PREFIX.to_vec();
    bytes.extend(serde_json::to_vec(keys).map_err(|_| EngineError::invalid("Invalid shared container layout"))?);
    Ok(bytes)
}
/** A restored root must have exactly the named layout and supported field count. */
fn layout(root: &SharedGc) -> Result<(Vec<Key>, bool), EngineError> {
    let (module, ty, array, count) = root.metadata()?;
    let bytes = module.strip_prefix(META_PREFIX).ok_or_else(|| EngineError::invalid("Invalid shared container arena"))?;
    let keys: Vec<Key> = serde_json::from_slice(bytes).map_err(|_| EngineError::invalid("Invalid shared container layout"))?;
    let unique: BTreeSet<_> = keys.iter().cloned().collect();
    if ty != 0 || keys.len() != count || unique.len() != count || keys.iter().any(Key::hidden) || keys.iter().any(|key| !matches!(key, Key::Str(_))) || array && keys.iter().enumerate().any(|(i, key)| key.index() != Some(i as u32)) {
        return Err(EngineError::invalid("Invalid shared container layout"));
    }
    Ok((keys, array))
}

/** Native primitives carry no isolate-local object IDs. */
fn primitive(vm: &mut Vm, value: &JsValue) -> Res<Option<SharedGcValue>> {
    Ok(Some(match value {
        JsValue::Undefined => SharedGcValue::Undefined, JsValue::Null => SharedGcValue::Null,
        JsValue::Boolean(value) => SharedGcValue::Boolean(*value), JsValue::Number(value) => SharedGcValue::Number(value.to_bits()),
        JsValue::LocalNumber(value) => SharedGcValue::Number(value.to_bits()),
        JsValue::Smi(value) => SharedGcValue::Smi(*value as i32),
        JsValue::BigInt(value) => SharedGcValue::BigInt(*value), JsValue::BigIntWide(value) => SharedGcValue::BigIntWide(value.clone()),
        JsValue::String(value) => SharedGcValue::SharedString(vm.share_string(value)?),
        _ => return Ok(None),
    }))
}

/** Obtain the session's dedicated arena; standalone pages reuse their own existing container arena. */
pub(in crate::v9) fn existing_js_arena(vm: &Vm) -> Option<SharedGcArena> {
    vm.heap.objects.values().find_map(|object| match &object.class {
        Class::SharedContainerRoot(root) => Some(root.arena()),
        _ => None,
    })
}

pub(super) fn arena(vm: &Vm) -> Result<SharedGcArena, EngineError> {
    if let Some(host) = &vm.page.environment.conformance_workers { return host.shared_container_arena(); }
    vm.heap.objects.values().find_map(|object| match &object.class { Class::SharedContainerRoot(root) | Class::WasmShared(root) => Some(root.arena()), _ => None }).map(Ok).unwrap_or_else(SharedGcArena::new)
}
/** Collect the local heap first, then reclaim unreachable nodes and weak string entries. */
pub(in crate::v9) fn collect_shared(vm: &mut Vm) -> Res<()> {
    let arena = arena(vm)?;
    let host = vm.page.environment.conformance_workers.clone();
    let requester = vm.page.environment.conformance_worker_id;
    let sweep_host = host.clone();
    let mut arenas = BTreeMap::new();
    for object in vm.heap.objects.values() {
        if let Class::WasmShared(root) | Class::SharedContainerRoot(root) = &object.class {
            let arena = root.arena();
            arenas.entry(arena.identity()).or_insert(arena);
        }
    }
    let finish = || -> Res<()> {
        arena.collect();
        for arena in arenas.into_values() { arena.collect(); }
        if let Some(host) = sweep_host { host.collect_shared_arenas(); }
        crate::v9::value::collect_shared_strings();
        Ok(())
    };
    match host {
        Some(host) => host.coordinate_collection(requester, || vm.collect_now(), finish),
        None => { vm.collect_now()?; finish() },
    }
}
pub(super) fn grant(vm: &Vm, value: SharedGc) -> Result<SharedGc, EngineError> {
    match &vm.page.environment.conformance_workers { Some(host) => host.shared_container(vm.page.environment.conformance_worker_id, value), None => Ok(value) }
}

/** Synchronization nodes share this arena so container edges can trace and transfer them. */
pub(super) fn new_synchronizer(vm: &Vm, condition: bool) -> Result<SharedGc, EngineError> {
    let arena = arena(vm)?;
    let values = if condition { Vec::new() } else { vec![SharedGcValue::BigInt(0)] };
    grant(vm, arena.allocate(if condition { CONDITION_META } else { MUTEX_META }, 0, false, values)?)
}
/** Return a real synchronization root only for an engine-branded local wrapper. */
fn synchronizer(vm: &mut Vm, id: ObjectId) -> Res<Option<SharedGc>> {
    if super::mutex::is_mutex(vm, id) { return super::mutex::storage(vm, id).map(Some); }
    if super::condition::is_condition(vm, id) { return super::condition::storage(vm, &JsValue::Object(id)).map(Some); }
    Ok(None)
}

/** Capture bounded reachable containers with cycles and reject unsupported capabilities up front. */
fn plan(vm: &mut Vm, start: ObjectId, arena: &SharedGcArena) -> Res<Vec<Plan>> {
    let mut pending = vec![start];
    let mut seen = BTreeSet::new();
    let mut plans = Vec::new();
    while let Some(id) = pending.pop() {
        if !seen.insert(id) { continue; }
        if seen.len() > vm.limits.objects { return Err(EngineError::limit("shared container graph", vm.limits.objects).into()); }
        if let Some(existing) = synchronizer(vm, id)? {
            if !existing.arena().same_store(arena) { return Err(vm.unsupported_error("shared synchronization across unrelated sessions")); }
            continue;
        }
        if let Some(existing) = root(vm, id) {
            if !existing.arena().same_store(arena) { return Err(vm.unsupported_error("shared container across unrelated sessions")); }
            continue;
        }
        if vm.heap.get(id).is_some_and(|o| o.props.get(&Key::private(LEGACY_KEY)).is_some()) { return Err(vm.unsupported_error("legacy shared container graph migration")); }
        if !shared::is_shared_object(vm, id) { return Err(vm.unsupported_error("unshareable shared-container graph")); }
        vm.pin(JsValue::Object(id));
        let array = matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Array(_)));
        let keys = keys(vm, id);
        let values = keys.iter().map(|key| vm.own(id, key).map(|prop| match prop.map(|p| p.slot) { Some(Slot::Data(value)) => value, _ => JsValue::Undefined })).collect::<Res<Vec<_>>>()?;
        for value in &values {
            if primitive(vm, value)?.is_some() { continue; }
            if let JsValue::SharedNumber(number) = value {
                if !matches!(vm.heap.get(number.root).map(|object| &object.class), Some(Class::SharedContainerRoot(root)) if root.arena().same_store(arena)) { return Err(vm.unsupported_error("shared number across unrelated sessions")); }
                continue;
            }
            if let JsValue::Object(id) = value {
                if let Some(Class::WasmShared(root)) = vm.heap.get(*id).map(|object| &object.class) {
                    if !root.arena().same_store(arena) { return Err(vm.unsupported_error("shared Wasm object across unrelated sessions")); }
                    continue;
                }
            }
            let Some(target) = value.object().filter(|target| shared::is_shared_object(vm, *target)) else { return Err(vm.unsupported_error("unshareable shared container graph value")); };
            pending.push(target);
        }
        plans.push(Plan { id, array, keys, values });
    }
    Ok(plans)
}

/** Allocate and link every native record before publishing any graph root into the source heap. */
pub(in crate::v9) fn activate(vm: &mut Vm, start: ObjectId) -> Res<SharedGc> {
    if let Some(root) = root(vm, start) { return Ok(root); }
    let arena = arena(vm)?;
    let plans = plan(vm, start, &arena)?;
    let mut roots = BTreeMap::new();
    for item in &plans {
        let values = item.values.iter().map(|value| primitive(vm, value).map(|value| value.unwrap_or(SharedGcValue::Undefined))).collect::<Res<Vec<_>>>()?;
        let root = grant(vm, arena.allocate(&metadata(&item.keys)?, 0, item.array, values)?)?;
        roots.insert(item.id, root);
    }
    for item in &plans {
        let root = &roots[&item.id];
        for (index, value) in item.values.iter().enumerate() {
            if let JsValue::SharedNumber(number) = value {
                let Some(Class::SharedContainerRoot(target)) = vm.heap.get(number.root).map(|object| &object.class) else { return Err(vm.type_error("Invalid shared number root")); };
                root.update(index, |_| SharedGcValue::Reference(target.node()))?;
                continue;
            }
            if let Some(target) = value.object() {
                if let Some(Class::WasmShared(child)) = vm.heap.get(target).map(|object| &object.class) {
                    if !child.arena().same_store(&arena) { return Err(vm.unsupported_error("shared Wasm object across unrelated sessions")); }
                    root.update(index, |_| SharedGcValue::Reference(child.node()))?;
                    continue;
                }
                let target = if let Some(value) = roots.get(&target).cloned().or_else(|| self::root(vm, target)) { value } else { synchronizer(vm, target)?.ok_or_else(|| vm.type_error("Invalid shared container graph"))? };
                if !target.arena().same_store(&arena) { return Err(vm.unsupported_error("shared container across unrelated sessions")); }
                root.update(index, |_| SharedGcValue::Reference(target.node()))?;
            }
        }
    }
    let mut wrappers = Vec::new();
    for item in &plans {
        let mut native = Object::new(Class::SharedContainerRoot(roots[&item.id].clone()), None);
        native.extensible = false;
        let id = vm.alloc(native)?;
        vm.pin(JsValue::Object(id));
        wrappers.push((item.id, id));
    }
    for (object, native) in wrappers { vm.heap.put(object, Key::private(ROOT_KEY), Prop::data(JsValue::Object(native))); }
    vm.heap.refresh_shared_gc(&arena);
    roots.get(&start).cloned().ok_or_else(|| vm.type_error("Invalid shared container graph"))
}

pub(super) fn packet(vm: &mut Vm, id: ObjectId) -> Res<Packet> { Ok(Packet { root: activate(vm, id)? }) }
/** Materialize the real shared node before claiming a branded object is shared. */
pub(in crate::v9) fn materialize_shared_object(vm: &mut Vm, id: ObjectId) -> Res<()> {
    activate(vm, id).map(|_| ())
}

/** A native root yields one local wrapper per agent, preserving aliases and cycles. */
pub(super) fn restore(vm: &mut Vm, root_value: &SharedGc) -> Res<ObjectId> {
    let root_value = grant(vm, root_value.clone())?;
    if let Some(id) = vm.heap.objects.keys().copied().find(|id| root(vm, *id).is_some_and(|other| other.node() == root_value.node() && other.arena().same_store(&root_value.arena()))) { return Ok(id); }
    let (keys, array) = layout(&root_value)?;
    let object = if array { vm.new_array(vec![JsValue::Undefined; keys.len()])? } else { vm.alloc(Object::new(Class::Plain, None))? };
    vm.pin(JsValue::Object(object));
    vm.set_proto(object, None)?;
    if !array { for key in &keys { vm.heap.put(object, key.clone(), Prop { slot: Slot::Data(JsValue::Undefined), attrs: WRITABLE | ENUMERABLE }); } }
    shared::mark_transport(vm, object, array)?;
    let mut native = Object::new(Class::SharedContainerRoot(root_value), None);
    native.extensible = false;
    let native = vm.alloc(native)?;
    vm.heap.put(object, Key::private(ROOT_KEY), Prop::data(JsValue::Object(native)));
    crate::v9::builtins::integrity::prevent_extensions(vm, object)?;
    Ok(object)
}

/** Reads root referenced nodes before exposing their local wrapper to script. */
fn value(vm: &mut Vm, value: SharedGcValue, reference: Option<SharedGc>) -> Res<JsValue> {
    Ok(match value {
        SharedGcValue::Undefined => JsValue::Undefined, SharedGcValue::Null => JsValue::Null,
        SharedGcValue::Boolean(value) => JsValue::Boolean(value), SharedGcValue::Number(value) => JsValue::Number(f64::from_bits(value)), SharedGcValue::Smi(value) => JsValue::Smi(f64::from(value)),
        SharedGcValue::BigInt(value) => JsValue::BigInt(value), SharedGcValue::BigIntWide(value) => JsValue::BigIntWide(value),
        SharedGcValue::String(value) => {
            let value = JsString::from_shared(value)?;
            JsValue::String(vm.share_string(&value)?)
        }
        SharedGcValue::SharedString(value) => JsValue::String(value),
        SharedGcValue::Reference(_) => {
            let reference = reference.ok_or_else(|| EngineError::invalid("Missing shared reference"))?;
            let metadata = reference.metadata()?.0;
            if metadata.as_ref() == NUMBER_META { return restore_shared_number(vm, reference); }
            if metadata.as_ref() == MUTEX_META { JsValue::Object(super::mutex::restore(vm, reference)?) }
            else if metadata.as_ref() == CONDITION_META { JsValue::Object(super::condition::restore(vm, reference)?) }
            else if metadata.starts_with(META_PREFIX) { JsValue::Object(restore(vm, &reference)?) }
            else { return crate::v9::builtins::wasm_worker_restore_shared_gc(vm, &reference); }
        },
        SharedGcValue::Vector(_) => return Err(EngineError::invalid("Invalid shared container value").into()),
    })
}
/** Resolve genuine d8 arena references without reclassifying Wasm-owned records. */
pub(in crate::v9) fn resolve_shared_js_reference(vm: &mut Vm, root: &SharedGc) -> Res<Option<JsValue>> {
    let metadata = root.metadata()?.0;
    if metadata.starts_with(META_PREFIX) || metadata.as_ref() == NUMBER_META || metadata.as_ref() == MUTEX_META || metadata.as_ref() == CONDITION_META {
        return value(vm, SharedGcValue::Reference(root.node()), Some(root.clone())).map(Some);
    }
    Ok(None)
}
fn stored(vm: &mut Vm, parent: &SharedGc, value: &JsValue) -> Res<SharedGcValue> {
    let number = match value {
        JsValue::LocalNumber(value) => Some(value.value),
        JsValue::Number(value) if !value.is_finite() || value.fract() != 0.0 || *value < -(1_i64 << 30) as f64 || *value >= (1_i64 << 30) as f64 || (*value == 0.0 && value.is_sign_negative()) => Some(*value),
        _ => None,
    };
    if let Some(number) = number {
        let root = allocate_shared_number(vm, &parent.arena(), number)?;
        let node = root.node();
        restore_shared_number(vm, root)?;
        return Ok(SharedGcValue::Reference(node));
    }
    if let JsValue::SharedNumber(number) = value {
        let Some(Class::SharedContainerRoot(root)) = vm.heap.get(number.root).map(|object| &object.class) else { return Err(vm.type_error("Invalid shared number root")); };
        if !root.arena().same_store(&parent.arena()) { return Err(vm.type_error("Shared number belongs to another arena")); }
        return Ok(SharedGcValue::Reference(root.node()));
    }
    if let Some(value) = primitive(vm, value)? { return Ok(value); }
    if let Some(id) = value.object() {
        if let Some(Class::WasmShared(root)) = vm.heap.get(id).map(|object| &object.class) {
            if !root.arena().same_store(&parent.arena()) { return Err(vm.unsupported_error("shared Wasm object across unrelated sessions")); }
            return Ok(SharedGcValue::Reference(root.node()));
        }
    }
    let Some(id) = value.object().filter(|id| shared::is_shared_object(vm, *id)) else { return Err(vm.type_error("Value cannot enter shared storage")); };
    let child = if let Some(value) = synchronizer(vm, id)? { value } else { activate(vm, id)? };
    if !child.arena().same_store(&parent.arena()) { return Err(vm.unsupported_error("shared container across unrelated sessions")); }
    Ok(SharedGcValue::Reference(child.node()))
}
pub(in crate::v9) fn read(vm: &mut Vm, id: ObjectId, key: &Key) -> Res<Option<JsValue>> {
    let Some(root) = root(vm, id) else { return Ok(None) };
    let Some(index) = index(vm, id, key) else { return Ok(None) };
    let (stored, reference) = root.read(index)?;
    Ok(Some(value(vm, stored, reference)?))
}
pub(in crate::v9) fn write(vm: &mut Vm, id: ObjectId, key: &Key, input: &JsValue) -> Res<Option<bool>> {
    let mark = vm.pins.len();
    let result = write_pinned(vm, id, key, input);
    vm.pins.truncate(mark);
    result
}
fn write_pinned(vm: &mut Vm, id: ObjectId, key: &Key, input: &JsValue) -> Res<Option<bool>> {
    let Some(root) = root(vm, id) else { return Ok(None) };
    let Some(index) = index(vm, id, key) else { return Ok(Some(false)) };
    let stored = stored(vm, &root, input)?;
    root.update(index, |_| stored)?;
    vm.heap.refresh_shared_gc(&root.arena());
    Ok(Some(true))
}
pub(in crate::v9) fn define(vm: &mut Vm, id: ObjectId, key: &Key, descriptor: &crate::v9::vm::property::Descriptor) -> Res<Option<bool>> {
    if !active(vm, id) || index(vm, id, key).is_none() { return Ok(None); }
    if descriptor.accessor() || descriptor.configurable == Some(true) || descriptor.enumerable == Some(false) || descriptor.writable == Some(false) { return Ok(Some(false)); }
    match &descriptor.value { Some(value) => write(vm, id, key, value), None => Ok(Some(true)) }
}

/** Atomic compare operates on stored value bits or arena node identity under the arena lock. */
fn equal(left: &SharedGcValue, right: &SharedGcValue) -> bool {
    match (left, right) {
        (SharedGcValue::Number(a), SharedGcValue::Number(b)) => f64::from_bits(*a) == f64::from_bits(*b),
        (SharedGcValue::String(a), SharedGcValue::SharedString(b)) | (SharedGcValue::SharedString(b), SharedGcValue::String(a)) => a.iter().copied().eq(b.code_units()),
        _ => left == right,
    }
}
pub(in crate::v9) fn atomic(vm: &mut Vm, call: &crate::v9::vm::Call, name: &str) -> Res<JsValue> {
    let mark = vm.pins.len();
    let result = atomic_pinned(vm, call, name);
    vm.pins.truncate(mark);
    result
}
fn atomic_pinned(vm: &mut Vm, call: &crate::v9::vm::Call, name: &str) -> Res<JsValue> {
    let id = call.arg(0).object().ok_or_else(|| vm.type_error("Invalid shared container"))?;
    let key = vm.to_key(&call.arg(1))?;
    if key == Key::from("length") && matches!(vm.heap.get(id).map(|o| &o.class), Some(Class::Array(_))) {
        if name == "load" { return vm.get(&JsValue::Object(id), &key); }
        return Err(vm.type_error("Cannot modify shared array length"));
    }
    let Some(index) = index(vm, id, &key) else { return Err(vm.range_error("Invalid shared container field")); };
    let root = activate(vm, id)?;
    if name == "load" { let (old, reference) = root.read(index)?; return value(vm, old, reference); }
    let expected_number = call.arg(2).number_value();
    let operand = if name == "compareExchange" { match expected_number { Some(number) => SharedGcValue::Number(number.to_bits()), None => stored(vm, &root, &call.arg(2))? } } else { stored(vm, &root, &call.arg(2))? };
    let replacement = if name == "compareExchange" { stored(vm, &root, &call.arg(3))? } else { operand.clone() };
    let (old, reference) = root.update_with_number(index, NUMBER_META, |old, resolved| {
        let numeric_match = match (old, resolved, expected_number) {
            (SharedGcValue::Number(bits), _, Some(expected)) => f64::from_bits(*bits) == expected,
            (SharedGcValue::Smi(value), _, Some(expected)) => f64::from(*value) == expected,
            (SharedGcValue::Reference(_), Some(bits), Some(expected)) => f64::from_bits(bits) == expected,
            _ => equal(old, &operand),
        };
        if name != "compareExchange" || numeric_match { replacement } else { old.clone() }
    })?;
    vm.heap.refresh_shared_gc(&root.arena());
    if name == "store" { Ok(call.arg(2)) } else { value(vm, old, reference) }
}

/** Validate one d8 record in a mixed arena; foreign metadata remains the caller's responsibility. */
pub(in crate::v9) fn validate_shared_js_record(arena: &SharedGcArena, records: &BTreeMap<u64, crate::v9::vm::data::SharedGcRecord>, node: u64) -> Result<bool, EngineError> {
    let root = arena.root(node)?;
    let (module, ty, array, count) = root.metadata()?;
    let recognized = module.starts_with(META_PREFIX) || module.as_ref() == MUTEX_META || module.as_ref() == CONDITION_META || module.as_ref() == NUMBER_META;
    if !recognized { return Ok(false); }
    let record = records.get(&node).ok_or_else(|| EngineError::invalid("Invalid shared container node"))?;
    if module.as_ref() == MUTEX_META {
        if ty != 0 || array || count != 1 || !matches!(record.values.first(), Some(SharedGcValue::BigInt(value)) if *value >= 0 && *value <= u64::MAX as i128) { return Err(EngineError::invalid("Invalid shared mutex state")); }
    } else if module.as_ref() == CONDITION_META {
        if ty != 0 || array || count != 0 { return Err(EngineError::invalid("Invalid shared condition state")); }
    } else if module.as_ref() == NUMBER_META {
        if ty != 0 || array || count != 1 || !matches!(record.values.first(), Some(SharedGcValue::Number(_))) { return Err(EngineError::invalid("Invalid shared number state")); }
    } else { let _ = layout(&root)?; }
    for value in &record.values {
        match value {
            SharedGcValue::Vector(_) => return Err(EngineError::invalid("Invalid shared container value")),
            SharedGcValue::Reference(id) if !records.contains_key(id) => return Err(EngineError::invalid("Invalid shared container reference")),
            _ => (),
        }
    }
    Ok(true)
}

/** Snapshot data must describe only bounded container nodes and same-arena container edges. */
fn validate_graph(root: &SharedGc) -> Result<(), EngineError> {
    let arena = root.arena();
    let (_, records) = arena.snapshot_records();
    let mut has_foreign = false;
    for node in records.keys() {
        if !validate_shared_js_record(&arena, &records, *node)? { has_foreign = true; }
    }
    if has_foreign { crate::v9::builtins::validate_shared_wasm_arena(&arena)?; }
    Ok(())
}

/** Reattach only endpoint-granted roots; inert IDs cannot recover another agent's capability. */
pub(in crate::v9) fn initialize(vm: &mut Vm) -> Result<(), EngineError> {
    let mut roots = Vec::new();
    for (id, object) in vm.heap.objects.iter() {
        if object.props.get(&Key::private(LEGACY_KEY)).is_some() {
            return Err(EngineError::invalid("Legacy shared container state cannot be restored"));
        }
        if let Class::SharedContainerRoot(root) = &object.class {
            if object.extensible || object.proto.is_some() || !object.props.is_empty() { return Err(EngineError::invalid("Invalid shared container root")); }
            validate_graph(root)?;
            roots.push((*id, root.clone()));
        }
        if let Some(prop) = object.props.get(&Key::private(ROOT_KEY)) {
            let Slot::Data(JsValue::Object(backing)) = &prop.slot else { return Err(EngineError::invalid("Invalid shared container backing")); };
            if !matches!(vm.heap.get(*backing).map(|object| &object.class), Some(Class::SharedContainerRoot(_))) { return Err(EngineError::invalid("Invalid shared container backing")); }
        }
    }
    let mut canonical: BTreeMap<u64, SharedGcArena> = BTreeMap::new();
    let mut merged = Vec::new();
    for (_, root) in &roots {
        if root.restored() { let arena = canonical.entry(root.arena().identity()).or_insert_with(|| root.arena()); merged.push(arena.merge_snapshot(root)?); }
    }
    for (id, original) in roots {
        let mut restored = if original.restored() { original.attach_to(&canonical[&original.arena().identity()])? } else { original.clone() };
        if let Some(host) = &vm.page.environment.conformance_workers {
            if original.restored() {
                let live = host.restore_shared_container(vm.page.environment.conformance_worker_id, original.arena().identity(), original.node()).ok_or_else(|| EngineError::invalid("Shared container root was not granted to this worker"))?;
                if original.metadata()? != live.metadata()? { return Err(EngineError::invalid("Shared container metadata changed")); }
                restored = live;
            }
            restored = host.shared_container(vm.page.environment.conformance_worker_id, restored)?;
        }
        vm.heap.get_mut(id).ok_or_else(|| EngineError::invalid("Missing shared container root"))?.class = Class::SharedContainerRoot(restored);
    }
    vm.heap.recount();
    Ok(())
}
