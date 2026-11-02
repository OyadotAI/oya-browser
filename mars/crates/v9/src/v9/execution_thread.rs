/** Engine-owned threads retain execution caches and provide the stack V9's recursive parser needs. */
use crate::protocol::{EngineError, EngineResult};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::thread::{Builder, JoinHandle};

thread_local! {
    /** Only threads constructed here qualify; thread names never establish stack capacity. */
    static HAS_EXECUTION_STACK: Cell<bool> = const { Cell::new(false) };
}

/** Start a persistent executor with sufficient stack; repeated V9 calls reuse its thread caches. */
pub fn spawn_execution_thread<T: Send + 'static>(
    name: String,
    work: impl FnOnce() -> T + Send + 'static,
) -> std::io::Result<JoinHandle<T>> {
    builder(name).spawn(move || {
        HAS_EXECUTION_STACK.set(true);
        work()
    })
}

/** All certified execution threads use the same stack allowance as standalone requests. */
fn builder(name: String) -> Builder {
    Builder::new()
        .name(name)
        .stack_size(crate::constants::V9_STACK_BYTES)
}

/** A request keeps its panic boundary, whether on an owned executor or a temporary scoped thread. */
pub(super) fn run<T: Send>(work: impl FnOnce() -> EngineResult<T> + Send) -> EngineResult<T> {
    if HAS_EXECUTION_STACK.get() {
        return catch_unwind(AssertUnwindSafe(work)).unwrap_or_else(|_| Err(panicked()));
    }
    std::thread::scope(|scope| {
        builder("v9".into())
            .spawn_scoped(scope, move || {
                HAS_EXECUTION_STACK.set(true);
                work()
            })
            .map_err(|_| EngineError::unsupported("V9 could not start its execution thread."))?
            .join()
            .unwrap_or_else(|_| Err(panicked()))
    })
}

/** Panic details are internal and never become page-visible engine state. */
fn panicked() -> EngineError {
    EngineError::invalid("V9 stopped on an internal error.")
}

#[cfg(test)]
mod tests {
    use super::*;

    /** Successive tasks run on the persistent executor instead of creating new thread-local caches. */
    #[test]
    fn executor_reuses_its_thread_between_tasks() {
        spawn_execution_thread("test-v9".into(), || {
            let owner = std::thread::current().id();
            for _ in 0..2 {
                assert_eq!(run(|| Ok(std::thread::current().id())).unwrap(), owner);
            }
        })
        .unwrap()
        .join()
        .unwrap();
    }

    /** A task panic cannot kill the executor or prevent a following task from completing. */
    #[test]
    fn task_panics_leave_the_executor_usable() {
        spawn_execution_thread("test-v9".into(), || {
            assert!(run::<()>(|| panic!("task failure")).is_err());
            assert_eq!(run(|| Ok(42)).unwrap(), 42);
        })
        .unwrap()
        .join()
        .unwrap();
    }

    /** Embedders on ordinary threads retain the standalone stack and panic isolation. */
    #[test]
    fn ordinary_threads_still_get_a_separate_execution_stack() {
        let caller = std::thread::current().id();
        assert_ne!(run(|| Ok(std::thread::current().id())).unwrap(), caller);
        assert!(run::<()>(|| panic!("task failure")).is_err());
    }
}
