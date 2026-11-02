/**
 * Web MIDI's device side over CoreMIDI: the system's sources and destinations become MIDIInput
 * and MIDIOutput ports (ids from each endpoint's unique ID), output messages go out with
 * MIDISend, and input arrives on CoreMIDI's thread into a queue the platform poll drains into
 * `midimessage` events for the contexts that opened that input. Access follows the shared
 * permission rule (`access`): granted with transient activation, otherwise NotAllowedError.
 */
use super::{Ctx, Handler, Reply, fail, ok};
use crate::protocol::EngineResult;
use crate::runtime::supervisor::Runtime;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

/** MIDI operations. */
pub(super) const HANDLERS: &[(&str, Handler)] = &[
    ("midi.access", access),
    ("midi.open", open),
    ("midi.close", close),
    ("midi.send", send),
];

/** The contexts that opened each input port, and the port list subscribers last saw. */
#[derive(Default)]
pub(in crate::runtime) struct Midi {
    open: BTreeMap<String, BTreeSet<String>>,
    ports: Vec<Value>,
}

/** requestMIDIAccess: decide permission, then list the ports. */
fn access(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let sysex = args["sysex"].as_bool() == Some(true);
    let grants = &mut runtime.platform.access;
    if !grants.midi(&ctx.origin, sysex) {
        if !ctx.active {
            return fail("NotAllowedError", "Permission denied.");
        }
        grants.grant_midi(&ctx.origin, sysex);
    }
    let ports = native::ports();
    runtime.platform.midi.ports = ports.clone();
    ok(json!({ "ports": ports }))
}

/** Open an input (start receiving its messages) or an output. */
fn open(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if !runtime.platform.access.midi(&ctx.origin, false) {
        return fail("NotAllowedError", "Permission denied.");
    }
    let id = args["id"].as_str().unwrap_or_default();
    if let Some(unique) = id.strip_prefix("in-").and_then(|n| n.parse().ok()) {
        if !native::connect(unique) {
            return fail("InvalidAccessError", "The port could not be opened.");
        }
        let readers = runtime.platform.midi.open.entry(id.into()).or_default();
        readers.insert(ctx.identity.clone());
    }
    ok(Value::Null)
}

/** Close a port for this context; the input disconnects when nobody reads it. */
fn close(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    let id = args["id"].as_str().unwrap_or_default();
    let midi = &mut runtime.platform.midi;
    if let Some(readers) = midi.open.get_mut(id) {
        readers.remove(&ctx.identity);
        if readers.is_empty() {
            midi.open.remove(id);
            if let Some(unique) = id.strip_prefix("in-").and_then(|n| n.parse().ok()) {
                native::disconnect(unique);
            }
        }
    }
    ok(Value::Null)
}

/** MIDIOutput.send: the bytes go to the destination now. */
fn send(runtime: &mut Runtime, ctx: &Ctx, args: &Value, _: &[u8]) -> EngineResult<Reply> {
    if !runtime.platform.access.midi(&ctx.origin, false) {
        return fail("NotAllowedError", "Permission denied.");
    }
    let id = args["id"].as_str().unwrap_or_default();
    let Some(unique) = id.strip_prefix("out-").and_then(|n| n.parse().ok()) else {
        return fail("InvalidAccessError", "The port is not an output.");
    };
    let data: Vec<u8> = args["data"]
        .as_array()
        .map(|list| {
            list.iter()
                .filter_map(Value::as_u64)
                .filter_map(|b| u8::try_from(b).ok())
                .collect()
        })
        .unwrap_or_default();
    if native::send(unique, &data) {
        ok(Value::Null)
    } else {
        fail("InvalidStateError", "The port is disconnected.")
    }
}

impl Runtime {
    /** Deliver queued input and port changes to subscribers; true when anything was queued. */
    pub(in crate::runtime) fn poll_midi(&mut self) -> bool {
        let subscribers = self.platform.events.subscribers("midi");
        if subscribers.is_empty() {
            return false;
        }
        let mut changed = false;
        for (unique, data) in native::drain() {
            let port = format!("in-{unique}");
            let readers = self
                .platform
                .midi
                .open
                .get(&port)
                .cloned()
                .unwrap_or_default();
            for identity in readers {
                changed |= self.platform.events.push(
                    &identity,
                    json!({"topic":"midi","kind":"message","port":port,"data":data}),
                );
            }
        }
        let ports = native::ports();
        if ports != self.platform.midi.ports {
            self.platform.midi.ports = ports.clone();
            for identity in &subscribers {
                changed |= self.platform.events.push(
                    identity,
                    json!({"topic":"midi","kind":"ports","ports":ports}),
                );
            }
        }
        changed
    }
}

#[cfg(target_os = "macos")]
#[path = "midi_native.rs"]
mod native;

/** Platforms without CoreMIDI have no ports. */
#[cfg(not(target_os = "macos"))]
mod native {
    /** No ports. */
    pub fn ports() -> Vec<serde_json::Value> {
        Vec::new()
    }
    /** Nothing to connect. */
    pub fn connect(_: i32) -> bool {
        false
    }
    /** Nothing to disconnect. */
    pub fn disconnect(_: i32) {}
    /** Nothing to send to. */
    pub fn send(_: i32, _: &[u8]) -> bool {
        false
    }
    /** No input. */
    pub fn drain() -> Vec<(i32, Vec<u8>)> {
        Vec::new()
    }
}
