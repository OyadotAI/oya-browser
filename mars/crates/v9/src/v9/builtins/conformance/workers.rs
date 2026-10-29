/** The opt-in d8 Worker constructor owns native isolated workers and performs real structured data messaging. */
mod clone;
use super::super::key;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{Key, Object, Prop};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use crate::v9::worker_runtime::ConformanceWorkerHost;
pub(in crate::v9) use clone::{Graph as WorkerGraph, decode as worker_decode};
pub(super) use clone::{deserialize, serialize};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

/** Worker prototype and worker-global entrypoints are immutable native identities. */
struct Natives {
    prototype: ObjectId,
    post: u32,
    close: u32,
    imports: u32,
    print: u32,
}
static NATIVES: OnceLock<Natives> = OnceLock::new();
/** A blocking receive shares the script's real deadline, with a bounded fallback before arming. */
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(10);
/** Private slots cannot be forged with property names, reflection or structured cloning. */
fn id_key() -> Key {
    Key::private("conformance-worker-id")
}
/** Endpoint ids grant authority only when this inaccessible nonce matches the current broker. */
fn session_key() -> Key {
    Key::private("conformance-worker-session")
}

/** Methods are ordinary non-constructible native methods on a genuine Worker prototype. */
pub(super) fn install(builder: &mut Builder) {
    let prototype = builder.object(Some(builder.ids.object_prototype));
    let constructor = builder.constructor("Worker", 1, construct, prototype, None);
    builder.global("Worker", constructor);
    builder.method(prototype, "postMessage", 1, post);
    builder.method(prototype, "getMessage", 0, get);
    builder.method(prototype, "terminate", 0, terminate);
    builder.method(prototype, "terminateAndWait", 0, terminate_wait);
    let _ = NATIVES.set(Natives {
        prototype,
        post: builder.native("postMessage", 1, post_parent),
        close: builder.native("close", 0, close),
        imports: builder.native("importScripts", 0, imports),
        print: builder.native("print", 0, print),
    });
}
/** Only a worker isolate receives worker-global functions; parent globals remain unchanged. */
pub(in crate::v9) fn activate(vm: &mut Vm) -> Res<()> {
    let natives = NATIVES.get().unwrap();
    let global = vm.global_object();
    for (name, native) in [
        ("postMessage", natives.post),
        ("close", natives.close),
        ("importScripts", natives.imports),
        ("print", natives.print),
    ] {
        let function = vm.native_closure(native, Vec::new())?;
        vm.put(global, name, JsValue::Object(function))?;
    }
    Ok(())
}
/** Environments must explicitly provide the session broker; no ambient process worker registry exists. */
fn host(vm: &mut Vm) -> Res<Arc<ConformanceWorkerHost>> {
    vm.page
        .environment
        .conformance_workers
        .clone()
        .ok_or_else(|| {
            vm.unsupported_error("Worker requires an explicit conformance worker session")
        })
}
/** Real function source and d8's JSON argument-source convention reproduce its function-worker invocation. */
fn function_source(
    vm: &mut Vm,
    function: JsValue,
    arguments: JsValue,
    callee: ObjectId,
) -> Res<String> {
    let source = crate::v9::builtins::function_to_string(
        vm,
        Call {
            this: function,
            args: &[],
            new_target: None,
            callee,
        },
    )?;
    let mut source = format!("({})(", vm.to_string(&source)?.lossy());
    if arguments != JsValue::Undefined {
        if !crate::v9::builtins::array::is_array_checked(vm, &arguments)? {
            return Err(vm.type_error("'arguments' must be an array"));
        }
        let length = vm.length_of(&arguments)?;
        for index in 0..length {
            vm.charge(1)?;
            if index > 0 {
                source.push(',');
            }
            let argument = vm.get(&arguments, &key(&index.to_string()))?;
            let encoded = crate::v9::builtins::json_stringify(
                vm,
                Call {
                    this: JsValue::Undefined,
                    args: &[argument],
                    new_target: None,
                    callee,
                },
            )?;
            source.push_str(&vm.to_string(&encoded)?.lossy());
            if source.len() > crate::constants::JS_STRING_UNITS {
                return Err(vm.range_error("Worker source is too large"));
            }
        }
    }
    source.push(')');
    Ok(source)
}
/** The default source is an inert supplied filename; string and function modes need no filesystem authority. */
fn construct(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let Some(target) = call.new_target.clone() else {
        return Err(vm.type_error("Worker must be constructed with new"));
    };
    if !matches!(call.arg(0), JsValue::String(_)) && !vm.is_callable(&call.arg(0)) {
        return Err(vm.type_error("1st argument must be a string or a function"));
    }
    let options = call.arg(1);
    let object = options.object().is_some();
    let kind = if object {
        vm.get(&options, &key("type"))?
    } else {
        JsValue::Undefined
    };
    let arguments = if object {
        vm.get(&options, &key("arguments"))?
    } else {
        JsValue::Undefined
    };
    let kind = match kind {
        JsValue::String(kind) => kind.lossy(),
        JsValue::Undefined if !object => "classic".into(),
        _ => return Err(vm.type_error("Invalid Worker source type")),
    };
    let source = match (kind.as_str(), call.arg(0)) {
        ("function", function) if vm.is_callable(&function) => {
            function_source(vm, function, arguments, call.callee)?
        }
        ("string", JsValue::String(source)) => source.lossy(),
        ("classic", JsValue::String(path)) => vm
            .page
            .environment
            .conformance_sources
            .get(&path.lossy())
            .cloned()
            .ok_or_else(|| vm.type_error("Worker source file was not supplied by the host"))?,
        _ => return Err(vm.type_error("Invalid Worker source or source type")),
    };
    if object && vm.get(&options, &key("flushDenormals"))?.truthy() {
        return Err(vm.unsupported_error("Worker flushDenormals mode"));
    }
    let host = host(vm)?;
    let environment = vm.page.environment.clone();
    let prototype = vm.prototype_for(&target, vm.intrinsic_id(NATIVES.get().unwrap().prototype))?;
    let instance = vm.alloc(Object::new(Class::Plain, Some(prototype)))?;
    vm.pin(JsValue::Object(instance));
    let deadline = vm
        .deadline
        .unwrap_or_else(|| Instant::now() + crate::constants::script_deadline());
    let id = host.spawn(environment, source, None, deadline)?;
    vm.heap
        .put(instance, id_key(), Prop::data(JsValue::BigInt(id as i128)));
    vm.heap.put(
        instance,
        session_key(),
        Prop::data(JsValue::BigInt(host.nonce() as i128)),
    );
    Ok(JsValue::Object(instance))
}
/** Every method reads its actual receiver's private brand and current session identity. */
fn receiver(vm: &mut Vm, call: &Call) -> Res<(Arc<ConformanceWorkerHost>, u64)> {
    let host = host(vm)?;
    let slots = call.this.object().and_then(|id| vm.heap.get(id));
    let id = slots
        .and_then(|object| object.props.get(&id_key()))
        .and_then(|prop| match prop.slot {
            crate::v9::vm::heap::Slot::Data(JsValue::BigInt(id)) => Some(id as u64),
            _ => None,
        });
    let nonce = slots
        .and_then(|object| object.props.get(&session_key()))
        .and_then(|prop| match prop.slot {
            crate::v9::vm::heap::Slot::Data(JsValue::BigInt(id)) => Some(id as u64),
            _ => None,
        });
    if nonce != Some(host.nonce()) {
        return Err(vm.type_error("Worker method called on an incompatible receiver"));
    }
    Ok((
        host,
        id.ok_or_else(|| vm.type_error("Worker method called on an incompatible receiver"))?,
    ))
}
/** Queue the actual cloned graph; transfer detaches buffers only after complete serialization succeeds. */
fn post(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (host, id) = receiver(vm, &call)?;
    let transfers = transfers(vm, &call.arg(1))?;
    let graph = clone::encode(vm, &call.arg(0), &transfers)?;
    transfer(vm, transfers)?;
    host.post(id, graph)?;
    Ok(JsValue::Undefined)
}
/** Blocking d8 receive returns the next FIFO packet or undefined after the worker has exited. */
fn get(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (host, id) = receiver(vm, &call)?;
    let deadline = vm
        .deadline
        .unwrap_or_else(|| Instant::now() + RECEIVE_TIMEOUT);
    let requester = vm.page.environment.conformance_worker_id;
    let mark = vm.pins.len();
    vm.pin(call.this.clone());
    let result = host.receive(id, requester, deadline, || {
        host.collection_safepoint(requester, vm)
    });
    vm.pins.truncate(mark);
    match result? {
        Some(graph) => clone::decode(vm, graph),
        None => Ok(JsValue::Undefined),
    }
}
/** A termination request is uncatchable inside the worker once an interpreter checkpoint observes it. */
fn terminate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (host, id) = receiver(vm, &call)?;
    host.terminate(id, false)?;
    Ok(JsValue::Undefined)
}
/** Join proves the worker thread stopped before this method returns. */
fn terminate_wait(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let (host, id) = receiver(vm, &call)?;
    host.terminate(id, true)?;
    Ok(JsValue::Undefined)
}
/** Worker messages to the creator use the same structured graph codec as parent-to-worker messages. */
fn post_parent(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let host = host(vm)?;
    let id = vm
        .page
        .environment
        .conformance_worker_id
        .ok_or_else(|| vm.type_error("postMessage requires a worker isolate"))?;
    let transfers = transfers(vm, &call.arg(1))?;
    let graph = clone::encode(vm, &call.arg(0), &transfers)?;
    transfer(vm, transfers)?;
    host.post_parent(id, graph)?;
    Ok(JsValue::Undefined)
}
/** Close ends the actual isolate and discards subsequent source statements and tasks. */
fn close(_vm: &mut Vm, _call: Call) -> Res<JsValue> {
    Err(
        crate::protocol::EngineError::new("conformance_exit", "0", "Worker requested close.")
            .into(),
    )
}
/** imports execute supplied fixture code as global Scripts, retaining their lexical declarations. */
fn imports(vm: &mut Vm, call: Call) -> Res<JsValue> {
    for path in call.args {
        super::execute(
            vm,
            Call {
                this: JsValue::Undefined,
                args: std::slice::from_ref(path),
                new_target: None,
                callee: call.callee,
            },
        )?;
    }
    Ok(JsValue::Undefined)
}
/** Print goes through the worker's real console rather than crossing executable callbacks. */
fn print(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let console = vm.get(&JsValue::Object(vm.global_object()), &key("console"))?;
    let log = vm.get(&console, &key("log"))?;
    vm.call(log, console, call.args.to_vec())?;
    Ok(JsValue::Undefined)
}
/** Validate every transfer entry before detaching any, including duplicates and non-buffer values. */
fn transfers(vm: &mut Vm, list: &JsValue) -> Res<Vec<ObjectId>> {
    if *list == JsValue::Undefined {
        return Ok(Vec::new());
    }
    if !crate::v9::builtins::array::is_array_checked(vm, list)? {
        return Err(vm.type_error("Worker transfer list must be an array"));
    }
    let length = vm.length_of(list)?;
    let mut buffers = Vec::new();
    for index in 0..length {
        let value = vm.get(list, &key(&index.to_string()))?;
        let id = value
            .object()
            .ok_or_else(|| vm.dom_exception("DataCloneError", "Invalid transferable"))?;
        if !matches!(
            vm.heap.get(id).map(|object| &object.class),
            Some(Class::ArrayBuffer(_) | Class::SparseWasmBuffer(_))
        ) || !crate::v9::builtins::buffer_detachable(vm, id)
            || buffers.contains(&id)
        {
            return Err(vm.dom_exception("DataCloneError", "Invalid or duplicate transferable"));
        }
        buffers.push(id);
    }
    Ok(buffers)
}

/** Registered transfers can detach an already empty backing, as d8's serializer permits. */
fn transfer(vm: &mut Vm, buffers: Vec<ObjectId>) -> Res<()> {
    for id in buffers {
        crate::v9::builtins::detach_buffer(vm, id);
    }
    Ok(())
}
