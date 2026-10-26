/** WebAssembly's binary constants and resource limits derived from V9's existing heap bounds. */
pub(super) const MAGIC: &[u8] = b"\0asm\x01\0\0\0";
pub(super) const PAGE_BYTES: usize = 65_536;
pub(super) const BUFFER_BYTES: usize = crate::constants::JS_STRING_UNITS * 2;
pub(super) const MAX_PAGES: usize = BUFFER_BYTES / PAGE_BYTES;
pub(super) const MAX_WASM_PAGES: u32 = 65_536;
pub(super) const MAX_TABLE: usize = crate::constants::VM_ARRAY_LENGTH;
pub(super) const MAX_LOCALS: usize = crate::constants::VM_BINDINGS;
pub(super) const MAX_DEPTH: usize = crate::constants::VM_CALL_DEPTH;
pub(super) const MODULE: &str = "wasm.module";
pub(super) const INSTANCE: &str = "wasm.instance";
pub(super) const MEMORY: &str = "wasm.memory";
pub(super) const TABLE: &str = "wasm.table";
pub(super) const GLOBAL: &str = "wasm.global";
pub(super) const MAXIMUM: &str = "wasm.maximum";
pub(super) const MUTABLE: &str = "wasm.mutable";
pub(super) const VALUE_TYPE: &str = "wasm.type";
pub(super) const FUNCTIONS: &str = "wasm.functions";
pub(super) const MEMORIES: &str = "wasm.memories";
pub(super) const TABLES: &str = "wasm.tables";
pub(super) const GLOBALS: &str = "wasm.globals";

/** Engine branch-table count is bounded below the binary format maximum to keep decoding tractable. */
pub(super) const MAX_BRANCH_TABLE: usize = 65_520;

/** Declared table minima follow the pinned engine's binary implementation limit; live allocation stays smaller. */
pub(super) const MAX_DECLARED_TABLE_MINIMUM: u64 = 10_000_000;
