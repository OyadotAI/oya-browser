#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod codegen;
pub(in crate::v9) mod compiled;
/**
 * V9's baseline JIT. Hot functions are compiled to ARM64 machine code that calls the same
 * instruction functions the interpreter runs, so the two tiers share one set of semantics. The
 * encoder and executable memory are plain Rust with no assembler dependency; the code generator
 * and tier-up policy are built on them. Off the target the JIT is not compiled at all.
 */
// The encoder and memory items are exercised by their unit tests and by the code generator; the
// allows keep builds that do not reach a path quiet.
#[allow(dead_code)]
mod constants;
#[allow(dead_code)]
pub(in crate::v9) mod encoder;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod helpers;
#[allow(dead_code)]
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(in crate::v9) mod memory;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod native;
pub(in crate::v9) mod policy;
pub(in crate::v9) mod slot;
pub(in crate::v9) use slot::JitSlot;
