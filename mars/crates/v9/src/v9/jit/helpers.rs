/**
 * The helpers compiled code calls. Each one runs a single instruction through the interpreter's
 * own `step_at`, so no instruction has two implementations. A helper returns one 64-bit word: the
 * next instruction index in the low half when execution stays in compiled code, or a status in
 * the high half when control has to go back to the interpreter. Outcomes that carry a Rust value
 * (an exit, an error, a panic payload) are parked in a thread-local slot that the interpreter
 * takes right after the compiled code returns, so no Rust value crosses the C boundary.
 */
use super::compiled::Outcome;
use crate::protocol::EngineError;
use crate::v9::vm::Vm;
use std::cell::RefCell;
use std::ffi::c_void;
use std::panic::{AssertUnwindSafe, catch_unwind};

/** Status: the instruction exited the boundary frame; the parked outcome is `Exit`. */
pub(super) const STATUS_EXIT: u64 = 1;
/** Status: the instruction threw; the parked outcome is `Error`. */
pub(super) const STATUS_ERROR: u64 = 2;
/** Status: the frame stack changed; the interpreter selects the running frame again. */
pub(super) const STATUS_SWITCH: u64 = 3;
/** Status: the instruction panicked; the parked outcome holds the payload. */
pub(super) const STATUS_PANIC: u64 = 4;
/** Bit position where the status sits in the helper's return word. */
pub(super) const STATUS_SHIFT: u32 = 32;

thread_local! {
    /** The outcome of the most recent helper that handed control back, until the interpreter takes it. */
    static PARKED: RefCell<Option<Outcome>> = const { RefCell::new(None) };
}

/** Runs the instruction at `index` of the running frame in `vm` and reports where execution goes next. */
pub(super) extern "C" fn jit_step(vm: *mut c_void, index: u64) -> u64 {
    // SAFETY: `vm` is the pointer `CompiledCode::enter` derived from the interpreter's `&mut Vm`.
    // The interpreter does not touch that Vm until the compiled code returns, and the reference made
    // here lives only for this call; nothing stores it.
    let vm = unsafe { &mut *vm.cast::<Vm<'static>>() };
    match catch_unwind(AssertUnwindSafe(|| step(vm, index as usize))) {
        Ok(word) => word,
        Err(payload) => park(Outcome::Panic(payload), STATUS_PANIC),
    }
}

/** One instruction: the result classified as continue, exit, error or switch. */
fn step(vm: &mut Vm<'_>, index: usize) -> u64 {
    let depth = vm.frames.len();
    match vm.step_at(index) {
        Ok(Some(exit)) => park(Outcome::Exit(exit), STATUS_EXIT),
        Err(thrown) => park(Outcome::Error(thrown), STATUS_ERROR),
        Ok(None) if vm.frames.len() != depth => status_word(STATUS_SWITCH),
        Ok(None) => next_index(vm),
    }
}

/** The index the running frame will fetch next, or a switch when it lies past the code's end, so
the interpreter reports the same error it reports for any code that runs off its end. */
fn next_index(vm: &Vm<'_>) -> u64 {
    match vm.frames.last() {
        Some(frame) if frame.pc <= frame.code.ops.len() => frame.pc as u64,
        _ => status_word(STATUS_SWITCH),
    }
}

/** Parks an outcome for the interpreter and returns the word that says so. */
fn park(outcome: Outcome, status: u64) -> u64 {
    PARKED.with(|slot| *slot.borrow_mut() = Some(outcome));
    status_word(status)
}

/** A helper return word that carries only a status. */
fn status_word(status: u64) -> u64 {
    status << STATUS_SHIFT
}

/** Takes the outcome the last helper parked. A status without a parked outcome is a broken
hand-back, reported as an engine error rather than resumed into the wrong instruction. */
pub(super) fn take_parked() -> Outcome {
    PARKED
        .with(|slot| slot.borrow_mut().take())
        .unwrap_or_else(|| Outcome::Error(EngineError::invalid("V9 JIT lost its outcome.").into()))
}
