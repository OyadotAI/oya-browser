/** Stringref instructions retain exact UTF16, expose real views and access actual bounded memory bytes. */
use super::*;
use types::Instruction;
const STRING: Type = Type::Ref(-25, true);
const NONNULL: Type = Type::Ref(-25, false);
const WTF8: Type = Type::Ref(-26, false);
const WTF16: Type = Type::Ref(-32, false);
const ITER: Type = Type::Ref(-31, false);
const ISOLATED: &str = "Failed to encode string as UTF-8: contains unpaired surrogate";
/** Resident-unit upper bounds for native output vectors per UTF-16 input unit. */
const MEMORY_OUTPUT_UNITS: usize = 3;
const VIEW_OUTPUT_UNITS: usize = 5;
const ARRAY_OUTPUT_UNITS: usize = 7;

/** Wasm substring construction normalizes encoding to the actual copied code units. */
pub(super) fn normalized_substring(vm: &mut Vm, value: &JsString, start: usize, end: usize) -> Res<JsString> {
    vm.copy_string_range(value, start, end)
}

/** WTF-8 accepts encoded lone surrogates but rejects overlong and malformed byte sequences. */
pub(super) fn decode_wtf8(bytes: &[u8]) -> Result<Vec<u16>, Error> {
    let mut units = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let first = bytes[at];
        let (mut point, width, minimum) = match first {
            0x00..=0x7f => (u32::from(first), 1, 0),
            0xc2..=0xdf => (u32::from(first & 0x1f), 2, 0x80),
            0xe0..=0xef => (u32::from(first & 0x0f), 3, 0x800),
            0xf0..=0xf4 => (u32::from(first & 0x07), 4, 0x10000),
            _ => return Err(Error::invalid("invalid WTF-8 string literal")),
        };
        if at + width > bytes.len() {
            return Err(Error::invalid("invalid WTF-8 string literal"));
        }
        for byte in &bytes[at + 1..at + width] {
            if byte & 0xc0 != 0x80 {
                return Err(Error::invalid("invalid WTF-8 string literal"));
            }
            point = (point << 6) | u32::from(byte & 0x3f);
        }
        if point < minimum || point > 0x10ffff {
            return Err(Error::invalid("invalid WTF-8 string literal"));
        }
        if point <= 0xffff {
            if (0xdc00..=0xdfff).contains(&point)
                && units
                    .last()
                    .is_some_and(|prior| (0xd800..=0xdbff).contains(prior))
            {
                return Err(Error::invalid("invalid WTF-8 string literal"));
            }
            units.push(point as u16);
        } else {
            let value = point - 0x10000;
            units.push(0xd800 + (value >> 10) as u16);
            units.push(0xdc00 + (value & 0x3ff) as u16);
        }
        at += width;
    }
    Ok(units)
}

#[cfg(test)]
mod literal_tests {
    use super::*;

    /** WTF-8 accepts lone surrogate code units but rejects malformed and overlong sequences. */
    #[test]
    fn literal_wtf8_preserves_surrogates_and_checks_encoding() {
        assert_eq!(
            decode_wtf8(&[0x41, 0xed, 0xa0, 0x80, 0xf0, 0x9f, 0x98, 0x80]).unwrap(),
            [0x41, 0xd800, 0xd83d, 0xde00]
        );
        for invalid in [
            &[0xc0, 0x80][..],
            &[0xe2, 0x82][..],
            &[0xf4, 0x90, 0x80, 0x80][..],
        ] {
            assert!(decode_wtf8(invalid).is_err());
        }
        assert!(decode_wtf8(&[0xed, 0xa0, 0x80, 0xed, 0xb0, 0x80]).is_err());
    }
}

/** Each implemented string instruction has its real immediate grammar; other proposals remain explicit. */
pub(super) fn decode(r: &mut binary::Reader, op: u32) -> Result<Instruction, Error> {
    let memory = matches!(
        op,
        0x80 | 0x81 | 0x86 | 0x87 | 0x8b..=0x8f | 0x92 | 0x94 | 0x95 | 0x9b
    );
    if !memory
        && !matches!(op,0x82..=0x85|0x88..=0x8a|0x90|0x91|0x93|0x98..=0x9a|0x9c|0xa0..=0xa4|0xa8..=0xaa|0xb0..=0xb8)
    {
        return Err(Error::unsupported(&format!(
            "WebAssembly stringref instruction 0x{op:x}"
        )));
    }
    Ok(Instruction::Gc(
        op,
        if memory || op == 0x82 {
            vec![i64::from(r.u32()?)]
        } else {
            vec![]
        },
    ))
}
/** Views have their own type identity and cannot be substituted for ordinary string references. */
pub(super) fn signature(
    m: &Module,
    op: u32,
    args: &[i64],
) -> Result<(Vec<Type>, Vec<Type>), Error> {
    if op == 0x82 {
        if m.string_literals.get(args[0] as usize).is_none() {
            return Err(Error::invalid(&format!(
                "Invalid string literal index: {}",
                args[0]
            )));
        }
        return Ok((vec![], vec![NONNULL]));
    }
    let address = if args.is_empty() {
        Type::I32
    } else {
        address::ty(m, args[0] as u32)?
    };
    Ok(match op {
        0x80 | 0x81 | 0x8b | 0x8c | 0x8f => (
            vec![address, Type::I32],
            vec![if op == 0x8f { STRING } else { NONNULL }],
        ),
        0x83..=0x85 | 0x8a => (vec![STRING], vec![Type::I32]),
        0x86 | 0x87 | 0x8d | 0x8e => (vec![STRING, address], vec![Type::I32]),
        0x88 => (vec![STRING, STRING], vec![NONNULL]),
        0x89 | 0xa8 => (vec![STRING, STRING], vec![Type::I32]),
        0x90 => (vec![STRING], vec![WTF8]),
        0x91 => (vec![WTF8, Type::I32, Type::I32], vec![Type::I32]),
        0x93 => (vec![WTF8, Type::I32, Type::I32], vec![NONNULL]),
        0x92 | 0x94 | 0x95 => (
            vec![WTF8, address, Type::I32, Type::I32],
            vec![Type::I32, Type::I32],
        ),
        0x98 => (vec![STRING], vec![WTF16]),
        0x99 => (vec![WTF16], vec![Type::I32]),
        0x9a => (vec![WTF16, Type::I32], vec![Type::I32]),
        0x9b => (vec![WTF16, address, Type::I32, Type::I32], vec![Type::I32]),
        0x9c => (vec![WTF16, Type::I32, Type::I32], vec![NONNULL]),
        0xa0 => (vec![STRING], vec![ITER]),
        0xa1 => (vec![ITER], vec![Type::I32]),
        0xa2 | 0xa3 => (vec![ITER, Type::I32], vec![Type::I32]),
        0xa4 => (vec![ITER, Type::I32], vec![NONNULL]),
        0xa9 => (vec![Type::I32], vec![NONNULL]),
        0xaa => (vec![STRING], vec![Type::I32]),
        0xb0 | 0xb1 | 0xb4 | 0xb5 | 0xb8 => (
            vec![Type::Ref(-22, true), Type::I32, Type::I32],
            vec![if op == 0xb8 { STRING } else { NONNULL }],
        ),
        0xb2 | 0xb3 | 0xb6 | 0xb7 => (
            vec![STRING, Type::Ref(-22, true), Type::I32],
            vec![Type::I32],
        ),
        _ => return Err(Error::unsupported("WebAssembly stringref instruction")),
    })
}
/** Packed-array operands must have the actual declared element width and writable destination. */
pub(super) fn array_operand(m: &Module, op: u32, ty: Type) -> bool {
    let writing = matches!(op, 0xb2 | 0xb3 | 0xb6 | 0xb7);
    let bits = if matches!(op, 0xb1 | 0xb3) { 16 } else { 8 };
    if ty.reference().is_some_and(|(heap, _)| heap == -15) {
        return true;
    }
    ty.reference().and_then(|(heap, _)| usize::try_from(heap).ok())
        .and_then(|index| m.definitions.get(index))
        .is_some_and(|def| matches!(&def.kind, super::gc_types::Kind::Array(field) if field.packed == Some(bits) && (!writing || field.mutable)))
}
/** Strings are never coerced through author code; null references trap before any memory access. */
fn string(vm: &mut Vm, value: JsValue) -> Res<JsString> {
    match value {
        JsValue::String(s) => Ok(s),
        JsValue::Null => Err(trap(vm, "dereferencing a null pointer")),
        _ => Err(vm.type_error("invalid Wasm string reference")),
    }
}
/** Byte counts and codeunit indices retain unsigned i32 semantics. */
fn index(stack: &mut Vec<JsValue>) -> Res<usize> {
    Ok(numeric::i32_value(&numeric::pop(stack)?)? as u32 as usize)
}
/** Iterator positions are UTF-16 offsets, advanced as Unicode code points. */
fn next_position(units: &JsString, position: usize) -> usize {
    let Some(first) = units.unit_at(position) else {
        return position;
    };
    if (0xd800..=0xdbff).contains(&first)
        && units
            .unit_at(position + 1)
            .is_some_and(|next| (0xdc00..=0xdfff).contains(&next))
    {
        position + 2
    } else {
        position + 1
    }
}
/** Rewinding never splits a valid surrogate pair. */
fn previous_position(units: &JsString, position: usize) -> usize {
    if position == 0 {
        return 0;
    }
    let at = position - 1;
    if at > 0
        && units
            .unit_at(at)
            .is_some_and(|unit| (0xdc00..=0xdfff).contains(&unit))
        && units
            .unit_at(at - 1)
            .is_some_and(|unit| (0xd800..=0xdbff).contains(&unit))
    {
        at - 1
    } else {
        at
    }
}
/** Move at most the available number of code points, independent of an untrusted count. */
fn move_position(
    units: &JsString,
    mut position: usize,
    count: usize,
    backwards: bool,
) -> (usize, usize) {
    let mut moved = 0;
    while moved < count {
        let next = if backwards {
            previous_position(units, position)
        } else {
            next_position(units, position)
        };
        if next == position {
            break;
        }
        position = next;
        moved += 1;
    }
    (position, moved)
}
/** Native iterator references carry private string and offset slots, not author properties. */
fn iterator_state(vm: &mut Vm, value: JsValue) -> Res<(ObjectId, JsString, usize)> {
    let id = value
        .object()
        .ok_or_else(|| trap(vm, "dereferencing a null pointer"))?;
    let Some(JsValue::String(string)) = get(vm, id, "wasm.string.iterator") else {
        return Err(vm.type_error("invalid Wasm string iterator"));
    };
    let Some(JsValue::Number(position)) = get(vm, id, "wasm.string.position") else {
        return Err(vm.type_error("invalid Wasm string iterator"));
    };
    if !position.is_finite()
        || position < 0.0
        || position.fract() != 0.0
        || position > string.len() as f64
    {
        return Err(vm.type_error("invalid Wasm string iterator"));
    }
    let position = position as usize;
    if position > string.len() {
        return Err(vm.type_error("invalid Wasm string iterator"));
    }
    Ok((id, string, position))
}
/** Array-based constructors read the actual packed GC array after validating both bounds. */
fn from_array(vm: &mut Vm, op: u32, stack: &mut Vec<JsValue>) -> Res<JsValue> {
    let end = index(stack)?;
    let start = index(stack)?;
    let array = numeric::pop(stack)?;
    let units = super::gc::string_array(vm, &array, start, end, if op == 0xb1 { 16 } else { 8 })?;
    if op == 0xb1 {
        return Ok(JsValue::String(JsString::from_units(units)?));
    }
    let bytes: Vec<u8> = units.into_iter().map(|unit| unit as u8).collect();
    if op == 0xb5 {
        let units = decode_wtf8(&bytes).map_err(|_| trap(vm, "invalid WTF-8 string"))?;
        return Ok(JsValue::String(JsString::from_units(units)?));
    }
    match std::str::from_utf8(&bytes) {
        Ok(value) => Ok(JsValue::string(value)?),
        Err(_) if op == 0xb8 => Ok(JsValue::Null),
        Err(_) if op == 0xb4 => Ok(JsValue::string(&String::from_utf8_lossy(&bytes))?),
        Err(_) => Err(trap(vm, "invalid UTF-8 string")),
    }
}
/** Array encoders validate the complete destination before writing packed elements. */
fn into_array(vm: &mut Vm, op: u32, stack: &mut Vec<JsValue>) -> Res<JsValue> {
    let start = index(stack)?;
    let array = numeric::pop(stack)?;
    let input = numeric::pop(stack)?;
    let value = string(vm, input)?;
    // Encoding can retain a byte vector while constructing packed UTF-16 elements.
    vm.with_string_output(
        &value,
        value.len().saturating_mul(ARRAY_OUTPUT_UNITS),
        |vm| {
            let bytes = if op == 0xb3 {
                Vec::new()
            } else {
                encode(&value, op == 0xb6)
            };
            let length = if op == 0xb3 { value.len() } else { bytes.len() };
            let size =
                super::gc::string_storage(vm, &array, if op == 0xb3 { 16 } else { 8 }, true)?;
            if start > size || length > size - start {
                return Err(trap(vm, "array element access out of bounds"));
            }
            if op == 0xb2 && surrogate(&bytes) {
                return Err(trap(vm, ISOLATED));
            }
            let units: Vec<u16> = if op == 0xb3 {
                value.code_units().collect()
            } else {
                bytes.into_iter().map(u16::from).collect()
            };
            super::gc::write_string_array(
                vm,
                &array,
                start,
                &units,
                if op == 0xb3 { 16 } else { 8 },
            )?;
            Ok(JsValue::Number(units.len() as f64))
        },
    )
}
/** WTF8 preserves lone surrogates while valid pairs become one four-byte scalar sequence. */
fn encode(string: &JsString, lossy: bool) -> Vec<u8> {
    if lossy {
        return string.lossy().into_bytes();
    }
    let mut bytes = Vec::new();
    let mut units = string.code_units().peekable();
    while let Some(unit) = units.next() {
        let point = if (0xd800..=0xdbff).contains(&unit)
            && units.peek().is_some_and(|n| (0xdc00..=0xdfff).contains(n))
        {
            0x10000
                + ((u32::from(unit) - 0xd800) << 10)
                + (u32::from(units.next().unwrap()) - 0xdc00)
        } else {
            u32::from(unit)
        };
        if point < 0x80 {
            bytes.push(point as u8)
        } else if point < 0x800 {
            bytes.extend([0xc0 | (point >> 6) as u8, 0x80 | (point & 63) as u8])
        } else if point < 0x10000 {
            bytes.extend([
                0xe0 | (point >> 12) as u8,
                0x80 | ((point >> 6) & 63) as u8,
                0x80 | (point & 63) as u8,
            ])
        } else {
            bytes.extend([
                0xf0 | (point >> 18) as u8,
                0x80 | ((point >> 12) & 63) as u8,
                0x80 | ((point >> 6) & 63) as u8,
                0x80 | (point & 63) as u8,
            ])
        }
    }
    bytes
}
/** Positions inside multibyte scalars move forward; byte budgets stop before splitting a scalar. */
fn advance(bytes: &[u8], position: usize, count: usize) -> usize {
    let mut start = position.min(bytes.len());
    while start < bytes.len() && bytes[start] & 0xc0 == 0x80 {
        start += 1
    }
    if count == 0 {
        return start;
    }
    let mut end = start.saturating_add(count).min(bytes.len());
    while end < bytes.len() && end > start && bytes[end] & 0xc0 == 0x80 {
        end -= 1
    }
    end
}
/** Surrogate triplets are recognized only at well-formed WTF8 codepoint boundaries. */
fn surrogate(bytes: &[u8]) -> bool {
    bytes
        .windows(3)
        .any(|p| p[0] == 0xed && (0xa0..=0xbf).contains(&p[1]) && (0x80..=0xbf).contains(&p[2]))
}
/** Memory decoding validates complete ranges before allocating or interpreting UTF code units. */
fn decode_memory(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    memory: u32,
    stack: &mut Vec<JsValue>,
) -> Res<JsValue> {
    let count = index(stack)?;
    let position = numeric::pop(stack)?;
    let memory = instance::item(vm, instance, MEMORIES, memory)?;
    let at = address::effective(vm, &memory, &position, 0)?;
    let length = count
        .checked_mul(if op == 0x81 { 2 } else { 1 })
        .ok_or_else(|| trap(vm, "memory access out of bounds"))?;
    let bytes = memory::read(vm, &memory, at, length)?;
    vm.charge(length)?;
    if op == 0x81 {
        if at % 2 != 0 {
            return Err(trap(vm, "operation does not support unaligned accesses"));
        }
        return Ok(JsValue::String(JsString::from_units(
            bytes
                .chunks_exact(2)
                .map(|b| u16::from_le_bytes([b[0], b[1]]))
                .collect(),
        )?));
    }
    if op == 0x8c {
        let units = decode_wtf8(&bytes).map_err(|_| trap(vm, "invalid WTF-8 string"))?;
        return Ok(JsValue::String(JsString::from_units(units)?));
    }
    match std::str::from_utf8(&bytes) {
        Ok(s) => Ok(JsValue::string(s)?),
        Err(_) if op == 0x8f => Ok(JsValue::Null),
        Err(_) if op == 0x8b => Ok(JsValue::string(&String::from_utf8_lossy(&bytes))?),
        Err(_) => Err(trap(vm, "invalid UTF-8 string")),
    }
}
/** Encoding checks the full destination and only then writes, preserving trap order and atomic bounds. */
fn encode_memory(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    memory: u32,
    stack: &mut Vec<JsValue>,
) -> Res<JsValue> {
    let position = numeric::pop(stack)?;
    let value = numeric::pop(stack)?;
    let value = string(vm, value)?;
    let memory = instance::item(vm, instance, MEMORIES, memory)?;
    let at = address::effective(vm, &memory, &position, 0)?;
    // Lossy UTF-8 may hold both an intermediate string and its output at once.
    let peak_units = value.len().saturating_mul(MEMORY_OUTPUT_UNITS);
    vm.with_string_output(&value, peak_units, |vm| {
        let bytes = if op == 0x87 {
            value.code_units().flat_map(|u| u.to_le_bytes()).collect()
        } else {
            encode(&value, op == 0x8d)
        };
        memory::read(vm, &memory, at, bytes.len())?;
        if op == 0x87 && at % 2 != 0 {
            return Err(trap(vm, "operation does not support unaligned accesses"));
        }
        if op == 0x86 && surrogate(&bytes) {
            return Err(trap(vm, ISOLATED));
        }
        vm.charge(bytes.len())?;
        memory::write(vm, &memory, at, &bytes)?;
        Ok(JsValue::Number(
            if op == 0x87 { value.len() } else { bytes.len() } as f64,
        ))
    })
}
/** View encoding clamps to complete scalars and reports the actual next position and byte count. */
fn encode_view(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    memory: u32,
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    let count = index(stack)?;
    let position = index(stack)?;
    let destination = numeric::pop(stack)?;
    let value = numeric::pop(stack)?;
    let value = string(vm, value)?;
    let memory = instance::item(vm, instance, MEMORIES, memory)?;
    let at = address::effective(vm, &memory, &destination, 0)?;
    // A WTF-8 view may keep both the encoded input and its selected output alive.
    vm.with_string_output(
        &value,
        value.len().saturating_mul(VIEW_OUTPUT_UNITS),
        |vm| {
            if op == 0x9b {
                let start = position.min(value.len());
                let end = start.saturating_add(count).min(value.len());
                let bytes = value
                    .code_units()
                    .skip(start)
                    .take(end - start)
                    .flat_map(|u| u.to_le_bytes())
                    .collect::<Vec<_>>();
                memory::read(vm, &memory, at, bytes.len())?;
                if at % 2 != 0 {
                    return Err(trap(vm, "operation does not support unaligned accesses"));
                }
                vm.charge(bytes.len())?;
                memory::write(vm, &memory, at, &bytes)?;
                stack.push(JsValue::Number((end - start) as f64));
                return Ok(());
            }
            let bytes = encode(&value, false);
            let start = advance(&bytes, position, 0);
            let end = advance(&bytes, start, count);
            let mut target = bytes[start..end].to_vec();
            memory::read(vm, &memory, at, target.len())?;
            if op == 0x92 && surrogate(&target) {
                return Err(trap(vm, ISOLATED));
            }
            if op == 0x94 {
                for index in 0..target.len().saturating_sub(2) {
                    if target[index] == 0xed && (0xa0..=0xbf).contains(&target[index + 1]) {
                        target[index..index + 3].copy_from_slice(&[0xef, 0xbf, 0xbd])
                    }
                }
            }
            vm.charge(target.len())?;
            memory::write(vm, &memory, at, &target)?;
            stack.extend([
                JsValue::Number(end as f64),
                JsValue::Number((end - start) as f64),
            ]);
            Ok(())
        },
    )
}
/** Every accepted instruction computes from actual strings or memory, without invoking author getters. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    module: &Module,
    op: u32,
    args: &[i64],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    if matches!(op, 0xb0 | 0xb1 | 0xb4 | 0xb5 | 0xb8) {
        let value = from_array(vm, op, stack)?;
        stack.push(value);
        return Ok(());
    }
    if matches!(op, 0xb2 | 0xb3 | 0xb6 | 0xb7) {
        let value = into_array(vm, op, stack)?;
        stack.push(value);
        return Ok(());
    }
    if op == 0x82 {
        let value = module
            .string_literals
            .get(args[0] as usize)
            .ok_or_else(|| vm.type_error("invalid string literal index"))?;
        stack.push(JsValue::String(value.clone()));
        return Ok(());
    }
    if matches!(op, 0x80 | 0x81 | 0x8b | 0x8c | 0x8f) {
        let value = decode_memory(vm, instance, op, args[0] as u32, stack)?;
        stack.push(value);
        return Ok(());
    }
    if matches!(op, 0x86 | 0x87 | 0x8d | 0x8e) {
        let value = encode_memory(vm, instance, op, args[0] as u32, stack)?;
        stack.push(value);
        return Ok(());
    }
    if matches!(op, 0x92 | 0x94 | 0x95 | 0x9b) {
        return encode_view(vm, instance, op, args[0] as u32, stack);
    }
    let value = match op {
        0xaa => {
            use std::hash::{Hash, Hasher};
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            vm.charge(value.len())?;
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            value.hash(&mut hasher);
            JsValue::Number((hasher.finish() as u32 as i32) as f64)
        }
        0xa1..=0xa4 => {
            let count = if op == 0xa1 { 1 } else { index(stack)? };
            let iterator = numeric::pop(stack)?;
            let (id, value, start) = iterator_state(vm, iterator)?;
            let (end, moved) = move_position(&value, start, count, op == 0xa3);
            vm.charge(moved)?;
            if op != 0xa4 {
                set(vm, id, "wasm.string.position", JsValue::Number(end as f64));
            }
            if op == 0xa4 {
                JsValue::String(normalized_substring(vm, &value, start, end)?)
            } else if op == 0xa1 {
                let point = if moved == 0 {
                    -1
                } else if end - start == 2 {
                    0x10000
                        + ((i32::from(value.unit_at(start).expect("iterator bounds")) - 0xd800)
                            << 10)
                        + (i32::from(value.unit_at(start + 1).expect("iterator bounds")) - 0xdc00)
                } else {
                    i32::from(value.unit_at(start).expect("iterator bounds"))
                };
                JsValue::Number(f64::from(point))
            } else {
                JsValue::Number(moved as f64)
            }
        }
        0xa9 => {
            let point = index(stack)? as u32;
            if point > 0x10ffff {
                return Err(trap(vm, &format!("Invalid code point {point}")));
            }
            let units = if point <= 0xffff {
                vec![point as u16]
            } else {
                let point = point - 0x10000;
                vec![
                    0xd800 + (point >> 10) as u16,
                    0xdc00 + (point & 1023) as u16,
                ]
            };
            JsValue::String(JsString::from_units(units)?)
        }
        0x89 => {
            let right = numeric::pop(stack)?;
            let left = numeric::pop(stack)?;
            JsValue::Number(if left == right { 1.0 } else { 0.0 })
        }
        0x88 | 0xa8 => {
            let right = numeric::pop(stack)?;
            let right = string(vm, right)?;
            let left = numeric::pop(stack)?;
            let left = string(vm, left)?;
            if op == 0x88 {
                JsValue::String(vm.concat_strings(&left, &right)?)
            } else {
                JsValue::Number(match left.code_units().cmp(right.code_units()) {
                    std::cmp::Ordering::Less => -1.0,
                    std::cmp::Ordering::Equal => 0.0,
                    std::cmp::Ordering::Greater => 1.0,
                })
            }
        }
        0x91 => {
            let count = index(stack)?;
            let position = index(stack)?;
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            JsValue::Number(advance(&encode(&value, false), position, count) as f64)
        }
        0x93 => {
            let end = index(stack)?;
            let start = index(stack)?;
            let value = numeric::pop(stack)?;
            let bytes = encode(&string(vm, value)?, false);
            let start = advance(&bytes, start, 0);
            let end = advance(&bytes, end, 0).max(start);
            let units =
                decode_wtf8(&bytes[start..end]).map_err(|_| trap(vm, "invalid WTF-8 string"))?;
            JsValue::String(JsString::from_units(units)?)
        }
        0x9a => {
            let index = index(stack)?;
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            JsValue::Number(f64::from(
                value
                    .unit_at(index)
                    .ok_or_else(|| trap(vm, "string offset out of bounds"))?,
            ))
        }
        0x9c => {
            let end = index(stack)?;
            let start = index(stack)?;
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            let start = start.min(value.len());
            let end = end.min(value.len()).max(start);
            JsValue::String(normalized_substring(vm, &value, start, end)?)
        }
        0x90 | 0x98 => {
            let value = numeric::pop(stack)?;
            JsValue::String(string(vm, value)?)
        }
        0x83..=0x85 | 0x8a | 0x99 => {
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            let bytes = encode(&value, false);
            JsValue::Number(match op {
                0x83 => {
                    if surrogate(&bytes) {
                        -1.0
                    } else {
                        bytes.len() as f64
                    }
                }
                0x84 => bytes.len() as f64,
                0x8a => {
                    if surrogate(&bytes) {
                        0.0
                    } else {
                        1.0
                    }
                }
                _ => value.len() as f64,
            })
        }
        0xa0 => {
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            let iterator = vm.new_object()?;
            set(vm, iterator, "wasm.string.iterator", JsValue::String(value));
            set(vm, iterator, "wasm.string.position", JsValue::Number(0.0));
            JsValue::Object(iterator)
        }
        _ => return Err(vm.unsupported_error("WebAssembly stringref instruction")),
    };
    stack.push(value);
    Ok(())
}
