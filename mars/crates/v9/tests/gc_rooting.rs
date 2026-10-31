/**
 * The collector runs only where every live value is visible: at an instruction boundary, or
 * inside a native whose allocations are pinned. A full heap noticed by the interpreter (an
 * `await` making its promise and resolving functions) must not collect the promise out from
 * under the Rust locals that still hold it, or the committed state carries a dangling
 * reference and the next request refuses it.
 */
use crate::common;

use common::{failure, host, input, load_with, round_trip};
use mars_v9::{PageEvent, constants::VM_FUNCTIONS};

/** Fill the heap to the function bound, then await repeatedly so collections happen while
promises are half built; the committed state must still open and the awaits must settle. */
#[test]
fn collections_during_await_keep_the_state_consistent() {
    let environment = common::environment();
    let sources = vec![format!(
        "const keep = [[], [], [], [], [], [], [], []];
         for (var i = 0; i < {fill}; i++) keep[i % 8].push(function () {{}});
         let settled = 0;
         async function step(n) {{ await null; settled += 1; return n }}
         for (var j = 0; j < 150; j++) {{ step(j); keep[j % 8].pop(); keep[j % 8].pop(); }}",
        fill = VM_FUNCTIONS - 200 - common::empty_page_functions()
    )];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(common::global(&execution, "settled"), "150");
    let host = host();
    let next = mars_v9::page_event(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        PageEvent::Load,
    )
    .expect("the committed state has no dangling reference");
    assert_eq!(common::global(&next, "settled"), "150");
}
