/**
 * Script compilation that overlaps a page load. Once the head's scripts have arrived, a thread
 * with V9's stack compiles them into the process code cache while the coordinator fetches the
 * sheets, styles the document and resolves its images; the load step then finds every classic
 * script's code cached and only runs it. The thread is joined before the scripts run, so a
 * compile still in progress is waited for rather than repeated on V9's thread.
 */
use crate::constants::{SCRIPT_BYTES, SCRIPT_COUNT, SCRIPT_TOTAL_BYTES};
use std::sync::Arc;
use std::thread::JoinHandle;

/** The page's classic scripts compiling on their own thread; None when none qualified. */
pub(super) struct Precompile(Option<JoinHandle<()>>);

impl Precompile {
    /** Start compiling what V9's load step will compile: the classic scripts within the page's
    script allowances, at the page offsets V9 assigns them (each source plus one separator). */
    pub fn start(sources: Arc<Vec<String>>, classic: &[bool]) -> Self {
        let jobs = eligible(&sources, classic);
        if jobs.is_empty() {
            return Self(None);
        }
        let handle = crate::v9::spawn_execution_thread("oya-precompile".into(), move || {
            for (index, offset) in jobs {
                let _ = crate::v9::precompile_script(&sources[index], offset);
            }
        });
        Self(handle.ok())
    }
    /** Wait for the compile, so the load step reads the cache rather than racing it. */
    pub fn finish(self) {
        if let Some(handle) = self.0 {
            let _ = handle.join();
        }
    }
}

/** Index and page offset of each script V9's `compile_sources` would compile, under the same
allowances, so the cache holds exactly the units the load step asks for. */
fn eligible(sources: &[String], classic: &[bool]) -> Vec<(usize, usize)> {
    let mut offset = 0;
    let mut total = 0;
    let mut jobs = Vec::new();
    for (index, source) in sources.iter().enumerate().take(SCRIPT_COUNT) {
        total += source.len();
        if total > SCRIPT_TOTAL_BYTES {
            break;
        }
        let wanted = classic.get(index).copied().unwrap_or(false);
        if wanted && !source.is_empty() && source.len() <= SCRIPT_BYTES {
            jobs.push((index, offset));
        }
        offset += source.len() + 1;
    }
    jobs
}
