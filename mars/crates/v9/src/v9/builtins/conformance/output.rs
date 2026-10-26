/** Opt-in d8 shell output has native coercion, assertion, timer and stack behavior distinct from the browser console. */
use super::*;
use crate::v9::value::{JsString, ObjectId};
use crate::v9::vm::Thrown;
use crate::v9::vm::error::ErrorKind;
mod timers;
mod writing;

/** Native shell output is available without loading an assertion harness or replacing native functions with JavaScript. */
pub(super) fn install(builder: &mut Builder) {
    let print = builder.function("print", 0, print);
    builder.global("print", print);
    let print_err = builder.function("printErr", 0, print_err);
    builder.global("printErr", print_err);
}

/** Stdout becomes a bounded real host effect, keeping worker protocol stdout reserved for its transport. */
fn print(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.with_api_activation(call.callee, call.this.clone(), |vm| {
        writing::write(vm, call.args, "log", "", true)?;
        Ok(JsValue::Undefined)
    })
}

/** Stderr has an independent output level and cannot be mistaken for an uncaught script exception. */
fn print_err(vm: &mut Vm, call: Call) -> Res<JsValue> {
    vm.with_api_activation(call.callee, call.this.clone(), |vm| {
        writing::write(vm, call.args, "error", "", true)?;
        Ok(JsValue::Undefined)
    })
}

/** D8Console::Assert writes all arguments, then throws its own Error after a falsy condition. */
fn assertion(vm: &mut Vm, call: &Call) -> Res<JsValue> {
    if call.arg(0).truthy() {
        return Ok(JsValue::Undefined);
    }
    let written = writing::write(vm, call.args, "log", "console.assert: ", false);
    if let Err(Thrown::Fatal(error)) = written {
        return Err(Thrown::Fatal(error));
    }
    let error = vm.make_error(ErrorKind::Error, "console.assert failed")?;
    Err(Thrown::Value(JsValue::Object(error)))
}

/** Actual method callbacks preserve d8 prefixes and output levels; profiling remains explicitly unsupported. */
pub(in crate::v9) fn console(vm: &mut Vm, call: Call, method: &str) -> Res<JsValue> {
    let (level, prefix) = match method {
        "assert" => return assertion(vm, &call),
        "time" | "timeLog" | "timeEnd" | "timeStamp" => return timers::timer(vm, &call, method),
        "trace" => {
            let trace = vm.throw_trace().join("\n");
            vm.page.console("error", trace);
            return Ok(JsValue::Undefined);
        }
        "log" => ("log", ""),
        "error" => ("error", "console.error: "),
        "warn" => ("warn", "console.warn: "),
        "info" => ("info", "console.info: "),
        "debug" => ("debug", "console.debug: "),
        "profile" | "profileEnd" => return Err(vm.unsupported_error("d8 CPU profiling")),
        // The pinned d8 ConsoleDelegate does not override these callbacks.
        _ => return Ok(JsValue::Undefined),
    };
    writing::write(vm, call.args, level, prefix, false)?;
    Ok(JsValue::Undefined)
}
