/**
 * Mars's own ARM64 instruction encoder: one function per instruction form the baseline JIT
 * emits, plus an `Assembler` that places labels and resolves branch fixups. Every encoding is
 * checked against the words a system assembler produces, so a wrong field shows up in a unit
 * test rather than as a crash inside generated code. Operands outside their field return an
 * error: the JIT then leaves the function interpreted instead of emitting a wrong branch.
 */
use super::constants::*;

/** One 32-bit ARM64 instruction word. */
pub(in crate::v9) type Word = u32;
/** A general register number, 0 to 31. */
pub(in crate::v9) type Reg = u8;
/** A double-precision SIMD register number, 0 to 31. */
pub(in crate::v9) type FReg = u8;

/** Register 31 as a base register or an add/sub immediate operand means the stack pointer. */
pub(in crate::v9) const SP: Reg = 31;
/** Register 31 as a data operand means the zero register. */
pub(in crate::v9) const XZR: Reg = 31;
/** The frame pointer. */
pub(in crate::v9) const FP: Reg = 29;
/** The link register. */
pub(in crate::v9) const LR: Reg = 30;
/** Scratch register for an absolute call target, caller-saved and free between helper calls. */
pub(in crate::v9) const SCRATCH: Reg = 16;

/** Why an instruction could not be encoded. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum EncodeError {
    /** An operand does not fit its instruction field. */
    OutOfRange,
    /** A branch names a label that was never bound. */
    UnboundLabel,
}

/** Result of encoding one instruction or assembling a unit. */
pub(in crate::v9) type Encoded<T> = Result<T, EncodeError>;

/** Condition codes for `b.cond`, numbered as the architecture encodes them. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum Cond {
    Eq = 0,
    Ne = 1,
    Cs = 2,
    Cc = 3,
    Mi = 4,
    Pl = 5,
    Vs = 6,
    Vc = 7,
    Hi = 8,
    Ls = 9,
    Ge = 10,
    Lt = 11,
    Gt = 12,
    Le = 13,
    Al = 14,
}

/** The register field of an instruction, checked to fit in five bits. */
fn reg(value: Reg) -> Encoded<Word> {
    if u32::from(value) <= REG_MASK {
        Ok(u32::from(value))
    } else {
        Err(EncodeError::OutOfRange)
    }
}

/** A register field placed at `shift`. */
fn field(value: Reg, shift: u32) -> Encoded<Word> {
    Ok(reg(value)? << shift)
}

/** `movz xd, #imm16, lsl #(16 * halfword)`: the first step of building a constant. */
pub(in crate::v9) fn movz(rd: Reg, imm16: u16, halfword: u32) -> Encoded<Word> {
    move_wide(OP_MOVZ, rd, imm16, halfword)
}

/** `movk xd, #imm16, lsl #(16 * halfword)`: fills one halfword and keeps the others. */
pub(in crate::v9) fn movk(rd: Reg, imm16: u16, halfword: u32) -> Encoded<Word> {
    move_wide(OP_MOVK, rd, imm16, halfword)
}

/** `movn xd, #imm16, lsl #(16 * halfword)`: a negated constant, for values with many ones. */
pub(in crate::v9) fn movn(rd: Reg, imm16: u16, halfword: u32) -> Encoded<Word> {
    move_wide(OP_MOVN, rd, imm16, halfword)
}

/** The shared shape of movz, movk and movn: halfword index, 16-bit immediate, register. */
fn move_wide(base: Word, rd: Reg, imm16: u16, halfword: u32) -> Encoded<Word> {
    if halfword >= HALFWORDS {
        return Err(EncodeError::OutOfRange);
    }
    Ok(base | (halfword << HALFWORD_SHIFT) | (u32::from(imm16) << IMM16_SHIFT) | field(rd, 0)?)
}

/** `mov xd, xm`: a register copy, encoded as `orr xd, xzr, xm`. */
pub(in crate::v9) fn mov_reg(rd: Reg, rm: Reg) -> Encoded<Word> {
    Ok(OP_MOV_REG | field(rm, RM_SHIFT)? | field(rd, 0)?)
}

/** `add xd, xn, #imm12`. */
pub(in crate::v9) fn add_imm(rd: Reg, rn: Reg, imm12: u32) -> Encoded<Word> {
    imm12_form(OP_ADD_IMM, rd, rn, imm12)
}

/** `sub xd, xn, #imm12`. */
pub(in crate::v9) fn sub_imm(rd: Reg, rn: Reg, imm12: u32) -> Encoded<Word> {
    imm12_form(OP_SUB_IMM, rd, rn, imm12)
}

/** `cmp xn, #imm12`: a subtract that discards its result and sets the flags. */
pub(in crate::v9) fn cmp_imm(rn: Reg, imm12: u32) -> Encoded<Word> {
    imm12_form(OP_SUBS_IMM, XZR, rn, imm12)
}

/** The shared shape of the add and sub immediates: destination, source, 12-bit value. */
fn imm12_form(base: Word, rd: Reg, rn: Reg, imm12: u32) -> Encoded<Word> {
    if imm12 > IMM12_MAX {
        return Err(EncodeError::OutOfRange);
    }
    Ok(base | (imm12 << IMM12_SHIFT) | field(rn, RN_SHIFT)? | field(rd, 0)?)
}

/** `add xd, xn, xm`. */
pub(in crate::v9) fn add_reg(rd: Reg, rn: Reg, rm: Reg) -> Encoded<Word> {
    reg3(OP_ADD_REG, rd, rn, rm)
}

/** `sub xd, xn, xm`. */
pub(in crate::v9) fn sub_reg(rd: Reg, rn: Reg, rm: Reg) -> Encoded<Word> {
    reg3(OP_SUB_REG, rd, rn, rm)
}

/** `cmp xn, xm`: a register subtract that discards its result and sets the flags. */
pub(in crate::v9) fn cmp_reg(rn: Reg, rm: Reg) -> Encoded<Word> {
    reg3(OP_SUBS_REG, XZR, rn, rm)
}

/** The shared shape of three-register forms: destination, first source, second source. */
fn reg3(base: Word, rd: Reg, rn: Reg, rm: Reg) -> Encoded<Word> {
    Ok(base | field(rm, RM_SHIFT)? | field(rn, RN_SHIFT)? | field(rd, 0)?)
}

/** `ldr xt, [xn, #offset]`: a 64-bit load at a multiple of 8 up to 32760. */
pub(in crate::v9) fn ldr_imm(rt: Reg, rn: Reg, offset: u32) -> Encoded<Word> {
    scaled_mem(OP_LDR_IMM, rt, rn, offset, DOUBLEWORD_SCALE)
}

/** `str xt, [xn, #offset]`: a 64-bit store at a multiple of 8 up to 32760. */
pub(in crate::v9) fn str_imm(rt: Reg, rn: Reg, offset: u32) -> Encoded<Word> {
    scaled_mem(OP_STR_IMM, rt, rn, offset, DOUBLEWORD_SCALE)
}

/** `ldr wt, [xn, #offset]`: a 32-bit load at a multiple of 4 up to 16380. */
pub(in crate::v9) fn ldr_imm_w(rt: Reg, rn: Reg, offset: u32) -> Encoded<Word> {
    scaled_mem(OP_LDR_IMM_W, rt, rn, offset, WORD_SCALE)
}

/** `str wt, [xn, #offset]`: a 32-bit store at a multiple of 4 up to 16380. */
pub(in crate::v9) fn str_imm_w(rt: Reg, rn: Reg, offset: u32) -> Encoded<Word> {
    scaled_mem(OP_STR_IMM_W, rt, rn, offset, WORD_SCALE)
}

/** The shared shape of unsigned-offset loads and stores: the field counts units of `scale`. */
fn scaled_mem(base: Word, rt: Reg, rn: Reg, offset: u32, scale: u32) -> Encoded<Word> {
    if !offset.is_multiple_of(scale) || offset / scale > UNSIGNED_OFFSET_MAX {
        return Err(EncodeError::OutOfRange);
    }
    Ok(base | ((offset / scale) << UNSIGNED_OFFSET_SHIFT) | field(rn, RN_SHIFT)? | field(rt, 0)?)
}

/** `ldr xt, [xn, xm, lsl #3]`: reads a jump-table entry indexed by a 64-bit value. */
pub(in crate::v9) fn ldr_reg_lsl3(rt: Reg, rn: Reg, rm: Reg) -> Encoded<Word> {
    reg3(OP_LDR_REG_LSL3, rt, rn, rm)
}

/** `stp xt1, xt2, [sp, #offset]!`: pre-indexed pair store, the usual prologue push. */
pub(in crate::v9) fn stp_pre(rt: Reg, rt2: Reg, rn: Reg, offset: i32) -> Encoded<Word> {
    pair(OP_STP_PRE, rt, rt2, rn, offset)
}

/** `ldp xt1, xt2, [sp], #offset`: post-indexed pair load, the usual epilogue pop. */
pub(in crate::v9) fn ldp_post(rt: Reg, rt2: Reg, rn: Reg, offset: i32) -> Encoded<Word> {
    pair(OP_LDP_POST, rt, rt2, rn, offset)
}

/** `stp xt1, xt2, [xn, #offset]`: signed-offset pair store. */
pub(in crate::v9) fn stp_off(rt: Reg, rt2: Reg, rn: Reg, offset: i32) -> Encoded<Word> {
    pair(OP_STP_OFF, rt, rt2, rn, offset)
}

/** `ldp xt1, xt2, [xn, #offset]`: signed-offset pair load. */
pub(in crate::v9) fn ldp_off(rt: Reg, rt2: Reg, rn: Reg, offset: i32) -> Encoded<Word> {
    pair(OP_LDP_OFF, rt, rt2, rn, offset)
}

/** The shared shape of pair loads and stores: a 7-bit offset in 8-byte units. */
fn pair(base: Word, rt: Reg, rt2: Reg, rn: Reg, offset: i32) -> Encoded<Word> {
    if offset.rem_euclid(PAIR_SCALE) != 0 {
        return Err(EncodeError::OutOfRange);
    }
    let units = offset / PAIR_SCALE;
    if !(PAIR_MIN..=PAIR_MAX).contains(&units) {
        return Err(EncodeError::OutOfRange);
    }
    let field_bits = (units as u32) & PAIR_FIELD_MASK;
    Ok(base
        | (field_bits << PAIR_SHIFT)
        | field(rt2, RT2_SHIFT)?
        | field(rn, RN_SHIFT)?
        | field(rt, 0)?)
}

/** `b`: unconditional branch by `words` instructions from here. */
pub(in crate::v9) fn b(words: i64) -> Encoded<Word> {
    branch26(OP_B, words)
}

/** `bl`: branch with link by `words` instructions from here. */
pub(in crate::v9) fn bl(words: i64) -> Encoded<Word> {
    branch26(OP_BL, words)
}

/** The 26-bit branch shape shared by `b` and `bl`. */
fn branch26(base: Word, words: i64) -> Encoded<Word> {
    if !(BRANCH_IMM26_MIN..=BRANCH_IMM26_MAX).contains(&words) {
        return Err(EncodeError::OutOfRange);
    }
    Ok(base | ((words as u32) & BRANCH_IMM26_MASK))
}

/** `b.cond`: conditional branch by `words` instructions from here. */
pub(in crate::v9) fn b_cond(cond: Cond, words: i64) -> Encoded<Word> {
    branch19(OP_B_COND | cond as Word, words)
}

/** `cbz xt, label`: branch when the register is zero, by `words` instructions. */
pub(in crate::v9) fn cbz(rt: Reg, words: i64) -> Encoded<Word> {
    Ok(branch19(OP_CBZ, words)? | field(rt, 0)?)
}

/** `cbnz xt, label`: branch when the register is not zero, by `words` instructions. */
pub(in crate::v9) fn cbnz(rt: Reg, words: i64) -> Encoded<Word> {
    Ok(branch19(OP_CBNZ, words)? | field(rt, 0)?)
}

/** The 19-bit branch shape shared by `b.cond`, `cbz` and `cbnz`, offset at bit 5. */
fn branch19(base: Word, words: i64) -> Encoded<Word> {
    if !(BRANCH_IMM19_MIN..=BRANCH_IMM19_MAX).contains(&words) {
        return Err(EncodeError::OutOfRange);
    }
    Ok(base | (((words as u32) & BRANCH_IMM19_MASK) << BRANCH_IMM19_SHIFT))
}

/** `br xn`: branch to the address in a register. */
pub(in crate::v9) fn br(rn: Reg) -> Encoded<Word> {
    Ok(OP_BR | field(rn, RN_SHIFT)?)
}

/** `blr xn`: call the address in a register. */
pub(in crate::v9) fn blr(rn: Reg) -> Encoded<Word> {
    Ok(OP_BLR | field(rn, RN_SHIFT)?)
}

/** `ret xn`: return to the address in a register, normally the link register. */
pub(in crate::v9) fn ret(rn: Reg) -> Encoded<Word> {
    Ok(OP_RET | field(rn, RN_SHIFT)?)
}

/** `adr xd, #offset`: a PC-relative address, offset in bytes within one megabyte. */
pub(in crate::v9) fn adr(rd: Reg, offset: i64) -> Encoded<Word> {
    if !(ADR_MIN..=ADR_MAX).contains(&offset) {
        return Err(EncodeError::OutOfRange);
    }
    let bits = (offset as u32) & ADR_OFFSET_MASK;
    let low = bits & ADR_LOW_MASK;
    let high = (bits >> 2) & ADR_HIGH_MASK;
    Ok(OP_ADR | (low << ADR_LOW_SHIFT) | (high << ADR_HIGH_SHIFT) | field(rd, 0)?)
}

/** `fmov dd, xn`: moves the bits of a 64-bit integer register into a double register. */
pub(in crate::v9) fn fmov_d_from_x(dd: FReg, xn: Reg) -> Encoded<Word> {
    Ok(OP_FMOV_D_FROM_X | field(xn, RN_SHIFT)? | field(dd, 0)?)
}

/** `fmov xd, dn`: moves the bits of a double register into a 64-bit integer register. */
pub(in crate::v9) fn fmov_x_from_d(xd: Reg, dn: FReg) -> Encoded<Word> {
    Ok(OP_FMOV_X_FROM_D | field(dn, RN_SHIFT)? | field(xd, 0)?)
}

/** `fmov dd, dn`: copies a double register. */
pub(in crate::v9) fn fmov_d(dd: FReg, dn: FReg) -> Encoded<Word> {
    Ok(OP_FMOV_D | field(dn, RN_SHIFT)? | field(dd, 0)?)
}

/** `fadd dd, dn, dm`. */
pub(in crate::v9) fn fadd_d(dd: FReg, dn: FReg, dm: FReg) -> Encoded<Word> {
    freg3(OP_FADD_D, dd, dn, dm)
}

/** `fsub dd, dn, dm`. */
pub(in crate::v9) fn fsub_d(dd: FReg, dn: FReg, dm: FReg) -> Encoded<Word> {
    freg3(OP_FSUB_D, dd, dn, dm)
}

/** `fmul dd, dn, dm`. */
pub(in crate::v9) fn fmul_d(dd: FReg, dn: FReg, dm: FReg) -> Encoded<Word> {
    freg3(OP_FMUL_D, dd, dn, dm)
}

/** `fdiv dd, dn, dm`. */
pub(in crate::v9) fn fdiv_d(dd: FReg, dn: FReg, dm: FReg) -> Encoded<Word> {
    freg3(OP_FDIV_D, dd, dn, dm)
}

/** The shared shape of double arithmetic: destination, first source, second source. */
fn freg3(base: Word, dd: FReg, dn: FReg, dm: FReg) -> Encoded<Word> {
    Ok(base | field(dm, RM_SHIFT)? | field(dn, RN_SHIFT)? | field(dd, 0)?)
}

/** `fcmp dn, dm`: compares two doubles and sets the flags as the NZCV table gives them. */
pub(in crate::v9) fn fcmp_d(dn: FReg, dm: FReg) -> Encoded<Word> {
    Ok(OP_FCMP_D | field(dm, RM_SHIFT)? | field(dn, RN_SHIFT)?)
}

/** `fcmp dn, #0.0`: compares a double against zero. */
pub(in crate::v9) fn fcmp_d_zero(dn: FReg) -> Encoded<Word> {
    Ok(OP_FCMP_D_ZERO | field(dn, RN_SHIFT)?)
}

/** Where a fixed-up branch word has to be rewritten once its label is bound. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    B,
    Bl,
    BCond(Cond),
    Cbz(Reg),
    Cbnz(Reg),
    Adr(Reg),
}

/** A jump target: an index into the assembler's label table. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) struct Label(usize);

/** A branch word waiting for its label's position. */
#[derive(Clone, Copy, Debug)]
struct Fixup {
    at: usize,
    label: Label,
    kind: Kind,
}

/** Accumulates instruction words, places labels, and resolves branches when finished. */
#[derive(Default)]
pub(in crate::v9) struct Assembler {
    words: Vec<Word>,
    labels: Vec<Option<usize>>,
    fixups: Vec<Fixup>,
}

impl Assembler {
    /** An empty assembler. */
    pub(in crate::v9) fn new() -> Self {
        Self::default()
    }
    /** Allocates a label that is not yet bound to a position. */
    pub(in crate::v9) fn label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }
    /** Binds a label to the next instruction to be emitted. */
    pub(in crate::v9) fn bind(&mut self, label: Label) {
        self.labels[label.0] = Some(self.words.len());
    }
    /** The index of the next instruction, in words from the start of the unit. */
    pub(in crate::v9) fn position(&self) -> usize {
        self.words.len()
    }
    /** Appends an already encoded instruction word. */
    pub(in crate::v9) fn emit(&mut self, word: Encoded<Word>) -> Encoded<()> {
        self.words.push(word?);
        Ok(())
    }
    /** Emits `b label`, resolved when the assembler finishes. */
    pub(in crate::v9) fn b(&mut self, label: Label) -> Encoded<()> {
        self.branch_to(label, Kind::B)
    }
    /** Emits `bl label`. */
    pub(in crate::v9) fn bl(&mut self, label: Label) -> Encoded<()> {
        self.branch_to(label, Kind::Bl)
    }
    /** Emits `b.cond label`. */
    pub(in crate::v9) fn b_cond(&mut self, cond: Cond, label: Label) -> Encoded<()> {
        self.branch_to(label, Kind::BCond(cond))
    }
    /** Emits `cbz xt, label`. */
    pub(in crate::v9) fn cbz(&mut self, rt: Reg, label: Label) -> Encoded<()> {
        self.branch_to(label, Kind::Cbz(rt))
    }
    /** Emits `cbnz xt, label`. */
    pub(in crate::v9) fn cbnz(&mut self, rt: Reg, label: Label) -> Encoded<()> {
        self.branch_to(label, Kind::Cbnz(rt))
    }
    /** Emits `adr xd, label`, for a table of addresses inside the unit. */
    pub(in crate::v9) fn adr(&mut self, rd: Reg, label: Label) -> Encoded<()> {
        self.branch_to(label, Kind::Adr(rd))
    }
    /** Records a fixup for a branch whose target is the label, and emits a placeholder word. */
    fn branch_to(&mut self, label: Label, kind: Kind) -> Encoded<()> {
        let at = self.words.len();
        self.fixups.push(Fixup { at, label, kind });
        self.words.push(0);
        Ok(())
    }
    /** Loads a 64-bit constant into a register with movz and movk, skipping zero halfwords. */
    pub(in crate::v9) fn load_imm64(&mut self, rd: Reg, value: u64) -> Encoded<()> {
        let halves: Vec<u16> = (0..HALFWORDS)
            .map(|index| (value >> (index * HALFWORD_BITS)) as u16)
            .collect();
        let first = halves.iter().position(|&half| half != 0).unwrap_or(0);
        self.emit(movz(rd, halves[first], first as u32))?;
        for (index, &half) in halves.iter().enumerate().skip(first + 1) {
            if half != 0 {
                self.emit(movk(rd, half, index as u32))?;
            }
        }
        Ok(())
    }
    /** Calls an absolute address through the scratch register. */
    pub(in crate::v9) fn call_absolute(&mut self, address: u64) -> Encoded<()> {
        self.load_imm64(SCRATCH, address)?;
        self.emit(blr(SCRATCH))
    }
    /** Resolves every fixup and returns the finished words. */
    pub(in crate::v9) fn finish(mut self) -> Encoded<Vec<Word>> {
        for fixup in std::mem::take(&mut self.fixups) {
            self.resolve(fixup)?;
        }
        Ok(self.words)
    }
    /** Rewrites one placeholder word now that its label's position is known. */
    fn resolve(&mut self, fixup: Fixup) -> Encoded<()> {
        let target = self.labels[fixup.label.0].ok_or(EncodeError::UnboundLabel)?;
        let words = target as i64 - fixup.at as i64;
        let word = match fixup.kind {
            Kind::B => b(words)?,
            Kind::Bl => bl(words)?,
            Kind::BCond(cond) => b_cond(cond, words)?,
            Kind::Cbz(rt) => cbz(rt, words)?,
            Kind::Cbnz(rt) => cbnz(rt, words)?,
            Kind::Adr(rd) => adr(rd, words * WORD_BYTES)?,
        };
        self.words[fixup.at] = word;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    // Encodings from a system assembler (`clang -arch arm64`) for the instruction forms below.
    use super::*;

    /** Each row is an encoder call and the word the assembler produced for the same text. */
    #[test]
    fn scalar_forms_match_the_assembler() {
        assert_eq!(movz(0, 42, 0), Ok(0xd280_0540));
        assert_eq!(movz(3, 0xbeef, 1), Ok(0xd2b7_dde3));
        assert_eq!(movk(5, 0x1234, 2), Ok(0xf2c2_4685));
        assert_eq!(movk(5, 0xffff, 3), Ok(0xf2ff_ffe5));
        assert_eq!(movn(1, 0, 0), Ok(0x9280_0001));
        assert_eq!(mov_reg(0, 1), Ok(0xaa01_03e0));
        assert_eq!(mov_reg(9, 30), Ok(0xaa1e_03e9));
        assert_eq!(add_imm(0, 1, 16), Ok(0x9100_4020));
        assert_eq!(add_imm(2, SP, 32), Ok(0x9100_83e2));
        assert_eq!(sub_imm(0, 1, 0xfff), Ok(0xd13f_fc20));
        assert_eq!(cmp_imm(4, 5), Ok(0xf100_149f));
        assert_eq!(add_reg(0, 1, 2), Ok(0x8b02_0020));
        assert_eq!(sub_reg(3, 4, 5), Ok(0xcb05_0083));
        assert_eq!(cmp_reg(6, 7), Ok(0xeb07_00df));
    }

    /** Loads, stores and jump-table reads, with the offsets the assembler scales them by. */
    #[test]
    fn memory_forms_match_the_assembler() {
        assert_eq!(ldr_imm(0, 1, 8), Ok(0xf940_0420));
        assert_eq!(ldr_imm(2, SP, 32_760), Ok(0xf97f_ffe2));
        assert_eq!(str_imm(3, 4, 16), Ok(0xf900_0883));
        assert_eq!(ldr_imm_w(5, 6, 4), Ok(0xb940_04c5));
        assert_eq!(str_imm_w(7, 8, 8), Ok(0xb900_0907));
        assert_eq!(ldr_reg_lsl3(9, 20, 2), Ok(0xf862_7a89));
    }

    /** Pair and frame forms: the prologue push and epilogue pop of a JIT entry. */
    #[test]
    fn pair_forms_match_the_assembler() {
        assert_eq!(stp_pre(FP, LR, SP, -16), Ok(0xa9bf_7bfd));
        assert_eq!(ldp_post(FP, LR, SP, 16), Ok(0xa8c1_7bfd));
        assert_eq!(stp_pre(19, 20, SP, -32), Ok(0xa9be_53f3));
        assert_eq!(ldp_post(19, 20, SP, 32), Ok(0xa8c2_53f3));
        assert_eq!(stp_off(21, 22, SP, 16), Ok(0xa901_5bf5));
        assert_eq!(ldp_off(21, 22, SP, 16), Ok(0xa941_5bf5));
    }

    /** Branches and returns, with offsets in words and the condition codes numbered as ARM does. */
    #[test]
    fn branch_forms_match_the_assembler() {
        assert_eq!(b(2), Ok(0x1400_0002));
        assert_eq!(bl(-4), Ok(0x97ff_fffc));
        assert_eq!(b_cond(Cond::Eq, 2), Ok(0x5400_0040));
        assert_eq!(b_cond(Cond::Ne, -2), Ok(0x54ff_ffc1));
        assert_eq!(cbz(3, 6), Ok(0xb400_00c3));
        assert_eq!(cbnz(4, -3), Ok(0xb5ff_ffa4));
        assert_eq!(br(16), Ok(0xd61f_0200));
        assert_eq!(blr(16), Ok(0xd63f_0200));
        assert_eq!(ret(LR), Ok(0xd65f_03c0));
        assert_eq!(ret(9), Ok(0xd65f_0120));
        assert_eq!(adr(0, 100), Ok(0x1000_0320));
    }

    /** Double-precision forms, for the number fast paths. */
    #[test]
    fn floating_forms_match_the_assembler() {
        assert_eq!(fmov_d_from_x(0, 1), Ok(0x9e67_0020));
        assert_eq!(fmov_x_from_d(2, 3), Ok(0x9e66_0062));
        assert_eq!(fadd_d(0, 1, 2), Ok(0x1e62_2820));
        assert_eq!(fsub_d(4, 5, 6), Ok(0x1e66_38a4));
        assert_eq!(fmul_d(7, 8, 9), Ok(0x1e69_0907));
        assert_eq!(fdiv_d(10, 11, 12), Ok(0x1e6c_196a));
        assert_eq!(fcmp_d(1, 2), Ok(0x1e62_2020));
        assert_eq!(fcmp_d_zero(3), Ok(0x1e60_2068));
        assert_eq!(fmov_d(1, 2), Ok(0x1e60_4041));
    }

    /** Operands outside their fields are refused rather than truncated into another instruction. */
    #[test]
    fn out_of_range_operands_are_refused() {
        assert_eq!(movz(32, 0, 0), Err(EncodeError::OutOfRange));
        assert_eq!(movz(0, 0, 4), Err(EncodeError::OutOfRange));
        assert_eq!(add_imm(0, 0, 0x1000), Err(EncodeError::OutOfRange));
        assert_eq!(ldr_imm(0, 0, 4), Err(EncodeError::OutOfRange));
        assert_eq!(ldr_imm(0, 0, 32_768), Err(EncodeError::OutOfRange));
        assert_eq!(stp_pre(0, 1, SP, 4), Err(EncodeError::OutOfRange));
        assert_eq!(stp_pre(0, 1, SP, 512), Err(EncodeError::OutOfRange));
        assert_eq!(b(1 << 25), Err(EncodeError::OutOfRange));
        assert_eq!(b_cond(Cond::Al, 1 << 18), Err(EncodeError::OutOfRange));
        assert_eq!(adr(0, 1 << 20), Err(EncodeError::OutOfRange));
    }

    /** A forward and a backward label resolve to the offsets the assembler gives them. */
    #[test]
    fn assembler_resolves_forward_and_backward_labels() {
        let mut asm = Assembler::new();
        let top = asm.label();
        let end = asm.label();
        asm.bind(top);
        asm.b_cond(Cond::Eq, end).unwrap();
        asm.b(top).unwrap();
        asm.bind(end);
        asm.emit(ret(LR)).unwrap();
        let words = asm.finish().unwrap();
        assert_eq!(words, vec![0x5400_0040, 0x17ff_ffff, 0xd65f_03c0]);
    }

    /** A label that is never bound is an error, never a branch to address zero. */
    #[test]
    fn unbound_label_is_an_error() {
        let mut asm = Assembler::new();
        let missing = asm.label();
        asm.b(missing).unwrap();
        assert_eq!(asm.finish(), Err(EncodeError::UnboundLabel));
    }

    /** A 64-bit constant is built from only the halfwords that are not zero. */
    #[test]
    fn load_imm64_skips_zero_halfwords() {
        let mut asm = Assembler::new();
        asm.load_imm64(3, 0x0000_1234_0000_5678).unwrap();
        let words = asm.finish().unwrap();
        assert_eq!(words, vec![0xd28a_cf03, 0xf2c2_4683]);
        let mut asm = Assembler::new();
        asm.load_imm64(0, 0).unwrap();
        assert_eq!(asm.finish().unwrap(), vec![0xd280_0000]);
    }
}
