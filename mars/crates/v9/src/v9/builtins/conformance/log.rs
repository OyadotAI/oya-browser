/** d8.log captures a real record of V9's own collections while `--log` is active, and hands it over once stopped. */
use super::*;
use crate::protocol::EngineError;
use crate::v9::value::ObjectId;
use crate::v9::vm::Thrown;
use crate::v9::vm::error::ErrorKind;

/** V8's message when getAndStop runs with no capture in progress. */
const NOT_LOGGING: &str = "Logging not enabled.";
/** One record per collection: the kind of collection and the objects it left live. */
const RECORD_SEPARATOR: char = '\n';

/** The d8 namespace gets a `log` object with a single `getAndStop` method, as the shell installs it. */
pub(super) fn install(builder: &mut Builder, d8: ObjectId) {
    let log = builder.object(Some(builder.ids.object_prototype));
    builder.method(log, "getAndStop", 0, get_and_stop);
    builder.alias(d8, "log", log);
}

/** Taking the text ends the capture, so a second call reports that logging is no longer enabled. */
fn get_and_stop(vm: &mut Vm, _call: Call) -> Res<JsValue> {
    let Some(text) = vm.page.effects.conformance_log.take() else {
        return Err(error(vm, NOT_LOGGING));
    };
    crate::v9::builtins::text(vm, &text)
}

/** Appends one collection record while a capture is active; collections outside a capture leave no trace. */
pub(in crate::v9) fn record_collection(vm: &mut Vm, kind: &str, live: usize) -> Res<()> {
    let Some(log) = vm.page.effects.conformance_log.as_mut() else {
        return Ok(());
    };
    let record = format!("gc,{kind},{live}{RECORD_SEPARATOR}");
    if log.len().saturating_add(record.len()) > vm.limits.state_bytes {
        return Err(EngineError::limit("V9 d8 log bytes", vm.limits.state_bytes).into());
    }
    log.push_str(&record);
    Ok(())
}

/** Ordinary Error objects carry the message V8 uses for the same refusal. */
fn error(vm: &mut Vm, message: &str) -> Thrown {
    match vm.make_error(ErrorKind::Error, message) {
        Ok(id) => Thrown::Value(JsValue::Object(id)),
        Err(error) => error.into(),
    }
}
