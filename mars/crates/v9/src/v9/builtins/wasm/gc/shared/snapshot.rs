/** Shared graph snapshots validate typed records before scripts and restore only granted native roots. */
use super::*;
use crate::protocol::{EngineError, EngineResult};
use crate::v9::vm::heap::Heap;
use std::collections::BTreeMap;
fn invalid() -> EngineError {
    EngineError::invalid("Invalid shared Wasm GC typed record.")
}

/** Shared graph records cannot encode function bodies, foreign IDs or values outside their validated types. */
fn validate_arena(arena: &SharedGcArena) -> EngineResult<()> {
    let (sources, records) = arena.snapshot_records();
    let modules = sources
        .iter()
        .map(|bytes| binary::parse(bytes).map_err(|_| invalid()))
        .collect::<EngineResult<Vec<_>>>()?;
    for record in records.values() {
        let module = modules.get(record.module as usize).ok_or_else(invalid)?;
        let definition = module
            .definitions
            .get(record.ty as usize)
            .filter(|d| d.shared)
            .ok_or_else(invalid)?;
        let fields = match &definition.kind {
            Kind::Struct(fields) if !record.array && fields.len() == record.values.len() => {
                fields.as_slice()
            }
            Kind::Array(field) if record.array => std::slice::from_ref(field),
            _ => return Err(invalid()),
        };
        for (index, value) in record.values.iter().enumerate() {
            let field = &fields[if record.array { 0 } else { index }];
            if !valid_value(&modules, &records, module, field.ty, value) {
                return Err(invalid());
            }
            if let Some(bits) = field.packed {
                if !matches!(value,SharedGcValue::Number(v) if f64::from_bits(*v).fract()==0.0 && f64::from_bits(*v)>=0.0 && f64::from_bits(*v)<(1u64<<bits)as f64)
                {
                    return Err(invalid());
                }
            }
        }
    }
    Ok(())
}
fn valid_value(
    modules: &[Module],
    records: &BTreeMap<u64, crate::v9::vm::data::SharedGcRecord>,
    m: &Module,
    ty: Type,
    value: &SharedGcValue,
) -> bool {
    match ty {
        Type::I32 => {
            matches!(value,SharedGcValue::Number(v) if f64::from_bits(*v).fract()==0.0 && f64::from_bits(*v)>=i32::MIN as f64 && f64::from_bits(*v)<=i32::MAX as f64)
        }
        Type::I64 => {
            matches!(value,SharedGcValue::BigInt(v) if *v>=i64::MIN as i128 && *v<=i64::MAX as i128)
        }
        Type::F32 | Type::F64 => matches!(value, SharedGcValue::Number(_)),
        Type::V128 => matches!(value, SharedGcValue::Vector(_)),
        _ => {
            let Some((heap, nullable)) = ty.reference() else {
                return false;
            };
            if !gc_types::is_shared(m, ty) {
                return false;
            }
            if *value == SharedGcValue::Null {
                return nullable;
            }
            if let SharedGcValue::Reference(node) = value {
                let Some(record) = records.get(node) else {
                    return false;
                };
                let source = &modules[record.module as usize];
                if heap < 0 {
                    return matches!(heap, -17 | -18 | -19)
                        || heap == -21 && !record.array
                        || heap == -22 && record.array;
                }
                let mut current = Some(record.ty);
                for _ in 0..=source.definitions.len() {
                    let Some(index) = current else {
                        return false;
                    };
                    if gc_types::equivalent(source, index, m, heap as u32) {
                        return true;
                    }
                    current = source.definitions[index as usize].parent;
                }
                false
            } else if heap == -17 {
                matches!(
                    value,
                    SharedGcValue::Undefined
                        | SharedGcValue::Boolean(_)
                        | SharedGcValue::Number(_)
                        | SharedGcValue::BigInt(_)
                        | SharedGcValue::BigIntWide(_)
                        | SharedGcValue::String(_)
                )
            } else {
                matches!(heap, -18 | -19 | -20)
                    && matches!(value,SharedGcValue::Number(v) if f64::from_bits(*v).fract()==0.0 && f64::from_bits(*v)>=-1_073_741_824.0 && f64::from_bits(*v)<1_073_741_824.0)
            }
        }
    }
}
/** Validation examines every graph, including unreachable forged wrappers, before any author code executes. */
pub(in crate::v9::builtins::wasm) fn validate(heap: &Heap) -> EngineResult<()> {
    for object in heap.objects.values() {
        if let Class::WasmShared(root) = &object.class {
            if object.extensible || object.proto.is_some() || !object.props.is_empty() {
                return Err(invalid());
            }
            validate_arena(&root.arena())?;
        }
    }
    Ok(())
}
/** Restored snapshots merge locally; only an exact endpoint/root grant may recover a live shared Arc. */
pub(in crate::v9::builtins::wasm) fn initialize(vm: &mut Vm) -> EngineResult<()> {
    validate(&vm.heap)?;
    let roots = vm
        .heap
        .objects
        .iter()
        .filter_map(|(id, o)| match &o.class {
            Class::WasmShared(root) => Some((*id, root.clone())),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut canonical: BTreeMap<u64, SharedGcArena> = BTreeMap::new();
    // Keep every copied root live while independent root subgraphs merge into one local arena.
    let mut merged_roots = Vec::new();
    for (_, root) in &roots {
        if root.restored() {
            let arena = canonical
                .entry(root.arena().identity())
                .or_insert_with(|| root.arena());
            merged_roots.push(arena.merge_snapshot(root)?);
        }
    }
    for (id, root) in roots {
        let restored = if root.restored() {
            root.attach_to(&canonical[&root.arena().identity()])?
        } else {
            root.clone()
        };
        #[cfg(feature = "conformance")]
        let restored = {
            let mut restored = restored;
            if let Some(host) = &vm.page.environment.conformance_workers {
                if let Some(live) = host.restore_shared_gc(
                    vm.page.environment.conformance_worker_id,
                    root.arena().identity(),
                    root.node(),
                ) {
                    if root.metadata()?.0 != live.metadata()?.0
                        || root.metadata()?.1 != live.metadata()?.1
                        || root.metadata()?.2 != live.metadata()?.2
                    {
                        return Err(invalid());
                    }
                    restored = live;
                } else {
                    restored =
                        host.shared_gc(vm.page.environment.conformance_worker_id, restored)?;
                }
            }
            restored
        };
        vm.heap.get_mut(id).ok_or_else(invalid)?.class = Class::WasmShared(restored);
    }
    vm.heap.recount();
    Ok(())
}
