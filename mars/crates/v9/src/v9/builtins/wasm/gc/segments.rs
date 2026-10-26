/** GC array initialization reads the same passive segments and drop state as bulk memory/table. */
use super::*;

/** Numeric element width follows its actual packed or scalar storage representation. */
fn width(field: &Field) -> usize {
    match field.packed {
        Some(8) => 1,
        Some(16) => 2,
        _ => match field.ty {
            Type::I32 | Type::F32 => 4,
            Type::I64 | Type::F64 => 8,
            Type::V128 => 16,
            _ => 0,
        },
    }
}
/** Decode binary storage without routing numeric bits through JavaScript coercion. */
fn decode(field: &Field, bytes: &[u8]) -> Res<JsValue> {
    Ok(match field.ty {
        Type::V128 => simd::value(bytes.try_into().unwrap())?,
        Type::I32 => numeric::load(
            match field.packed {
                Some(8) => 0x2d,
                Some(16) => 0x2f,
                _ => 0x28,
            },
            bytes,
        ),
        Type::I64 => numeric::load(0x29, bytes),
        Type::F32 => numeric::load(0x2a, bytes),
        Type::F64 => numeric::load(0x2b, bytes),
        _ => {
            return Err(
                crate::protocol::EngineError::invalid("Invalid GC data element storage.").into(),
            );
        }
    })
}
/** Source ranges are checked in bytes for data segments and in references for element segments. */
fn values(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    args: &[i64],
    offset: usize,
    count: usize,
) -> Res<Vec<JsValue>> {
    let field = gc_types::element(m, args[0] as u32).map_err(|e| compile_failure(vm, e))?;
    if op == 9 || op == 18 {
        let size = width(field);
        if size == 0 {
            return Err(vm.type_error("invalid array data storage"));
        }
        let bytes = super::super::segments::data(vm, instance, m, args[1] as u32)?;
        let length = count
            .checked_mul(size)
            .ok_or_else(|| trap(vm, "data segment out of bounds"))?;
        let range = super::super::segments::range(
            vm,
            offset,
            length,
            bytes.len(),
            "data segment out of bounds",
        )?;
        vm.charge(count)?;
        bytes[range]
            .chunks_exact(size)
            .map(|b| decode(field, b))
            .collect()
    } else {
        let elements = super::super::segments::elements(vm, instance, m, args[1] as u32)?;
        let range = super::super::segments::range(
            vm,
            offset,
            count,
            elements.len(),
            "element segment is out of bounds",
        )?;
        vm.charge(count)?;
        elements[range]
            .iter()
            .map(|c| instance::constant_value(vm, instance, c))
            .collect()
    }
}
/** new_data/new_elem allocate only after the complete initializer range has validated. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    m: &Module,
    op: u32,
    args: &[i64],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    let count = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    let source = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
    if op == 9 || op == 10 {
        let values = values(vm, instance, m, op, args, source, count)?;
        stack.push(allocate(vm, instance, args[0] as u32, true, values)?)
    } else {
        let start = numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize;
        let array = numeric::pop(stack)?;
        check(vm, m, &array, args[0] as u32)?;
        let length = length(vm, &array)?;
        super::super::segments::range(
            vm,
            start,
            count,
            length,
            "array element access out of bounds",
        )?;
        let values = values(vm, instance, m, op, args, source, count)?;
        for (i, value) in values.into_iter().enumerate() {
            write(vm, &array, start + i, value)?
        }
    }
    Ok(())
}
