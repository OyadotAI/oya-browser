/** Compiler diagnostics report actual byte positions, authored function names and producing opcodes. */
use super::*;
use types::Instruction;

/** The optional name section supplies labels only; malformed custom metadata never changes validity. */
pub(super) fn function_names(bytes: &[u8]) -> std::collections::BTreeMap<u32, String> {
    fn read(bytes: &[u8]) -> Result<std::collections::BTreeMap<u32, String>, Error> {
        let mut result = std::collections::BTreeMap::new();
        let mut reader = binary::Reader::new(bytes);
        reader.take(MAGIC.len())?;
        while reader.at < bytes.len() {
            let id = reader.byte()?;
            let count = reader.u32()? as usize;
            let bytes = reader.take(count)?;
            if id != 0 {
                continue;
            }
            let mut section = binary::Reader::new(bytes);
            if section.name()? != "name" {
                continue;
            }
            while section.at < section.bytes.len() {
                let kind = section.byte()?;
                let count = section.u32()? as usize;
                let mut subsection = binary::Reader::new(section.take(count)?);
                if kind != 1 {
                    continue;
                }
                let count = subsection.count()?;
                for _ in 0..count {
                    let index = subsection.u32()?;
                    let name = subsection.name()?;
                    result.entry(index).or_insert(name);
                }
            }
        }
        Ok(result)
    }
    read(bytes).unwrap_or_default()
}
/** Opcode spellings come from the binary instruction, independently of any host test name. */
pub(super) fn opcode(op: u8) -> &'static str {
    match op {
        0 => "unreachable",
        1 => "nop",
        2 => "block",
        3 => "loop",
        4 => "if",
        0x0f => "return",
        0x10 => "call",
        0x11 => "call_indirect",
        0x1a => "drop",
        0x1b => "select",
        0x20 => "local.get",
        0x21 => "local.set",
        0x22 => "local.tee",
        0x23 => "global.get",
        0x24 => "global.set",
        0x41 => "i32.const",
        0x42 => "i64.const",
        0x43 => "f32.const",
        0x44 => "f64.const",
        0xd0 => "ref.null",
        0xd2 => "ref.func",
        _ => "instruction",
    }
}
/** GC operand contracts name both the operation and its particular consumed operand. */
pub(super) fn gc_opcode(op: u32) -> &'static str {
    match op {
        0 => "struct.new",
        1 => "struct.new_default",
        2 => "struct.get",
        3 => "struct.get_s",
        4 => "struct.get_u",
        5 => "struct.set",
        6 => "array.new",
        7 => "array.new_default",
        8 => "array.new_fixed",
        11 => "array.get",
        12 => "array.get_s",
        13 => "array.get_u",
        14 => "array.set",
        15 => "array.len",
        32 => "struct.new_desc",
        33 => "struct.new_default_desc",
        34 => "ref.get_desc",
        35 | 36 => "ref.cast_desc_eq",
        20 | 21 => "ref.test",
        22 | 23 => "ref.cast",
        _ => "gc instruction",
    }
}
/** Stack provenance survives intervening pushes and distinguishes local.get from local.tee failures. */
pub(super) fn producer(instruction: &Instruction) -> &'static str {
    match instruction {
        Instruction::Local(op, _) | Instruction::Global(op, _) | Instruction::Simple(op) => {
            opcode(*op)
        }
        Instruction::Constant(types::Constant::I32(_)) => "i32.const",
        Instruction::Constant(types::Constant::I64(_)) => "i64.const",
        Instruction::Constant(types::Constant::Null(_)) => "ref.null",
        Instruction::Call(_) => "call",
        Instruction::Gc(op, _) => gc_opcode(*op),
        Instruction::Block(3, ..) => "loop",
        Instruction::Block(4, ..) => "if",
        Instruction::Block(..) => "block",
        _ => "expression",
    }
}
