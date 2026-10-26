/** The ECMAScript standard library V9 implements in Rust: one file per builtin family. */
mod array;
mod auto_accessors;
pub(in crate::v9) use auto_accessors::{
    create as create_auto_accessors, public_name as inferred_function_name,
};
mod bigint;
mod clone;
mod collections;
#[cfg(feature = "conformance")]
mod conformance;
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::{
    ProfileClock, ProfileState, profile_tick, validate_profile, validate_profile_state,
};
mod date;
pub(in crate::v9) use date::validate_timezone;
mod disposal;
pub(in crate::v9) use disposal::{
    add as disposal_add, begin as disposal_begin, create as disposal_create,
    dispose as disposal_dispose, error as disposal_error, rejected as disposal_rejected,
    step as disposal_step,
};
mod encoding;
mod error;
mod function;
mod globals;
mod grouping;
mod integrity;
mod intl;
mod iterators;
mod json;
mod math;
mod number;
mod object;
pub(in crate::v9) use object::freeze_object;
pub(in crate::v9) mod promise;
mod promise_combinators;
pub(in crate::v9) use promise_combinators::{AllTrace, all_trace as promise_all_trace};
mod proxy;
mod reflect;
mod regexp;
mod streams;
mod string;
mod symbol;
mod temporal;
pub(in crate::v9) mod typed;
mod wasm;
mod weak;
pub(in crate::v9) use wasm::trace_metadata as wasm_trace_metadata;
#[cfg(feature = "conformance")]
pub(in crate::v9) use wasm::wasm_worker_restore_shared_gc;
#[cfg(feature = "conformance")]
pub(in crate::v9) use wasm::{
    WasmMemoryPacket, WasmModulePacket, wasm_worker_memory, wasm_worker_module, wasm_worker_opaque,
    wasm_worker_restore, wasm_worker_restore_memory,
};
pub(in crate::v9) use wasm::{initialize_shared_gc, validate_shared_gc};

#[cfg(feature = "conformance")]
pub(in crate::v9) use error::formatted_stack as error_stack;
pub(in crate::v9) use error::legacy_code;
#[cfg(feature = "conformance")]
pub(in crate::v9) use function::compile_indirect_eval;
pub(in crate::v9) use function::{dynamic_base, evaluate as evaluate_source};
pub(in crate::v9) use integrity::{
    get_prototype_of as object_get_prototype, is_extensible as object_is_extensible,
    set_prototype_of as object_set_prototype,
};
pub(in crate::v9) use json::{is_raw_json, json_parse_text};
pub(in crate::v9) use object::{from_descriptor, to_descriptor};
#[cfg(feature = "conformance")]
pub(in crate::v9) use typed::{buffer_detachable, buffer_detached};
pub(in crate::v9) use typed::{
    buffer_immutable, buffer_maximum, define_integer as typed_define_integer, detach_buffer,
    integer_index as typed_index, keys_length as typed_keys_length, load as typed_load,
    make_buffer_immutable, numeric_index as typed_numeric_index, protect_buffer, random_values,
    set_buffer_maximum, set_integer as typed_set_integer, valid_buffer, valid_immutable_brand,
    valid_view as valid_typed_view, view_of,
};
pub(in crate::v9) use typed::{
    import_shared_buffer, initialize_shared, notify_shared, validate_shared,
};
#[cfg(feature = "conformance")]
pub(in crate::v9) use weak::cancel_finalizers_for_realm;
pub(in crate::v9) use weak::schedule_cleanup as schedule_finalization;

/** A symbol's description for display. */
pub(in crate::v9) fn symbol_text(vm: &Vm, id: u32) -> String {
    symbol_description(vm, id).unwrap_or_default()
}

/** Native shell output distinguishes a missing symbol description from an explicitly empty description. */
pub(in crate::v9) fn symbol_description(vm: &Vm, id: u32) -> Option<String> {
    symbol::symbol_description(vm, id)
}

use super::value::{JsString, JsValue, ObjectId};
use super::vm::heap::Key;
use super::vm::realm::Builder;
use super::vm::{Call, Res, Vm};

#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::enqueue_shell_microtask;
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::is_test_exit;
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::measure_memory;
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::report_terminal_exception;
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::{WorkerGraph, worker_activate, worker_decode};
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::{
    async_hook_after, async_hook_before, async_hook_init, async_hook_resolve, async_hook_resource,
    async_hook_unwind, conformance_host_object, expose_async_hooks, expose_conformance_globals,
};
#[cfg(feature = "conformance")]
pub(in crate::v9) use conformance::{
    console_output as conformance_console,
    validate_environment as validate_conformance_environment, validate_native_test_object,
};
#[cfg(feature = "conformance")]
pub(in crate::v9) use function::to_string as function_to_string;
#[cfg(feature = "conformance")]
pub(in crate::v9) use json::stringify as json_stringify;

/** Install every builtin, in dependency order. Prototypes come first so constructors can link them. */
pub(in crate::v9) fn install(builder: &mut Builder) {
    object::prototypes(builder);
    function::install(builder);
    auto_accessors::install(builder);
    object::install(builder);
    symbol::install(builder);
    bigint::install(builder);
    error::install(builder);
    disposal::install(builder);
    iterators::install(builder);
    array::install(builder);
    string::install(builder);
    number::install(builder);
    math::install(builder);
    intl::install(builder);
    json::install(builder);
    date::install(builder);
    regexp::install(builder);
    collections::install(builder);
    promise::install(builder);
    reflect::install(builder);
    globals::install(builder);
    clone::install(builder);
    encoding::install(builder);
    streams::install(builder);
    weak::install(builder);
    wasm::install(builder);
    proxy::install(builder);
    temporal::install(builder);
    #[cfg(feature = "conformance")]
    conformance::install(builder);
}

/** A string argument, converted. */
pub(in crate::v9) fn arg_string(vm: &mut Vm, call: &Call, index: usize) -> Res<JsString> {
    let value = call.arg(index);
    vm.to_string(&value)
}

/** A number argument, converted. */
pub(in crate::v9) fn arg_number(vm: &mut Vm, call: &Call, index: usize) -> Res<f64> {
    let value = call.arg(index);
    vm.to_number(&value)
}

/** `this` as an object, or a TypeError naming the method. */
pub(in crate::v9) fn this_object(vm: &mut Vm, call: &Call, method: &str) -> Res<ObjectId> {
    match &call.this {
        JsValue::Object(id) => Ok(*id),
        JsValue::Undefined | JsValue::Null => {
            Err(vm.type_error(&format!("{method} called on null or undefined")))
        }
        other => vm.to_object(other),
    }
}

/** A string from Rust text. */
pub(in crate::v9) fn text(vm: &mut Vm, text: &str) -> Res<JsValue> {
    let _ = vm;
    Ok(JsValue::String(JsString::from_utf8(text)?))
}

/** A key from Rust text. */
pub(in crate::v9) fn key(name: &str) -> Key {
    Key::from(name)
}
