/**
 * V9's baseline JIT. Hot functions are compiled to ARM64 machine code that calls the same
 * instruction functions the interpreter runs, so the two tiers share one set of semantics. The
 * encoder and executable memory are plain Rust with no assembler dependency; the code generator
 * and tier-up policy are built on them. Off the target the JIT is not compiled at all.
 */
// Until the tier-up path (a later stage) calls these, the encoder and memory items are unused in
// a normal build; the unit tests exercise every one of them.
#[allow(dead_code)]
mod constants;
#[allow(dead_code)]
pub(in crate::v9) mod encoder;
#[allow(dead_code)]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(in crate::v9) mod memory;
