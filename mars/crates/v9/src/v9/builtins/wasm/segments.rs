/** Passive data/element lifetimes and bulk operations retain only inert per-instance drop flags. */
use super::*;

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
/** Element constants resolve to actual per-instance function references at the point of use. */
pub(super) fn elements<'a>(
    vm: &mut Vm,
    instance: ObjectId,
    m: &'a Module,
    index: u32,
) -> Res<&'a [types::Constant]> {
    let segment = m
        .elements
        .get(index as usize)
        .ok_or_else(|| vm.type_error("invalid Wasm element segment index"))?;
    if dropped(vm, instance, "wasm.elements.dropped", index)? {
        Ok(&[])
    } else {
        Ok(&segment.values)
    }
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
    Ok(())
}
/** All offsets and sizes use the unsigned interpretation of their typed i32 operands. */
fn index(stack: &mut Vec<JsValue>) -> Res<usize> {
    Ok(numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize)
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
        "data segment out of bounds",
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
            let destination = index(stack)?;
            let values = elements(vm, instance, m, args[0])?;
            let source = range(
                vm,
                source,
                count,
                values.len(),
                "element segment is out of bounds",
            )?;
            let table = instance::item(vm, instance, TABLES, args[1])?;
            let length = table::length(vm, &table)?;
            range(
                vm,
                destination,
                count,
                length,
                "table index is out of bounds",
            )?;
            vm.charge(count)?;
            let values = values[source]
                .iter()
                .map(|c| instance::constant_value(vm, instance, c))
                .collect::<Res<Vec<_>>>()?;
            for (i, value) in values.iter().enumerate() {
                table::write(vm, &table, (destination + i) as u32, value)?;
            }
        }
        13 => drop_segment(vm, instance, "wasm.elements.dropped", args[0])?,
        14 => {
            let count = index(stack)?;
            let source = index(stack)?;
            let destination = index(stack)?;
            let target = instance::item(vm, instance, TABLES, args[0])?;
            let from = instance::item(vm, instance, TABLES, args[1])?;
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
            let delta = index(stack)? as u32;
            let value = numeric::pop(stack)?;
            let table = instance::item(vm, instance, TABLES, args[0])?;
            let old = match table::resize(vm, &table, delta, &value) {
                Ok(length) => length as i32,
                Err(Thrown::Value(_)) => -1,
                Err(fatal) => return Err(fatal),
            };
            stack.push(JsValue::Number(f64::from(old)));
        }
        16 => {
            let table = instance::item(vm, instance, TABLES, args[0])?;
            stack.push(JsValue::Number(table::length(vm, &table)? as f64));
        }
        17 => {
            let count = index(stack)?;
            let value = numeric::pop(stack)?;
            let start = index(stack)?;
            let table = instance::item(vm, instance, TABLES, args[0])?;
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
            (vec![I32, I32, I32], vec![])
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
            (vec![I32, I32, I32], vec![])
        }
        15 => (vec![ty(args[0])?, I32], vec![I32]),
        16 => {
            ty(args[0])?;
            (vec![], vec![I32])
        }
        17 => (vec![I32, ty(args[0])?, I32], vec![]),
        _ => return Err(Error::unsupported("WebAssembly table bulk instruction")),
    })
}
