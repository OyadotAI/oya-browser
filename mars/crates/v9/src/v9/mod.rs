/**
 * V9, Mars's own JavaScript engine: a bytecode VM with a bounded, serializable heap, the
 * ECMAScript library, and a web platform over the app's inert document projection. Every entry
 * point takes explicit data and returns proposals; nothing here has native authority.
 */
mod binding;
mod builtins;
mod compile;
mod errors;
mod keyboard;
pub use keyboard::{KeyboardKey, keyboard_page};
mod legacy_handlers;
mod lexer;
mod literals;
mod modules;
mod operator;
mod platform;
mod regexp;
mod source;
mod syntax;
mod value;
mod vm;
mod web_worker;
mod worker_clone;
pub use platform::workers::{
    WORKER_DEPTH, WORKER_TASKS, WorkerCommand, WorkerCredentials, WorkerType,
};
pub use web_worker::ImportScriptsHost;
pub use web_worker::{load_worker, resume_worker_modules};
pub use worker_clone::{WORKER_MESSAGE_BYTES, WorkerData};
#[cfg(feature = "conformance")]
mod worker_runtime;
#[cfg(feature = "conformance")]
pub use worker_runtime::{ConformanceWorkerHost, ConformanceWorkerSession};

pub use binding::{Binding, BindingKind, Globals};
pub use modules::{EntryKind, ModuleEntry, ModuleInput, ModuleRequest, ModuleSource};
#[cfg(feature = "conformance")]
pub use platform::CpuTraceMark;
pub use platform::NodeBox;
pub use platform::blob_urls::BlobUrlSource;
pub use platform::broadcast::{
    BROADCAST_CHANNELS, BROADCAST_COMMANDS, BROADCAST_EFFECT_BYTES, BROADCAST_NAME_UNITS,
    BroadcastCommand, BroadcastEndpoint,
};
pub use platform::channel::PortMessage;
pub use platform::devtools;
pub use platform::frame_directory::{
    FrameContext, FrameDetach, FrameRoute, MAX_FRAME_WINDOW_ID, sibling_target, sibling_window,
};
pub use platform::indexed_db::{
    INDEXED_DB_DATABASE_BYTES, INDEXED_DB_EFFECT_BYTES, INDEXED_DB_KIND, INDEXED_DB_LIST,
    INDEXED_DB_OPEN, INDEXED_DB_ORIGIN_BYTES, INDEXED_DB_ORIGIN_RECORD_BYTES,
    INDEXED_DB_RESPONSE_BYTES, IndexedDbCommit, IndexedDbDatabase, IndexedDbOperation,
    IndexedDbOrigin, IndexedDbStore,
};
pub use platform::messaging::QueuedPortMessage;
pub use platform::messaging::{FrameMessage, MessageData, PARENT as MESSAGE_PARENT};
pub use platform::resize_observers::{RESIZE_BROADCAST_LIMIT, ResizeTurn};
pub use platform::{
    AnimationTarget, ConsoleEntry, DomLevel, FrameDocument, FrameRequest, HistoryRecord,
    HostEffects, HostRequest, HostResponse, ImageAdoption, ImageCancellation, ImageCors,
    ImageRequest, ImageResponse, MediaCommand, MediaSettings, MediaSupport, NavigationRequest,
    OpacityEffect, PageEnvironment, SimpleFragmentParser,
};
pub use platform::{HANDLER_POLICY_LIMIT, HandlerHash, HandlerPolicy, HandlerRestriction};
pub use platform::{LAYOUT_CALLS, LayoutAnswer, LayoutHook, LayoutHost};
pub use value::{BigInteger, CodePointCursor, JsString, JsValue};

use crate::constants::{
    NEAR_BOUNDS_FRACTION, SCRIPT_BYTES, SCRIPT_COUNT, SCRIPT_TOTAL_BYTES, STORAGE_AREA_BYTES,
    STORAGE_ENTRIES, STORAGE_VALUE_BYTES, TEXT_BYTES, script_deadline,
};
use crate::document::{
    DomChange, FragmentParser, ScriptChange, ScriptDocument, ScriptNode, TextChange,
};
use crate::protocol::{EngineError, EngineResult, SourceLocation};
use compile::{Code, Registry};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use vm::data::{Class, HostData, Job};
use vm::heap::Heap;
use vm::{Limits, Symbols, Thrown, Vm};

/** Dynamic code (new Function, indirect eval) gets offsets past every page source. */
pub(in crate::v9) const DYNAMIC_OFFSET: usize = 256 * 1024 * 1024;

/// A page's committed engine state: heap, lexical globals and platform state. The app keeps it
/// between requests and passes it back; it is validated on the way in.
#[derive(Clone, Debug, Default)]
#[cfg_attr(not(feature = "conformance"), derive(Deserialize, Serialize))]
#[cfg_attr(not(feature = "conformance"), serde(deny_unknown_fields))]
pub struct VmState {
    /** Conformance contexts retain distinct globals and intrinsic ownership across requests. */
    #[cfg(feature = "conformance")]
    #[cfg_attr(not(feature = "conformance"), serde(default))]
    realms: vm::realms::State,
    #[cfg_attr(not(feature = "conformance"), serde(default))]
    globals: Globals,
    #[cfg_attr(not(feature = "conformance"), serde(default))]
    heap: Heap,
    #[cfg_attr(not(feature = "conformance"), serde(default))]
    symbols: Symbols,
    #[cfg_attr(
        not(feature = "conformance"),
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    jobs: Vec<Job>,
    #[cfg_attr(not(feature = "conformance"), serde(default))]
    page: platform::PageState,
    #[cfg_attr(
        not(feature = "conformance"),
        serde(default, skip_serializing_if = "Vec::is_empty")
    )]
    dynamic: Vec<vm::DynamicSource>,
    #[cfg_attr(
        not(feature = "conformance"),
        serde(default, skip_serializing_if = "vm::templates::Cache::is_empty")
    )]
    templates: vm::templates::Cache,
    #[cfg_attr(
        not(feature = "conformance"),
        serde(default, skip_serializing_if = "modules::ModuleState::is_empty")
    )]
    modules: modules::ModuleState,
    /** Set only on a state this process built and validated itself (`finish`), never on one
    decoded from a frame, so a state kept resident in the service skips re-validating its
    heap on the next step (about 170 ms on YouTube) while every state from outside is checked. */
    #[cfg_attr(not(feature = "conformance"), serde(skip))]
    verified: bool,
    /** The page's compiled code, carried to the next step so it is not filed again; never
    serialized: a decoded state compiles its sources afresh. */
    #[cfg_attr(not(feature = "conformance"), serde(skip))]
    registry: compile::Registry,
}

#[cfg(feature = "conformance")]
fn serialize_state_fields<S: serde::ser::SerializeStruct>(
    state: &mut S,
    value: &VmState,
) -> Result<(), S::Error> {
    state.serialize_field("realms", &value.realms)?;
    state.serialize_field("globals", &value.globals)?;
    state.serialize_field("heap", &value.heap)?;
    state.serialize_field("symbols", &value.symbols)?;
    if !value.jobs.is_empty() {
        state.serialize_field("jobs", &value.jobs)?;
    }
    state.serialize_field("page", &value.page)?;
    if !value.dynamic.is_empty() {
        state.serialize_field("dynamic", &value.dynamic)?;
    }
    if !value.templates.is_empty() {
        state.serialize_field("templates", &value.templates)?;
    }
    if !value.modules.is_empty() {
        state.serialize_field("modules", &value.modules)?;
    }
    Ok(())
}
#[cfg(feature = "conformance")]
struct StateFields<'a>(&'a VmState);
#[cfg(feature = "conformance")]
impl Serialize for StateFields<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut state = serializer.serialize_struct("VmState", 9)?;
        serialize_state_fields(&mut state, self.0)?;
        state.end()
    }
}
#[cfg(feature = "conformance")]
impl Serialize for VmState {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        crate::v9::value::with_string_identity_encoding(|| {
            crate::v9::value::with_slice_parents(Default::default(), || {
                serde_json::to_writer(std::io::sink(), &StateFields(self))
                    .map_err(serde::ser::Error::custom)?;
                let mut state = serializer.serialize_struct("VmState", 10)?;
                struct Parents;
                impl Serialize for Parents {
                    fn serialize<S: serde::Serializer>(
                        &self,
                        serializer: S,
                    ) -> Result<S::Ok, S::Error> {
                        crate::v9::value::serialize_slice_parents(serializer)
                    }
                }
                state.serialize_field("$slice_parents", &Parents)?;
                serialize_state_fields(&mut state, self)?;
                state.end()
            })
        })
    }
}

/** The wire helper is decoded under one isolated string identity namespace. */
#[cfg(feature = "conformance")]
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct VmStateWire {
    #[serde(default)]
    realms: vm::realms::State,
    #[serde(default)]
    globals: Globals,
    #[serde(default)]
    heap: Heap,
    #[serde(default)]
    symbols: Symbols,
    #[serde(default)]
    jobs: Vec<Job>,
    #[serde(default)]
    page: platform::PageState,
    #[serde(default)]
    dynamic: Vec<vm::DynamicSource>,
    #[serde(default)]
    templates: vm::templates::Cache,
    #[serde(default)]
    modules: modules::ModuleState,
}
/** Conservatively cover map nodes, array growth, and the temporary input strings. */
#[cfg(feature = "conformance")]
const BUFFERED_STATE_ENTRY_BYTES: usize = 128;
/** Vec growth may temporarily reserve twice as many value slots as it holds. */
#[cfg(feature = "conformance")]
const BUFFERED_STATE_ARRAY_ENTRY_BYTES: usize = 2 * std::mem::size_of::<serde_json::Value>();
#[cfg(feature = "conformance")]
thread_local! { static BUFFERED_STATE_BYTES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
#[cfg(feature = "conformance")]
fn charge_buffered_state<E: serde::de::Error>(bytes: usize) -> Result<(), E> {
    BUFFERED_STATE_BYTES.with(|used| {
        let next = used.get().saturating_add(bytes);
        if next > crate::constants::VM_STATE_BYTES {
            return Err(E::custom("V9 snapshot decoding buffer limit"));
        }
        used.set(next);
        Ok(())
    })
}
#[cfg(feature = "conformance")]
fn with_buffered_state_limit<T>(f: impl FnOnce() -> T) -> T {
    BUFFERED_STATE_BYTES.with(|used| {
        let previous = used.replace(0);
        struct Restore<'a>(&'a std::cell::Cell<usize>, usize);
        impl Drop for Restore<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let _restore = Restore(used, previous);
        f()
    })
}
#[cfg(all(test, feature = "conformance"))]
mod buffered_state_tests {
    #[test]
    fn buffer_charges_are_bounded_and_scoped_to_each_decode() {
        use super::*;
        with_buffered_state_limit(|| {
            charge_buffered_state::<serde_json::Error>(crate::constants::VM_STATE_BYTES).unwrap();
            assert!(charge_buffered_state::<serde_json::Error>(1).is_err());
            with_buffered_state_limit(|| charge_buffered_state::<serde_json::Error>(1).unwrap());
            assert!(charge_buffered_state::<serde_json::Error>(1).is_err());
        });
        with_buffered_state_limit(|| charge_buffered_state::<serde_json::Error>(1).unwrap());
    }
}
/** Buffer arbitrary JSON while refusing duplicate keys at every depth. */
#[cfg(feature = "conformance")]
struct StrictJson(serde_json::Value);
#[cfg(feature = "conformance")]
impl<'de> Deserialize<'de> for StrictJson {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Value;
        impl<'de> serde::de::Visitor<'de> for Value {
            type Value = StrictJson;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a JSON value")
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                Ok(StrictJson(v.into()))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                Ok(StrictJson(v.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                Ok(StrictJson(v.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(v)
                    .map(|n| StrictJson(n.into()))
                    .ok_or_else(|| E::custom("nonfinite JSON number"))
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                charge_buffered_state::<E>(v.len().saturating_mul(2))?;
                Ok(StrictJson(v.into()))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                charge_buffered_state::<E>(v.capacity().saturating_mul(2))?;
                Ok(StrictJson(v.into()))
            }
            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(StrictJson(serde_json::Value::Null))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                self.visit_none()
            }
            fn visit_some<D: serde::Deserializer<'de>>(
                self,
                d: D,
            ) -> Result<Self::Value, D::Error> {
                StrictJson::deserialize(d)
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = seq.next_element::<StrictJson>()? {
                    charge_buffered_state::<A::Error>(BUFFERED_STATE_ARRAY_ENTRY_BYTES)?;
                    values.push(value.0);
                }
                Ok(StrictJson(values.into()))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error;
                let mut fields = serde_json::Map::new();
                while let Some((key, value)) = map.next_entry::<String, StrictJson>()? {
                    charge_buffered_state::<A::Error>(
                        BUFFERED_STATE_ENTRY_BYTES.saturating_add(key.capacity().saturating_mul(2)),
                    )?;
                    if fields.insert(key, value.0).is_some() {
                        return Err(A::Error::custom("duplicate JSON field"));
                    }
                }
                Ok(StrictJson(fields.into()))
            }
        }
        deserializer.deserialize_any(Value)
    }
}
/** Decode directly after the parent table; buffer only fields reordered ahead of it. */
#[cfg(feature = "conformance")]
struct StateVisitor;
/** Validate and bound parent-table entries as they are read, before allocating the next entry. */
#[cfg(feature = "conformance")]
struct SliceParentTable(std::collections::BTreeMap<u64, value::JsString>);
#[cfg(feature = "conformance")]
impl<'de> Deserialize<'de> for SliceParentTable {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Entries;
        impl<'de> serde::de::Visitor<'de> for Entries {
            type Value = SliceParentTable;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a slice parent table")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error;
                let mut parents = std::collections::BTreeMap::new();
                while let Some((id, parent)) = seq.next_element::<(u64, value::JsString)>()? {
                    charge_buffered_state::<A::Error>(BUFFERED_STATE_ENTRY_BYTES)?;
                    if id == 0 || parents.insert(id, parent).is_some() {
                        return Err(A::Error::custom("invalid slice parent identity"));
                    }
                }
                Ok(SliceParentTable(parents))
            }
        }
        deserializer.deserialize_seq(Entries)
    }
}
#[cfg(feature = "conformance")]
impl<'de> serde::de::Visitor<'de> for StateVisitor {
    type Value = VmState;
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("a state object")
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        use serde::de::Error;
        let mut wire = VmStateWire::default();
        let mut deferred = serde_json::Map::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut ready = false;
        while let Some(key) = map.next_key::<String>()? {
            if !seen.insert(key.clone()) {
                return Err(A::Error::custom("duplicate state field"));
            }
            if key == "$slice_parents" || key == "slice_parents" {
                if ready {
                    return Err(A::Error::custom("duplicate slice parent table"));
                }
                let parents = map.next_value::<SliceParentTable>()?;
                crate::v9::value::set_slice_parents(parents.0);
                wire = serde_json::from_value(serde_json::Value::Object(std::mem::take(
                    &mut deferred,
                )))
                .map_err(A::Error::custom)?;
                ready = true;
                continue;
            }
            if !ready {
                let value = map.next_value::<StrictJson>()?;
                charge_buffered_state::<A::Error>(
                    BUFFERED_STATE_ENTRY_BYTES.saturating_add(key.capacity().saturating_mul(2)),
                )?;
                deferred.insert(key, value.0);
                continue;
            }
            match key.as_str() {
                "realms" => wire.realms = map.next_value()?,
                "globals" => wire.globals = map.next_value()?,
                "heap" => wire.heap = map.next_value()?,
                "symbols" => wire.symbols = map.next_value()?,
                "jobs" => wire.jobs = map.next_value()?,
                "page" => wire.page = map.next_value()?,
                "dynamic" => wire.dynamic = map.next_value()?,
                "templates" => wire.templates = map.next_value()?,
                "modules" => wire.modules = map.next_value()?,
                _ => return Err(A::Error::custom("unknown state field")),
            }
        }
        if !ready {
            wire = serde_json::from_value(serde_json::Value::Object(deferred))
                .map_err(A::Error::custom)?;
        }
        Ok(VmState {
            realms: wire.realms,
            globals: wire.globals,
            heap: wire.heap,
            symbols: wire.symbols,
            jobs: wire.jobs,
            page: wire.page,
            dynamic: wire.dynamic,
            templates: wire.templates,
            modules: wire.modules,
            verified: false,
            registry: compile::Registry::default(),
        })
    }
}
#[cfg(feature = "conformance")]
impl<'de> Deserialize<'de> for VmState {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        with_buffered_state_limit(|| {
            crate::v9::value::with_string_identities(|| {
                crate::v9::value::with_slice_parents(Default::default(), || {
                    deserializer.deserialize_map(StateVisitor)
                })
            })
        })
    }
}

/** Count encoded state bytes without allocating a second complete snapshot. */
struct StateSizeWriter {
    bytes: usize,
    limit: usize,
    exceeded: bool,
}
impl std::io::Write for StateSizeWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let next = self.bytes.saturating_add(bytes.len());
        if next > self.limit {
            self.exceeded = true;
            return Err(std::io::Error::other("V9 state byte limit"));
        }
        self.bytes = next;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[cfg(test)]
mod state_size_writer_tests {
    #[test]
    fn stops_at_the_exact_encoded_byte_limit() {
        use std::io::Write;
        let mut writer = super::StateSizeWriter {
            bytes: 0,
            limit: 3,
            exceeded: false,
        };
        writer.write_all(b"abc").unwrap();
        assert!(writer.write_all(b"d").is_err());
        assert!(writer.exceeded);
        assert_eq!(writer.bytes, 3);
    }
}

impl VmState {
    /// Committed top-level let, const and class bindings, for inspection.
    pub fn globals(&self) -> &Globals {
        &self.globals
    }
    /// A state holding only lexical globals (used by tests and legacy callers).
    pub fn from_globals(globals: Globals) -> Self {
        Self {
            globals,
            ..Self::default()
        }
    }
    /// The document identity this state's node references belong to.
    pub fn identity(&self) -> &str {
        &self.page.identity
    }
    /// The page-platform part alone, without the heap, globals or compiled templates: what the
    /// app reads between steps (listeners, detached nodes, images) when the full state stays
    /// resident in the V9 service. Sending a summary back as a step's state would lose the
    /// page's heap, so the app never does.
    pub fn summary(&self) -> Self {
        let page = {
            #[cfg(feature = "conformance")]
            {
                let mut page = self.page.clone();
                page.code_constants.clear();
                page.number_constants.clear();
                page.interned_strings.clear();
                page
            }
            #[cfg(not(feature = "conformance"))]
            self.page.clone()
        };
        Self {
            page,
            ..Self::default()
        }
    }
    /// Whether a registered click listener sits on this node (directly).
    pub fn has_click_listener(&self, identity: &str, local_id: usize) -> bool {
        self.page.identity == identity
            && self.page.listeners.iter().any(|listener| {
                listener.kind == "click" && listener.target == platform::Target::Node(local_id)
            })
    }
    /// Whether a click on this node reaches any listener: the node, an ancestor, the document
    /// or the window (`document` is the current projection).
    pub fn click_reaches_listener(&self, document: &ScriptDocument, local_id: usize) -> bool {
        if self.page.identity != document.identity {
            return false;
        }
        let mut path = vec![local_id];
        let mut cursor = Some(local_id);
        while let Some(id) = cursor {
            cursor = document
                .nodes
                .iter()
                .find(|node| node.local_id == id)
                .and_then(|node| node.parent);
            if let Some(parent) = cursor {
                path.push(parent);
            }
            if path.len() > crate::constants::DOCUMENT_DEPTH * 2 {
                break;
            }
        }
        self.page.listeners.iter().any(|listener| {
            listener.kind == "click"
                && match listener.target {
                    platform::Target::Node(node) => {
                        path.contains(&node) || node >= platform::BODY - 2
                    }
                    platform::Target::Document | platform::Target::Window => true,
                    platform::Target::Object(_) => false,
                }
        })
    }
    /// Check that retained node references fit the document the app just committed.
    pub fn validate_document(&self, document: &ScriptDocument) -> EngineResult<()> {
        self.validate(&Limits::default())?;
        self.validate_document_references(document)
    }
    /// Check a resident service's heap-less projection without treating its object IDs as a full heap.
    /// Only coordinators holding a resident state token may use this; restoring execution still uses
    /// full validation, including native signal identities and observer callbacks.
    pub fn validate_summary_document(&self, document: &ScriptDocument) -> EngineResult<()> {
        let limits = Limits::default();
        self.validate_projection_bounds(&limits, false)?;
        self.validate_size(&limits)?;
        self.validate_document_references(document)
    }
    /// Document identity and retained node ownership apply equally to full states and summaries.
    fn validate_document_references(&self, document: &ScriptDocument) -> EngineResult<()> {
        document.validate()?;
        if self.page.initialized && self.page.identity != document.identity {
            return Err(EngineError::invalid(
                "V9 state belongs to another document.",
            ));
        }
        let projected: std::collections::BTreeSet<usize> =
            document.nodes.iter().map(|node| node.local_id).collect();
        let known = |node: &usize| {
            projected.contains(node)
                || self.page.detached.contains_key(node)
                || *node >= platform::nodes_virtual_base()
        };
        // A listener on a node is registered through a wrapper of a node the page had; one on a
        // node the document never held is forged.
        if self.page.listeners.iter().any(
            |listener| matches!(listener.target, platform::Target::Node(node) if !known(&node)),
        ) {
            return Err(EngineError::invalid("Invalid V9 listener target."));
        }
        if self.page.images.keys().any(|node| !known(node)) {
            return Err(EngineError::invalid("Invalid V9 image target."));
        }
        for node in self.page.wrappers.keys() {
            let known = projected.contains(node)
                || self.page.detached.contains_key(node)
                || *node >= platform::nodes_virtual_base();
            if !known {
                return Err(EngineError::unsupported(
                    "Retaining references to nodes the document no longer has is not supported.",
                ));
            }
        }
        Ok(())
    }
    /// Structural and size validation of untrusted saved state.
    fn validate(&self, limits: &Limits) -> EngineResult<()> {
        self.validate_bounds(limits)?;
        self.validate_size(limits)
    }
    /// Even heap-less projections retain the private wire's encoded state ceiling.
    fn validate_size(&self, limits: &Limits) -> EngineResult<()> {
        let mut writer = StateSizeWriter {
            bytes: 0,
            limit: limits.state_bytes,
            exceeded: false,
        };
        if serde_json::to_writer(&mut writer, self).is_err() {
            return Err(if writer.exceeded {
                EngineError::limit("V9 state bytes", limits.state_bytes)
            } else {
                EngineError::invalid("Cannot encode V9 state.")
            });
        }
        Ok(())
    }
    /// Every check but the encoded size, for a state this process just built: its object,
    /// property, string and binding counts bound its memory, and measuring its size means
    /// encoding it whole (13% of V9's time on YouTube). A state that later crosses a pipe is
    /// measured by the frame bound, and one decoded from a frame gets the full `validate`.
    /// The heap-wide branding walks (abort signals, response bodies, observers) re-check what V9's
    /// own code just established, at O(heap) per step; release builds keep the page summaries
    /// and every count bound, and debug builds (the tests) still walk the whole heap.
    fn validate_bounds(&self, limits: &Limits) -> EngineResult<()> {
        self.validate_projection_bounds(limits, cfg!(debug_assertions))
    }
    /// Page bounds are always checked; heap-linked identities require the complete resident state.
    fn validate_projection_bounds(&self, limits: &Limits, full_heap: bool) -> EngineResult<()> {
        platform::clock::validate(self.page.clock, self.page.observed_clock, self.page.epoch)?;
        if self.page.legacy_handlers.len() > crate::constants::VM_DYNAMIC_SOURCES {
            return Err(EngineError::invalid("Invalid retained legacy handler."));
        }
        if self.page.workers.len() > platform::workers::HANDLES
            || self
                .page
                .workers
                .keys()
                .enumerate()
                .any(|(index, id)| *id as usize != index + 1)
        {
            return Err(EngineError::invalid("Invalid Worker handles."));
        }
        if self.page.worker_imports.len() > platform::worker_global::imports::IMPORTS
            || self.page.worker_depth > WORKER_DEPTH
            || self.page.worker_name.is_none() && self.page.worker_depth != 0
            || self
                .page
                .worker_imports
                .values()
                .any(|url| url.len() > crate::constants::URL_BYTES)
            || self
                .page
                .worker_name
                .as_ref()
                .is_some_and(|name| name.len() > crate::constants::URL_BYTES)
        {
            return Err(EngineError::invalid("Invalid worker metadata."));
        }
        #[cfg(feature = "conformance")]
        if self.page.interned_strings.len() > limits.units {
            return Err(EngineError::limit("V9 weak intern entries", limits.units));
        }
        #[cfg(feature = "conformance")]
        if self.page.number_constants.len() > limits.objects
            || self
                .page
                .number_constants
                .values()
                .fold(0usize, |count, pool| {
                    count.saturating_add(pool.values.len())
                })
                > limits.objects
        {
            return Err(EngineError::limit(
                "V9 numeric constant cells",
                limits.objects,
            ));
        }
        #[cfg(feature = "conformance")]
        if self.page.code_constants.len() > limits.units {
            return Err(EngineError::limit("V9 code constant cells", limits.units));
        }
        #[cfg(feature = "conformance")]
        builtins::validate_profile_state(&self.page.cpu_profile, limits)?;
        platform::images::validate(&self.page.images)?;
        if full_heap {
            platform::intersection_observers::validate(&self.page, &self.heap)?;
            platform::resize_observers::validate(&self.page, &self.heap)?;
            platform::validate_network_state(&self.page, &self.heap)?;
        } else {
            platform::intersection_observers::validate_summary(&self.page)?;
            platform::resize_observers::validate_summary(&self.page)?;
            platform::validate_network_summary(&self.page)?;
        }
        platform::blob_urls::validate(&self.page.blob_urls)?;
        platform::canvas::validate(&self.page.canvases)?;
        platform::canvas::path2d::validate(&self.page.path2d)?;
        platform::frame_directory::validate_state(&self.page)?;
        platform::channel::validate_bounds(&self.page)?;
        builtins::validate_shared(&self.heap)?;
        builtins::validate_shared_gc(&self.heap)?;
        #[cfg(feature = "conformance")]
        self.realms.validate(&self.heap, &self.dynamic)?;
        #[cfg(feature = "conformance")]
        if self.modules.records.len()
            + self
                .realms
                .contexts
                .iter()
                .map(|context| context.modules.records.len())
                .sum::<usize>()
            > crate::constants::MODULES
            || self.modules.waiting.len()
                + self
                    .realms
                    .contexts
                    .iter()
                    .map(|context| context.modules.waiting.len())
                    .sum::<usize>()
                > crate::constants::MODULES
        {
            return Err(EngineError::limit(
                "V9 module records",
                crate::constants::MODULES,
            ));
        }
        let counts = self.heap.counts;
        // Global lexical strings live outside the heap yet are retained environment text too.
        let global_units: usize = self
            .globals
            .values()
            .map(|binding| binding.value.as_ref().map_or(0, JsValue::string_units))
            .sum();
        let global_bindings = self.globals.len();
        #[cfg(feature = "conformance")]
        let global_units = global_units + self.page.cpu_profile.units();
        #[cfg(feature = "conformance")]
        let (global_units, global_bindings) = (
            global_units + self.realms.units(),
            global_bindings + self.realms.bindings(),
        );
        for (count, limit, name) in [
            (counts.objects, limits.objects + 2, "V9 heap objects"),
            (counts.functions, limits.functions, "V9 functions"),
            (
                counts.properties,
                limits.properties,
                "V9 aggregate own properties",
            ),
            (
                counts.units + global_units,
                limits.units,
                "V9 environment UTF-16 units",
            ),
            (counts.environments, limits.environments, "V9 environments"),
            (
                counts.bindings + global_bindings,
                limits.bindings,
                "V9 bindings",
            ),
        ] {
            if count > limit {
                return Err(EngineError::limit(name, limit));
            }
        }
        if self
            .heap
            .objects
            .keys()
            .any(|id| *id >= self.heap.next_object && *id < vm::heap::INTRINSIC_BASE)
            || self.heap.envs.keys().any(|id| *id >= self.heap.next_env)
        {
            return Err(EngineError::invalid("Invalid V9 heap identities."));
        }
        self.page.listeners.validate(self.page.next_listener)?;
        platform::broadcast::validate(&self.page.broadcast, full_heap.then_some(&self.heap))?;
        if self.page.timers.len() > crate::constants::VM_TIMERS
            || !platform::host_bridge::validate(&self.page)
            || self.page.pending.len() > crate::constants::VM_HOST_REQUESTS
            || self
                .page
                .parsed_scripts
                .as_ref()
                .is_some_and(|scripts| scripts.len() > crate::constants::SCRIPT_COUNT)
            || self.page.dirty_options.len() > crate::constants::VM_DETACHED_NODES
            || self.page.selected_options.len() > crate::constants::VM_DETACHED_NODES
            || self.page.detached.len() > crate::constants::VM_DETACHED_NODES
            || self.jobs.len().saturating_add({
                #[cfg(feature = "conformance")]
                {
                    self.realms.custom_job_count()
                }
                #[cfg(not(feature = "conformance"))]
                {
                    0
                }
            }) > limits.jobs
            || self.dynamic.len() > crate::constants::VM_DYNAMIC_SOURCES
            || self.templates.len() > limits.objects
            || self.symbols.descriptions.len() > crate::constants::VM_SYMBOLS
        {
            return Err(EngineError::limit(
                "V9 page state",
                crate::constants::VM_TIMERS,
            ));
        }
        Ok(())
    }
}

/// Only input IDs and literal values cross into V9, never native agent handles.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputValue {
    /// Source lookup key within the exact candidate document.
    pub html_id: String,
    /// Bounded current field text.
    pub value: String,
}

/// The outcome of one request: proposals, the new state and what the app must do next.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Execution {
    /// Ordered DomChange proposals (the legacy log); the document engine replays them.
    pub changes: Vec<DomChange>,
    /// The extended log, used instead of `changes` when `PageEnvironment::dom_level` is Extended.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_changes: Vec<ScriptChange>,
    /// Digest of the final script projection; the applying engine must reproduce it exactly.
    #[serde(default)]
    pub structure: u64,
    /// Globals, heap and platform state commit atomically with document proposals.
    pub state: VmState,
    /// Storage proposals remain candidates until the coordinator commits the page.
    pub storage: StorageState,
    /// Scripts an isolated page load stopped and rolled back, in source order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failed: Vec<ScriptFailure>,
    /// Whole-load finalization failed and returned a validated empty state; no script is blamed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub load_failure: Option<EngineError>,
    /// Names the full state a V9 service kept for the next step, when it keeps states resident;
    /// `state` is then only its summary (see `VmState::summary`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state_token: Option<u64>,
    /// Requests, navigations, history, cookies, console output and the next timer.
    #[serde(default)]
    pub effects: HostEffects,
    /// Modules the page's graphs need; deliver them with `load_modules`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub module_requests: Vec<ModuleRequest>,
    /// Classic scripts the page's elements asked for; deliver each as a classic `ModuleSource`
    /// (or as failed) with `load_modules`, which runs it and fires the element's event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_requests: Vec<ScriptLoadRequest>,
    /// Frame documents the page's iframes asked for: the app fetches each URL and delivers it
    /// with `load_modules`, which fires the frame's `load` event.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frame_requests: Vec<FrameRequest>,
}

/** A `<script src>` a script connected to the document, for the app to fetch the way a browser
fetches a classic script: under the page's Content-Security-Policy (the element's nonce counts),
with a JavaScript media type, and against the element's integrity metadata. */
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptLoadRequest {
    /// The absolute script URL, which is its identity in the delivered sources.
    pub url: String,
    /// The element's `nonce` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// The element's `integrity` attribute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub integrity: Option<String>,
    /// The element's `crossorigin` attribute ("anonymous" or "use-credentials").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cross_origin: Option<String>,
}

/** One script that stopped during an isolated page load; none of its effects were kept. */
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptFailure {
    /// Zero-based position in the sources the caller passed.
    pub index: usize,
    /// Why it stopped: a syntax error, an exception, an unsupported feature or a limit.
    pub error: EngineError,
}

/** Only string keys and values cross the V9 storage boundary. */
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct StorageState {
    /// Origin-scoped localStorage candidate.
    pub local: BTreeMap<String, String>,
    /// Tab and origin-scoped sessionStorage candidate.
    pub session: BTreeMap<String, String>,
}

impl StorageState {
    /// Enforce independent entry and byte budgets before a candidate enters or leaves V9.
    pub fn validate(&self) -> EngineResult<()> {
        validate_storage_area(&self.local)?;
        validate_storage_area(&self.session)
    }
}

/** Storage limits apply to UTF-8 key/value bytes and never count profile encoding overhead. */
fn validate_storage_area(area: &BTreeMap<String, String>) -> EngineResult<()> {
    let bytes = area
        .iter()
        .map(|(key, value)| key.len().saturating_add(value.len()))
        .sum::<usize>();
    if area.len() > STORAGE_ENTRIES
        || area.iter().any(|(key, value)| {
            key.len() > STORAGE_VALUE_BYTES || value.len() > STORAGE_VALUE_BYTES
        })
        || bytes > STORAGE_AREA_BYTES
    {
        return Err(EngineError::limit(
            "Web Storage area bytes",
            STORAGE_AREA_BYTES,
        ));
    }
    Ok(())
}

/// Per-execution browser facts and the fragment parser, supplied by the app at the service edge.
pub struct DocumentHost<'a> {
    /// Synchronous worker script acquisition is governed by the owner, never by the VM.
    pub imports: Option<&'a dyn ImportScriptsHost>,
    /// Synchronous native graphics; standalone consumers can explicitly keep it unavailable.
    pub canvas: &'a dyn mars_core::canvas::CanvasHost,
    /// Preferred language tags, most preferred first, as `navigator.languages` reports them.
    pub languages: &'a [String],
    /// Parses inserted HTML so the same run sees new nodes; V9 never imports Eye for this.
    pub fragments: &'a (dyn FragmentParser + Sync),
}

impl Default for DocumentHost<'static> {
    /** No languages and a parser that refuses every fragment. */
    fn default() -> Self {
        Self {
            imports: None,
            languages: &[],
            canvas: &mars_core::canvas::Unavailable,
            fragments: &RefuseFragments,
        }
    }
}

/** Callers that supply no parser get an explicit unsupported error, never a silent no-op. */
struct RefuseFragments;

impl FragmentParser for RefuseFragments {
    /** Every fragment is refused before any node identity is consumed. */
    fn parse(&self, _: &str, _: &str, _: usize) -> EngineResult<Vec<ScriptNode>> {
        Err(EngineError::unsupported(
            "insertAdjacentHTML requires a host fragment parser.",
        ))
    }
}

/// The content key of one classic script, from `source_key`.
pub type SourceKey = [u8; 32];

/// The key under which a classic script's compiled code is cached, so a caller that keeps the
/// source resident computes it once instead of V9 hashing the source on every request.
pub fn source_key(source: &str) -> SourceKey {
    compile::source_key(source)
}

/// Everything one page request needs besides committed state.
pub struct PageInput<'a> {
    /// The page's classic scripts in execution order. A failed script keeps its text, since its
    /// declarations outlive it.
    pub sources: &'a [String],
    /// Each source's `source_key`, or empty to hash them here.
    pub source_keys: &'a [SourceKey],
    /// The current projection.
    pub document: ScriptDocument,
    /// Origin storage candidates.
    pub storage: StorageState,
    /// Languages and the fragment parser.
    pub host: &'a DocumentHost<'a>,
    /// URL, viewport, clock, cookie and the change-log level.
    pub environment: &'a PageEnvironment,
    /// Which sources are module scripts or import maps, and the modules fetched so far.
    pub modules: &'a ModuleInput,
}

/** The module input of callers that have no modules. */
const NO_MODULES: &ModuleInput = ModuleInput::NONE;

/// Page lifecycle events the app fires after scripts load.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PageEvent {
    /// A cloned asynchronous worker task targets only a context-local handle.
    WorkerMessage {
        id: u32,
        data: WorkerData,
        ports: Vec<u64>,
    },
    /// Loading or initial execution failed for this exact Worker handle.
    WorkerError { id: u32, message: String },
    /// readyState becomes "interactive"; DOMContentLoaded fires at the document.
    DomContentLoaded,
    /// readyState becomes "complete"; load fires at the window.
    Load,
    /// The viewport scrolled: `scroll` fires at the document and bubbles to the window, as
    /// Blink's `ScrollableArea` queues it once per frame that moved. Lazy loaders listen for it.
    Scroll,
    /// Back or Forward moved between two entries of this document (`environment.url` is the
    /// entry's URL): `popstate` fires with the entry's saved state, then `hashchange` when only
    /// the fragment differs, so a single-page router renders the route without a reload.
    PopState {
        /// The entry's state as JSON (`HistoryRecord::state`), or None for null.
        state: Option<String>,
        /// The two entries differ only in their fragment.
        hash_changed: bool,
    },
    /// A person or agent submits this form (Enter in a field, a submit button): `submit` fires
    /// at it first, so validation and single-page handlers run, and `effects.submit` names the
    /// form when nobody cancelled it.
    Submit {
        /// The form's projected id.
        form: usize,
    },
    /// A message another document posted to this window (`postMessage`): `message` fires at
    /// the window with the data, the sender's origin and its proxy as `source`.
    Message {
        /// The data as it crossed.
        data: platform::messaging::MessageData,
        /// The sender's serialized origin.
        origin: String,
        /// The sender: `PARENT` for the embedding page, or the iframe it runs in.
        #[serde(default)]
        source: Option<i64>,
        /// Links of the MessagePorts sent with it; each arrives as a port in `event.ports`.
        #[serde(default)]
        ports: Vec<u64>,
        /// Native messages queued before two endpoints reunited in this realm.
        #[serde(default)]
        queued_port_messages: Vec<QueuedPortMessage>,
    },
    /// A fenced native task delivers one structured packet to an exact local channel identity.
    BroadcastMessage {
        /// A never-reused channel identity within the task's owning context incarnation.
        id: u64,
        /// An immutable graph captured before the sender returned from postMessage.
        data: WorkerData,
        /// Sender origin is assigned by the native broker, never by page script.
        origin: String,
    },
    /// Media players changed: JSON updates (metadata, time, buffering, seeks, ends, errors and
    /// control clicks) for the platform's media script, which fires the element events.
    Media { updates: String },
    /// A native router retired the far end of one linked port.
    PortClose {
        /// A native router retired this link; clear the local endpoint without firing script.
        link: u64,
    },
    /// Deliver data on one current external channel endpoint.
    PortMessage {
        /// The channel link.
        link: u64,
        /// The data as it crossed.
        data: platform::messaging::MessageData,
    },
}

/**
 * Parsing and evaluation recurse along the syntax tree, so every entry point runs on its own
 * thread with V9_STACK_BYTES of stack. A panic stays on that thread and becomes an error.
 */
fn on_v9_stack<T: Send>(work: impl FnOnce() -> EngineResult<T> + Send) -> EngineResult<T> {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("v9".into())
            .stack_size(crate::constants::V9_STACK_BYTES)
            .spawn_scoped(scope, work)
            .map_err(|_| EngineError::unsupported("V9 could not start its execution thread."))?
            .join()
            .unwrap_or_else(|_| Err(EngineError::invalid("V9 stopped on an internal error.")))
    })
}

/** Tokenize and parse one source whose bytes start at `offset` of the page. */
pub(in crate::v9) fn parse_at(
    source: &str,
    offset: usize,
    index: Option<usize>,
) -> EngineResult<syntax::Program> {
    parse_with_mode(source, offset, index, false, &[], 0, false)
}

/** A trusted shell fixture uses its own native grammar and retains its original source text. */
#[cfg(feature = "conformance")]
pub(in crate::v9) fn parse_native_fixture(
    source: &str,
    offset: usize,
    index: Option<usize>,
) -> EngineResult<syntax::Program> {
    parse_with_mode(source, offset, index, false, &[], 0, true)
}

/** A direct eval's inherited grammar never comes from rewriting the script source. */
fn parse_with_mode(
    source: &str,
    offset: usize,
    index: Option<usize>,
    strict: bool,
    private_names: &[syntax::Name],
    grammar: u8,
    native_fixture: bool,
) -> EngineResult<syntax::Program> {
    let tokens = lexer::tokenize_at(source, offset).map_err(|mut error| {
        if let Some(location) = error.location.as_deref_mut() {
            location.script_index = index.map(|index| index + 1);
        }
        error
    })?;
    let result = if !private_names.is_empty() || grammar != 0 || native_fixture {
        syntax::parse_eval(tokens, strict, private_names, grammar, native_fixture)
    } else if strict {
        syntax::parse_strict(tokens)
    } else {
        syntax::parse(tokens)
    };
    result.map_err(|mut error| {
        if let Some(location) = error.location.as_deref_mut() {
            location.script_index = index.map(|index| index + 1);
        }
        error
    })
}

/** Compile dynamic source at `base` into the registry; returns its top-level code. `context` is
the compiler-made caller context of direct eval code, never read from the source. `native` is
whether the page admitted the shell intrinsic grammar, which its dynamic code inherits. */
pub(in crate::v9) fn compile_dynamic(
    registry: &mut Registry,
    source: &str,
    base: usize,
    context: Option<&str>,
    native: bool,
) -> EngineResult<Arc<Code>> {
    let site = syntax::site_of(syntax::Pos {
        offset: base as u32,
        line: 1,
        column: 1,
    });
    // Only code this step compiled here answers without a lookup: a carried registry may hold
    // an earlier step's different source or context at this offset, which the keyed cache
    // below tells apart.
    if let Some(code) = registry.adopted_this_step(site) {
        return Ok(code);
    }
    match context {
        Some(context) => registry.eval(source, base, context, |strict, private_names, grammar| {
            if source.len() > SCRIPT_BYTES {
                return Err(EngineError::limit("script bytes", SCRIPT_BYTES));
            }
            parse_with_mode(source, base, None, strict, private_names, grammar, native)
        }),
        None => registry.script(source, base, || on_parse(source, base, false, native)),
    }
}

/** Parse dynamic or handler source. */
fn on_parse(
    source: &str,
    base: usize,
    strict: bool,
    native: bool,
) -> EngineResult<syntax::Program> {
    if source.len() > SCRIPT_BYTES {
        return Err(EngineError::limit("script bytes", SCRIPT_BYTES));
    }
    parse_with_mode(source, base, None, strict, &[], 0, native)
}

/** Parse one classic script without running it, the way a page load would read it. */
pub fn check(source: &str) -> EngineResult<()> {
    on_v9_stack(|| {
        let program = parse_at(source, 0, None)?;
        Registry::default().add(&program).map(|_| ())
    })
}

/** Byte offsets of each source in the page. */
fn offsets(sources: &[String]) -> Vec<usize> {
    let mut offset = 0;
    sources
        .iter()
        .map(|source| {
            let start = offset;
            offset += source.len() + 1;
            start
        })
        .collect()
}

/** Validate host language tags: bounded count and length, ASCII tag characters only. */
fn validate_languages(languages: &[String]) -> EngineResult<Vec<String>> {
    mars_core::languages::validate(languages)?;
    Ok(languages.to_vec())
}

/** What a session compiles from the sources. */
enum Compiled {
    /** Every script must compile; the first failure fails the request. */
    All(Vec<Arc<Code>>),
    /** Each script compiles alone; failures are reported per index. */
    Each(Vec<EngineResult<Arc<Code>>>),
}

/** The text of each inline page script by source index; external scripts read as empty, since a
`<script src>` element's text is its children, not the fetched source. */
fn inline_sources(sources: &[String], urls: &[String]) -> Vec<String> {
    sources
        .iter()
        .enumerate()
        .map(|(index, source)| match urls.get(index) {
            Some(url) if url.is_empty() => source.clone(),
            _ => String::new(),
        })
        .collect()
}

/** Open a VM for a request: restore state, compile sources, set the clock and deadline. The
deadline is armed once compilation is done: scanning and parsing are already bounded by
SCRIPT_BYTES, TOKENS and STATEMENTS, and a max-token script takes several times the script deadline
to compile in an unoptimized build, so charging it would refuse admitted sources. */
#[allow(clippy::too_many_arguments)]
fn session<'h>(
    limits: Limits,
    sources: &[String],
    keys: &[SourceKey],
    isolated: bool,
    document: Option<ScriptDocument>,
    storage: StorageState,
    host: &DocumentHost<'h>,
    environment: &PageEnvironment,
    state: Option<VmState>,
    modules: &ModuleInput,
) -> EngineResult<(Vm<'h>, Compiled)> {
    #[allow(unused_mut)]
    let mut limits = limits;
    #[cfg(feature = "conformance")]
    if environment.conformance_d8
        && let Some(requested) = environment.conformance_operation_limit
    {
        const MAX_CONFORMANCE_OPERATIONS: usize = crate::constants::OPERATIONS * 10;
        if !(crate::constants::OPERATIONS..=MAX_CONFORMANCE_OPERATIONS).contains(&requested) {
            return Err(EngineError::invalid(
                "Invalid conformance operation budget.",
            ));
        }
        limits.operations = requested;
    }
    if let Some(document) = &document {
        document.validate()?;
    }
    storage.validate()?;
    environment.handler_policy.validate()?;
    mars_core::dom::validate_metadata(&environment.metadata, &environment.script_attributes)?;
    platform::clock::validate(0.0, 0.0, environment.time_origin_ms)?;
    platform::frames::validate_routes(environment, document.as_ref())?;
    platform::frame_directory::validate(environment)?;
    builtins::validate_timezone(environment)?;
    #[cfg(feature = "conformance")]
    builtins::validate_conformance_environment(environment)?;
    let languages = validate_languages(host.languages)?;
    let state = state.unwrap_or_default();
    if environment.worker_depth > WORKER_DEPTH
        || environment.worker_name.is_none() && environment.worker_depth != 0
    {
        return Err(EngineError::invalid("Invalid worker nesting profile."));
    }
    if state.page.initialized
        && (state.page.worker_name != environment.worker_name
            || state.page.worker_depth != environment.worker_depth
            || state.page.worker_module != environment.worker_module)
    {
        return Err(EngineError::invalid(
            "V9 state belongs to another global profile.",
        ));
    }
    // Only the first step of a page runs the platform bootstrap; every step compiles it (see below).
    let fresh = !state.page.initialized;
    let verified = state.verified;
    if !verified {
        state.validate(&limits)?;
        platform::channel::validate_state(&state.page, &state.heap)?;
    }
    vm::validate_dynamic_sources(&state.dynamic)?;
    legacy_handlers::validate(&state)?;
    if let Some(document) = &document
        && state.page.initialized
        && state.page.identity != document.identity
    {
        return Err(EngineError::invalid(
            "V9 state belongs to another document.",
        ));
    }
    if sources.len() > SCRIPT_COUNT && !isolated {
        return Err(EngineError::limit("classic scripts", SCRIPT_COUNT));
    }
    let mut page = platform::Page::new(
        environment.clone(),
        languages,
        host.fragments,
        document,
        state.page,
        storage,
    );
    page.canvas_host = host.canvas;
    page.imports = host.imports;
    page.inline_scripts = inline_sources(sources, &environment.scripts);
    let mut vm = Vm::new(state.heap, page, limits);
    vm.registry = state.registry;
    vm.registry.begin_step();
    builtins::initialize_shared(&mut vm)?;
    builtins::initialize_shared_gc(&mut vm)?;
    #[cfg(feature = "conformance")]
    builtins::initialize_shared_containers(&mut vm)?;
    #[cfg(feature = "conformance")]
    {
        vm.realms = state.realms;
    }
    vm.globals = state.globals;
    vm.register_global_strings();
    #[cfg(feature = "conformance")]
    if fresh && vm.page.environment.conformance_d8 {
        let arguments = vm.page.environment.conformance_script_arguments.clone();
        if arguments.len() > SCRIPT_COUNT
            || arguments.iter().fold(0usize, |total, value| {
                total.saturating_add(value.encode_utf16().count())
            }) > crate::constants::JS_STRING_UNITS
        {
            return Err(EngineError::limit(
                "d8 script arguments",
                crate::constants::JS_STRING_UNITS,
            ));
        }
        let values = arguments
            .iter()
            .map(|value| crate::v9::value::JsString::from_utf8(value).map(JsValue::String))
            .collect::<EngineResult<Vec<_>>>()?;
        let array = vm.new_array(values).map_err(fatal)?;
        vm.heap.put(
            vm.global_object(),
            vm::heap::Key::from("arguments"),
            vm::heap::Prop::data(JsValue::Object(array)),
        );
    }
    #[cfg(feature = "conformance")]
    if fresh && vm.page.environment.conformance_expose_externalize_string {
        builtins::install_external_string_lexicals(&mut vm, 0);
    }

    vm.symbols = state.symbols;
    vm.jobs = state.jobs.into();
    initialize_page(&mut vm);
    modules::Modules::open(&mut vm, modules, sources, &offsets(sources), state.modules)?;
    let compiled = compile_sources(&mut vm, sources, keys, isolated)?;
    let mut imports = Vec::new();
    for (index, source) in state.dynamic.iter().enumerate() {
        let base = vm::dynamic_source_base(&state.dynamic, index);
        let native = vm.native_dynamic();
        let code = compile_dynamic(
            &mut vm.registry,
            &source.source,
            base,
            source.context.as_deref(),
            native,
        )?;
        if vm.page.state.worker_imports.contains_key(&base) {
            if vm.page.environment.worker_name.is_none() || source.context.is_some() {
                return Err(EngineError::invalid("Invalid retained worker Script."));
            }
            imports.push(code);
        } else if vm.page.state.legacy_handlers.contains(&base) {
            imports.push(code);
        }
    }
    vm.dynamic = state.dynamic;
    vm.templates = state.templates;
    // The platform bootstrap is not a dynamic source: every step compiles it at its fixed offset, so the heap's closures always find their code.
    let bootstrap = platform::observable::compile(&mut vm)?;
    let indexed_db = platform::indexed_db::compile(&mut vm)?;
    if !verified {
        vm.validate_graph()?;
    }
    platform::images::validate_targets(&vm.page)?;
    platform::media::validate(&vm)?;
    platform::canvas::validate_targets(&mut vm)?;
    platform::canvas::path2d::validate_targets(&mut vm)?;
    let loaded = vm.modules.loaded_codes();
    let mut scripts: Vec<&Code> = match &compiled {
        Compiled::All(codes) => codes.iter().map(|code| &**code).collect(),
        Compiled::Each(codes) => codes.iter().flatten().map(|code| &**code).collect(),
    };
    scripts.extend(loaded.iter().map(|code| &**code));
    scripts.extend(imports.iter().map(|code| &**code));
    #[cfg(feature = "conformance")]
    scripts.extend(vm.realm_codes(0).into_iter().map(|code| &**code));
    vm.validate_globals(&scripts)?;
    vm.page.start_execution_clock();
    vm.deadline = Some(std::time::Instant::now() + script_deadline());
    if fresh {
        platform::observable::run(&mut vm, bootstrap)?;
        platform::indexed_db::run(&mut vm, indexed_db)?;
    }
    Ok((vm, compiled))
}

/** First-request page facts: identity, clock, URL, cookie, random seed. */
fn initialize_page(vm: &mut Vm) {
    let identity = vm
        .page
        .document
        .as_ref()
        .map(|document| document.identity.clone())
        .unwrap_or_else(|| "legacy".into());
    let charset = vm
        .page
        .document
        .as_ref()
        .map(|document| document.charset.clone())
        .unwrap_or_default();
    let state = &mut vm.page.state;
    if state.initialized {
        return;
    }
    let environment = &vm.page.environment;
    state.identity = identity;
    state.worker_name = environment.worker_name.clone();
    state.worker_depth = environment.worker_depth;
    state.worker_module = environment.worker_module;
    state.charset = charset;
    state.epoch = environment.time_origin_ms;
    state.random = environment.random_seed;
    state.href = environment.url.clone();
    state.cookie = environment.cookie.clone();
    state.history_length = 1;
    state.initialized = true;
}

/** Compile page sources at their page offsets. */
fn compile_sources(
    vm: &mut Vm,
    sources: &[String],
    keys: &[SourceKey],
    isolated: bool,
) -> EngineResult<Compiled> {
    let starts = offsets(sources);
    let mut total = 0usize;
    let mut each = Vec::with_capacity(sources.len());
    for (index, (source, start)) in sources.iter().zip(starts).enumerate() {
        total = total.saturating_add(source.len());
        let result = if index >= SCRIPT_COUNT {
            Err(EngineError::limit("classic scripts", SCRIPT_COUNT))
        } else if source.len() > SCRIPT_BYTES {
            Err(EngineError::limit("script bytes", SCRIPT_BYTES))
        } else if total > SCRIPT_TOTAL_BYTES {
            Err(EngineError::limit("page script bytes", SCRIPT_TOTAL_BYTES))
        } else if vm.modules.entry(index).is_some() {
            // Module scripts and import maps compiled with the module map; never as classic.
            Err(EngineError::invalid("V9 module entry."))
        } else {
            compile_page_script(vm, source, start, index, keys)
        };
        if !isolated {
            each.push(Ok(result?));
        } else {
            each.push(result);
        }
    }
    Ok(if isolated {
        Compiled::Each(each)
    } else {
        Compiled::All(
            each.into_iter()
                .map(|code| code.expect("checked"))
                .collect(),
        )
    })
}

/** A host-selected native script uses a separate cache key without changing its source. */
fn compile_page_script(
    vm: &mut Vm,
    source: &str,
    start: usize,
    index: usize,
    keys: &[SourceKey],
) -> EngineResult<std::sync::Arc<compile::Code>> {
    #[cfg(feature = "conformance")]
    if vm.page.environment.conformance_d8
        && vm.page.environment.conformance_native_main_source == Some(index)
    {
        return vm.registry.script_keyed(
            compile::native_fixture_key(source),
            source,
            start,
            || parse_native_fixture(source, start, Some(index)),
        );
    }
    let key = keys
        .get(index)
        .copied()
        .unwrap_or_else(|| compile::source_key(source));
    vm.registry
        .script_keyed(key, source, start, || parse_at(source, start, Some(index)))
}

/** Run one script's top-level code, then its microtasks. */
fn run_one(vm: &mut Vm, code: Arc<Code>, index: usize) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    vm.page.throw_location = None;
    let ran = vm.run_script(code).map(|_| ());
    let location = vm.page.throw_location.unwrap_or_default();
    // As in browsers, the microtask checkpoint follows every script, one that threw included:
    // skipping it left that script's promise reactions (and unhandled rejections) unreported.
    let drained = match &ran {
        Err(Thrown::Fatal(_)) => Ok(()),
        _ => vm.drain_jobs(),
    };
    let result = match (ran, drained) {
        (_, Err(fatal @ Thrown::Fatal(_))) | (Err(fatal), _) => Err(fatal),
        (Ok(()), result) => result,
    };
    result.map_err(|thrown| {
        #[cfg(feature = "conformance")]
        let mut error = if vm.page.environment.conformance_d8 {
            builtins::report_terminal_exception(vm, thrown, location)
        } else {
            vm.uncaught(thrown, location)
        };
        #[cfg(not(feature = "conformance"))]
        let mut error = vm.uncaught(thrown, location);
        if let Some(location) = error.location.as_deref_mut() {
            location.script_index = Some(index + 1);
        }
        error
    })
}

/** Everything a failed script could change, restored on rollback. */
struct Snapshot {
    #[cfg(feature = "conformance")]
    realms: vm::realms::State,
    heap: Heap,
    kept_weak: vm::heap::FastMap<vm::data::TableKey, JsValue>,
    globals: Globals,
    symbols: Symbols,
    jobs: std::collections::VecDeque<Job>,
    state: platform::PageState,
    storage: StorageState,
    document: Option<ScriptDocument>,
    changes: Vec<DomChange>,
    script_changes: Vec<ScriptChange>,
    effects: HostEffects,
    dynamic: Vec<vm::DynamicSource>,
    templates: vm::templates::Cache,
    modules: modules::ModuleState,
    next_id: usize,
    change_bytes: usize,
    fragment_bytes: usize,
}

/** Take a snapshot between scripts. */
fn snapshot(vm: &Vm) -> Snapshot {
    Snapshot {
        #[cfg(feature = "conformance")]
        realms: vm.realms.clone(),
        heap: vm.heap.clone(),
        kept_weak: vm.kept_weak.clone(),
        globals: vm.globals.clone(),
        symbols: vm.symbols.clone(),
        jobs: vm.jobs.clone(),
        state: vm.page.state.clone(),
        storage: vm.page.storage.clone(),
        document: vm.page.document.clone(),
        changes: vm.page.changes.clone(),
        script_changes: vm.page.script_changes.clone(),
        effects: vm.page.effects.clone(),
        dynamic: vm.dynamic.clone(),
        templates: vm.templates.clone(),
        modules: vm.modules.state.clone(),
        next_id: vm.page.next_id,
        change_bytes: vm.page.change_bytes,
        fragment_bytes: vm.page.fragment_bytes,
    }
}

/** Restore a snapshot after a failed script. */
fn restore(vm: &mut Vm, saved: Snapshot) {
    #[cfg(feature = "conformance")]
    {
        vm.realms = saved.realms;
    }
    vm.heap = saved.heap;
    vm.kept_weak = saved.kept_weak;
    vm.globals = saved.globals;
    vm.symbols = saved.symbols;
    vm.jobs = saved.jobs;
    vm.page.state = saved.state;
    vm.page.storage = saved.storage;
    vm.page.document = saved.document;
    vm.page.changes = saved.changes;
    vm.page.script_changes = saved.script_changes;
    vm.page.effects.requests = saved.effects.requests;
    vm.page.effects.cancelled_requests = saved.effects.cancelled_requests;
    vm.page.effects.image_requests = saved.effects.image_requests;
    vm.page.effects.image_cancellations = saved.effects.image_cancellations;
    vm.page.effects.image_adoptions = saved.effects.image_adoptions;
    vm.page.effects.media = saved.effects.media;
    vm.page.effects.worker_commands = saved.effects.worker_commands;
    vm.page.effects.worker_errors = saved.effects.worker_errors;
    vm.page.effects.messages = saved.effects.messages;
    vm.page.effects.port_messages = saved.effects.port_messages;
    vm.page.effects.port_closes = saved.effects.port_closes;
    vm.page.effects.broadcast_commands = saved.effects.broadcast_commands;
    vm.page.effects.broadcast_channels = saved.effects.broadcast_channels;
    vm.page.effects.navigation = saved.effects.navigation;
    vm.page.effects.history = saved.effects.history;
    vm.page.effects.cookies = saved.effects.cookies;
    vm.page.effects.title = saved.effects.title;
    vm.dynamic = saved.dynamic;
    vm.templates = saved.templates;
    vm.modules.state = saved.modules;
    vm.page.next_id = saved.next_id;
    vm.page.change_bytes = saved.change_bytes;
    vm.page.fragment_bytes = saved.fragment_bytes;
    vm.page.invalidate();
    vm.frames.clear();
    vm.pins.clear();
    vm.native_depth = 0;
    vm.pinning = false;
}

/** Whether any count has reached three quarters of its bound. Counts are exact when a request
starts (every one ends collected or recounted), so this only collects garbage before a script
could run into a bound mid-request; at half, YouTube's 525,000 live properties of 1,048,576
collected on every request. */
fn near_bounds(counts: &vm::heap::Counts, limits: &Limits) -> bool {
    past_fraction(counts, limits, NEAR_BOUNDS_FRACTION)
}

/** Whether any count has reached `numerator / denominator` of its bound. */
fn past_fraction(
    counts: &vm::heap::Counts,
    limits: &Limits,
    (numerator, denominator): (usize, usize),
) -> bool {
    [
        (counts.objects, limits.objects),
        (counts.functions, limits.functions),
        (counts.properties, limits.properties),
        (counts.units, limits.units),
        (counts.environments, limits.environments),
        (counts.bindings, limits.bindings),
    ]
    .iter()
    .any(|(count, limit)| count.saturating_mul(denominator) >= limit.saturating_mul(numerator))
}

/** Collect, validate and package the request's outcome. */
fn finish(mut vm: Vm) -> EngineResult<Execution> {
    vm.select_realm(0).map_err(fatal)?;
    if vm.page.environment.worker_name.is_some() {
        vm.page.effects.worker_ready = Some(modules::worker_ready(&vm));
    }
    platform::canvas::publish(&mut vm.page)?;
    platform::images::requests(&mut vm)?;
    if !vm.page.sampled_observer_turn {
        platform::intersection_observers::sample(&mut vm).map_err(fatal)?;
        platform::resize_observers::sample(&mut vm).map_err(fatal)?;
    }
    vm.page.effects.opacity_effects = platform::animations::snapshot(&mut vm);
    vm.frames.clear();
    vm.pins.clear();
    #[cfg(feature = "conformance")]
    vm.prune_number_constants();
    // Counts only overestimate between collections, so a heap near any bound is collected
    // (and counted exactly) before its bounds are checked. Elsewhere the running counts stand:
    // a bound reached mid-script collects there (`collect_if_needed`), so an overestimate
    // carried into the next request can only cause a collection, never a refusal.
    if vm.heap.grew_since_collection() || near_bounds(&vm.heap.counts, &vm.limits) {
        vm.collect_snapshot().map_err(fatal)?;
    }
    #[cfg(feature = "conformance")]
    vm.save_interned_strings();
    platform::broadcast::publish(&mut vm.page);
    platform::publish_local_sheets(&mut vm);
    let effects_timer = vm.next_timer();
    let frame_requests = std::mem::take(&mut vm.page.state.frame_requests);
    let script_requests: Vec<ScriptLoadRequest> = vm
        .page
        .state
        .script_loads
        .iter()
        .filter(|load| !load.done && !load.url.is_empty() && !vm.modules.delivered(&load.url))
        .map(|load| ScriptLoadRequest {
            url: load.url.clone(),
            nonce: load.nonce.clone(),
            integrity: load.integrity.clone(),
            cross_origin: load.cross_origin.clone(),
        })
        .collect();
    vm.page.retain_clock();
    let platform::Page {
        state: page,
        storage,
        document,
        changes,
        script_changes,
        mut effects,
        ..
    } = vm.page;
    effects.next_timer_ms = effects_timer;
    let module_requests = std::mem::take(&mut vm.modules.requests);
    let state = VmState {
        #[cfg(feature = "conformance")]
        realms: vm.realms,
        globals: vm.globals,
        heap: vm.heap,
        symbols: vm.symbols,
        jobs: vm.jobs.into(),
        page,
        dynamic: vm.dynamic,
        templates: vm.templates,
        modules: std::mem::take(&mut vm.modules.state),
        verified: true,
        registry: std::mem::take(&mut vm.registry),
    };
    state.validate_bounds(&vm.limits)?;
    storage.validate()?;
    #[cfg(feature = "measure")]
    measure::record(&state);
    Ok(Execution {
        structure: document.as_ref().map_or(0, ScriptDocument::digest),
        changes,
        script_changes,
        state,
        storage,
        failed: Vec::new(),
        load_failure: None,
        state_token: None,
        effects,
        module_requests,
        script_requests,
        frame_requests,
    })
}

/** An engine error out of a script-level result. */
fn fatal(thrown: Thrown) -> EngineError {
    match thrown {
        Thrown::Fatal(error) => error,
        Thrown::Value(_) => EngineError::invalid("V9 stopped on an uncaught internal exception."),
    }
}

/** Load ordered scripts one at a time; each failure rolls back only that script. With `resume`
the load continues where a deferred module script paused it, once the app delivered modules. */
fn load_isolated(
    input: PageInput<'_>,
    state: Option<VmState>,
    limits: Limits,
    resume: bool,
) -> EngineResult<Execution> {
    let initial = (input.document.clone(), input.storage.clone());
    let (mut vm, compiled) = session(
        limits,
        input.sources,
        input.source_keys,
        true,
        Some(input.document),
        input.storage,
        input.host,
        input.environment,
        state,
        input.modules,
    )?;
    let Compiled::Each(codes) = compiled else {
        return Err(EngineError::invalid(
            "V9 compiled scripts in the wrong mode.",
        ));
    };
    let mut failed = Vec::new();
    let mut ran = Vec::new();
    let from = if resume {
        vm.modules.state.cursor.take()
    } else {
        queue_async_modules(&mut vm, codes.len());
        Some(0)
    };
    if let Some(from) = from {
        run_sources(&mut vm, &codes, from, !resume, &mut failed, &mut ran)?;
    }
    if resume {
        platform::scripts::run_delivered(&mut vm)?;
        platform::frames::run_frames(&mut vm)?;
    }
    run_async_modules(&mut vm, &mut failed, &mut ran);
    if resume {
        run_dynamic_imports(&mut vm);
    }
    let mut load_failure = None;
    let execution = match finish(vm) {
        Ok(execution) => execution,
        Err(error) => {
            load_failure = Some(error);
            let empty = PageInput {
                sources: &[],
                source_keys: &[],
                document: initial.0,
                storage: initial.1,
                host: input.host,
                environment: input.environment,
                modules: NO_MODULES,
            };
            let (vm, _) = session(
                limits,
                empty.sources,
                empty.source_keys,
                true,
                Some(empty.document),
                empty.storage,
                empty.host,
                empty.environment,
                None,
                empty.modules,
            )?;
            finish(vm)?
        }
    };
    failed.sort_by_key(|failure| failure.index);
    Ok(Execution {
        failed,
        load_failure,
        ..execution
    })
}

/** Run page sources in order from `from`, each classic script rolled back alone on failure. A
deferred module script runs its whole graph the same way; while that graph is still loading the
load pauses there (later scripts wait, as they would in a browser) until `load_modules`. A
`fresh` load also queues the parsed sheets' `load` events once their elements exist. */
fn run_sources(
    vm: &mut Vm,
    codes: &[EngineResult<Arc<Code>>],
    from: usize,
    fresh: bool,
    failed: &mut Vec<ScriptFailure>,
    ran: &mut Vec<usize>,
) -> EngineResult<()> {
    let elements = vm.page.script_elements(codes.len().min(SCRIPT_COUNT))?;
    if fresh {
        platform::scripts::parsed_sheets(vm).map_err(fatal)?;
    }
    for (index, code) in codes.iter().enumerate().skip(from) {
        if index >= SCRIPT_COUNT {
            failed.push(failure(
                index,
                EngineError::limit("classic scripts", SCRIPT_COUNT),
            ));
            continue;
        }
        if vm.page.environment.refused_scripts.contains(&index) {
            platform::scripts::completed(vm, elements[index], "error")?;
            continue;
        }
        if let Some(kind) = vm.modules.entry(index) {
            if kind == EntryKind::Module {
                match modules::run_entry(vm, index) {
                    modules::Outcome::Ran => ran.push(index),
                    modules::Outcome::Failed(error) => failed.push(failure(index, error)),
                    modules::Outcome::Waiting => {
                        vm.modules.state.cursor = Some(index);
                        // Later module scripts' graphs load meanwhile, as browsers fetch them.
                        modules::discover(vm, index + 1..codes.len());
                        return Ok(());
                    }
                }
            }
            continue;
        }
        vm.page.current_script = elements.get(index).copied();
        let code = match code {
            Ok(code) => code.clone(),
            Err(error) => {
                failed.push(failure(index, error.clone()));
                parsed_script_completed(vm, elements[index], index)?;
                continue;
            }
        };
        if vm.expired() {
            failed.push(ScriptFailure {
                index,
                error: platform::timeout(),
            });
            continue;
        }
        // As in browsers, a script that throws keeps what it did before the throw; only the
        // error is reported, and later scripts still run. Restoring the page instead meant
        // copying the whole VM before every script, over a third of YouTube's load.
        match run_one(vm, code, index) {
            Ok(()) => ran.push(index),
            Err(error) => {
                #[cfg(feature = "conformance")]
                if builtins::is_test_exit(&error) {
                    return Err(error);
                }
                // A script stopped at a bound left garbage over it; the next script starts clean.
                vm.collect_now().map_err(fatal)?;
                failed.push(ScriptFailure { index, error });
            }
        }
        parsed_script_completed(vm, elements[index], index)?;
    }
    Ok(())
}

/** Successful acquisition still fires load when evaluating the script reports an exception. */
fn parsed_script_completed(vm: &mut Vm, element: usize, index: usize) -> EngineResult<()> {
    vm.page.current_script = None;
    if vm
        .page
        .environment
        .scripts
        .get(index)
        .is_some_and(|src| !src.is_empty())
    {
        platform::scripts::completed(vm, element, "load")?;
    }
    Ok(())
}

/** A failure at a source index, located there. */
fn failure(index: usize, mut error: EngineError) -> ScriptFailure {
    if let Some(location) = error.location.as_deref_mut() {
        location.script_index = Some(index + 1);
    }
    ScriptFailure { index, error }
}

/** Async module scripts do not wait for the parse or each other: they wait for their graphs. */
fn queue_async_modules(vm: &mut Vm, count: usize) {
    for index in 0..count {
        if vm.modules.entry(index) == Some(EntryKind::AsyncModule) {
            let key = vm.modules.entry_key(index);
            vm.modules.state.waiting.push(modules::Waiting {
                key,
                entry: Some(index),
                promise: None,
            });
        }
    }
}

/** Run the async module scripts whose graphs are complete, each isolated like any script. */
fn run_async_modules(vm: &mut Vm, failed: &mut Vec<ScriptFailure>, ran: &mut Vec<usize>) {
    let waiting = std::mem::take(&mut vm.modules.state.waiting);
    let mut kept = Vec::new();
    for item in waiting {
        let Some(index) = item.entry else {
            kept.push(item);
            continue;
        };
        if vm.expired() {
            failed.push(ScriptFailure {
                index,
                error: platform::timeout(),
            });
            continue;
        }
        match modules::run_entry(vm, index) {
            modules::Outcome::Ran => ran.push(index),
            modules::Outcome::Failed(error) => failed.push(failure(index, error)),
            modules::Outcome::Waiting => kept.push(item),
        }
    }
    kept.append(&mut vm.modules.state.waiting);
    vm.modules.state.waiting = kept;
}

/** Evaluate the dynamic imports the delivered modules completed. An engine failure (a limit,
the deadline) rolls the attempt back and rejects those imports, so the page keeps running. */
fn run_dynamic_imports(vm: &mut Vm) {
    let saved = snapshot(vm);
    vm.remaining = vm.limits.operations;
    let result = modules::advance_dynamic(vm).and_then(|()| vm.drain_jobs());
    if let Err(thrown) = result {
        let error = vm.uncaught(thrown, Default::default());
        restore(vm, saved);
        modules::refuse_dynamic(vm, &error.message);
    }
}

/// Load a page's scripts like a browser: a failing script is reported and later ones still run.
/// Microtasks drain
/// after every script; timers wait for `run_timers`; lifecycle events wait for `page_event`.
pub fn load_page(input: PageInput<'_>) -> EngineResult<Execution> {
    on_v9_stack(|| load_isolated(input, None, Limits::default(), false))
}

/// Deliver fetched modules (`PageInput::modules` now holds them) against committed state: a page
/// load paused at a module script continues, and async module scripts and dynamic imports whose
/// graphs are complete evaluate. `Execution::module_requests` may ask for more.
pub fn load_modules(input: PageInput<'_>, state: VmState) -> EngineResult<Execution> {
    on_v9_stack(|| load_isolated(input, Some(state), Limits::default(), true))
}

/// Fire DOMContentLoaded or load against committed state.
pub fn page_event(
    input: PageInput<'_>,
    state: VmState,
    event: PageEvent,
) -> EngineResult<Execution> {
    on_v9_stack(|| page_event_with(input, state, event, Limits::default()))
}

/** page_event under explicit bounds. */
fn page_event_with(
    input: PageInput<'_>,
    state: VmState,
    event: PageEvent,
    limits: Limits,
) -> EngineResult<Execution> {
    {
        let (mut vm, _) = open(&input, state, limits)?;
        match event {
            PageEvent::WorkerMessage { id, data, ports } => {
                let result = platform::workers::receive(&mut vm, id, &data, &ports)
                    .and_then(|()| vm.drain_jobs());
                report_thrown(&mut vm, result)?;
            }
            PageEvent::WorkerError { id, message } => {
                let result =
                    platform::workers::error(&mut vm, id, &message).and_then(|()| vm.drain_jobs());
                report_thrown(&mut vm, result)?;
            }
            PageEvent::DomContentLoaded => {
                vm.page.state.ready = platform::ReadyState::Interactive;
                fire_lifecycle(
                    &mut vm,
                    platform::Target::Document,
                    "readystatechange",
                    false,
                )?;
                fire_lifecycle(
                    &mut vm,
                    platform::Target::Document,
                    "DOMContentLoaded",
                    true,
                )?;
            }
            PageEvent::Load => {
                vm.page.state.ready = platform::ReadyState::Complete;
                fire_lifecycle(
                    &mut vm,
                    platform::Target::Document,
                    "readystatechange",
                    false,
                )?;
                fire_lifecycle(&mut vm, platform::Target::Window, "load", false)?;
                fire_lifecycle(&mut vm, platform::Target::Window, "pageshow", false)?;
            }
            PageEvent::Scroll => {
                fire_lifecycle(&mut vm, platform::Target::Document, "scroll", true)?;
            }
            PageEvent::PopState {
                state,
                hash_changed,
            } => pop_state(&mut vm, state.as_deref(), hash_changed)?,
            PageEvent::Submit { form } => submit_event(&mut vm, form)?,
            PageEvent::Message {
                data,
                origin,
                source,
                ports,
                queued_port_messages,
            } => {
                vm.remaining = vm.limits.operations;
                let received = platform::messaging::receive(
                    &mut vm,
                    &data,
                    &origin,
                    source,
                    &ports,
                    &queued_port_messages,
                )
                .and_then(|_| vm.drain_jobs());
                report_thrown(&mut vm, received)?;
            }
            PageEvent::BroadcastMessage { id, data, origin } => {
                let result = platform::broadcast::receive(&mut vm, id, &data, &origin)
                    .and_then(|()| vm.drain_jobs());
                report_thrown(&mut vm, result)?;
            }
            PageEvent::PortClose { link } => platform::channel::remote_close(&mut vm, link),
            PageEvent::Media { updates } => {
                vm.remaining = vm.limits.operations;
                let result =
                    platform::media::deliver(&mut vm, &updates).and_then(|()| vm.drain_jobs());
                report_thrown(&mut vm, result)?;
            }
            PageEvent::PortMessage { link, data } => {
                vm.remaining = vm.limits.operations;
                let received = platform::messaging::receive_on_port(&mut vm, link, &data)
                    .and_then(|_| vm.drain_jobs());
                report_thrown(&mut vm, received)?;
            }
        }
        finish(vm)
    }
}

/** Open a session over committed state for an event, timer or response. */
fn open<'h>(
    input: &PageInput<'h>,
    state: VmState,
    limits: Limits,
) -> EngineResult<(Vm<'h>, Compiled)> {
    session(
        limits,
        input.sources,
        input.source_keys,
        true,
        Some(input.document.clone()),
        input.storage.clone(),
        input.host,
        input.environment,
        Some(state),
        input.modules,
    )
}

/** Fire one lifecycle event and drain microtasks; script errors go to the console. */
fn fire_lifecycle(
    vm: &mut Vm,
    target: platform::Target,
    kind: &str,
    bubbles: bool,
) -> EngineResult<()> {
    vm.remaining = vm.limits.operations;
    let result = platform::fire(vm, target, kind, bubbles).and_then(|_| vm.drain_jobs());
    match result {
        Ok(()) => Ok(()),
        Err(Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(Thrown::Fatal(error)) => Err(error),
    }
}

/** Same-document traversal: the URL and `history.state` become the entry's, then `popstate`
carries that state, then `hashchange` for a fragment-only move. A state that no longer parses
reads as null rather than failing the traversal. */
fn pop_state(vm: &mut Vm, state: Option<&str>, hash_changed: bool) -> EngineResult<()> {
    vm.page.state.href = vm.page.environment.url.clone();
    let restored = state.and_then(|json| platform::json_value(vm, json));
    vm.page.state.history_state = restored.clone();
    vm.remaining = vm.limits.operations;
    let fired = platform::fire_pop_state(vm, restored.unwrap_or(JsValue::Null))
        .and_then(|_| vm.drain_jobs());
    report_thrown(vm, fired)?;
    if hash_changed {
        fire_lifecycle(vm, platform::Target::Window, "hashchange", false)?;
    }
    Ok(())
}

/** Fires `submit` at a person's form submission and records whether it may go ahead. A handler
that throws does not cancel it, as in a browser. */
fn submit_event(vm: &mut Vm, form: usize) -> EngineResult<()> {
    if !vm.page.exists(form) {
        return Err(EngineError::invalid("Invalid projected form."));
    }
    vm.remaining = vm.limits.operations;
    let fired = platform::fire_submit(vm, form);
    let proceed = !matches!(fired, Ok(false));
    let drained = fired.and_then(|_| vm.drain_jobs());
    report_thrown(vm, drained)?;
    if proceed {
        vm.page.effects.submit = Some(form);
    }
    Ok(())
}

/** A thrown page error is reported like an uncaught exception; only a fatal error stops. */
fn report_thrown(vm: &mut Vm, result: Result<(), Thrown>) -> EngineResult<()> {
    match result {
        Ok(()) => Ok(()),
        Err(Thrown::Value(error)) => {
            vm.report_error(&error);
            Ok(())
        }
        Err(Thrown::Fatal(error)) => Err(error),
    }
}

/// Resample measured viewport geometry and deliver only observer callbacks, without firing author timers.
pub fn observe_page(input: PageInput<'_>, state: VmState) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        platform::resize_observers::advance(&mut vm, true).map_err(fatal)?;
        finish(vm)
    })
}

/// Continue after actual host layout, or finalize at the host deadline without broadcasting again.
pub fn continue_resize_observers(
    input: PageInput<'_>,
    state: VmState,
    complete: bool,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        if complete {
            platform::resize_observers::complete(&mut vm).map_err(fatal)?;
        } else {
            platform::resize_observers::advance(&mut vm, false).map_err(fatal)?;
        }
        finish(vm)
    })
}

/// A person typed into a field the app already updated: V9 records the value (None for a
/// secret field, whose value never enters the engine) and fires trusted `input` and `change`
/// events at the node, so a form's script learns what was typed.
pub fn input_page(
    input: PageInput<'_>,
    state: VmState,
    target: usize,
    value: Option<String>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        if !vm.page.exists(target) {
            return Err(EngineError::invalid("Invalid projected input target."));
        }
        if vm.page.tag(target) == "select" {
            for option in vm.page.select_options(target) {
                vm.page.state.selected_options.remove(&option);
                vm.page.state.dirty_options.insert(option);
            }
        } else if let Some(value) = value {
            vm.page.state.values.insert(target, value);
        }
        vm.remaining = vm.limits.operations;
        platform::host_bridge::activate(&mut vm);
        platform::media::activate(&mut vm.page);
        if let Err(Thrown::Fatal(error)) = platform::focus(&mut vm, target) {
            return Err(error);
        }
        for kind in ["input", "change"] {
            fire_lifecycle(&mut vm, platform::Target::Node(target), kind, true)?;
        }
        finish(vm)
    })
}

/// Dispatch a trusted click at a projected node (bubbling to the document and window), then
/// drain microtasks. An unprevented click on a link records a navigation in the effects.
pub fn click_page(input: PageInput<'_>, state: VmState, target: usize) -> EngineResult<Execution> {
    on_v9_stack(|| click_page_with(input, state, target, Limits::default()))
}

/** click_page under explicit bounds. */
fn click_page_with(
    input: PageInput<'_>,
    state: VmState,
    target: usize,
    limits: Limits,
) -> EngineResult<Execution> {
    let (mut vm, _) = open(&input, state, limits)?;
    if !vm.page.exists(target) {
        return Err(EngineError::invalid("Invalid projected click target."));
    }
    vm.remaining = vm.limits.operations;
    platform::host_bridge::activate(&mut vm);
    platform::media::activate(&mut vm.page);
    // A press focuses what it lands on before the click, as a pointer's mousedown does.
    if let Err(Thrown::Fatal(error)) = platform::focus(&mut vm, target) {
        return Err(error);
    }
    let fired = platform::fire(&mut vm, platform::Target::Node(target), "click", true);
    let proceed = !matches!(fired, Ok(false));
    let drained = fired.map(|_| ()).and_then(|_| vm.drain_jobs());
    report_thrown(&mut vm, drained)?;
    if proceed {
        platform::follow_link(&mut vm, target);
    }
    finish(vm)
}

/// Advance the virtual clock to `now_ms`, running due timers and animation frames (earliest
/// first, at most `budget` callbacks, each followed by its microtasks). `effects.next_timer_ms`
/// says when to call again.
pub fn run_timers(
    input: PageInput<'_>,
    state: VmState,
    now_ms: f64,
    budget: usize,
) -> EngineResult<Execution> {
    on_v9_stack(|| run_timers_with(input, state, now_ms, budget, Limits::default()))
}

/** run_timers under explicit bounds. */
fn run_timers_with(
    input: PageInput<'_>,
    state: VmState,
    now_ms: f64,
    budget: usize,
    limits: Limits,
) -> EngineResult<Execution> {
    let (mut vm, _) = open(&input, state, limits)?;
    if !now_ms.is_finite() {
        return Err(EngineError::invalid("Invalid V9 timer clock."));
    }
    vm.remaining = vm.limits.operations;
    platform::intersection_observers::deliver_all(&mut vm).map_err(fatal)?;
    vm.run_due_timers(now_ms, budget).map_err(fatal)?;
    finish(vm)
}

/// Hand back responses to `effects.requests`; fetch promises settle and XMLHttpRequests fire.
pub fn resume_requests(
    input: PageInput<'_>,
    state: VmState,
    responses: Vec<HostResponse>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        vm.remaining = vm.limits.operations;
        vm.deliver(responses).map_err(fatal)?;
        finish(vm)
    })
}

/// Deliver actual host image fetch/decode outcomes in a later task, ignoring stale source generations.
pub fn resume_images(
    input: PageInput<'_>,
    state: VmState,
    responses: Vec<ImageResponse>,
) -> EngineResult<Execution> {
    on_v9_stack(|| {
        let (mut vm, _) = open(&input, state, Limits::default())?;
        platform::images::deliver(&mut vm, responses)?;
        finish(vm)
    })
}

/** The default environment for entry points that predate PageEnvironment. */
fn default_environment() -> PageEnvironment {
    PageEnvironment::default()
}

/** Load projected scripts as one all-or-nothing transaction: any script's failure fails the call. */
pub fn load_document_scripts(
    sources: &[String],
    document: ScriptDocument,
    storage: StorageState,
) -> EngineResult<Execution> {
    let environment = default_environment();
    let host = DocumentHost::default();
    on_v9_stack(|| {
        let (mut vm, compiled) = session(
            Limits::default(),
            sources,
            &[],
            false,
            Some(document),
            storage,
            &host,
            &environment,
            None,
            NO_MODULES,
        )?;
        let Compiled::All(codes) = compiled else {
            return Err(EngineError::invalid(
                "V9 compiled scripts in the wrong mode.",
            ));
        };
        for (index, code) in codes.into_iter().enumerate() {
            run_one(&mut vm, code, index)?;
        }
        finish(vm)
    })
}

/** The page-load entry point the V9 service uses: isolated, with the default environment. */
pub fn load_document_scripts_with(
    sources: &[String],
    document: ScriptDocument,
    storage: StorageState,
    host: &DocumentHost,
) -> EngineResult<Execution> {
    load_document_scripts_isolated(sources, document, storage, host)
}

/** Load projected scripts one at a time, rolling back each failing script alone. */
pub fn load_document_scripts_isolated(
    sources: &[String],
    document: ScriptDocument,
    storage: StorageState,
    host: &DocumentHost,
) -> EngineResult<Execution> {
    let environment = default_environment();
    load_page(PageInput {
        sources,
        source_keys: &[],
        document,
        storage,
        host,
        environment: &environment,
        modules: NO_MODULES,
    })
}

/** Dispatch one projected click with the default host. */
pub fn click_document_scripts(
    sources: &[String],
    target: usize,
    state: VmState,
    document: ScriptDocument,
    storage: StorageState,
) -> EngineResult<Execution> {
    click_document_scripts_with(
        sources,
        target,
        state,
        document,
        storage,
        &DocumentHost::default(),
    )
}

/** Dispatch one projected click with the same host facts and fragment seam as page load. */
pub fn click_document_scripts_with(
    sources: &[String],
    target: usize,
    state: VmState,
    document: ScriptDocument,
    storage: StorageState,
    host: &DocumentHost,
) -> EngineResult<Execution> {
    let environment = default_environment();
    click_page(
        PageInput {
            sources,
            source_keys: &[],
            document,
            storage,
            host,
            environment: &environment,
            modules: NO_MODULES,
        },
        state,
        target,
    )
}

/** A synthetic projection for legacy callers that address fields by id. */
fn legacy_document(inputs: &[InputValue]) -> EngineResult<ScriptDocument> {
    let mut nodes = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        if input.value.len() > TEXT_BYTES {
            return Err(EngineError::limit("V9 DOM input UTF-8 bytes", TEXT_BYTES));
        }
        if nodes
            .iter()
            .any(|node: &ScriptNode| node.html_id == input.html_id)
        {
            continue;
        }
        nodes.push(ScriptNode {
            selected: None,
            local_id: index,
            html_id: input.html_id.clone(),
            parent: None,
            value: Some(input.value.clone()),
            text_writable: true,
            click_listener: false,
            inline_handler: None,
            tag: "input".into(),
            attributes: [("id".to_string(), input.html_id.clone())]
                .into_iter()
                .collect(),
        });
    }
    for (index, node) in nodes.iter_mut().enumerate() {
        node.local_id = index;
    }
    Ok(ScriptDocument {
        identity: "legacy".into(),
        charset: "UTF-8".into(),
        nodes,
    })
}

/** Run legacy scripts (and optionally a handler) all-or-nothing. */
fn legacy(
    sources: &[String],
    handler: Option<&str>,
    state: Option<VmState>,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    let environment = default_environment();
    let host = DocumentHost::default();
    let document = legacy_document(inputs)?;
    on_v9_stack(|| {
        let (mut vm, compiled) = session(
            Limits::default(),
            sources,
            &[],
            false,
            Some(document),
            storage,
            &host,
            &environment,
            state,
            NO_MODULES,
        )?;
        vm.page.legacy = true;
        let Compiled::All(codes) = compiled else {
            return Err(EngineError::invalid(
                "V9 compiled scripts in the wrong mode.",
            ));
        };
        match handler {
            None => {
                for (index, code) in codes.into_iter().enumerate() {
                    run_one(&mut vm, code, index)?;
                }
            }
            Some(handler) => {
                let (code, base) = legacy_handlers::compile(&mut vm, handler)?;
                run_one(&mut vm, code, 0)
                    .map_err(|error| legacy_handlers::location(error, base))?;
            }
        }
        finish(vm)
    })
}

/// Compatibility entry point for source-only callers: the text changes a script proposes.
pub fn execute(source: &str) -> EngineResult<Vec<TextChange>> {
    Ok(load(source, &[])?
        .changes
        .into_iter()
        .filter_map(|change| match change {
            DomChange::Text { html_id, text } => Some(TextChange { html_id, text }),
            _ => None,
        })
        .collect())
}

/// Load source once against field values and return its state.
pub fn load(source: &str, inputs: &[InputValue]) -> EngineResult<Execution> {
    legacy(
        &[source.to_string()],
        None,
        None,
        inputs,
        StorageState::default(),
    )
}

/// Load ordered classic scripts with shared state.
pub fn load_scripts(sources: &[String], inputs: &[InputValue]) -> EngineResult<Execution> {
    load_scripts_with_storage(sources, inputs, StorageState::default())
}

/** Load scripts against coordinator-owned storage candidates. */
pub fn load_scripts_with_storage(
    sources: &[String],
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    legacy(sources, None, None, inputs, storage)
}

/// Run a handler (such as `run()`) against state produced by the same source.
pub fn event(
    source: &str,
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    event_scripts(&[source.to_string()], handler, state, inputs)
}

/// Run a handler against state produced by the same ordered scripts.
pub fn event_scripts(
    sources: &[String],
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
) -> EngineResult<Execution> {
    event_scripts_with_storage(sources, handler, state, inputs, StorageState::default())
}

/** Run a handler against storage copies supplied by the coordinator. */
pub fn event_scripts_with_storage(
    sources: &[String],
    handler: &str,
    state: VmState,
    inputs: &[InputValue],
    storage: StorageState,
) -> EngineResult<Execution> {
    legacy(sources, Some(handler), Some(state), inputs, storage)
}

impl Vm<'_> {
    /** Run a compiled top-level script with the global object as `this`. */
    pub(in crate::v9) fn run_script(&mut self, code: Arc<Code>) -> Result<JsValue, Thrown> {
        let regs = vec![JsValue::Undefined; code.registers as usize];
        self.frames.push(vm::Frame {
            realm: self.active_realm,
            code,
            pc: 0,
            stack: Vec::new(),
            regs,
            env: None,
            this: JsValue::Object(self.global_object()),
            callee: JsValue::Undefined,
            args: Vec::new(),
            new_target: JsValue::Undefined,
            handlers: Vec::new(),
            envs: 0,
            boundary: true,
            caller_hidden: false,
            construct: false,
            resume: vm::Resume::Next,
        });
        let previous_realm = self.active_realm;
        let pinning = std::mem::replace(&mut self.pinning, false);
        self.native_depth += 1;
        let result = self.run();
        self.native_depth -= 1;
        self.pinning = pinning;
        self.select_realm(previous_realm)?;
        match result? {
            vm::Exit::Return(value) => Ok(value),
            _ => Err(EngineError::invalid("V9 script suspended at the top level.").into()),
        }
    }
}

/** Measurement-only entry points (feature `measure`); the service build never contains them. */
#[cfg(feature = "measure")]
pub mod measure {
    use super::*;
    use std::sync::Mutex;

    /// Objects, functions, properties, string units, environments, bindings and state bytes.
    pub type Counts = (usize, usize, usize, usize, usize, usize, usize);

    /** The live counts of the last measured load's committed state. */
    static LAST: Mutex<Option<Counts>> = Mutex::new(None);

    /** Record the committed state's live counts and bytes. */
    pub(super) fn record(state: &VmState) {
        let counts = state.heap.counts;
        let bytes = serde_json::to_vec(state).map_or(0, |bytes| bytes.len());
        if let Ok(mut last) = LAST.lock() {
            *last = Some((
                counts.objects,
                counts.functions,
                counts.properties,
                counts.units,
                counts.environments,
                counts.bindings,
                bytes,
            ));
        }
    }

    /// Live objects, functions, properties, string units, environments, bindings and state
    /// bytes after the last measured load, as the bounds would count them.
    pub fn last_counts() -> Option<Counts> {
        LAST.lock().ok().and_then(|last| *last)
    }

    /** Every heap bound multiplied by `scale`. */
    fn scaled(scale: usize) -> Limits {
        let base = Limits::default();
        Limits {
            objects: base.objects * scale,
            functions: base.functions * scale,
            properties: base.properties * scale,
            units: base.units * scale,
            environments: base.environments * scale,
            bindings: base.bindings * scale,
            array_length: base.array_length * scale,
            state_bytes: base.state_bytes * scale,
            ..base
        }
    }

    /// Load with every heap bound multiplied by `scale`, to find what a script needs.
    pub fn load_page_scaled(input: PageInput<'_>, scale: usize) -> EngineResult<Execution> {
        on_v9_stack(|| load_isolated(input, None, scaled(scale), false))
    }

    /// `page_event` with scaled bounds.
    pub fn page_event_scaled(
        input: PageInput<'_>,
        state: VmState,
        event: PageEvent,
        scale: usize,
    ) -> EngineResult<Execution> {
        on_v9_stack(|| page_event_with(input, state, event, scaled(scale)))
    }

    /// `click_page` with scaled bounds.
    pub fn click_page_scaled(
        input: PageInput<'_>,
        state: VmState,
        target: usize,
        scale: usize,
    ) -> EngineResult<Execution> {
        on_v9_stack(|| click_page_with(input, state, target, scaled(scale)))
    }

    /// `run_timers` with scaled bounds.
    pub fn run_timers_scaled(
        input: PageInput<'_>,
        state: VmState,
        now_ms: f64,
        budget: usize,
        scale: usize,
    ) -> EngineResult<Execution> {
        on_v9_stack(|| run_timers_with(input, state, now_ms, budget, scaled(scale)))
    }
}

/// Replay an execution's changes onto a projection, as the app's document engine does, for
/// callers that run V9 without Eye (tests, tools). Text that V9's own fragment reader sees is
/// added to `node_text` so later requests can read it.
pub fn replay(
    document: &mut ScriptDocument,
    execution: &Execution,
    fragments: &dyn FragmentParser,
    node_text: &mut BTreeMap<usize, String>,
) -> EngineResult<()> {
    let changes = execution
        .changes
        .iter()
        .cloned()
        .map(|change| ScriptChange::Dom { change })
        .chain(execution.script_changes.iter().cloned());
    for change in changes {
        match change {
            // Definedness is a document-engine fact for `:defined`; the projection has no field.
            ScriptChange::Defined { .. } | ScriptChange::Canvas { .. } => {}
            // The virtual html and body hold their attributes in V9's own state already.
            ScriptChange::RootAttribute { .. } | ScriptChange::Focus { .. } => {}
            ScriptChange::Dom { change } => match change {
                DomChange::Text { html_id, text } => {
                    let node = document
                        .get_by_id(&html_id)
                        .ok_or_else(|| EngineError::invalid("Unknown text target."))?;
                    document.set_text(node, &text)?;
                    node_text.insert(node, text);
                }
                DomChange::InsertHtml {
                    target,
                    position,
                    html,
                    first_id,
                } => {
                    let context = target
                        .and_then(|target| {
                            document.nodes.iter().find(|node| node.local_id == target)
                        })
                        .map_or_else(|| "body".to_string(), |node| node.tag.clone());
                    let nodes = fragments.parse(&context, &html, first_id)?;
                    for (id, text) in platform::html::fragment_texts(&html, first_id)? {
                        node_text.insert(id, text);
                    }
                    document.insert(target, position, nodes)?;
                }
                DomChange::Move {
                    node,
                    target,
                    position,
                } => {
                    document.relocate(node, target, position)?;
                }
                DomChange::SetAttribute { node, name, value } => {
                    document.set_attribute(node, &name, &value)?;
                }
            },
            ScriptChange::Remove { node } => {
                document.remove(node)?;
            }
            ScriptChange::Attribute { node, name, value } => {
                document.set_any_attribute(node, &name, value.as_deref())?;
            }
            ScriptChange::NodeText { node, text } => {
                document.set_node_text(node, &text)?;
                node_text.insert(node, text);
            }
            ScriptChange::Value { .. } | ScriptChange::Selection { .. } => {}
        }
    }
    if document.digest() != execution.structure {
        return Err(EngineError::invalid(
            "Replayed projection diverged from V9's structure digest.",
        ));
    }
    Ok(())
}

/** Keep the unused host data import referenced for documentation of the wrapper model. */
#[allow(dead_code)]
fn _host(_: HostData, _: Class, _: SourceLocation) {}
