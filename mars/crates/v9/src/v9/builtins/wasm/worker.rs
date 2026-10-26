/** Worker modules cross page heaps as owned source and primitive compile options, never object identities. */
use super::*;

/** Serialized worker messages cannot carry VM pointers, callables or authority through a module packet. */
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::v9) struct WasmModulePacket {
    pub bytes: Vec<u8>,
    pub builtins: Vec<String>,
    pub string_constants: Option<Vec<u16>>,
}
/** Extract only inaccessible Module storage, leaving all validation to reconstruction in the receiver. */
pub(in crate::v9) fn wasm_worker_module(vm: &Vm, value: &JsValue) -> Option<WasmModulePacket> {
    let module = value.object()?;
    let buffer = get(vm, module, MODULE)?.object()?;
    let Some(Class::ArrayBuffer(bytes)) = vm.heap.get(buffer).map(|o| &o.class) else {
        return None;
    };
    let (builtins, string_constants) = imports::snapshot(vm, module);
    Some(WasmModulePacket {
        bytes: bytes.0.clone(),
        builtins,
        string_constants,
    })
}
/** Receiver construction pays normal byte fuel, binary validation, signature checks and heap quotas. */
pub(in crate::v9) fn wasm_worker_restore(vm: &mut Vm, packet: &WasmModulePacket) -> Res<JsValue> {
    if packet.bytes.len() > BUFFER_BYTES {
        return Err(vm.range_error("WebAssembly worker module exceeds buffer limit"));
    }
    vm.charge(packet.bytes.len())?;
    let selected = packet
        .builtins
        .iter()
        .map(|s| JsValue::string(s))
        .collect::<crate::protocol::EngineResult<Vec<_>>>()?;
    let namespace = packet
        .string_constants
        .as_ref()
        .map(|units| JsString::from_units(units.clone()).map(JsValue::String))
        .transpose()?;
    finish_module(vm, packet.bytes.clone(), (selected, namespace), None).map(JsValue::Object)
}
/** Other opaque wrappers cannot silently degrade to an empty ordinary object during structured cloning. */
pub(in crate::v9) fn wasm_worker_opaque(vm: &Vm, value: &JsValue) -> bool {
    value.object().is_some_and(|id| {
        [
            MODULE,
            INSTANCE,
            MEMORY,
            TABLE,
            GLOBAL,
            "wasm.tag.types",
            "wasm.exception.tag",
            "wasm.gc.module",
            "wasm.suspending.callback",
            "wasm.string.iterator",
        ]
        .iter()
        .any(|name| get(vm, id, name).is_some())
            || exceptions::is_js_tag(vm, value)
    })
}

/** Shared memory packets grant the actual synchronized backing and retain declared Wasm growth bounds. */
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub(in crate::v9) struct WasmMemoryPacket {
    pub backing: crate::v9::vm::data::SharedBytes,
    pub maximum: u32,
    pub address64: bool,
    #[serde(default)]
    pub growable: bool,
}
/** Unshared memories remain unclonable; only genuinely shared backings can cross Workers. */
pub(in crate::v9) fn wasm_worker_memory(vm: &Vm, value: &JsValue) -> Option<WasmMemoryPacket> {
    let memory = value.object()?;
    let storage = get(vm, memory, MEMORY)?.object()?;
    let backing = shared_memory::backing(vm, storage)?;
    let maximum = match get(vm, memory, MAXIMUM)? {
        JsValue::Number(n)
            if n.is_finite()
                && n >= 0.0
                && n.fract() == 0.0
                && n <= address::MAX_MEMORY64_PAGES as f64 =>
        {
            n as u32
        }
        _ => return None,
    };
    Some(WasmMemoryPacket {
        backing,
        maximum,
        address64: address::is64(vm, memory),
        growable: matches!(vm.heap.get(storage).map(|o|&o.class),Some(Class::SharedArrayBuffer(bytes)) if bytes.wasm_maximum().is_some()),
    })
}
/** Imported memory wrappers retain broker-authorized backing identity and normal receiver brands. */
pub(in crate::v9) fn wasm_worker_restore_memory(
    vm: &mut Vm,
    packet: &WasmMemoryPacket,
) -> Res<JsValue> {
    if packet.maximum
        > if packet.address64 {
            address::MAX_MEMORY64_PAGES
        } else {
            MAX_WASM_PAGES
        }
        || packet.backing.backing_len() > packet.maximum as usize * PAGE_BYTES
        || packet.backing.backing_len() % PAGE_BYTES != 0
    {
        return Err(vm.type_error("invalid shared Wasm memory packet"));
    }
    let visible = if packet.growable {
        packet
            .backing
            .growable_view(packet.maximum as usize * PAGE_BYTES)
    } else {
        packet.backing.fixed_view(packet.backing.backing_len())
    }
    .ok_or_else(|| vm.type_error("invalid shared Wasm memory window"))?;
    let storage = super::super::import_shared_buffer(vm, visible)?;
    let memory = vm.alloc(Object::new(Class::Plain, Some(ids(vm).memory)))?;
    set(vm, memory, MEMORY, JsValue::Object(storage));
    address::mark(vm, memory, packet.address64);
    set(vm, memory, MAXIMUM, JsValue::Number(packet.maximum as f64));
    Ok(JsValue::Object(memory))
}
