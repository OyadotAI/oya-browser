/** Worker graph encode keeps all references within the current message packet. */
use super::*;

/** Serialize before any transfer detaches an object, so clone refusal leaves the sender intact. */
pub(in crate::v9) fn encode(vm: &mut Vm, value: &JsValue) -> Res<WorkerData> {
    let mut graph = WorkerData {
        root: Atom::Value(Primitive::Undefined),
        nodes: Vec::new(),
    };
    graph.root = atom(vm, value, &mut graph, &mut BTreeMap::new(), &mut 0, 0)?;
    graph.bytes()?;
    Ok(graph)
}

/** Unsupported values produce the standard language-visible clone failure. */
pub(super) fn refused(vm: &mut Vm) -> crate::v9::vm::Thrown {
    vm.dom_exception("DataCloneError", "The value cannot be cloned for a Worker")
}

/** Memoization precedes recursive property reads, retaining cycles and shared buffer identity. */
fn atom(
    vm: &mut Vm,
    value: &JsValue,
    graph: &mut WorkerData,
    seen: &mut BTreeMap<ObjectId, usize>,
    bytes: &mut usize,
    depth: usize,
) -> Res<Atom> {
    vm.charge(1)?;
    let units = match value {
        JsValue::String(text) => text.len(),
        JsValue::BigIntWide(value) => value.units(),
        _ => 1,
    };
    *bytes = bytes
        .saturating_add(units.saturating_mul(2))
        .saturating_add(1);
    if depth > DEPTH || *bytes > WORKER_MESSAGE_BYTES {
        return Err(refused(vm));
    }
    if let Some(value) = primitive(value) {
        return Ok(Atom::Value(value));
    }
    let Some(id) = value.object() else {
        return Err(refused(vm));
    };
    if let Some(index) = seen.get(&id) {
        return Ok(Atom::Reference(*index));
    }
    if vm.is_callable(value)
        || crate::v9::platform::channel::is_port(vm, value)
        || graph.nodes.len() >= NODES
        || vm
            .page
            .state
            .workers
            .values()
            .any(|handle| handle.object == id)
    {
        return Err(refused(vm));
    }
    let index = graph.nodes.len();
    seen.insert(id, index);
    graph.nodes.push(Node {
        kind: Kind::Plain,
        fields: Vec::new(),
    });
    let class = vm
        .heap
        .get(id)
        .map(|object| object.class.clone())
        .ok_or_else(|| refused(vm))?;
    let mut nested = |vm: &mut Vm, item: &JsValue, graph: &mut WorkerData| {
        atom(vm, item, graph, seen, bytes, depth + 1)
    };
    if let Some((kind, fields)) = super::kinds::encode(vm, value, &class, graph, &mut nested)? {
        graph.nodes[index] = Node { kind, fields };
        return Ok(Atom::Reference(index));
    }
    let kind = match class {
        Class::Plain => {
            if vm.heap.get(id).is_some_and(|object| {
                object
                    .props
                    .iter()
                    .any(|(key, _)| matches!(key, Key::Private(_)))
            }) {
                return Err(refused(vm));
            }
            Kind::Plain
        }
        Class::Array(elements) => Kind::Array(elements.len() as u32),
        Class::ArrayBuffer(data) => {
            if crate::v9::builtins::typed::buffer_state_detached(vm, id) {
                return Err(refused(vm));
            }
            *bytes = bytes.saturating_add(data.0.len());
            if *bytes > WORKER_MESSAGE_BYTES {
                return Err(refused(vm));
            }
            Kind::Buffer(data.0)
        }
        Class::View(_) => {
            let view = crate::v9::builtins::typed::view_of(vm, id)
                .filter(|view| !view.invalid)
                .ok_or_else(|| refused(vm))?;
            let Atom::Reference(buffer) = atom(
                vm,
                &JsValue::Object(view.buffer),
                graph,
                seen,
                bytes,
                depth + 1,
            )?
            else {
                return Err(refused(vm));
            };
            Kind::View(view.kind, buffer, view.offset, view.length)
        }
        _ => return Err(refused(vm)),
    };
    let mut fields = Vec::new();
    if matches!(kind, Kind::Plain | Kind::Array(_)) {
        for key in vm.enumerable_keys(id)? {
            let Key::Str(name) = key else {
                continue;
            };
            *bytes = bytes.saturating_add(name.len().saturating_mul(2));
            let value = vm.get(value, &Key::Str(name.clone()))?;
            fields.push((name, atom(vm, &value, graph, seen, bytes, depth + 1)?));
        }
    }
    graph.nodes[index] = Node { kind, fields };
    Ok(Atom::Reference(index))
}
