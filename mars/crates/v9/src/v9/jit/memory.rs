/**
 * Executable memory for compiled code. On macOS arm64 a region is mapped `MAP_JIT`, so the
 * kernel allows it to be both written and executed. The owning thread drops write protection
 * only around the copy (Apple requires that per thread), then invalidates the instruction cache
 * before any branch can reach the new words. The total mapped size is bounded, and a region
 * unmaps when its owner drops. This module exists only on the target the JIT runs on.
 */
use super::constants::{INSTRUCTION_BYTES, MAX_CODE_BYTES, PAGE_BYTES};
use std::ffi::{c_int, c_void};
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering};

/** `mmap` protection: pages may be read. */
const PROT_READ: c_int = 0x1;
/** `mmap` protection: pages may be written. */
const PROT_WRITE: c_int = 0x2;
/** `mmap` protection: pages may be executed. */
const PROT_EXEC: c_int = 0x4;
/** `mmap` flag: a private copy-on-write mapping. */
const MAP_PRIVATE: c_int = 0x0002;
/** `mmap` flag: anonymous memory, not backed by a file. */
const MAP_ANON: c_int = 0x1000;
/** `mmap` flag: a region the JIT may write and execute, which macOS requires for executable code. */
const MAP_JIT: c_int = 0x0800;
/** The value `mmap` returns on failure. */
const MAP_FAILED: *mut c_void = usize::MAX as *mut c_void;
/** `pthread_jit_write_protect_np` argument that makes the calling thread's JIT pages writable. */
const JIT_WRITABLE: c_int = 0;
/** `pthread_jit_write_protect_np` argument that makes the calling thread's JIT pages executable. */
const JIT_EXECUTABLE: c_int = 1;

unsafe extern "C" {
    /** Maps anonymous memory; see mmap(2). */
    fn mmap(
        address: *mut c_void,
        length: usize,
        protection: c_int,
        flags: c_int,
        fd: c_int,
        offset: i64,
    ) -> *mut c_void;
    /** Unmaps a region returned by `mmap`. */
    fn munmap(address: *mut c_void, length: usize) -> c_int;
    /** Toggles the calling thread's write protection on `MAP_JIT` pages. */
    fn pthread_jit_write_protect_np(enabled: c_int);
    /** Makes instruction-cache contents of a range match its data. */
    fn sys_icache_invalidate(start: *mut c_void, length: usize);
}

/** Bytes of executable memory currently mapped by every live region, across threads. */
static MAPPED_BYTES: AtomicUsize = AtomicUsize::new(0);

/** Why a region could not be made or written. */
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::v9) enum MemoryError {
    /** Mapping would take the JIT past its total budget. */
    Budget,
    /** The kernel refused the mapping. */
    Map,
    /** The words do not fit the region.*/
    TooLarge,
}

/** One mapped region of executable memory, unmapped on drop. */
pub(in crate::v9) struct ExecutableMemory {
    base: NonNull<u8>,
    length: usize,
}

// SAFETY: the region is owned and never aliased; it is written only before it is published, and
// the code in it does not depend on the thread that mapped it, so it may move between threads.
unsafe impl Send for ExecutableMemory {}
// SAFETY: after `write` returns the bytes are never modified again, so shared reads are sound.
unsafe impl Sync for ExecutableMemory {}

impl ExecutableMemory {
    /** Maps a region of at least `bytes` bytes, rounded up to whole pages, within the budget. */
    pub(in crate::v9) fn map(bytes: usize) -> Result<Self, MemoryError> {
        let length = round_up(bytes.max(PAGE_BYTES), PAGE_BYTES);
        reserve(length)?;
        // SAFETY: an anonymous mapping with no address hint; the kernel picks the range, and on
        // failure we return before touching it.
        let address = unsafe {
            mmap(
                std::ptr::null_mut(),
                length,
                PROT_READ | PROT_WRITE | PROT_EXEC,
                MAP_PRIVATE | MAP_ANON | MAP_JIT,
                -1,
                0,
            )
        };
        if address == MAP_FAILED {
            release(length);
            return Err(MemoryError::Map);
        }
        let base = NonNull::new(address.cast::<u8>()).ok_or(MemoryError::Map)?;
        Ok(Self { base, length })
    }
    /** Copies instruction words to the start of the region and makes them visible to fetch. */
    pub(in crate::v9) fn write(&mut self, words: &[u32]) -> Result<(), MemoryError> {
        let bytes = words.len() * INSTRUCTION_BYTES;
        if bytes > self.length {
            return Err(MemoryError::TooLarge);
        }
        // SAFETY: the thread's JIT pages are made writable for this copy only; the destination
        // is inside the region we mapped, which is at least `bytes` long, and the source is a
        // valid slice. Protection is restored and the icache invalidated before returning.
        unsafe {
            pthread_jit_write_protect_np(JIT_WRITABLE);
            std::ptr::copy_nonoverlapping(words.as_ptr().cast::<u8>(), self.base.as_ptr(), bytes);
            pthread_jit_write_protect_np(JIT_EXECUTABLE);
            sys_icache_invalidate(self.base.as_ptr().cast(), bytes);
        }
        Ok(())
    }
    /** The first byte of the region, the entry point of the code written at its start. */
    pub(in crate::v9) fn base(&self) -> *const u8 {
        self.base.as_ptr()
    }
}

impl Drop for ExecutableMemory {
    /** Unmaps the region and returns its bytes to the budget; a failed unmap leaks nothing we own. */
    fn drop(&mut self) {
        // SAFETY: `base` and `length` are exactly what `mmap` returned, and nothing else
        // references the region once its owner is dropped.
        unsafe {
            munmap(self.base.as_ptr().cast(), self.length);
        }
        release(self.length);
    }
}

/** Rounds `value` up to a multiple of `unit`. */
fn round_up(value: usize, unit: usize) -> usize {
    value.div_ceil(unit) * unit
}

/** Claims `length` bytes of the budget, or refuses if the total would pass the limit. */
fn reserve(length: usize) -> Result<(), MemoryError> {
    let mut current = MAPPED_BYTES.load(Ordering::Relaxed);
    loop {
        let next = match current.checked_add(length) {
            Some(next) if next <= MAX_CODE_BYTES => next,
            _ => return Err(MemoryError::Budget),
        };
        match MAPPED_BYTES.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            Ok(_) => return Ok(()),
            Err(actual) => current = actual,
        }
    }
}

/** Returns `length` bytes to the budget. */
fn release(length: usize) {
    MAPPED_BYTES.fetch_sub(length, Ordering::Relaxed);
}

#[cfg(test)]
mod tests {
    /** The tests run only where the JIT does, and map real pages. */
    use super::*;

    /** `movz x0, #42` then `ret`: the smallest function that returns a value. */
    const RETURN_42: [u32; 2] = [0xD280_0540, 0xD65F_03C0];

    /** Runs the code at the region's base and returns what it left in x0. */
    fn call(base: *const u8) -> u64 {
        // SAFETY: the test wrote a well-formed leaf function at `base` that takes no arguments
        // and returns a 64-bit value in x0, so the signature matches what the code does.
        let function: extern "C" fn() -> u64 = unsafe { std::mem::transmute(base) };
        function()
    }

    /** A written region runs: the words reach fetch through the write-protect toggle. */
    #[test]
    fn written_code_runs() {
        let mut region = ExecutableMemory::map(PAGE_BYTES).expect("map");
        region.write(&RETURN_42).expect("write");
        assert_eq!(call(region.base()), 42);
    }

    /** Rewriting a region replaces the code the next call runs. */
    #[test]
    fn rewritten_code_runs_the_new_words() {
        let mut region = ExecutableMemory::map(PAGE_BYTES).expect("map");
        region.write(&RETURN_42).expect("write");
        assert_eq!(call(region.base()), 42);
        region.write(&[0xD280_0F60, 0xD65F_03C0]).expect("rewrite");
        assert_eq!(call(region.base()), 123);
    }

    /** A region is never written past its length, so an oversized unit is refused. */
    #[test]
    fn oversized_write_is_refused() {
        let mut region = ExecutableMemory::map(1).expect("map");
        let words = vec![0xD503_201F; PAGE_BYTES / INSTRUCTION_BYTES + 1];
        assert_eq!(region.write(&words), Err(MemoryError::TooLarge));
    }

    /** The budget refuses a region larger than the whole budget, and maps nothing. */
    #[test]
    fn budget_refuses_an_oversized_region() {
        assert_eq!(
            ExecutableMemory::map(MAX_CODE_BYTES + 1).err(),
            Some(MemoryError::Budget)
        );
    }

    /** Sizes round up to whole pages, so a tiny request still gets a page. */
    #[test]
    fn sizes_round_to_pages() {
        assert_eq!(round_up(1, PAGE_BYTES), PAGE_BYTES);
        assert_eq!(round_up(PAGE_BYTES, PAGE_BYTES), PAGE_BYTES);
        assert_eq!(round_up(PAGE_BYTES + 1, PAGE_BYTES), 2 * PAGE_BYTES);
    }
}
