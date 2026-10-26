/** Stringref instructions retain exact UTF16, expose real views and access actual bounded memory bytes. */
use super::*;
use types::Instruction;
const STRING: Type = Type::Ref(-25, true);
const NONNULL: Type = Type::Ref(-25, false);
const WTF8: Type = Type::Ref(-26, false);
const WTF16: Type = Type::Ref(-32, false);
const ISOLATED: &str = "Failed to encode string as UTF-8: contains unpaired surrogate";

/** Each implemented string instruction has its real immediate grammar; other proposals remain explicit. */
pub(super) fn decode(r: &mut binary::Reader, op: u32) -> Result<Instruction, Error> {
    let memory = matches!(
        op,
        0x80 | 0x81 | 0x86 | 0x87 | 0x8b | 0x8d..=0x8f | 0x92 | 0x94 | 0x95 | 0x9b
    );
    if !memory && !matches!(op,0x83..=0x85|0x88..=0x8a|0x90|0x91|0x98..=0x9a|0x9c|0xa0|0xa8|0xa9) {
        return Err(Error::unsupported("WebAssembly stringref instruction"));
    }
    Ok(Instruction::Gc(
        op,
        if memory {
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
    let address = if args.is_empty() {
        Type::I32
    } else {
        address::ty(m, args[0] as u32)?
    };
    Ok(match op {
        0x80 | 0x81 | 0x8b | 0x8f => (
            vec![address, Type::I32],
            vec![if op == 0x8f { STRING } else { NONNULL }],
        ),
        0x83..=0x85 | 0x8a => (vec![STRING], vec![Type::I32]),
        0x86 | 0x87 | 0x8d | 0x8e => (vec![STRING, address], vec![Type::I32]),
        0x88 => (vec![STRING, STRING], vec![NONNULL]),
        0x89 | 0xa8 => (vec![STRING, STRING], vec![Type::I32]),
        0x90 => (vec![STRING], vec![WTF8]),
        0x91 => (vec![WTF8, Type::I32, Type::I32], vec![Type::I32]),
        0x92 | 0x94 | 0x95 => (
            vec![WTF8, address, Type::I32, Type::I32],
            vec![Type::I32, Type::I32],
        ),
        0x98 => (vec![STRING], vec![WTF16]),
        0x99 => (vec![WTF16], vec![Type::I32]),
        0x9a => (vec![WTF16, Type::I32], vec![Type::I32]),
        0x9b => (vec![WTF16, address, Type::I32, Type::I32], vec![Type::I32]),
        0x9c => (vec![WTF16, Type::I32, Type::I32], vec![NONNULL]),
        0xa0 => (vec![STRING], vec![Type::Ref(-31, false)]),
        0xa9 => (vec![Type::I32], vec![NONNULL]),
        _ => return Err(Error::unsupported("WebAssembly stringref instruction")),
    })
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
/** WTF8 preserves lone surrogates while valid pairs become one four-byte scalar sequence. */
fn encode(string: &JsString, lossy: bool) -> Vec<u8> {
    if lossy {
        return string.lossy().into_bytes();
    }
    let mut bytes = Vec::new();
    let mut units = string.as_units().iter().copied().peekable();
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
    let bytes = if op == 0x87 {
        value
            .as_units()
            .iter()
            .flat_map(|u| u.to_le_bytes())
            .collect()
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
    if op == 0x9b {
        let start = position.min(value.len());
        let end = start.saturating_add(count).min(value.len());
        let bytes = value.as_units()[start..end]
            .iter()
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
}
/** Every accepted instruction computes from actual strings or memory, without invoking author getters. */
pub(super) fn run(
    vm: &mut Vm,
    instance: ObjectId,
    op: u32,
    args: &[i64],
    stack: &mut Vec<JsValue>,
) -> Res<()> {
    if matches!(op, 0x80 | 0x81 | 0x8b | 0x8f) {
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
                let mut units = left.as_units().to_vec();
                units.extend(right.as_units());
                JsValue::String(JsString::from_units(units)?)
            } else {
                JsValue::Number(match left.as_units().cmp(right.as_units()) {
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
        0x9a => {
            let index = index(stack)?;
            let value = numeric::pop(stack)?;
            let value = string(vm, value)?;
            JsValue::Number(f64::from(
                *value
                    .as_units()
                    .get(index)
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
            JsValue::String(JsString::from_units(value.as_units()[start..end].to_vec())?)
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
