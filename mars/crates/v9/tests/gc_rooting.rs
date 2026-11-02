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

/** An escaping closure must not retain unrelated temporary callbacks from its creator. */
#[test]
fn closures_release_locals_they_do_not_capture() {
    let mut environment = common::environment();
    environment.live_gc = true;
    let baseline = load_with(
        "let held;(()=>{const kept=42;held=()=>kept})();",
        &environment,
    );
    let execution = load_with(
        "let held;(()=>{const garbage=[];for(let i=0;i<128;i++)garbage.push(()=>i);const kept=42;held=()=>kept})();const r=held();",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "42");
    assert!(
        execution.state.allocation_counts()[1] <= baseline.state.allocation_counts()[1] + 2,
        "an unrelated local retained callbacks: {:?} -> {:?}",
        baseline.state.allocation_counts(),
        execution.state.allocation_counts()
    );
}

/** A returned callback that never reads this must not keep its creator's temporary receiver alive. */
#[test]
fn closures_release_receivers_they_do_not_capture() {
    let mut environment = common::environment();
    environment.live_gc = true;
    let baseline = load_with(
        "let held;(()=>{function make(){held=()=>42;return this}make.call({})})();",
        &environment,
    );
    let execution = load_with(
        "let held;(()=>{const receiver={garbage:[]};for(let i=0;i<128;i++)receiver.garbage.push(()=>i);function make(){held=()=>42;return this}make.call(receiver)})();const r=held();",
        &environment,
    );
    assert_eq!(failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "42");
    assert!(
        execution.state.allocation_counts()[1] <= baseline.state.allocation_counts()[1] + 2,
        "an unrelated receiver retained callbacks: {:?} -> {:?}",
        baseline.state.allocation_counts(),
        execution.state.allocation_counts()
    );
}

/** Native task checkpoints release dead weak-key cycles while replay snapshots preserve their identities. */
#[test]
fn live_task_collection_releases_weak_key_closures() {
    let source = "const map=new WeakMap();(()=>{for(let i=0;i<128;i++){const key={};map.set(key,()=>key);}})();";
    let mut environment = common::environment();
    environment.live_gc = true;
    let baseline = load_with("const map=new WeakMap();", &environment);
    let execution = load_with(source, &environment);
    assert_eq!(failure(&execution), None);
    assert!(
        execution.state.allocation_counts()[1] <= baseline.state.allocation_counts()[1] + 2,
        "weak-key closures survived native collection: {:?} -> {:?}",
        baseline.state.allocation_counts(),
        execution.state.allocation_counts()
    );
    environment.live_gc = false;
    let snapshot = load_with(source, &environment);
    assert!(snapshot.state.allocation_counts()[1] >= baseline.state.allocation_counts()[1] + 128);
}

/** Collection after a native task preserves live weak keys and the values its microtasks dereferenced. */
#[test]
fn live_task_collection_preserves_reachable_weak_keys() {
    let mut environment = common::environment();
    environment.live_gc = true;
    let sources = vec!["const key={};const map=new WeakMap([[key,()=>42]]);let r='';(()=>{const ref=new WeakRef({value:7});Promise.resolve().then(()=>r=ref.deref().value);})();setTimeout(()=>r+=':'+map.get(key)(),1);".to_owned()];
    let execution = load_with(&sources[0], &environment);
    assert_eq!(failure(&execution), None);
    assert_eq!(common::global(&execution, "r"), "7");
    let host = host();
    let next = mars_v9::run_timers(
        input(&sources, &host, &environment),
        round_trip(execution.state),
        1.0,
        1,
    )
    .unwrap();
    assert_eq!(failure(&next), None);
    assert_eq!(common::global(&next, "r"), "7:42");
}

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
