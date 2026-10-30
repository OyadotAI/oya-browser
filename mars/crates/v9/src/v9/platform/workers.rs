/** Dedicated Worker handles record candidate-owned tasks; only the coordinator creates isolates. */
mod constructor;
mod delivery;
use constructor::construct;
pub(in crate::v9) use delivery::{error, receive, report_exception};

use super::{Target, channel, events};
use crate::protocol::EngineError;
use crate::v9::value::{JsValue, ObjectId};
use crate::v9::vm::data::Class;
use crate::v9::vm::heap::{HIDDEN_ATTRS, Key, Object};
use crate::v9::vm::realm::Builder;
use crate::v9::vm::{Call, Res, Vm};
use crate::v9::{WorkerData, worker_clone};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/** Handles survive termination for repeat calls, with a bounded lifetime record table. */
pub(in crate::v9) const HANDLES: usize = 64;
/** One turn cannot enqueue unbounded cross-isolate tasks or messages. */
pub const WORKER_TASKS: usize = 32;
static PROTOTYPE: OnceLock<u32> = OnceLock::new();

/** The script grammar is fixed at construction and retained by the native worker owner. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkerType {
    /// Classic scripts retain synchronous importScripts support.
    #[default]
    Classic,
    /// Module graphs use asynchronous governed acquisition and module scope.
    Module,
}

/** Module requests retain their constructor credentials mode across the whole graph. */
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorkerCredentials {
    /// No request in the module graph receives cookies or other credentials.
    Omit,
    /// Only requests to the worker's own origin receive credentials.
    #[default]
    SameOrigin,
    /// Cross-origin responses must explicitly permit credentialed CORS.
    Include,
}

/** Commands contain inert data only and are discarded with an unpublished candidate. */
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WorkerCommand {
    /// The coordinator reserves capacity and governs source acquisition before execution.
    Create {
        id: u32,
        url: String,
        name: String,
        #[serde(default)]
        worker_type: WorkerType,
        #[serde(default)]
        credentials: WorkerCredentials,
    },
    /// Zero denotes a worker's owner; positive identities denote a page's exact handle.
    Post {
        id: u32,
        data: WorkerData,
        ports: Vec<u64>,
        /// Existing links cannot recreate a route after either endpoint closed it.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        moved_ports: Vec<u64>,
    },
    /// Zero closes the current worker; other values terminate a child handle.
    Terminate { id: u32 },
}
/** A handle remains branded after termination, but cannot enqueue new work. */
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::v9) struct Handle {
    pub object: ObjectId,
    pub active: bool,
}

/** The explicit implementation is installed before generic interface shape completion. */
pub(in crate::v9) fn install(b: &mut Builder) {
    let proto = b.object(Some(b.ids.event_target_prototype));
    b.host_method(proto, "postMessage", 1, post);
    b.host_method(proto, "terminate", 0, terminate);
    for name in ["onmessage", "onmessageerror", "onerror"] {
        b.value(proto, name, JsValue::Null, HIDDEN_ATTRS);
    }
    let ctor = b.constructor("Worker", 1, construct, proto, None);
    b.global("Worker", ctor);
    let _ = PROTOTYPE.set(proto);
}

/** Branding uses the owning context's table, never script-writable object properties. */
fn identity(vm: &mut Vm, this: &JsValue) -> Res<u32> {
    vm.page
        .state
        .workers
        .iter()
        .find(|(_, handle)| Some(handle.object) == this.object())
        .map(|(id, _)| *id)
        .ok_or_else(|| vm.type_error("Illegal invocation"))
}

/** Validate transfer types and duplicates before serializing or detaching any port. */
fn transfers(vm: &mut Vm, options: JsValue) -> Res<Vec<JsValue>> {
    if options.nullish() {
        return Ok(Vec::new());
    }
    let list = if options
        .object()
        .and_then(|id| vm.heap.get(id))
        .is_some_and(|o| matches!(o.class, Class::Array(_)))
    {
        options
    } else {
        vm.get(&options, &Key::from("transfer"))?
    };
    if list.nullish() {
        return Ok(Vec::new());
    }
    let ports = crate::v9::builtins::list_from_array_like(vm, &list)?;
    if ports.len() > WORKER_TASKS
        || ports
            .iter()
            .enumerate()
            .any(|(index, port)| !channel::transferable(vm, port) || ports[..index].contains(port))
    {
        return Err(vm.dom_exception("DataCloneError", "Unsupported or duplicate Worker transfer"));
    }
    Ok(ports)
}

/** A message is cloned now and delivered only in a later coordinator turn. */
fn post(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    if !vm.page.state.workers[&id].active {
        return Ok(JsValue::Undefined);
    }
    send(vm, &call, id)
}

/** Worker-global postMessage sends to its owner with the same clone and queue rules. */
pub(super) fn post_to_owner(vm: &mut Vm, call: Call) -> Res<JsValue> {
    if vm.page.state.worker_closed {
        return Ok(JsValue::Undefined);
    }
    send(vm, &call, 0)
}

/** Clone refusal leaves every transfer attached; native failures abort the candidate. */
fn send(vm: &mut Vm, call: &Call, id: u32) -> Res<JsValue> {
    capacity(vm)?;
    if call.args.is_empty() {
        return Err(vm.type_error("postMessage requires a value"));
    }
    let ports = transfers(vm, call.arg(1))?;
    let data = worker_clone::encode(vm, &call.arg(0))?;
    let queued =
        vm.page
            .effects
            .worker_commands
            .iter()
            .try_fold(data.bytes()?, |total, command| {
                Ok::<_, EngineError>(total.saturating_add(match command {
                    WorkerCommand::Post { data, .. } => data.bytes()?,
                    _ => 0,
                }))
            })?;
    if queued > crate::v9::WORKER_MESSAGE_BYTES {
        return Err(
            EngineError::limit("Worker queued bytes", crate::v9::WORKER_MESSAGE_BYTES).into(),
        );
    }
    let moved_ports = channel::moved_links(vm, &ports);
    let ports = channel::transfer_many(vm, &ports, channel::Route::Worker(id))?;
    vm.page.effects.worker_commands.push(WorkerCommand::Post {
        id,
        data,
        ports,
        moved_ports,
    });
    Ok(JsValue::Undefined)
}

/** A turn's messages consume a bounded aggregate queue, not one allowance per message. */
fn capacity(vm: &mut Vm) -> Res<()> {
    let bytes = vm
        .page
        .effects
        .worker_commands
        .iter()
        .try_fold(0usize, |sum, command| {
            Ok::<_, EngineError>(sum.saturating_add(match command {
                WorkerCommand::Post { data, .. } => data.bytes()?,
                _ => 0,
            }))
        })?;
    if vm.page.effects.worker_commands.len() >= WORKER_TASKS
        || bytes >= crate::v9::WORKER_MESSAGE_BYTES
    {
        return Err(EngineError::limit("Worker queued tasks", WORKER_TASKS).into());
    }
    Ok(())
}

/** Termination is idempotent and affects only this context's exact handle. */
fn terminate(vm: &mut Vm, call: Call) -> Res<JsValue> {
    let id = identity(vm, &call.this)?;
    if vm.page.state.workers[&id].active {
        capacity(vm)?;
        vm.page
            .state
            .workers
            .get_mut(&id)
            .expect("checked handle")
            .active = false;
        vm.page
            .effects
            .worker_commands
            .push(WorkerCommand::Terminate { id });
    }
    Ok(JsValue::Undefined)
}

/** Closing stops later tasks, while the current worker turn may finish its synchronous code. */
pub(in crate::v9) fn close(vm: &mut Vm, _: Call) -> Res<JsValue> {
    if vm.page.state.worker_closed {
        return Ok(JsValue::Undefined);
    }
    capacity(vm)?;
    vm.page.state.worker_closed = true;
    vm.page
        .effects
        .worker_commands
        .push(WorkerCommand::Terminate { id: 0 });
    Ok(JsValue::Undefined)
}
