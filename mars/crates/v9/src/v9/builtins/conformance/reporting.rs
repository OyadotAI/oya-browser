/** The shell reports caught host-callback and microtask exceptions in their real active context. */
use crate::v9::builtins::{key, text};
use crate::v9::value::JsValue;
use crate::v9::vm::{Res, Thrown, Vm};

/** Assertion/native provenance remains a scoring failure even though ordinary callback reporting is caught. */
pub(super) fn exception(vm: &mut Vm, error: JsValue) -> Res<()> {
    note(vm, &error);
    let roots = vm.pins.len();
    vm.pin(error.clone());
    let result = report(vm, error);
    vm.pins.truncate(roots);
    result
}

/** Native/assertion provenance cannot disappear inside another caught reporting callback. */
fn note(vm: &mut Vm, error: &JsValue) {
    vm.note_conformance_native_error(error);
    let assertion = vm.is_conformance_assertion(error);
    if assertion {
        let description = vm.describe(error);
        vm.page
            .effects
            .conformance_error
            .get_or_insert(format!("MjsUnitAssertionError: {description}"));
    }
}

/** The pinned shell gives a real onerror callback the original value before formatting it for output. */
fn report(vm: &mut Vm, error: JsValue) -> Res<()> {
    if handled(vm, &error)? {
        return Ok(());
    }
    match vm.to_string(&error) {
        Ok(message) => vm.page.console("error", message.lossy()),
        // ReportException's own non-verbose TryCatch catches failed author error formatting.
        Err(Thrown::Value(error)) => note(vm, &error),
        Err(fatal) => return Err(fatal),
    }
    Ok(())
}

/** ReportException runs in the caller's active context without entering another host context scope. */
fn handled(vm: &mut Vm, error: &JsValue) -> Res<bool> {
    let global = JsValue::Object(vm.global_object());
    let handler = match vm.get(&global, &key("onerror")) {
        Ok(handler) => handler,
        Err(Thrown::Value(error)) => {
            note(vm, &error);
            return Ok(false);
        }
        Err(fatal) => return Err(fatal),
    };
    if !vm.is_callable(&handler) {
        return Ok(false);
    }
    let location = vm.page.throw_location.unwrap_or_default();
    let message = format!("Uncaught {}", vm.describe(error));
    let source = vm.modules.source_name(location.byte_offset);
    let args = vec![
        text(vm, &message)?,
        text(vm, &source)?,
        JsValue::Number(location.line as f64),
        JsValue::Number(location.column as f64),
        error.clone(),
    ];
    match vm.call(handler, global, args) {
        Ok(JsValue::Boolean(true)) => Ok(true),
        Ok(_) => Ok(false),
        Err(Thrown::Value(error)) => {
            note(vm, &error);
            Ok(false)
        }
        Err(fatal) => Err(fatal),
    }
}
