/** WebAssembly.Memory owns actual ArrayBuffer bytes and grows without retaining old aliases. */
use super::*;
mod buffers;
pub(super) use buffers::resize_buffer;

/** V8's test intrinsic detaches a Wasm wrapper while preserving its live memory owner. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn force_detach(vm: &mut Vm, value: &JsValue) -> Res<JsValue> {
    let old = value
        .object()
        .ok_or_else(|| vm.type_error("Expected an ArrayBuffer"))?;
    let class = match vm.heap.get(old).map(|object| &object.class) {
        Some(Class::ArrayBuffer(bytes)) => Class::ArrayBuffer(bytes.clone()),
        Some(Class::SparseWasmBuffer(bytes)) => Class::SparseWasmBuffer(bytes.clone()),
        _ => return Err(vm.type_error("Expected an unshared ArrayBuffer")),
    };
    if super::super::buffer_immutable(vm, old) {
        return Err(vm.type_error("Cannot detach an immutable ArrayBuffer"));
    }
    if super::super::buffer_detached(vm, old) {
        return Ok(JsValue::Undefined);
    }
    let owners: Vec<_> = vm
        .heap
        .objects
        .keys()
        .copied()
        .filter(|id| get(vm, *id, MEMORY) == Some(JsValue::Object(old)))
        .collect();
    if !owners.is_empty() {
        vm.pin(JsValue::Object(old));
        for owner in &owners {
            vm.pin(JsValue::Object(*owner));
        }
        let maximum = super::super::buffer_maximum(vm, old);
        let new = vm.alloc(Object::new(
            class,
            Some(vm.intrinsic_id(realm().ids.array_buffer_prototype)),
        ))?;
        super::super::protect_buffer(vm, new);
        if let Some(maximum) = maximum {
            super::super::set_buffer_maximum(vm, new, maximum);
        }
        for owner in owners {
            set(vm, owner, MEMORY, JsValue::Object(new));
        }
    }
    super::super::detach_buffer(vm, old);
    Ok(JsValue::Undefined)
}

/** Install the native memory wrapper and its two observable operations. */
pub(super) fn install(b: &mut Builder, namespace: ObjectId) -> ObjectId {
    let proto = b.object(Some(b.ids.object_prototype));
    let ctor = b.constructor("Memory", 1, construct, proto, None);
    b.alias(namespace, "Memory", ctor);
    b.builtin_accessor_with_attrs(
        proto,
        "buffer",
        Some(buffer_get),
        None,
        crate::v9::vm::heap::CONFIGURABLE | crate::v9::vm::heap::ENUMERABLE,
    );
    let grow = b.function("grow", 1, grow);
    b.value(
        proto,
        "grow",
        JsValue::Object(grow),
        crate::v9::vm::heap::WRITABLE
            | crate::v9::vm::heap::CONFIGURABLE
            | crate::v9::vm::heap::ENUMERABLE,
    );
    buffers::install(b, proto);
    proto
}
/** Limit values are unsigned indices, not saturating Rust casts from NaN or infinity. */
pub(super) fn index(vm: &mut Vm, value: &JsValue) -> Res<u32> {
    let n = vm.to_number(value)?;
    if !n.is_finite() {
        return Err(vm.type_error("WebAssembly index must be convertible to a valid number"));
    }
    if n < 0.0 {
        return Err(vm.type_error("WebAssembly index must be non-negative"));
    }
    if !(0.0..=f64::from(u32::MAX)).contains(&n) {
        return Err(vm.type_error("WebAssembly index must be in the unsigned long range"));
    }
    Ok(n as u32)
}
/** Read the descriptor's minimum and maximum in specification order. */
pub(super) fn limits(vm: &mut Vm, descriptor: &JsValue, bound: u32) -> Res<types::Limits> {
    address::read_limits(vm, descriptor, false, bound)
}
/** A memory is a branded wrapper around the existing ArrayBuffer representation. */
pub(super) fn create(vm: &mut Vm, limits: types::Limits, proto: ObjectId) -> Res<ObjectId> {
    if limits.min > page_limit(vm, limits.address64) as u64 {
        return Err(vm.range_error("WebAssembly memory allocation failed"));
    }
    let length = usize::try_from(limits.min)
        .ok()
        .and_then(|pages| pages.checked_mul(PAGE_BYTES))
        .ok_or_else(|| vm.range_error("WebAssembly memory allocation failed"))?;
    let memory = vm.alloc(Object::new(Class::Plain, Some(proto)))?;
    let storage = if limits.shared {
        shared_memory::create(vm, limits)?
    } else if length > BUFFER_BYTES
        || (length > 0
            && vm.heap.counts.units.saturating_add(length.div_ceil(2))
                > vm.limits.units / SPARSE_MEMORY_PRESSURE_DIVISOR)
    {
        let sparse = crate::v9::vm::data::SparseBuffer::new(length);
        let storage = vm.alloc(Object::new(
            Class::SparseWasmBuffer(sparse),
            Some(vm.intrinsic_id(realm().ids.array_buffer_prototype)),
        ))?;
        super::super::protect_buffer(vm, storage);
        storage
    } else {
        let storage = super::buffer(vm, vec![0; length])?;
        super::super::protect_buffer(vm, storage);
        storage
    };
    set(vm, memory, MEMORY, JsValue::Object(storage));
    address::mark(vm, memory, limits.address64);
    set(
        vm,
        memory,
        MAXIMUM,
        limits
            .max
            .map_or(JsValue::Undefined, |v| JsValue::Number(v as f64)),
    );
    Ok(memory)
}
/** Host page flags can only lower the engine's existing logical-memory limit. */
pub(super) fn page_limit(vm: &Vm, address64: bool) -> usize {
    let limit = if address64 {
        address::MAX_MEMORY64_PAGES
    } else {
        MAX_WASM_PAGES
    } as usize;
    #[cfg(feature = "conformance")]
    {
        vm.page
            .environment
            .conformance_wasm_max_memory_pages
            .map_or(limit, |n| limit.min(n))
    }
    #[cfg(not(feature = "conformance"))]
    {
        let _ = vm;
        limit
    }
}
/** Shared memories are explicitly unsupported rather than behaving like unshared buffers. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_none() {
        return Err(vm.type_error("WebAssembly.Memory must be invoked with 'new'"));
    }
    if call.arg(0).object().is_none() {
        return Err(vm.type_error("WebAssembly.Memory(): Argument 0 must be a memory descriptor"));
    }
    let address64 = address::descriptor(vm, &call.arg(0))?;
    let mut limits = address::read_limits(
        vm,
        &call.arg(0),
        address64,
        if address64 {
            address::MAX_MEMORY64_PAGES
        } else {
            MAX_WASM_PAGES
        },
    )?;
    let shared = vm.get(&call.arg(0), &key("shared"))?;
    limits.shared = shared.truthy();
    if limits.shared && limits.max.is_none() {
        return Err(vm.type_error("Shared WebAssembly memory requires a maximum"));
    }
    let proto = vm.prototype_for(call.new_target.as_ref().unwrap(), ids(vm).memory)?;
    create(vm, limits, proto).map(JsValue::Object)
}
/** Validate an inert memory record again after state restoration. */
pub(super) fn storage(vm: &mut Vm, memory: &JsValue) -> Res<ObjectId> {
    let object = brand(vm, memory, MEMORY)?;
    let storage = get(vm, object, MEMORY)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid WebAssembly memory record"))?;
    if super::super::buffer_immutable(vm, storage) {
        return Err(vm.type_error("WebAssembly memory cannot have immutable backing"));
    }
    let length = match vm.heap.get(storage).map(|o| &o.class) {
        Some(Class::ArrayBuffer(b)) if b.0.len() <= BUFFER_BYTES => Some(b.0.len()),
        Some(Class::SparseWasmBuffer(b)) if b.valid() => Some(b.length),
        Some(Class::SharedArrayBuffer(b)) if b.is_sparse() || b.backing_len() <= BUFFER_BYTES => {
            Some(b.backing_len())
        }
        _ => None,
    };
    if length.is_none_or(|length| length % PAGE_BYTES != 0) {
        return Err(vm.type_error("invalid WebAssembly memory buffer"));
    }
    Ok(storage)
}
/** The current memory buffer is a real ArrayBuffer shared with typed views and Wasm loads. */
fn buffer_get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call
        .this
        .object()
        .is_none_or(|id| get(vm, id, MEMORY).is_none())
    {
        return Err(vm.type_error("Receiver is not a WebAssembly.Memory"));
    }
    let memory = brand(vm, &call.this, MEMORY)?;
    let storage = storage(vm, &call.this)?;
    shared_memory::refresh(vm, memory, storage).map(JsValue::Object)
}
/** Byte length is checked at each native read, rather than trusting a serialized length field. */
pub(super) fn length(vm: &mut Vm, memory: &JsValue) -> Res<usize> {
    let storage = storage(vm, memory)?;
    Ok(match &vm.heap.get(storage).unwrap().class {
        Class::ArrayBuffer(bytes) => bytes.0.len(),
        Class::SparseWasmBuffer(bytes) => bytes.length,
        Class::SharedArrayBuffer(bytes) => bytes.backing_len(),
        _ => unreachable!(),
    })
}
/** Growth replaces and detaches the old unshared buffer even when delta is zero. */
pub(super) fn resize(vm: &mut Vm, memory: &JsValue, delta: u32) -> Res<u32> {
    let object = brand(vm, memory, MEMORY)?;
    let old = storage(vm, memory)?;
    let length = length(vm, memory)?;
    let pages = length / PAGE_BYTES;
    let next = pages
        .checked_add(delta as usize)
        .ok_or_else(|| vm.range_error("WebAssembly memory growth overflow"))?;
    let maximum = match get(vm, object, MAXIMUM) {
        Some(JsValue::Number(n))
            if n.is_finite()
                && n >= 0.0
                && n.fract() == 0.0
                && n <= f64::from(if address::is64(vm, object) {
                    address::MAX_MEMORY64_PAGES
                } else {
                    MAX_WASM_PAGES
                }) =>
        {
            n as usize
        }
        Some(JsValue::Undefined) => {
            if address::is64(vm, object) {
                address::MAX_MEMORY64_PAGES as usize
            } else {
                MAX_WASM_PAGES as usize
            }
        }
        _ => return Err(vm.type_error("invalid WebAssembly memory maximum")),
    }
    .min(page_limit(vm, address::is64(vm, object)));
    if shared_memory::backing(vm, old).is_some() {
        return shared_memory::grow(vm, object, old, delta, maximum);
    }
    if next > maximum {
        let message =
            if address::is64(vm, object) && get(vm, object, MAXIMUM) == Some(JsValue::Undefined) {
                "WebAssembly.Memory.grow(): Unable to grow instance memory"
            } else {
                "WebAssembly.Memory.grow(): Maximum memory size exceeded"
            };
        return Err(vm.range_error(message));
    }
    if next > MAX_PAGES
        || matches!(
            vm.heap.get(old).map(|o| &o.class),
            Some(Class::SparseWasmBuffer(_))
        )
    {
        return grow_sparse(vm, object, old, pages, next);
    }
    if super::super::buffer_maximum(vm, old).is_some() {
        if let Some(Class::ArrayBuffer(bytes)) = vm.heap.get_mut(old).map(|o| &mut o.class) {
            bytes.0.resize(next * PAGE_BYTES, 0);
        }
        vm.heap
            .adjust(0, ((next - pages) * PAGE_BYTES / 2) as isize);
        vm.collect_if_needed()?;
        return Ok(pages as u32);
    }
    let mut bytes = match &vm.heap.get(old).unwrap().class {
        Class::ArrayBuffer(bytes) => bytes.0.clone(),
        _ => unreachable!(),
    };
    bytes.resize(next * PAGE_BYTES, 0);
    let new = super::buffer(vm, bytes)?;
    super::super::protect_buffer(vm, new);
    super::super::detach_buffer(vm, old);
    set(vm, object, MEMORY, JsValue::Object(new));
    Ok(pages as u32)
}
/** Growth across the dense limit transfers actual resident bytes into a bounded sparse buffer. */
fn grow_sparse(
    vm: &mut Vm,
    memory: ObjectId,
    old: ObjectId,
    pages: usize,
    next: usize,
) -> Res<u32> {
    if super::super::buffer_maximum(vm, old).is_some() {
        return Err(vm.unsupported_error("sparse resizable WebAssembly memory"));
    }
    let length = next
        .checked_mul(PAGE_BYTES)
        .ok_or_else(|| vm.range_error("WebAssembly memory growth overflow"))?;
    let mut sparse = match &vm.heap.get(old).unwrap().class {
        Class::SparseWasmBuffer(bytes) => bytes.clone(),
        Class::ArrayBuffer(bytes) => {
            let mut sparse = crate::v9::vm::data::SparseBuffer::new(length);
            if !sparse.write(0, &bytes.0) {
                return Err(vm.range_error("WebAssembly memory resident byte limit"));
            }
            sparse
        }
        _ => return Err(vm.type_error("invalid WebAssembly memory backing")),
    };
    sparse.length = length;
    let new = vm.alloc(Object::new(
        Class::SparseWasmBuffer(sparse),
        Some(vm.intrinsic_id(realm().ids.array_buffer_prototype)),
    ))?;
    super::super::protect_buffer(vm, new);
    super::super::detach_buffer(vm, old);
    set(vm, memory, MEMORY, JsValue::Object(new));
    Ok(pages as u32)
}
/** JavaScript growth failures throw; Wasm memory.grow translates only allocation failures to -1. */
fn grow(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call
        .this
        .object()
        .is_none_or(|id| get(vm, id, MEMORY).is_none())
    {
        return Err(vm.type_error("Receiver is not a WebAssembly.Memory"));
    }
    let _ = storage(vm, &call.this)?;
    let object = brand(vm, &call.this, MEMORY)?;
    let address64 = address::is64(vm, object);
    let delta = address::index(vm, &call.arg(0), address64)?;
    address::grow(vm, &call.this, delta).map(|v| {
        if address64 {
            JsValue::BigInt(i128::from(v))
        } else {
            JsValue::Number(f64::from(v))
        }
    })
}
/** Linker checks actual limits and allocation, including a required declared maximum. */
pub(super) fn compatible(vm: &mut Vm, memory: &JsValue, required: types::Limits) -> Res<bool> {
    let id = brand(vm, memory, MEMORY)?;
    let pages = length(vm, memory)? / PAGE_BYTES;
    let max = match get(vm, id, MAXIMUM) {
        Some(JsValue::Number(n)) => Some(n as u32),
        _ => None,
    };
    let storage = storage(vm, memory)?;
    Ok(address::is64(vm, id) == required.address64
        && shared_memory::backing(vm, storage).is_some() == required.shared
        && pages >= required.min as usize
        && required
            .max
            .is_none_or(|r| max.is_some_and(|m| u64::from(m) <= r)))
}
/** Checked loads clone only the requested scalar's bytes. */
pub(super) fn read(vm: &mut Vm, memory: &JsValue, at: usize, count: usize) -> Res<Vec<u8>> {
    let storage = storage(vm, memory)?;
    if let Some(shared) = shared_memory::backing(vm, storage) {
        if at
            .checked_add(count)
            .is_none_or(|end| end > shared.backing_len())
        {
            return Err(trap(vm, "memory access out of bounds"));
        }
        return shared.read(at, count).ok_or_else(|| {
            if shared.is_sparse() {
                crate::protocol::EngineError::limit("V9 buffer resident bytes", BUFFER_BYTES).into()
            } else {
                trap(vm, "memory access out of bounds")
            }
        });
    }
    if let Some(Class::SparseWasmBuffer(bytes)) = vm.heap.get(storage).map(|o| &o.class) {
        if at.checked_add(count).is_none_or(|end| end > bytes.length) {
            return Err(trap(vm, "memory access out of bounds"));
        }
        return bytes.read(at, count).ok_or_else(|| {
            crate::protocol::EngineError::limit("V9 buffer resident bytes", BUFFER_BYTES).into()
        });
    }
    let end = at
        .checked_add(count)
        .ok_or_else(|| trap(vm, "memory access out of bounds"))?;
    let Class::ArrayBuffer(bytes) = &vm.heap.get(storage).unwrap().class else {
        unreachable!()
    };
    bytes
        .0
        .get(at..end)
        .map(<[u8]>::to_vec)
        .ok_or_else(|| trap(vm, "memory access out of bounds"))
}
/** Checked writes never expose a partial store when an address is out of bounds. */
pub(super) fn write(vm: &mut Vm, memory: &JsValue, at: usize, source: &[u8]) -> Res<()> {
    let storage = storage(vm, memory)?;
    if let Some(shared) = shared_memory::backing(vm, storage) {
        if at
            .checked_add(source.len())
            .is_none_or(|end| end > shared.backing_len())
        {
            return Err(trap(vm, "memory access out of bounds"));
        }
        return if shared.write(at, source) {
            Ok(())
        } else if shared.is_sparse() {
            Err(
                crate::protocol::EngineError::limit("V9 buffer resident bytes", BUFFER_BYTES)
                    .into(),
            )
        } else {
            Err(trap(vm, "memory access out of bounds"))
        };
    }
    let end = at
        .checked_add(source.len())
        .ok_or_else(|| trap(vm, "memory access out of bounds"))?;
    if end > length(vm, memory)? {
        return Err(trap(vm, "memory access out of bounds"));
    }
    if matches!(
        vm.heap.get(storage).map(|o| &o.class),
        Some(Class::SparseWasmBuffer(_))
    ) {
        return if super::super::typed::write_range(vm, storage, at, source) {
            Ok(())
        } else {
            Err(
                crate::protocol::EngineError::limit("V9 buffer resident bytes", BUFFER_BYTES)
                    .into(),
            )
        };
    }
    let Class::ArrayBuffer(bytes) = &mut vm.heap.get_mut(storage).unwrap().class else {
        unreachable!()
    };
    bytes.0[at..end].copy_from_slice(source);
    Ok(())
}
