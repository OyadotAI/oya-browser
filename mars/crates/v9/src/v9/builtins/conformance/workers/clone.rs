/** Worker messages are bounded typed object graphs, independent of either isolate's object identities. */
use crate::v9::builtins::key;
use crate::v9::value::{JsString, JsValue, ObjectId};
use crate::v9::vm::data::{Bytes, Class, ElementKind, SharedBytes, Table, ViewData};
use crate::v9::vm::heap::{Key, Object};
use crate::v9::vm::property::Descriptor;
use crate::v9::vm::{Res, Vm};
use std::collections::BTreeMap;
mod wire;
pub(in crate::v9::builtins::conformance) use wire::{deserialize, serialize};

/** Message storage is bounded separately from per-isolate heap quotas. */
const MESSAGE_UNITS: usize = 1_048_576;
/** A graph packet preserves references without retaining objects in the sending VM. */
#[derive(Clone, Debug)]
pub(in crate::v9) struct Graph {
    root: Atom,
    nodes: Vec<Node>,
    /** Registered transfers may contain an already detached empty backing. */
    allowed_detached: Vec<ObjectId>,
    /** Worker delegates can share actual native capabilities; the standalone shell serializer cannot. */
    worker: bool,
}
/** Primitives retain exact Number, UTF-16 and BigInt values; references use packet-local indices. */
#[derive(Clone, Debug)]
enum Atom {
    Primitive(JsValue),
    Reference(usize),
}
/** Only structured-clone-supported data classes cross an isolate boundary. */
#[derive(Clone, Debug)]
enum Kind {
    Plain,
    Array(usize),
    Date(f64),
    RegExp(JsString, String),
    Map(Vec<(Atom, Atom)>),
    Set(Vec<Atom>),
    Wrapper(JsValue),
    Error(String, Vec<(String, Atom)>),
    Buffer(Vec<u8>, Option<usize>, bool),
    Shared(SharedBytes),
    SharedGc(crate::v9::vm::data::SharedGc),
    WasmModule(crate::v9::builtins::WasmModulePacket),
    WasmMemory(crate::v9::builtins::WasmMemoryPacket),
    View(ElementKind, Atom, usize, usize, bool),
}
/** Enumeration order is preserved and accessors have already become data through real Gets. */
#[derive(Clone, Debug)]
struct Node {
    kind: Kind,
    fields: Vec<(Key, Atom)>,
}
/** Serialization reads enumerable string keys and never transfers callable/native authority. */
pub(super) fn encode(vm: &mut Vm, value: &JsValue, transfers: &[ObjectId]) -> Res<Graph> {
    let mut graph = Graph {
        root: Atom::Primitive(JsValue::Undefined),
        nodes: Vec::new(),
        allowed_detached: transfers.to_vec(),
        worker: true,
    };
    let root = atom(vm, value, &mut graph, &mut BTreeMap::new(), &mut 0, 0)?;
    graph.root = root;
    graph.allowed_detached.clear();
    Ok(graph)
}
/** Refused values throw a language DataCloneError, rather than silently dropping fields. */
fn refused(vm: &mut Vm, worker: bool) -> crate::v9::vm::Thrown {
    if !worker {
        return match vm.make_error(
            crate::v9::vm::error::ErrorKind::Error,
            "The value could not be cloned.",
        ) {
            Ok(id) => crate::v9::vm::Thrown::Value(JsValue::Object(id)),
            Err(error) => error.into(),
        };
    }
    vm.dom_exception(
        "DataCloneError",
        "The value could not be cloned for a Worker.",
    )
}
/** Both cyclic references and shared buffers are memoized before recursively reading children. */
fn atom(
    vm: &mut Vm,
    value: &JsValue,
    graph: &mut Graph,
    seen: &mut BTreeMap<ObjectId, usize>,
    units: &mut usize,
    depth: usize,
) -> Res<Atom> {
    vm.charge(1)?;
    if depth >= vm.limits.frames {
        return Err(vm.range_error("Maximum call stack size exceeded"));
    }
    *units += 1;
    if let JsValue::String(text) = value {
        *units += text.as_units().len();
    }
    if let JsValue::BigIntWide(value) = value {
        *units += value.words().len() * 2;
    }
    if *units > MESSAGE_UNITS {
        return Err(storage_error(vm));
    }
    let id = match value {
        JsValue::Symbol(_) | JsValue::Hole => return Err(refused(vm, graph.worker)),
        JsValue::Object(id) => *id,
        value => return Ok(Atom::Primitive(value.clone())),
    };
    if let Some(index) = seen.get(&id) {
        return Ok(Atom::Reference(*index));
    }
    if vm.is_callable(value) {
        return Err(refused(vm, graph.worker));
    }
    if crate::v9::builtins::is_raw_json(vm, value) {
        return Err(refused(vm, graph.worker));
    }
    if crate::v9::builtins::conformance_host_object(vm, value) {
        return Err(refused(vm, graph.worker));
    }
    let Some(class) = vm.heap.get(id).map(|object| object.class.clone()) else {
        return Err(refused(vm, graph.worker));
    };
    let index = graph.nodes.len();
    seen.insert(id, index);
    graph.nodes.push(Node {
        kind: Kind::Plain,
        fields: Vec::new(),
    });
    if !graph.worker
        && (matches!(class, Class::SharedArrayBuffer(_) | Class::WasmShared(_))
            || crate::v9::builtins::wasm_worker_opaque(vm, value))
    {
        return Err(refused(vm, false));
    }
    let kind = if let Some(packet) = crate::v9::builtins::wasm_worker_module(vm, value) {
        *units += packet.bytes.len()
            + packet.builtins.iter().map(String::len).sum::<usize>()
            + packet.string_constants.as_ref().map_or(0, Vec::len);
        if *units > MESSAGE_UNITS {
            return Err(storage_error(vm));
        }
        Kind::WasmModule(packet)
    } else if let Some(packet) = crate::v9::builtins::wasm_worker_memory(vm, value) {
        Kind::WasmMemory(packet)
    } else if crate::v9::builtins::wasm_worker_opaque(vm, value) {
        return Err(refused(vm, graph.worker));
    } else {
        match class {
            Class::Plain => Kind::Plain,
            Class::Array(values) => Kind::Array(values.len()),
            Class::Date(value) => Kind::Date(value),
            Class::RegExp(data) => Kind::RegExp(data.source, data.flags),
            Class::Boolean(value) => Kind::Wrapper(JsValue::Boolean(value)),
            Class::Number(value) => Kind::Wrapper(JsValue::Number(value)),
            Class::String(value) => Kind::Wrapper(JsValue::String(value)),
            Class::BigInt(value) => Kind::Wrapper(JsValue::BigInt(value)),
            Class::BigIntWide(value) => Kind::Wrapper(JsValue::BigIntWide(value)),
            Class::Map(table) => {
                let mut entries = Vec::new();
                for (key, value) in table.entries.into_iter().flatten() {
                    entries.push((
                        atom(vm, &key, graph, seen, units, depth + 1)?,
                        atom(vm, &value, graph, seen, units, depth + 1)?,
                    ));
                }
                Kind::Map(entries)
            }
            Class::Set(table) => {
                let mut entries = Vec::new();
                for (key, _) in table.entries.into_iter().flatten() {
                    entries.push(atom(vm, &key, graph, seen, units, depth + 1)?);
                }
                Kind::Set(entries)
            }
            Class::Error => {
                let name = vm.get(value, &key("name"))?;
                let name = vm.to_string(&name)?.lossy();
                let mut fields = Vec::new();
                for name in ["message", "stack", "cause"] {
                    let descriptor = vm.own(id, &key(name))?;
                    let field = if name == "stack" {
                        let field = vm.get(value, &key(name))?;
                        if !matches!(field, JsValue::String(_)) {
                            continue;
                        }
                        field
                    } else {
                        let Some(field) = descriptor.and_then(|descriptor| match descriptor.slot {
                            crate::v9::vm::heap::Slot::Data(value) => Some(value),
                            _ => None,
                        }) else {
                            continue;
                        };
                        if name == "message" {
                            JsValue::String(vm.to_string(&field)?)
                        } else {
                            field
                        }
                    };
                    fields.push((
                        name.into(),
                        atom(vm, &field, graph, seen, units, depth + 1)?,
                    ));
                }
                Kind::Error(name, fields)
            }
            Class::ArrayBuffer(bytes) => {
                let detached = crate::v9::builtins::buffer_detached(vm, id);
                if detached && !graph.allowed_detached.contains(&id) {
                    return Err(refused(vm, graph.worker));
                }
                *units += bytes.0.len();
                if *units > MESSAGE_UNITS {
                    return Err(storage_error(vm));
                }
                Kind::Buffer(
                    bytes.0,
                    if detached {
                        None
                    } else {
                        crate::v9::builtins::buffer_maximum(vm, id)
                    },
                    crate::v9::builtins::buffer_immutable(vm, id),
                )
            }
            Class::SharedArrayBuffer(buffer) => Kind::Shared(buffer),
            Class::WasmShared(value) => Kind::SharedGc(value),
            Class::View(_) => {
                let view = crate::v9::builtins::view_of(vm, id)
                    .expect("View class has effective metadata");
                if view.invalid {
                    return Err(refused(vm, graph.worker));
                }
                let buffer = atom(
                    vm,
                    &JsValue::Object(view.buffer),
                    graph,
                    seen,
                    units,
                    depth + 1,
                )?;
                Kind::View(view.kind, buffer, view.offset, view.length, view.tracking)
            }
            _ => return Err(refused(vm, graph.worker)),
        }
    };
    let mut fields = Vec::new();
    if matches!(kind, Kind::Plain | Kind::Array(_)) {
        for key in vm.enumerable_keys(id)? {
            if !matches!(key, Key::Str(_)) {
                continue;
            }
            if vm
                .own(id, &key)?
                .is_none_or(|property| property.attrs & crate::v9::vm::heap::ENUMERABLE == 0)
            {
                continue;
            }
            if let Key::Str(name) = &key {
                *units += name.as_units().len();
            }
            if *units > MESSAGE_UNITS {
                return Err(storage_error(vm));
            }
            let value = vm.get(value, &key)?;
            fields.push((key, atom(vm, &value, graph, seen, units, depth + 1)?));
        }
    }
    graph.nodes[index] = Node { kind, fields };
    Ok(Atom::Reference(index))
}
/** Allocate every shell before adding edges, preserving cycles and repeated references exactly. */
pub(in crate::v9) fn decode(vm: &mut Vm, graph: Graph) -> Res<JsValue> {
    let mut objects = Vec::new();
    for node in &graph.nodes {
        let id = shell(vm, &node.kind)?;
        vm.pin(JsValue::Object(id));
        objects.push(id);
    }
    for (index, node) in graph.nodes.into_iter().enumerate() {
        fill(vm, objects[index], node, &objects)?;
    }
    vm.check_heap_bounds()?;
    Ok(resolve(graph.root, &objects))
}
/** References can only name indices made by the trusted encoder in this process. */
fn resolve(atom: Atom, objects: &[ObjectId]) -> JsValue {
    match atom {
        Atom::Primitive(value) => value,
        Atom::Reference(index) => JsValue::Object(objects[index]),
    }
}
/** Intrinsic prototypes belong to the receiving realm; author prototypes never cross. */
fn shell(vm: &mut Vm, kind: &Kind) -> Res<ObjectId> {
    let ids = &crate::v9::vm::realm::realm().ids;
    match kind {
        Kind::Array(length) => vm.new_array_length(*length),
        Kind::Shared(buffer) => crate::v9::builtins::import_shared_buffer(vm, buffer.clone()),
        Kind::SharedGc(value) => crate::v9::builtins::wasm_worker_restore_shared_gc(vm, value)
            .map(|value| value.object().expect("Shared GC restore returns an object")),
        Kind::WasmMemory(packet) => crate::v9::builtins::wasm_worker_restore_memory(vm, packet)
            .map(|value| value.object().expect("Memory restore returns an object")),
        Kind::WasmModule(packet) => crate::v9::builtins::wasm_worker_restore(vm, packet)
            .map(|value| value.object().expect("Module restore returns an object")),
        Kind::RegExp(source, flags) => vm.new_regexp(source.clone(), flags),
        Kind::Date(value) => vm.alloc(Object::new(Class::Date(*value), Some(ids.date_prototype))),
        Kind::Wrapper(value) => vm.to_object(value),
        Kind::Map(_) => vm.alloc(Object::new(
            Class::Map(Box::<Table>::default()),
            Some(ids.map_prototype),
        )),
        Kind::Set(_) => vm.alloc(Object::new(
            Class::Set(Box::<Table>::default()),
            Some(ids.set_prototype),
        )),
        Kind::Buffer(bytes, _, _) => vm.alloc(Object::new(
            Class::ArrayBuffer(Bytes(bytes.clone())),
            Some(ids.array_buffer_prototype),
        )),
        Kind::Error(name, _) => {
            let prototype = match name.as_str() {
                "TypeError" => ids.type_error_prototype,
                "RangeError" => ids.range_error_prototype,
                "ReferenceError" => ids.reference_error_prototype,
                "SyntaxError" => ids.syntax_error_prototype,
                "EvalError" => ids.eval_error_prototype,
                "URIError" => ids.uri_error_prototype,
                _ => ids.error_prototype,
            };
            vm.alloc(Object::new(Class::Error, Some(prototype)))
        }
        _ => vm.new_object(),
    }
}
/** Install cloned data directly; this cannot invoke setters installed by the receiving script. */
fn define(vm: &mut Vm, id: ObjectId, key: Key, value: JsValue) -> Res<()> {
    if !vm.define(id, key, Descriptor::data(value))? {
        return Err(vm.type_error("Cannot decode Worker message"));
    }
    Ok(())
}
/** Map, Set and view edges are filled only after every referred-to shell exists. */
fn fill(vm: &mut Vm, id: ObjectId, node: Node, objects: &[ObjectId]) -> Res<()> {
    match node.kind {
        Kind::Map(entries) => {
            let mut table = Table::default();
            for (key, value) in entries {
                table.set(resolve(key, objects), resolve(value, objects));
            }
            replace_class(vm, id, Class::Map(Box::new(table)));
        }
        Kind::Set(entries) => {
            let mut table = Table::default();
            for entry in entries {
                let value = resolve(entry, objects);
                table.set(value.clone(), value);
            }
            replace_class(vm, id, Class::Set(Box::new(table)));
        }
        Kind::View(kind, buffer, offset, length, tracking) => {
            let buffer = resolve(buffer, objects).object().unwrap();
            vm.heap.get_mut(id).unwrap().class = Class::View(Box::new(ViewData {
                kind,
                buffer,
                offset,
                length,
                tracking,
                invalid: false,
            }));
            vm.set_proto(
                id,
                Some(crate::v9::vm::realm::realm().ids.view_prototypes[kind as usize]),
            )?;
        }
        Kind::Buffer(_, maximum, immutable) => {
            if let Some(maximum) = maximum {
                crate::v9::builtins::set_buffer_maximum(vm, id, maximum);
            }
            if immutable {
                crate::v9::builtins::make_buffer_immutable(vm, id);
            }
        }
        Kind::Error(_, fields) => {
            for (name, value) in fields {
                vm.define(
                    id,
                    key(&name),
                    Descriptor {
                        enumerable: Some(false),
                        ..Descriptor::data(resolve(value, objects))
                    },
                )?;
            }
        }
        _ => {}
    }
    for (key, value) in node.fields {
        define(vm, id, key, resolve(value, objects))?;
    }
    Ok(())
}

/** Filling an initially empty Map/Set adjusts only that object's live counts, avoiding a whole-heap recount. */
fn replace_class(vm: &mut Vm, id: ObjectId, class: Class) {
    let object = vm.heap.get_mut(id).unwrap();
    let previous = (object.class.element_count(), object.class.units());
    let next = (class.element_count(), class.units());
    object.class = class;
    vm.heap.adjust(
        next.0 as isize - previous.0 as isize,
        next.1 as isize - previous.1 as isize,
    );
}

/** D8's serializer reports storage exhaustion as a cloning failure before committing transfers. */
fn storage_error(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception(
        "DataCloneError",
        "Data cannot be cloned: Worker message storage limit exceeded.",
    )
}
