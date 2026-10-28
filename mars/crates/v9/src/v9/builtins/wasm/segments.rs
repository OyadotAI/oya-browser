/** Passive data/element lifetimes and bulk operations retain only inert per-instance drop flags. */
use super::*;
const ELEMENT_VALUES: &str = "wasm.elements.values";

/** Active and declarative segments become unavailable after initialization; passive ones remain. */
pub(super) fn initialize(vm: &mut Vm, instance: ObjectId, m: &Module) -> Res<()> {
    for (name, flags) in [
        (
            "wasm.data.dropped",
            m.data
                .iter()
                .map(|d| JsValue::Boolean(d.memory.is_some()))
                .collect::<Vec<_>>(),
        ),
        (
            "wasm.elements.dropped",
            m.elements
                .iter()
                .map(|e| JsValue::Boolean(!e.passive))
                .collect::<Vec<_>>(),
        ),
    ] {
        let array = vm.new_array(flags)?;
        set(vm, instance, name, JsValue::Object(array))
    }
    cache_passive_elements(vm, instance, m)?;
    Ok(())
}
/** Element expressions are evaluated once and retained in traced, private instance storage. */
fn cache_passive_elements(vm: &mut Vm, instance: ObjectId, m: &Module) -> Res<()> {
    let segments = vm.new_array(vec![JsValue::Undefined; m.elements.len()])?;
    set(vm, instance, ELEMENT_VALUES, JsValue::Object(segments));
    for (index, segment) in m.elements.iter().enumerate().filter(|(_, e)| e.passive) {
        let values = vm.new_array(vec![JsValue::Undefined; segment.values.len()])?;
        vm.define(
            segments,
            key(&index.to_string()),
            Descriptor::data(JsValue::Object(values)),
        )?;
        for (index, constant) in segment.values.iter().enumerate() {
            let value = instance::constant_value(vm, instance, constant)?;
            vm.define(values, key(&index.to_string()), Descriptor::data(value))?;
        }
    }
    Ok(())
}
/** Restored drop flags are a closed boolean value, never a JS truthiness test. */
fn dropped(vm: &mut Vm, instance: ObjectId, name: &str, index: u32) -> Res<bool> {
    match instance::item(vm, instance, name, index)? {
        JsValue::Boolean(value) => Ok(value),
        _ => Err(vm.type_error("invalid Wasm segment lifetime record")),
    }
}
/** Reading a dropped segment yields an empty segment, preserving bounds semantics for size zero. */
pub(super) fn data<'a>(
    vm: &mut Vm,
    instance: ObjectId,
    m: &'a Module,
    index: u32,
) -> Res<&'a [u8]> {
    let segment = m
        .data
        .get(index as usize)
        .ok_or_else(|| vm.type_error("invalid Wasm data segment index"))?;
    if dropped(vm, instance, "wasm.data.dropped", index)? {
        Ok(&[])
    } else {
        Ok(&segment.bytes)
    }
}
/** The segment cache is checked after restoring state before any references are exposed. */
fn elements(vm: &mut Vm, instance: ObjectId, m: &Module, index: u32) -> Res<Option<ObjectId>> {
    let segment = m
        .elements
        .get(index as usize)
        .ok_or_else(|| vm.type_error("invalid Wasm element segment index"))?;
    if dropped(vm, instance, "wasm.elements.dropped", index)? {
        Ok(None)
    } else {
        let values = instance::item(vm, instance, ELEMENT_VALUES, index)?
            .object()
            .ok_or_else(|| vm.type_error("invalid Wasm element cache"))?;
        if !matches!(vm.heap.get(values).map(|o| &o.class), Some(Class::Array(v)) if v.len() == segment.values.len())
        {
            return Err(vm.type_error("invalid Wasm element cache"));
        }
        Ok(Some(values))
    }
}
/** Bounds checking is based on the cached segment or the empty dropped segment. */
pub(super) fn element_range(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    index: u32,
    offset: usize,
    count: usize,
    message: &str,
) -> Res<(Option<ObjectId>, std::ops::Range<usize>)> {
    let cache = elements(vm, instance, m, index)?;
    let length = cache.map_or(0, |_| m.elements[index as usize].values.len());
    Ok((cache, range(vm, offset, count, length, message)?))
}
/** Clone only the already checked range, keeping identity of referenced heap objects. */
pub(super) fn element_values(
    vm: &mut Vm,
    cache: Option<ObjectId>,
    range: std::ops::Range<usize>,
) -> Res<Vec<JsValue>> {
    let Some(cache) = cache else {
        return Ok(Vec::new());
    };
    let Some(Class::Array(values)) = vm.heap.get(cache).map(|o| &o.class) else {
        return Err(vm.type_error("invalid Wasm element cache"));
    };
    let result: Vec<_> = range.map(|i| values.get(i).cloned()).collect();
    if result
        .iter()
        .any(|value| matches!(value, None | Some(JsValue::Hole)))
    {
        return Err(vm.type_error("invalid Wasm element cache"));
    }
    Ok(result.into_iter().flatten().collect())
}
/** Drop is idempotent and mutates only the instance's inaccessible lifetime flags. */
fn drop_segment(vm: &mut Vm, instance: ObjectId, name: &str, index: u32) -> Res<()> {
    let array = get(vm, instance, name)
        .and_then(|v| v.object())
        .ok_or_else(|| vm.type_error("invalid Wasm segment state"))?;
    let _ = instance::item(vm, instance, name, index)?;
    vm.define(
        array,
        key(&index.to_string()),
        Descriptor::data(JsValue::Boolean(true)),
    )?;
    if name == "wasm.elements.dropped" {
        let values = get(vm, instance, ELEMENT_VALUES)
            .and_then(|v| v.object())
            .ok_or_else(|| vm.type_error("invalid Wasm element cache"))?;
        vm.define(
            values,
            key(&index.to_string()),
            Descriptor::data(JsValue::Undefined),
        )?;
    }
    Ok(())
}
/** All offsets and sizes use the unsigned interpretation of their typed i32 operands. */
fn index(stack: &mut Vec<JsValue>) -> Res<usize> {
    Ok(numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize)
}
/** Table indices are narrowed only after interpreting their declared address width. */
fn table_index(vm: &mut Vm, table: &JsValue, stack: &mut Vec<JsValue>) -> Res<usize> {
    let value = numeric::pop(stack)?;
    let value = table::operand(vm, table, &value)?;
    usize::try_from(value).map_err(|_| trap(vm, "table index is out of bounds"))
}
/** Check the entire range before allocating a copy or applying any destination mutation. */
pub(super) fn range(
    vm: &mut Vm,
    start: usize,
    count: usize,
    length: usize,
    message: &str,
) -> Res<std::ops::Range<usize>> {
    let end = start
        .checked_add(count)
        .filter(|end| *end <= length)
        .ok_or_else(|| trap(vm, message))?;
    Ok(start..end)
}
/** Memory initialization and drops use actual module bytes and ordinary memory bounds. */
pub(super) fn memory(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    args: &[u32],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    if op == 9 {
        return drop_segment(vm, instance, "wasm.data.dropped", args[0]);
    }
    let count = index(stack)?;
    let source = index(stack)?;
    let memory = instance::item(vm, instance, MEMORIES, args[1])?;
    let value = numeric::pop(stack)?;
    let destination = address::effective(vm, &memory, &value, 0)?;
    let segment = data(vm, instance, m, args[0])?;
    let source = range(
        vm,
        source,
        count,
        segment.len(),
        "memory access out of bounds",
    )?;
    let memory_length = memory::length(vm, &memory)?;
    range(
        vm,
        destination,
        count,
        memory_length,
        "memory access out of bounds",
    )?;
    vm.charge(count)?;
    memory::write(vm, &memory, destination, &segment[source])
}
/** Table instructions use checked references and preserve overlapping-copy semantics. */
pub(super) fn table(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    args: &[u32],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    match op {
        12 => {
            let count = index(stack)?;
            let source = index(stack)?;
            let table = instance::item(vm, instance, TABLES, args[1])?;
            let destination = table_index(vm, &table, stack)?;
            let length = table::length(vm, &table)?;
            range(
                vm,
                destination,
                count,
                length,
                "table index is out of bounds",
            )?;
            let (cache, source) = element_range(
                vm,
                instance,
                m,
                args[0],
                source,
                count,
                "element segment out of bounds",
            )?;
            vm.charge(count)?;
            let values = element_values(vm, cache, source)?;
            for (i, value) in values.iter().enumerate() {
                table::write(vm, &table, (destination + i) as u32, value)?;
            }
        }
        13 => drop_segment(vm, instance, "wasm.elements.dropped", args[0])?,
        14 => {
            let target = instance::item(vm, instance, TABLES, args[0])?;
            let from = instance::item(vm, instance, TABLES, args[1])?;
            let target_id = brand(vm, &target, TABLE)?;
            let from_id = brand(vm, &from, TABLE)?;
            let count = if table::is64(vm, target_id) && table::is64(vm, from_id) {
                table_index(vm, &target, stack)?
            } else {
                index(stack)?
            };
            let source = table_index(vm, &from, stack)?;
            let destination = table_index(vm, &target, stack)?;
            let from_len = table::length(vm, &from)?;
            let target_len = table::length(vm, &target)?;
            let source = range(vm, source, count, from_len, "table index is out of bounds")?;
            range(
                vm,
                destination,
                count,
                target_len,
                "table index is out of bounds",
            )?;
            vm.charge(count)?;
            let values = source
                .map(|i| table::read(vm, &from, i as u32))
                .collect::<Res<Vec<_>>>()?;
            for (i, value) in values.iter().enumerate() {
                table::write(vm, &target, (destination + i) as u32, value)?;
            }
        }
        15 => {
            let table = instance::item(vm, instance, TABLES, args[0])?;
            let delta = table_index(vm, &table, stack)?;
            let value = numeric::pop(stack)?;
            let old = match u32::try_from(delta)
                .ok()
                .map(|delta| table::resize(vm, &table, delta, &value))
            {
                Some(Ok(length)) => length as i32,
                None | Some(Err(Thrown::Value(_))) => -1,
                Some(Err(fatal)) => return Err(fatal),
            };
            let object = brand(vm, &table, TABLE)?;
            stack.push(if table::is64(vm, object) {
                JsValue::BigInt(old as i128)
            } else {
                JsValue::Number(f64::from(old))
            });
        }
        16 => {
            let table = instance::item(vm, instance, TABLES, args[0])?;
            let object = brand(vm, &table, TABLE)?;
            let length = table::length(vm, &table)?;
            stack.push(if table::is64(vm, object) {
                JsValue::BigInt(length as i128)
            } else {
                JsValue::Number(length as f64)
            });
        }
        17 => {
            let table = instance::item(vm, instance, TABLES, args[0])?;
            let count = table_index(vm, &table, stack)?;
            let value = numeric::pop(stack)?;
            let start = table_index(vm, &table, stack)?;
            let length = table::length(vm, &table)?;
            let destination = range(vm, start, count, length, "table index is out of bounds")?;
            vm.charge(count)?;
            for i in destination {
                table::write(vm, &table, i as u32, &value)?;
            }
        }
        _ => return Err(vm.unsupported_error("WebAssembly table bulk instruction")),
    }
    Ok(())
}
/** Bulk static checks retain every referenced table's type, including empty element segments. */
pub(super) fn table_signature(
    m: &Module,
    op: u32,
    args: &[u32],
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    use Type::*;
    let tables = validate::table_types(m);
    let ty = |index: u32| {
        tables
            .get(index as usize)
            .copied()
            .ok_or_else(|| Error::invalid("unknown bulk table index"))
    };
    Ok(match op {
        12 => {
            let element = m
                .elements
                .get(args[0] as usize)
                .ok_or_else(|| Error::invalid("unknown element segment index"))?;
            if !gc_types::subtype(m, element.ty, ty(args[1])?) {
                return Err(Error::invalid("table.init element types differ"));
            }
            (vec![table::address_type(m, args[1])?, I32, I32], vec![])
        }
        13 => {
            if args[0] as usize >= m.elements.len() {
                return Err(Error::invalid("unknown element segment index"));
            }
            (vec![], vec![])
        }
        14 => {
            if !gc_types::subtype(m, ty(args[1])?, ty(args[0])?) {
                return Err(Error::invalid("table.copy element types differ"));
            }
            let dst = table::address_type(m, args[0])?;
            let src = table::address_type(m, args[1])?;
            (
                vec![dst, src, if dst == I64 && src == I64 { I64 } else { I32 }],
                vec![],
            )
        }
        15 => (
            vec![ty(args[0])?, table::address_type(m, args[0])?],
            vec![table::address_type(m, args[0])?],
        ),
        16 => {
            ty(args[0])?;
            (vec![], vec![table::address_type(m, args[0])?])
        }
        17 => (
            vec![
                table::address_type(m, args[0])?,
                ty(args[0])?,
                table::address_type(m, args[0])?,
            ],
            vec![],
        ),
        _ => return Err(Error::unsupported("WebAssembly table bulk instruction")),
    })
}
