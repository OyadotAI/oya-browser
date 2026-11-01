/**
 * Lowers one function's instructions to ARM64 words. Each instruction gets a block that calls its
 * helper and then dispatches on the index the helper returned, through a table of block addresses
 * the interpreter owns. Forward jumps have no side effect beyond moving the program counter, so
 * they become direct branches. The unit is entered at any instruction, which is what lets the
 * interpreter hand a running frame to compiled code mid-function.
 */
use super::encoder::{
    Assembler, EncodeError, Encoded, FP, LR, Label, Reg, SP, Word, add_imm, br, ldp_off, ldp_post,
    ldr_reg_lsl3, lsr_imm, mov_reg, mov_reg_w, ret, stp_off, stp_pre,
};
use super::helpers::{STATUS_SHIFT, jit_step};
use crate::v9::compile::{Code, Op};

/** The register that holds the `Vm` pointer for the whole compiled activation. */
const VM_REG: Reg = 19;
/** The register that holds the block table for the whole compiled activation. */
const TABLE_REG: Reg = 20;
/** Argument register that carries the entry instruction index, and the dispatch index. */
const INDEX_REG: Reg = 2;
/** Scratch register for a block's dispatch address. */
const TARGET_REG: Reg = 9;
/** Scratch register that holds the helper's status while it is tested. */
const STATUS_REG: Reg = 1;
/** Register the helper's return word arrives in, and the one the epilogue hands back. */
const RESULT_REG: Reg = 0;
/** Argument register for the instruction index passed to the helper. */
const HELPER_INDEX_REG: Reg = 1;
/** Bytes the prologue reserves for the frame record and the two saved registers. */
const FRAME_BYTES: i32 = 32;
/** Offset of the second saved pair inside the prologue's reserved bytes. */
const SAVED_PAIR_OFFSET: i32 = 16;

/** Words for one function, with the word offset where each instruction's block begins. */
pub(super) struct Unit {
    /** The instruction words, ready to copy into executable memory. */
    pub words: Vec<Word>,
    /** Word offset of each block, one per instruction plus one for the end of the code. */
    pub blocks: Vec<usize>,
}

/** Assembles a whole function, or `None` when some branch or immediate cannot be encoded. */
pub(super) fn assemble(code: &Code) -> Option<Unit> {
    let end = code.ops.len();
    let mut asm = Assembler::new();
    let exit = asm.label();
    let blocks: Vec<Label> = (0..=end).map(|_| asm.label()).collect();
    emit_prologue(&mut asm).ok()?;
    emit_epilogue(&mut asm, exit).ok()?;
    let mut offsets = Vec::with_capacity(end + 1);
    for (index, label) in blocks.iter().enumerate() {
        asm.bind(*label);
        offsets.push(asm.position());
        emit_block(&mut asm, index, code.ops.get(index), &blocks, exit).ok()?;
    }
    Some(Unit {
        words: asm.finish().ok()?,
        blocks: offsets,
    })
}

/** Saves the callee-saved registers, takes the Vm and table, and jumps to the entry instruction. */
fn emit_prologue(asm: &mut Assembler) -> Encoded<()> {
    asm.emit(stp_pre(FP, LR, SP, -FRAME_BYTES))?;
    asm.emit(stp_off(VM_REG, TABLE_REG, SP, SAVED_PAIR_OFFSET))?;
    asm.emit(add_imm(FP, SP, 0))?;
    asm.emit(mov_reg(VM_REG, 0))?;
    asm.emit(mov_reg(TABLE_REG, 1))?;
    asm.emit(ldr_reg_lsl3(TARGET_REG, TABLE_REG, INDEX_REG))?;
    asm.emit(br(TARGET_REG))
}

/** Returns the helper's status word to the interpreter, restoring the registers the prologue saved. */
fn emit_epilogue(asm: &mut Assembler, exit: Label) -> Encoded<()> {
    asm.bind(exit);
    asm.emit(ldp_off(VM_REG, TABLE_REG, SP, SAVED_PAIR_OFFSET))?;
    asm.emit(ldp_post(FP, LR, SP, FRAME_BYTES))?;
    asm.emit(ret(LR))
}

/** One instruction's block: a direct branch for a forward jump, otherwise a helper call. */
fn emit_block(
    asm: &mut Assembler,
    index: usize,
    op: Option<&Op>,
    blocks: &[Label],
    exit: Label,
) -> Encoded<()> {
    match op {
        Some(Op::Jump(target)) if (*target as usize) > index => {
            let label = blocks
                .get(*target as usize)
                .ok_or(EncodeError::OutOfRange)?;
            asm.b(*label)
        }
        _ => emit_step(asm, index, exit),
    }
}

/** Calls the helper for one instruction, leaves on any status but continue, and otherwise
dispatches to the block of the index the helper returned. */
fn emit_step(asm: &mut Assembler, index: usize, exit: Label) -> Encoded<()> {
    asm.emit(mov_reg(0, VM_REG))?;
    asm.load_imm64(HELPER_INDEX_REG, index as u64)?;
    asm.call_absolute(jit_step as *const () as usize as u64)?;
    asm.emit(lsr_imm(STATUS_REG, RESULT_REG, STATUS_SHIFT))?;
    let next = asm.label();
    asm.cbz(STATUS_REG, next)?;
    asm.b(exit)?;
    asm.bind(next);
    asm.emit(mov_reg_w(INDEX_REG, RESULT_REG))?;
    asm.emit(ldr_reg_lsl3(TARGET_REG, TABLE_REG, INDEX_REG))?;
    asm.emit(br(TARGET_REG))
}
