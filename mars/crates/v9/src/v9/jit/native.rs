/**
 * A compiled function in executable memory, and the call that runs it. Compilation is
 * all-or-nothing: a function whose code cannot be encoded or mapped stays interpreted. The block
 * table holds the absolute address of every instruction's block, and the interpreter owns it for
 * as long as the compiled function lives.
 */
use super::codegen::assemble;
use super::compiled::Outcome;
use super::constants::INSTRUCTION_BYTES;
use super::helpers::{STATUS_SHIFT, STATUS_SWITCH, take_parked};
use super::memory::ExecutableMemory;
use super::policy;
use crate::v9::compile::Code;
use crate::v9::vm::Vm;
use std::ffi::c_void;

/** The signature of a compiled unit's prologue: the Vm, the block table, the entry index. */
type Entry = extern "C" fn(*mut c_void, *const usize, u64) -> u64;

/** A compiled function: its executable region and the block addresses inside it. */
pub(in crate::v9) struct CompiledCode {
    /** The region the code was written into; it stays mapped for as long as this value lives. */
    memory: ExecutableMemory,
    /** Absolute address of each instruction's block, plus one for the end of the code. */
    table: Box<[usize]>,
}

impl CompiledCode {
    /** Runs the frame on top of `vm` from its current instruction until the code hands back. */
    pub(in crate::v9) fn enter(&self, vm: &mut Vm<'_>) -> Outcome {
        let pc = vm.frames.last().map_or(0, |frame| frame.pc) as u64;
        let word = (self.entry())(std::ptr::from_mut(vm).cast(), self.table.as_ptr(), pc);
        match word >> STATUS_SHIFT {
            STATUS_SWITCH => Outcome::Switch,
            _ => take_parked(),
        }
    }
    /** The prologue at the start of the region, as a callable function. */
    fn entry(&self) -> Entry {
        // SAFETY: `compile` wrote a prologue with this exact signature at the start of the region,
        // and the region stays mapped and unmodified while `self` is alive.
        unsafe { std::mem::transmute::<*const u8, Entry>(self.memory.base()) }
    }
}

/** Compiles a function to machine code, or `None` when it has to stay interpreted. */
pub(in crate::v9) fn compile(code: &Code) -> Option<CompiledCode> {
    let unit = assemble(code)?;
    let mut memory = ExecutableMemory::map(unit.words.len() * INSTRUCTION_BYTES).ok()?;
    memory.write(&unit.words).ok()?;
    let base = memory.base() as usize;
    let table: Box<[usize]> = unit
        .blocks
        .iter()
        .map(|&word| base + word * INSTRUCTION_BYTES)
        .collect();
    if policy::tracing() {
        eprintln!("V9 JIT compiled {} instructions", code.ops.len());
    }
    Some(CompiledCode { memory, table })
}
