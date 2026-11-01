/**
 * Named constants for the baseline JIT: ARM64 opcode bases, field widths, the tier-up policy and
 * the executable-memory budget. Every number the JIT uses is named here so the encoder and the
 * code generator read as steps rather than bit patterns.
 */
/** Movz, 64-bit: the base word before the halfword, immediate and register fields are OR-ed in. */
pub(super) const OP_MOVZ: u32 = 0xD280_0000;
/** Movk, 64-bit. */
pub(super) const OP_MOVK: u32 = 0xF280_0000;
/** Movn, 64-bit. */
pub(super) const OP_MOVN: u32 = 0x9280_0000;
/** Orr with the zero register, 64-bit: the register move. */
pub(super) const OP_MOV_REG: u32 = 0xAA00_03E0;
/** Add immediate, 64-bit. */
pub(super) const OP_ADD_IMM: u32 = 0x9100_0000;
/** Subtract immediate, 64-bit. */
pub(super) const OP_SUB_IMM: u32 = 0xD100_0000;
/** Subtract immediate setting flags, 64-bit: `cmp` when the destination is the zero register. */
pub(super) const OP_SUBS_IMM: u32 = 0xF100_0000;
/** Add register, 64-bit. */
pub(super) const OP_ADD_REG: u32 = 0x8B00_0000;
/** Subtract register, 64-bit. */
pub(super) const OP_SUB_REG: u32 = 0xCB00_0000;
/** Subtract register setting flags, 64-bit. */
pub(super) const OP_SUBS_REG: u32 = 0xEB00_0000;
/** Load unsigned offset, 64-bit. */
pub(super) const OP_LDR_IMM: u32 = 0xF940_0000;
/** Store unsigned offset, 64-bit. */
pub(super) const OP_STR_IMM: u32 = 0xF900_0000;
/** Load unsigned offset, 32-bit. */
pub(super) const OP_LDR_IMM_W: u32 = 0xB940_0000;
/** Store unsigned offset, 32-bit. */
pub(super) const OP_STR_IMM_W: u32 = 0xB900_0000;
/** Load register offset scaled by 8, 64-bit: a jump-table read. */
pub(super) const OP_LDR_REG_LSL3: u32 = 0xF860_7800;
/** Store pair, pre-index, 64-bit. */
pub(super) const OP_STP_PRE: u32 = 0xA980_0000;
/** Load pair, post-index, 64-bit. */
pub(super) const OP_LDP_POST: u32 = 0xA8C0_0000;
/** Store pair, signed offset, 64-bit. */
pub(super) const OP_STP_OFF: u32 = 0xA900_0000;
/** Load pair, signed offset, 64-bit. */
pub(super) const OP_LDP_OFF: u32 = 0xA940_0000;
/** Unconditional branch. */
pub(super) const OP_B: u32 = 0x1400_0000;
/** Branch with link. */
pub(super) const OP_BL: u32 = 0x9400_0000;
/** Conditional branch. */
pub(super) const OP_B_COND: u32 = 0x5400_0000;
/** Compare and branch if zero, 64-bit. */
pub(super) const OP_CBZ: u32 = 0xB400_0000;
/** Compare and branch if not zero, 64-bit. */
pub(super) const OP_CBNZ: u32 = 0xB500_0000;
/** Branch to register. */
pub(super) const OP_BR: u32 = 0xD61F_0000;
/** Branch with link to register. */
pub(super) const OP_BLR: u32 = 0xD63F_0000;
/** Return to register. */
pub(super) const OP_RET: u32 = 0xD65F_0000;
/** Form a PC-relative address. */
pub(super) const OP_ADR: u32 = 0x1000_0000;
/** Move a 64-bit general register to a double register. */
pub(super) const OP_FMOV_D_FROM_X: u32 = 0x9E67_0000;
/** Move a double register to a 64-bit general register. */
pub(super) const OP_FMOV_X_FROM_D: u32 = 0x9E66_0000;
/** Double add. */
pub(super) const OP_FADD_D: u32 = 0x1E60_2800;
/** Double subtract. */
pub(super) const OP_FSUB_D: u32 = 0x1E60_3800;
/** Double multiply. */
pub(super) const OP_FMUL_D: u32 = 0x1E60_0800;
/** Double divide. */
pub(super) const OP_FDIV_D: u32 = 0x1E60_1800;
/** Double compare of two registers. */
pub(super) const OP_FCMP_D: u32 = 0x1E60_2000;
/** Double compare against zero. */
pub(super) const OP_FCMP_D_ZERO: u32 = 0x1E60_2008;
/** Double register move. */
pub(super) const OP_FMOV_D: u32 = 0x1E60_4000;

/** Width of a general register field, in bits. */
pub(super) const REG_BITS: u32 = 5;
/** Mask selecting one register field. */
pub(super) const REG_MASK: u32 = 0x1F;
/** Halfword index field of movz, movk and movn. */
pub(super) const HALFWORD_SHIFT: u32 = 21;
/** Immediate field of movz, movk, movn. */
pub(super) const IMM16_SHIFT: u32 = 5;
/** Largest value of a 16-bit immediate. */
pub(super) const IMM16_MAX: u64 = 0xFFFF;
/** Number of 16-bit halfwords in a 64-bit constant. */
pub(super) const HALFWORDS: u32 = 4;
/** Bit width of one halfword. */
pub(super) const HALFWORD_BITS: u32 = 16;
/** Immediate field of add and sub, 12 bits wide. */
pub(super) const IMM12_MAX: u32 = 0xFFF;
/** Shift of the 12-bit immediate in add and sub. */
pub(super) const IMM12_SHIFT: u32 = 10;
/** Register field positions in three-register and two-register forms. */
pub(super) const RN_SHIFT: u32 = 5;
pub(super) const RM_SHIFT: u32 = 16;
/** Scale of the unsigned 64-bit load and store offset: the field counts 8-byte units. */
pub(super) const DOUBLEWORD_SCALE: u32 = 8;
/** Scale of the unsigned 32-bit load and store offset: the field counts 4-byte units. */
pub(super) const WORD_SCALE: u32 = 4;
/** Largest unsigned scaled offset field, 12 bits. */
pub(super) const UNSIGNED_OFFSET_MAX: u32 = 0xFFF;
/** Shift of the unsigned offset field. */
pub(super) const UNSIGNED_OFFSET_SHIFT: u32 = 10;
/** Pair offset field: 7 bits signed, counted in 8-byte units. */
pub(super) const PAIR_SCALE: i32 = 8;
pub(super) const PAIR_MIN: i32 = -64;
pub(super) const PAIR_MAX: i32 = 63;
pub(super) const PAIR_FIELD_MASK: u32 = 0x7F;
pub(super) const PAIR_SHIFT: u32 = 15;
/** Second register of a pair. */
pub(super) const RT2_SHIFT: u32 = 10;
/** Branch offsets are counted in 4-byte words. */
pub(super) const WORD_BYTES: i64 = 4;
/** Unconditional branch offset: 26 signed bits of words. */
pub(super) const BRANCH_IMM26_MASK: u32 = 0x03FF_FFFF;
pub(super) const BRANCH_IMM26_MIN: i64 = -(1 << 25);
pub(super) const BRANCH_IMM26_MAX: i64 = (1 << 25) - 1;
/** Conditional and compare-and-branch offset: 19 signed bits of words, at bit 5. */
pub(super) const BRANCH_IMM19_MASK: u32 = 0x7_FFFF;
pub(super) const BRANCH_IMM19_MIN: i64 = -(1 << 18);
pub(super) const BRANCH_IMM19_MAX: i64 = (1 << 18) - 1;
pub(super) const BRANCH_IMM19_SHIFT: u32 = 5;
/** ADR offset: 21 signed bits of bytes, split into a 2-bit low part and a 19-bit high part. */
pub(super) const ADR_MIN: i64 = -(1 << 20);
pub(super) const ADR_MAX: i64 = (1 << 20) - 1;
/** The 21 offset bits an ADR carries, after the sign is dropped. */
pub(super) const ADR_OFFSET_MASK: u32 = 0x1F_FFFF;
pub(super) const ADR_LOW_MASK: u32 = 0x3;
pub(super) const ADR_LOW_SHIFT: u32 = 29;
pub(super) const ADR_HIGH_MASK: u32 = 0x7_FFFF;
pub(super) const ADR_HIGH_SHIFT: u32 = 5;
/** Condition field of b.cond. */
pub(super) const COND_BITS: u32 = 0xF;

/** Largest executable memory the JIT may hold across all compiled functions, in bytes. */
pub(super) const MAX_CODE_BYTES: usize = 64 * 1024 * 1024;
/** Page size the executable mappings round to. */
pub(super) const PAGE_BYTES: usize = 16 * 1024;
/** Calls, and taken backward jumps, a function must reach before it is compiled. */
pub(super) const TIER_UP_THRESHOLD: u32 = 1_000;
/** Environment variable that turns the JIT on (`1`) or off (`0`). */
pub(super) const JIT_ENV: &str = "MARS_V9_JIT";
/** Environment variable that overrides the tier-up threshold, used to force compilation in tests. */
pub(super) const JIT_THRESHOLD_ENV: &str = "MARS_V9_JIT_THRESHOLD";
