/** The shell reports caught host-callback and microtask exceptions in their real active context. */
use crate::protocol::{EngineError, SourceLocation};
use crate::v9::builtins::{key, text};
use crate::v9::value::{JsString, JsValue};
use crate::v9::vm::{Res, Thrown, Vm};

mod diagnostics;
mod terminal;
use diagnostics::{describe, is_assertion};
pub(super) use terminal::is_failed_exit;
pub(in crate::v9) use terminal::terminal;

/** Assertion/native provenance remains a scoring failure even though ordinary callback reporting is caught. */
pub(in crate::v9) fn exception(vm: &mut Vm, error: JsValue) -> Res<()> {
    detail(vm, error).map(|_| ())
}

/** The same real reporter supplies a terminal diagnostic without duplicate author evaluation. */
fn detail(vm: &mut Vm, error: JsValue) -> Res<Option<JsString>> {
    let roots = vm.pins.len();
    vm.pins.push(error.clone());
    note(vm, &error);
    let result = report(vm, error);
    vm.pins.truncate(roots);
    result
}

/** Native/assertion provenance cannot disappear inside another caught reporting callback. */
fn note(vm: &mut Vm, error: &JsValue) {
    vm.note_conformance_native_error(error);
    let assertion = is_assertion(vm, error);
    if assertion {
        let description = describe(vm, error);
        vm.page
            .effects
            .conformance_error
            .get_or_insert(format!("MjsUnitAssertionError: {description}"));
    }
}

/** The pinned shell gives a real onerror callback the original value before formatting it for output. */
fn report(vm: &mut Vm, error: JsValue) -> Res<Option<JsString>> {
    if handled(vm, &error)? {
        return Ok(None);
    }
    match vm.to_string(&error) {
        Ok(message) => {
            vm.page.console("error", message.lossy());
            Ok(Some(message))
        }
        // ReportException's own non-verbose TryCatch catches failed author error formatting.
        Err(Thrown::Value(error)) => {
            note(vm, &error);
            Ok(None)
        }
        Err(fatal) => Err(fatal),
    }
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
    let message = format!("Uncaught {}", describe(vm, error));
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
