/** Worker graph decode keeps all references within the current message packet. */
use super::*;

/** Allocate every node before wiring references, so aliases and cycles never require recursion. */
pub(in crate::v9) fn decode(vm: &mut Vm, graph: &WorkerData) -> Res<JsValue> {
    graph.bytes()?;
    let mut objects = Vec::new();
    for node in &graph.nodes {
        let id = match &node.kind {
            Kind::Array(_) => vm.new_array(Vec::new())?,
            Kind::Buffer(bytes) => vm.alloc(Object::new(
                Class::ArrayBuffer(Bytes(bytes.clone())),
                Some(realm().ids.array_buffer_prototype),
            ))?,
            _ => vm.new_object()?,
        };
        vm.pin(JsValue::Object(id));
        objects.push(id);
    }
    for (index, node) in graph.nodes.iter().enumerate() {
        let id = objects[index];
        match node.kind {
            Kind::Array(length) => {
                vm.set(
                    &JsValue::Object(id),
                    Key::length(),
                    JsValue::Number(length as f64),
                    true,
                )?;
            }
            Kind::View(kind, buffer, offset, length) => {
                vm.heap.get_mut(id).expect("allocated worker node").class =
                    Class::View(Box::new(ViewData {
                        kind,
                        buffer: objects[buffer],
                        offset,
                        length,
                        tracking: false,
                        invalid: false,
                    }));
                vm.set_proto(id, Some(realm().ids.view_prototypes[kind as usize]))?;
            }
            _ => {}
        }
        for (key, value) in &node.fields {
            // Define data properties directly: '__proto__' is data, not a prototype setter.
            vm.define(
                id,
                Key::Str(key.clone()),
                crate::v9::vm::property::Descriptor::data(resolve(value, &objects)),
            )?;
        }
    }
    Ok(resolve(&graph.root, &objects))
}

/** The checked packet table is the only namespace a reference can address. */
fn resolve(value: &Atom, objects: &[ObjectId]) -> JsValue {
    match value {
        Atom::Value(value) => match value {
            Primitive::Undefined => JsValue::Undefined,
            Primitive::Null => JsValue::Null,
            Primitive::Boolean(value) => JsValue::Boolean(*value),
            Primitive::Number(bits) => JsValue::Number(f64::from_bits(*bits)),
            Primitive::String(value) => JsValue::String(value.clone()),
            Primitive::BigInt(value) => JsValue::bigint(value.clone()),
        },
        Atom::Reference(index) => JsValue::Object(objects[*index]),
    }
}
