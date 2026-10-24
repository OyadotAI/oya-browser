/** Concurrent subresource fetches over a bounded set of worker threads. */
use super::Prepared;
use crate::network::constants::BATCH_WORKERS;
use crate::network::{Method, WebRequest, WebResponse};
use crate::protocol::{EngineError, EngineResult};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

/** One result slot per request, filled by whichever worker took it. */
type Slot = Mutex<Option<EngineResult<WebResponse>>>;

/// Fetches independent GET subresources (stylesheets, scripts, images) concurrently on at most
/// `BATCH_WORKERS` threads, each through the same `Prepared` path, policy, cache and pool as a
/// single fetch. Results keep request order. POSTs are refused: a submission is one explicit,
/// separately admitted exchange. Each request gets `remaining_ms` from the batch's start.
pub fn fetch_batch(
    requests: Vec<WebRequest>,
    grants: &[String],
    remaining_ms: u64,
) -> Vec<EngineResult<WebResponse>> {
    let start = std::time::Instant::now();
    let next = AtomicUsize::new(0);
    let slots: Vec<Slot> = requests.iter().map(|_| Mutex::new(None)).collect();
    std::thread::scope(|scope| {
        for _ in 0..BATCH_WORKERS.min(requests.len()) {
            scope.spawn(|| {
                loop {
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(request) = requests.get(index) else {
                        return;
                    };
                    let spent = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let result =
                        fetch_one(request.clone(), grants, remaining_ms.saturating_sub(spent));
                    if let Ok(mut slot) = slots[index].lock() {
                        *slot = Some(result);
                    }
                }
            });
        }
    });
    slots
        .into_iter()
        .map(|slot| {
            slot.into_inner()
                .ok()
                .flatten()
                .unwrap_or_else(|| Err(EngineError::service("network", false)))
        })
        .collect()
}

/** One GET through the ordinary prepare and transmit steps. */
fn fetch_one(
    request: WebRequest,
    grants: &[String],
    remaining_ms: u64,
) -> EngineResult<WebResponse> {
    if request.method != Method::Get {
        return Err(EngineError::invalid("Batch fetches are GET only."));
    }
    Prepared::connect(request, grants, remaining_ms)?.transmit()
}
