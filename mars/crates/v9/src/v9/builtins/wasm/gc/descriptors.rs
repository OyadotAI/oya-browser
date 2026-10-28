/** Custom descriptors retain real allocation identity, captured prototypes and checked exact reference types. */
use super::*;
const DESCRIPTOR: &str = "wasm.gc.descriptor";
const PROTOTYPE: &str = "wasm.gc.descriptor.prototype";

/** Described structs and descriptor structs point back to one another in the validated type graph. */
pub(super) fn descriptor(m: &Module, index: u32) -> Result<u32, Error> {
    m.definitions
        .get(index as usize)
        .and_then(|d| d.descriptor)
        .ok_or_else(|| Error::invalid("struct type has no custom descriptor"))
}
/** Explicit descriptor allocations require exact descriptor types, even when null remains representable. */
pub(super) fn descriptor_type(m: &Module, target: Type) -> Result<Type, Error> {
    let (heap, _) = target
        .reference()
        .ok_or_else(|| Error::invalid("descriptor target requires an indexed struct"))?;
    if heap < 0 {
        return Err(Error::invalid(
            "descriptor target requires an indexed struct",
        ));
    }
    let index = descriptor(m, heap as u32)?;
    Ok(if matches!(target, Type::Exact(..)) {
        Type::Exact(i64::from(index), true)
    } else {
        Type::Ref(i64::from(index), true)
    })
}
/** Static construction contracts consume fields followed by the selected descriptor reference. */
pub(super) fn signature(
    m: &Module,
    op: u32,
    index: u32,
    exact: bool,
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    let fields = gc_types::fields(m, index)?;
    match op {
        0 | 1 | 32 | 33 => {
            let default = matches!(op, 1 | 33);
            if default && fields.iter().any(|f| !gc_types::defaultable(f.ty)) {
                return Err(Error::invalid(
                    "struct.new_default requires defaultable fields",
                ));
            }
            let definition = &m.definitions[index as usize];
            if op >= 32 && definition.descriptor.is_none() {
                return Err(Error::invalid(&format!("descriptor allocation used for type {index} without descriptor")));
            }
            if op < 32 && definition.descriptor.is_some() {
                return Err(Error::invalid(&format!("non-descriptor allocation used for type {index} with descriptor")));
            }
            let mut params = if default {
                vec![]
            } else {
                fields.iter().map(|f| f.ty).collect()
            };
            if let Some(desc) = definition.descriptor {
                params.push(Type::Exact(i64::from(desc), true))
            }
            Ok((params, vec![Type::Exact(i64::from(index), false)]))
        }
        34 => {
            let desc = descriptor(m, index)?;
            Ok((
                vec![Type::Ref(i64::from(index), true)],
                vec![Type::Ref(i64::from(desc), false)],
            ))
        }
        35 | 36 => {
            let target = if exact {
                Type::Exact(i64::from(index), op == 36)
            } else {
                Type::Ref(i64::from(index), op == 36)
            };
            let desc = descriptor_type(m, target)?;
            let source = if gc_types::is_shared(m, target) { Type::Shared(-18, true) } else { Type::Ref(-18, true) };
            Ok((vec![source, desc], vec![target]))
        }
        _ => Err(Error::unsupported("WebAssembly descriptor instruction")),
    }
}
/** Descriptor identity is an opaque object link and cannot be forged by authored own properties. */
pub(super) fn equality(
    vm: &mut Vm,
    m: &Module,
    value: &JsValue,
    desc: &JsValue,
    target: Type,
) -> Res<bool> {
    if *desc == JsValue::Null {
        return Err(trap(vm, "dereferencing a null pointer"));
    }
    if *value == JsValue::Null {
        return Ok(target.reference().is_some_and(|(_, nullable)| nullable));
    }
    if !matches(vm, m, target, value)? {
        return Ok(false);
    }
    if let Some(root) = shared::handle(vm, value) {
        let descriptor_index = root.metadata()?.3.checked_sub(1).ok_or_else(|| vm.type_error("missing shared descriptor"))?;
        return Ok(shared::read(vm, &root, descriptor_index)? == *desc);
    }
    Ok(value
        .object()
        .and_then(|id| get(vm, id, DESCRIPTOR))
        .as_ref()
        == Some(desc))
}
/** Construction captures the descriptor's prototype once, independently of future field mutations. */
pub(super) fn construct(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    index: u32,
    stack: &mut Vec<JsValue>,
) -> Res<JsValue> {
    let definition = &m.definitions[index as usize];
    let desc = if let Some(desc_type) = definition.descriptor {
        let value = numeric::pop(stack)?;
        if value == JsValue::Null {
            return Err(trap(vm, "dereferencing a null pointer"));
        }
        if !matches(vm, m, Type::Exact(i64::from(desc_type), false), &value)? {
            return Err(vm.type_error("descriptor does not have the exact required type"));
        }
        Some(value)
    } else {
        None
    };
    let fields = gc_types::fields(m, index).map_err(|e| compile_failure(vm, e))?;
    let values = if matches!(op, 1 | 33) {
        fields.iter().map(|f| global::default(f.ty)).collect()
    } else {
        take(stack, fields.len())?
    };
    let values = fields
        .iter()
        .zip(values)
        .map(|(f, v)| packed(vm, f, &v))
        .collect::<Res<Vec<_>>>()?;
    if definition.shared {
        if let Some(value) = &desc { vm.pin(value.clone()); }
        if values.len() > vm.limits.array_length { return Err(trap(vm, "requested new array is too large")); }
        vm.charge(values.len())?;
        let source = get(vm, instance, INSTANCE).ok_or_else(|| vm.type_error("invalid Wasm GC instance"))?;
        return shared::allocate_described(vm, &source, m, index, false, values, desc);
    }
    let prototype = if definition.describes.is_some() && js_interop_enabled(vm) {
        values.first().and_then(|v| v.object())
    } else {
        None
    };
    let value = allocate(vm, instance, index, false, values)?;
    let object = value.object().unwrap();
    if definition.describes.is_some() {
        if let Some(proto) = prototype {
            set(vm, object, PROTOTYPE, JsValue::Object(proto))
        }
    }
    if let Some(desc) = desc {
        let proto = desc
            .object()
            .and_then(|id| get(vm, id, PROTOTYPE))
            .and_then(|v| v.object());
        set(vm, object, DESCRIPTOR, desc);
        vm.heap.get_mut(object).unwrap().proto = proto;
    }
    Ok(value)
}
/** A descriptor read checks the actual object's subtype and preserves the stored descriptor's identity. */
pub(super) fn get_descriptor(vm: &mut Vm, m: &Module, index: u32, value: &JsValue) -> Res<JsValue> {
    check(vm, m, value, index)?;
    if let Some(root) = shared::handle(vm, value) {
        let descriptor_index = root.metadata()?.3.checked_sub(1).ok_or_else(|| vm.type_error("missing shared descriptor"))?;
        return shared::read(vm, &root, descriptor_index);
    }
    get(vm, value.object().unwrap(), DESCRIPTOR)
        .ok_or_else(|| vm.type_error("invalid Wasm custom descriptor record"))
}
/** Descriptor casts compare actual allocation identity after their ordinary static type contract. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    args: &[i64],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    let index = args[0] as u32;
    let value = match op {
        32 | 33 => construct(vm, instance, m, op, index, stack)?,
        34 => {
            let value = numeric::pop(stack)?;
            get_descriptor(vm, m, index, &value)?
        }
        35 | 36 => {
            let desc = numeric::pop(stack)?;
            let value = numeric::pop(stack)?;
            let target = if args.get(1) == Some(&1) {
                Type::Exact(i64::from(index), op == 36)
            } else {
                Type::Ref(i64::from(index), op == 36)
            };
            if !equality(vm, m, &value, &desc, target)? {
                return Err(trap(vm, "illegal cast"));
            }
            value
        }
        _ => return Err(vm.unsupported_error("WebAssembly descriptor instruction")),
    };
    stack.push(value);
    Ok(())
}
