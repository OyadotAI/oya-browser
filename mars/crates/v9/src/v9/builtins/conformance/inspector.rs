/** The V8 inspector surface d8 exposes under --enable-inspector: a global `send(text)` that takes
JSON-RPC requests, replies through the page's global `receive`, and runs the page's
`handleInspectorMessage` while a requested pause holds. Conformance host only; the browser build
has no `send`, no debugger, and no pause points. */
use crate::v9::builtins::{arg_string, function};
use crate::v9::value::{JsValue, ObjectId, number_to_string};
use crate::v9::vm::GLOBAL;
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object, Prop, Slot};
use crate::v9::vm::realm::{Builder, banks};
use crate::v9::vm::{Call, Res, Thrown, Vm};
use serde_json::{Value, json};
use std::sync::OnceLock;

/** The `send` native's identity, the same in every bank once installed. */
static SEND: OnceLock<ObjectId> = OnceLock::new();
/** Hidden record on the isolate global that holds the agent's flags. */
const SESSION_KEY: &str = "inspector-session";
/** Debugger.enable has been received and not disabled. */
const ENABLED: &str = "debugger-enabled";
/** Debugger.pause was received and no pause has run yet. */
const PAUSE_REQUESTED: &str = "pause-requested";
/** A pause is active: the page's handler keeps running until Debugger.resume clears this. */
const PAUSED: &str = "paused";
/** The page callback that receives every reply and notification, as d8 looks it up. */
const RECEIVE: &str = "receive";
/** The page callback a paused agent runs until it resumes, as d8 looks it up. */
const HANDLE_MESSAGE: &str = "handleInspectorMessage";
/** The identifier V8 returns from Debugger.enable; this host runs one agent per isolate. */
const DEBUGGER_ID: &str = "oya-test-debugger";
/** JSON-RPC: the message could not be parsed. */
const PARSE_ERROR: i64 = -32700;
/** JSON-RPC: the method is not one this host implements. */
const METHOD_NOT_FOUND: i64 = -32601;
/** V8's code for a command refused because the debugger agent is off. */
const SERVER_ERROR: i64 = -32000;
/** V8's message when a debugger command arrives before Debugger.enable. */
const NOT_ENABLED: &str = "Debugger agent is not enabled";
/** Runtime.evaluate exception id; the host reports one exception per evaluation. */
const EXCEPTION_ID: u32 = 1;
/** Exception text V8 uses for an uncaught evaluation error. */
const UNCAUGHT: &str = "Uncaught";

/** A command's outcome: a result object, or a protocol error with its code and message. */
enum Reply {
    Result(Value),
    Error(i64, String),
}

/** A protocol command: receives its params and answers with a Reply. */
type Command = fn(&mut Vm, &Value) -> Res<Reply>;

/** Commands this host implements. Breakpoints and script events need wasm debug info this host lacks. */
const COMMANDS: &[(&str, Command)] = &[
    ("Runtime.enable", runtime_enable),
    ("Runtime.evaluate", runtime_evaluate),
    ("Debugger.enable", debugger_enable),
    ("Debugger.disable", debugger_disable),
    ("Debugger.pause", debugger_pause),
    ("Debugger.resume", debugger_resume),
];

/** Installs the `send` native for the expose step. */
pub(super) fn install(builder: &mut Builder) {
    let _ = SEND.set(builder.function("send", 1, send));
}

/** Exposes `send` on a fresh global only when the actual shell flag enabled the inspector. */
pub(in crate::v9) fn expose(vm: &mut Vm, global: ObjectId, bank: u32) {
    if !vm.page.environment.conformance_inspector {
        return;
    }
    let id = *SEND.get().expect("installed send intrinsic") + bank * banks::BANK_STRIDE;
    vm.materialize(id).expect("installed send intrinsic");
    vm.heap
        .put(global, Key::from("send"), Prop::data(JsValue::Object(id)));
}

/** `send(text)`: parses one request, dispatches it, and delivers the reply to the page's `receive`. */
fn send(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let text = arg_string(vm, &call, 0)?.lossy();
    let reply = match serde_json::from_str::<Value>(&text) {
        Ok(message) => dispatch(vm, &message)?,
        Err(_) => error_reply(Value::Null, PARSE_ERROR, "Parse error".into()),
    };
    deliver(vm, &reply)?;
    Ok(JsValue::Undefined)
}

/** Looks the method up in the command table; an unknown method is a protocol error, not a crash. */
fn dispatch(vm: &mut Vm, message: &Value) -> Res<Value> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let method = message.get("method").and_then(Value::as_str).unwrap_or("");
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    let Some((_, command)) = COMMANDS.iter().find(|(name, _)| *name == method) else {
        let text = format!("'{method}' is not supported by the V9 test host");
        return Ok(error_reply(id, METHOD_NOT_FOUND, text));
    };
    Ok(match command(vm, &params)? {
        Reply::Result(result) => json!({"id": id, "result": result}),
        Reply::Error(code, text) => error_reply(id, code, text),
    })
}

/** A JSON-RPC error envelope. */
fn error_reply(id: Value, code: i64, message: String) -> Value {
    json!({"id": id, "error": {"code": code, "message": message}})
}

/** Hands the reply to the page's `receive`. An exception it throws reaches the sender, as d8's pending exception does. */
fn deliver(vm: &mut Vm, reply: &Value) -> Res<()> {
    let receiver = vm.get(&JsValue::Object(GLOBAL), &Key::from(RECEIVE))?;
    if !vm.is_callable(&receiver) {
        return Ok(());
    }
    let text = JsValue::string(&reply.to_string())?;
    vm.call(receiver, JsValue::Undefined, vec![text])?;
    Ok(())
}

/** Runtime.enable acknowledges the domain; this host has no per-domain state to switch. */
fn runtime_enable(_: &mut Vm, _: &Value) -> Res<Reply> {
    Ok(Reply::Result(json!({})))
}

/** Runtime.evaluate: the expression runs as indirect eval in the realm and its completion value is the result. */
fn runtime_evaluate(vm: &mut Vm, params: &Value) -> Res<Reply> {
    let expression = params
        .get("expression")
        .and_then(Value::as_str)
        .unwrap_or("");
    match function::evaluate(vm, expression.to_owned()) {
        Ok(value) => Ok(Reply::Result(json!({"result": mirror(&value)}))),
        Err(Thrown::Value(exception)) => Ok(Reply::Result(thrown(&exception))),
        Err(other) => Err(other),
    }
}

/** The result shape of an uncaught exception: the thrown value and its exception details. */
fn thrown(exception: &JsValue) -> Value {
    json!({
        "result": mirror(exception),
        "exceptionDetails": {
            "exceptionId": EXCEPTION_ID,
            "text": UNCAUGHT,
            "lineNumber": 0,
            "columnNumber": 0,
            "exception": mirror(exception),
        },
    })
}

/** A RemoteObject. Objects get a type but no objectId: this host keeps no object handles. */
fn mirror(value: &JsValue) -> Value {
    match value {
        JsValue::Undefined | JsValue::Hole => json!({"type": "undefined"}),
        JsValue::Null => json!({"type": "object", "subtype": "null", "value": null}),
        JsValue::Boolean(flag) => json!({"type": "boolean", "value": flag}),
        JsValue::Number(number) => number_mirror(*number),
        JsValue::String(text) => json!({"type": "string", "value": text.lossy()}),
        JsValue::Symbol(_) => json!({"type": "symbol"}),
        JsValue::BigInt(_) | JsValue::BigIntWide(_) => json!({"type": "bigint"}),
        JsValue::Object(_) => {
            json!({"type": "object", "className": "Object", "description": "Object"})
        }
    }
}

/** A Number mirror; NaN and the infinities have no JSON form, so V8 sends them as unserializable. */
fn number_mirror(number: f64) -> Value {
    if number.is_finite() {
        json!({"type": "number", "value": number, "description": number_to_string(number)})
    } else {
        json!({"type": "number", "unserializableValue": number_to_string(number)})
    }
}

/** Debugger.enable turns the agent on; pauses are refused until then, as V8 refuses them. */
fn debugger_enable(vm: &mut Vm, _: &Value) -> Res<Reply> {
    set_flag(vm, ENABLED, true)?;
    Ok(Reply::Result(json!({"debuggerId": DEBUGGER_ID})))
}

/** Debugger.disable clears every pending or active pause along with the agent. */
fn debugger_disable(vm: &mut Vm, _: &Value) -> Res<Reply> {
    for name in [ENABLED, PAUSE_REQUESTED, PAUSED] {
        set_flag(vm, name, false)?;
    }
    Ok(Reply::Result(json!({})))
}

/** Debugger.pause asks for a pause at the next loop stack check; it does not pause here. */
fn debugger_pause(vm: &mut Vm, _: &Value) -> Res<Reply> {
    if !flag(vm, ENABLED)? {
        return Ok(Reply::Error(SERVER_ERROR, NOT_ENABLED.into()));
    }
    set_flag(vm, PAUSE_REQUESTED, true)?;
    Ok(Reply::Result(json!({})))
}

/** Debugger.resume ends an active pause, so the handler's loop returns to the paused code. */
fn debugger_resume(vm: &mut Vm, _: &Value) -> Res<Reply> {
    if !flag(vm, ENABLED)? {
        return Ok(Reply::Error(SERVER_ERROR, NOT_ENABLED.into()));
    }
    set_flag(vm, PAUSED, false)?;
    Ok(Reply::Result(json!({})))
}

/** The loop stack check: a requested pause runs the page's handler until it resumes, as V8's runMessageLoopOnPause does. */
pub(in crate::v9) fn pause_point(vm: &mut Vm) -> Res<()> {
    if !flag(vm, ENABLED)? || !flag(vm, PAUSE_REQUESTED)? {
        return Ok(());
    }
    set_flag(vm, PAUSE_REQUESTED, false)?;
    set_flag(vm, PAUSED, true)?;
    while flag(vm, PAUSED)? {
        let handler = vm.get(&JsValue::Object(GLOBAL), &Key::from(HANDLE_MESSAGE))?;
        if !vm.is_callable(&handler) {
            set_flag(vm, PAUSED, false)?;
            break;
        }
        match vm.call(handler, JsValue::Undefined, Vec::new()) {
            Ok(_) => {}
            // A handler exception ends the pause and reaches the paused code. V8 only reports it and
            // resumes; this host must not turn a failed assertion in the handler into a passing run.
            Err(thrown) => {
                set_flag(vm, PAUSED, false)?;
                return Err(thrown);
            }
        }
    }
    Ok(())
}

/** The agent record on the isolate global, created on first use and rooted there. */
fn record(vm: &mut Vm) -> Res<ObjectId> {
    let existing = vm
        .heap
        .get(GLOBAL)
        .and_then(|object| object.props.get(&Key::private(SESSION_KEY)))
        .map(|prop| prop.slot.clone());
    if let Some(Slot::Data(JsValue::Object(id))) = existing {
        return Ok(id);
    }
    let id = vm.alloc(Object::new(Class::Plain, None))?;
    vm.heap.put(
        GLOBAL,
        Key::private(SESSION_KEY),
        Prop::data(JsValue::Object(id)),
    );
    Ok(id)
}

/** Reads one agent flag; a missing flag is off. */
fn flag(vm: &mut Vm, name: &str) -> Res<bool> {
    let id = record(vm)?;
    let value = vm
        .heap
        .get(id)
        .and_then(|object| object.props.get(&Key::private(name)));
    Ok(matches!(
        value.map(|prop| &prop.slot),
        Some(Slot::Data(JsValue::Boolean(true)))
    ))
}

/** Writes one agent flag on the record. */
fn set_flag(vm: &mut Vm, name: &str, value: bool) -> Res<()> {
    let id = record(vm)?;
    vm.heap
        .put(id, Key::private(name), Prop::data(JsValue::Boolean(value)));
    Ok(())
}
