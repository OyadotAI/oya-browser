/** D8 file aliases use only owned fixture bytes and precompiled global Scripts supplied by the test host. */
use super::*;
use crate::v9::vm::data::{Bytes, Class};
use crate::v9::vm::heap::Object;
use crate::v9::vm::realm::realm;

/** Answer only paths whose existence the host explicitly supplied; the VM never stats files. */
pub(super) fn exists(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.args.is_empty() {
        let error = vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            "exists() takes one argument",
        )?;
        return Err(crate::v9::vm::Thrown::Value(JsValue::Object(error)));
    }
    let path = arg_string(vm, &call, 0)?.lossy();
    match vm.page.environment.conformance_exists.get(&path) {
        Some(found) => Ok(JsValue::Boolean(*found)),
        None => Err(vm.unsupported_error("d8.file.exists path not supplied by the test host")),
    }
}

/** Legacy shell functions are genuine constructible FunctionTemplates with ordinary per-function prototypes. */
pub(super) fn install(builder: &mut Builder) {
    for (name, function) in [
        ("load", load as crate::v9::vm::realm::NativeFn),
        ("readbuffer", read_buffer),
        ("version", version),
    ] {
        let prototype = builder.object(Some(builder.ids.object_prototype));
        let function = builder.constructor(name, 0, function, prototype, None);
        builder.global(name, function);
    }
}
/** Native construction returns the receiver implied by the actual new.target prototype. */
pub(super) fn receiver(vm: &mut Vm, target: Option<JsValue>) -> Res<JsValue> {
    let Some(target) = target else {
        return Ok(JsValue::Undefined);
    };
    let prototype = vm.prototype_for(&target, realm().ids.object_prototype)?;
    vm.alloc(Object::new(Class::Plain, Some(prototype)))
        .map(JsValue::Object)
}
/** Every file runs in the same global Script scope, in argument order, stopping on the first throw. */
fn load(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let target = call.new_target.clone();
    execute(vm, call)?;
    receiver(vm, target)
}

/** ExecuteFile processes real global Scripts sequentially and stops at the first reported failure. */
pub(super) fn execute(vm: &mut Vm, call: Call) -> Res<JsValue> {
    for index in 0..call.args.len() {
        let path = arg_string(vm, &call, index)?.lossy();
        if let Err(error) = execute_one(vm, &path) {
            return wrap_error(vm, &path, error);
        }
    }
    Ok(JsValue::Undefined)
}
/** Host file identities refer only to source already supplied and compiled through normal VM bounds. */
fn execute_one(vm: &mut Vm, path: &str) -> Res<JsValue> {
    let code = vm
        .modules
        .loaded(path)
        .ok_or_else(|| vm.type_error(&format!("Test host did not supply fixture {path}.")))?;
    let code = code.map_err(|error| match error.code.as_str() {
        "script_syntax_error" => vm.syntax_error(&error.message),
        _ => error.into(),
    })?;
    vm.run_script(code)?;
    Ok(JsValue::Undefined)
}
/** Ordinary file exceptions are reported and wrapped; native limits, unsupported capabilities and termination retain provenance. */
fn wrap_error(vm: &mut Vm, path: &str, error: crate::v9::vm::Thrown) -> Res<JsValue> {
    let crate::v9::vm::Thrown::Value(original) = error else {
        return Err(error);
    };
    vm.note_conformance_native_error(&original);
    let assertion = vm.is_conformance_assertion(&original);
    let description = vm
        .conformance_description(&original)
        .unwrap_or_else(|| vm.describe(&original));
    vm.page.console("error", format!("Uncaught {description}"));
    let error = vm.make_error(
        crate::v9::vm::error::ErrorKind::Error,
        &format!("Error executing file: \"{path}\""),
    )?;
    if assertion {
        vm.heap.put(
            error,
            crate::v9::vm::heap::Key::private("conformance-file-assertion"),
            crate::v9::vm::heap::Prop::data(JsValue::Boolean(true)),
        );
    }
    Err(crate::v9::vm::Thrown::Value(JsValue::Object(error)))
}
/** The shell reports this interpreter's real package version and never impersonates V8's version string. */
fn version(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if call.new_target.is_some() {
        return receiver(vm, call.new_target);
    }
    text(vm, concat!("Mars V9 ", env!("CARGO_PKG_VERSION")))
}
/** Binary fixtures retain every byte, including invalid UTF-8, and allocate an actual independent ArrayBuffer. */
fn read_buffer(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let path = arg_string(vm, &call, 0)?.lossy();
    let Some(bytes) = vm.page.environment.conformance_files.get(&path).cloned() else {
        let error = vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            &format!("Error reading file \"{path}\""),
        )?;
        return Err(crate::v9::vm::Thrown::Value(JsValue::Object(error)));
    };
    if bytes.len() > 2 * crate::constants::JS_STRING_UNITS {
        return Err(vm.range_error("Array buffer allocation failed"));
    }
    vm.charge(bytes.len())?;
    vm.alloc(Object::new(
        Class::ArrayBuffer(Bytes(bytes)),
        Some(realm().ids.array_buffer_prototype),
    ))
    .map(JsValue::Object)
}
