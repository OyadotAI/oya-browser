/**
 * The hand-back protocol between compiled code and the interpreter, and the compiled function
 * type. On the supported target a function compiles to machine code; elsewhere `CompiledCode` has
 * no value, `compile` refuses every function, and the interpreter runs everything.
 */
use crate::v9::vm::{Exit, Thrown};

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub(in crate::v9) use super::native::{CompiledCode, compile};

/** A compiled function. Off the supported target none can exist, so the type has no values. */
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub(in crate::v9) enum CompiledCode {}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
impl CompiledCode {
    /** Unreachable: no compiled function exists off the supported target. */
    pub(in crate::v9) fn enter(&self, _vm: &mut crate::v9::vm::Vm<'_>) -> Outcome {
        match *self {}
    }
}

/** Compiles a function, or returns `None` to keep it interpreted. */
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
pub(in crate::v9) fn compile(_code: &crate::v9::compile::Code) -> Option<CompiledCode> {
    None
}

/** Why compiled code returned control to the interpreter. */
#[allow(dead_code)]
pub(in crate::v9) enum Outcome {
    /** The frame stack changed; the interpreter re-selects the running frame. */
    Switch,
    /** The boundary frame returned, yielded or awaited. */
    Exit(Exit),
    /** An instruction threw; the interpreter unwinds exactly as it would after its own throw. */
    Error(Thrown),
    /** An instruction panicked; the payload is resumed on the Rust side, not unwound through code. */
    Panic(Box<dyn std::any::Any + Send>),
}
