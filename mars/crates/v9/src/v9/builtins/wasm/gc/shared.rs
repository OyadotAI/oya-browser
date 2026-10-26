/** Shared GC bindings translate isolate wrappers into inert records in one real synchronized arena. */
use super::*;
use crate::v9::vm::data::{SharedGc, SharedGcArena, SharedGcValue};
mod snapshot;
pub(in crate::v9::builtins::wasm) use snapshot::{initialize, validate};

/** Only the distinct native class carries a shared root; authored private-looking properties cannot forge it. */
pub(in crate::v9::builtins::wasm) fn handle(vm: &Vm, value: &JsValue) -> Option<SharedGc> {
    match &vm.heap.get(value.object()?)?.class {
        Class::WasmShared(value) => Some(value.clone()),
        _ => None,
    }
}
/** Native workers in a session allocate in one owned arena; ordinary pages reuse their own live arena. */
fn arena(vm: &Vm) -> crate::protocol::EngineResult<SharedGcArena> {
    #[cfg(feature = "conformance")]
    if let Some(host) = &vm.page.environment.conformance_workers {
        return host.shared_gc_arena();
    }
    vm.heap
        .objects
        .values()
        .find_map(|o| match &o.class {
            Class::WasmShared(value) => Some(value.arena()),
            _ => None,
        })
        .map(Ok)
        .unwrap_or_else(SharedGcArena::new)
}
/** Converting a reference validates its actual backing capability before storing its arena-local node number. */
fn value(vm: &mut Vm, arena: &SharedGcArena, ty: Type, value: &JsValue) -> Res<SharedGcValue> {
    if ty == Type::V128 {
        return Ok(SharedGcValue::Vector(simd::vector(value)?));
    }
    Ok(match value {
        JsValue::Undefined => SharedGcValue::Undefined,
        JsValue::Null => SharedGcValue::Null,
        JsValue::Boolean(v) => SharedGcValue::Boolean(*v),
        JsValue::Number(v) => SharedGcValue::Number(v.to_bits()),
        JsValue::BigInt(v) => SharedGcValue::BigInt(*v),
        JsValue::BigIntWide(v) => SharedGcValue::BigIntWide(v.clone()),
        JsValue::String(v) => SharedGcValue::String(v.as_units().into()),
        JsValue::Object(_) => {
            let shared = handle(vm, value)
                .ok_or_else(|| vm.type_error("shared Wasm storage requires a shared reference"))?;
            if !arena.same_store(&shared.arena()) {
                return Err(
                    vm.unsupported_error("shared GC references between unrelated agent families")
                );
            }
            SharedGcValue::Reference(shared.node())
        }
        _ => return Err(vm.type_error("value cannot be stored in shared Wasm storage")),
    })
}
/** Field types were statically checked; the native record also checks graph bounds before allocation. */
pub(super) fn allocate(
    vm: &mut Vm,
    source: &JsValue,
    m: &Module,
    index: u32,
    array: bool,
    fields: Vec<JsValue>,
) -> Res<JsValue> {
    let object = source
        .object()
        .ok_or_else(|| vm.type_error("invalid shared Wasm module"))?;
    let buffer = get(vm, object, MODULE)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid shared Wasm module bytes"))?;
    let Some(Class::ArrayBuffer(bytes)) = vm.heap.get(buffer).map(|o| &o.class) else {
        return Err(vm.type_error("invalid shared Wasm binary"));
    };
    let bytes = bytes.0.clone();
    let arena = arena(vm)?;
    let types = if array {
        vec![
            gc_types::element(m, index)
                .map_err(|e| compile_failure(vm, e))?
                .ty;
            fields.len()
        ]
    } else {
        gc_types::fields(m, index)
            .map_err(|e| compile_failure(vm, e))?
            .iter()
            .map(|f| f.ty)
            .collect()
    };
    let values = types
        .into_iter()
        .zip(fields)
        .map(|(ty, v)| value(vm, &arena, ty, &v))
        .collect::<Res<Vec<_>>>()?;
    let root = match arena.allocate(&bytes, index, array, values.clone()) {
        Ok(root) => root,
        Err(error) if error.code == "limit_exceeded" => {
            vm.collect_now()?;
            arena.allocate(&bytes, index, array, values)?
        }
        Err(error) => return Err(error.into()),
    };
    let result = wrap(vm, &root)?;
    vm.heap.refresh_shared_gc(&arena);
    vm.collect_if_needed()?;
    Ok(result)
}
/** A VM reuses its existing wrapper identity for every read of the same shared graph node. */
pub(in crate::v9::builtins::wasm) fn wrap(vm: &mut Vm, root: &SharedGc) -> Res<JsValue> {
    let existing=vm.heap.objects.iter().find_map(|(id,o)|matches!(&o.class,Class::WasmShared(other)if other.node()==root.node()&&other.arena().same_store(&root.arena())).then_some(*id));
    if let Some(id) = existing {
        vm.pin(JsValue::Object(id));
        return Ok(JsValue::Object(id));
    }

    let root = root.clone();
    #[cfg(feature = "conformance")]
    let root = if let Some(host) = &vm.page.environment.conformance_workers {
        host.shared_gc(vm.page.environment.conformance_worker_id, root)?
    } else {
        root
    };
    let mut object = Object::new(Class::WasmShared(root), None);
    object.extensible = false;
    let id = vm.alloc(object)?;
    vm.collect_if_needed()?;
    Ok(JsValue::Object(id))
}
/** Primitive reads copy immutable data; graph reads retain a real root before the lock is released. */
fn from_value(
    vm: &mut Vm,
    _root: &SharedGc,
    result: (SharedGcValue, Option<SharedGc>),
) -> Res<JsValue> {
    let (value, reference) = result;
    Ok(match value {
        SharedGcValue::Undefined => JsValue::Undefined,
        SharedGcValue::Null => JsValue::Null,
        SharedGcValue::Boolean(v) => JsValue::Boolean(v),
        SharedGcValue::Number(v) => JsValue::Number(f64::from_bits(v)),
        SharedGcValue::BigInt(v) => JsValue::BigInt(v),
        SharedGcValue::BigIntWide(v) => JsValue::bigint(v),
        SharedGcValue::Vector(v) => simd::value(v)?,
        SharedGcValue::String(v) => JsValue::String(JsString::from_units(v.to_vec())?),
        SharedGcValue::Reference(_) => {
            let reference =
                reference.ok_or_else(|| vm.type_error("missing shared GC read root"))?;
            return wrap(vm, &reference);
        }
    })
}
pub(super) fn read(vm: &mut Vm, root: &SharedGc, index: usize) -> Res<JsValue> {
    if index >= root.metadata()?.3 {
        return Err(trap(vm, "array element access out of bounds"));
    }
    let value = root.read(index)?;
    vm.heap.refresh_shared_gc(&root.arena());
    vm.collect_if_needed()?;
    from_value(vm, root, value)
}
pub(super) fn write(vm: &mut Vm, root: &SharedGc, index: usize, item: JsValue) -> Res<()> {
    let (bytes, ty, array, count) = root.metadata()?;
    if index >= count {
        return Err(trap(vm, "array element access out of bounds"));
    }
    let m = binary::parse(&bytes).map_err(|e| compile_failure(vm, e))?;
    let ty = if array {
        gc_types::element(&m, ty)
            .map_err(|e| compile_failure(vm, e))?
            .ty
    } else {
        gc_types::fields(&m, ty).map_err(|e| compile_failure(vm, e))?[index].ty
    };
    let value = value(vm, &root.arena(), ty, &item)?;

    root.update(index, |_| value)?;
    vm.heap.refresh_shared_gc(&root.arena());
    vm.collect_if_needed()
}
/** Cross-module checks compare the record's real validated type, including its shared qualifier. */
pub(super) fn matches(vm: &mut Vm, m: &Module, ty: Type, root: &SharedGc) -> Res<bool> {
    if !gc_types::is_shared(m, ty) {
        return Ok(false);
    }
    let (bytes, index, array, _) = root.metadata()?;
    let (heap, _) = ty.reference().unwrap();
    if heap < 0 {
        return Ok(match heap {
            -18 | -19 | -17 => true,
            -21 => !array,
            -22 => array,
            _ => false,
        });
    }
    let actual = binary::parse(&bytes).map_err(|e| compile_failure(vm, e))?;
    let mut current = Some(index);
    for _ in 0..=actual.definitions.len() {
        let Some(index) = current else {
            return Ok(false);
        };
        if gc_types::equivalent(&actual, index, m, heap as u32) {
            return Ok(true);
        }
        if matches!(ty, Type::Exact(..)) {
            return Ok(false);
        }
        current = actual.definitions[index as usize].parent;
    }
    Ok(false)
}
/** Numeric RMW and reference comparison happen under the arena mutex, with no JS callback or VM borrow. */
pub(super) fn atomic(
    vm: &mut Vm,
    root: &SharedGc,
    index: usize,
    action: u32,
    ty: Type,
    stored: Option<JsValue>,
    expected: Option<JsValue>,
) -> Res<JsValue> {
    if index >= root.metadata()?.3 {
        return Err(trap(vm, "array element access out of bounds"));
    }
    let arena = root.arena();
    let stored = stored
        .as_ref()
        .map(|v| value(vm, &arena, ty, v))
        .transpose()?;
    let expected = expected
        .as_ref()
        .map(|v| value(vm, &arena, ty, v))
        .transpose()?;

    let old = root.update(index, |old| {
        let Some(value) = &stored else {
            return old.clone();
        };
        if action == 10 {
            return if expected.as_ref() == Some(old) {
                value
            } else {
                old
            }
            .clone();
        }
        if action == 3 || action == 9 {
            return value.clone();
        }
        match (old, value) {
            (SharedGcValue::Number(a), SharedGcValue::Number(b)) => {
                let a = f64::from_bits(*a) as i32 as u32;
                let b = f64::from_bits(*b) as i32 as u32;
                let result = match action {
                    4 => a.wrapping_add(b),
                    5 => a.wrapping_sub(b),
                    6 => a & b,
                    7 => a | b,
                    8 => a ^ b,
                    _ => a,
                };
                SharedGcValue::Number((result as i32 as f64).to_bits())
            }
            (SharedGcValue::BigInt(a), SharedGcValue::BigInt(b)) => {
                let a = *a as u64;
                let b = *b as u64;
                let result = match action {
                    4 => a.wrapping_add(b),
                    5 => a.wrapping_sub(b),
                    6 => a & b,
                    7 => a | b,
                    8 => a ^ b,
                    _ => a,
                };
                SharedGcValue::BigInt(result as i64 as i128)
            }
            _ => old.clone(),
        }
    })?;
    vm.heap.refresh_shared_gc(&root.arena());
    vm.collect_if_needed()?;
    from_value(vm, root, old)
}
