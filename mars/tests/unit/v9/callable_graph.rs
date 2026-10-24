/** Callable graph boundaries reject forged authority and preserve only genuinely reachable bound state. */
use mars::v9::{VmState, event, load};
use serde_json::{Value, json};

/** Every event crosses the same serialized state boundary used by the V9 service. */
fn next(source: &str, state: VmState) -> mars::v9::Execution {
    event(
        source,
        "run()",
        serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap(),
        &[],
    )
    .unwrap()
}
/** Private wire records are inspected only to simulate hostile service input through public APIs. */
fn wire(source: &str) -> Value {
    serde_json::to_value(load(source, &[]).unwrap().state).unwrap()
}
/** Shape-valid hostile snapshots must fail before any script body executes. */
fn rejected(source: &str, wire: Value) {
    assert_eq!(
        event(source, "run()", serde_json::from_value(wire).unwrap(), &[])
            .unwrap_err()
            .code,
        "invalid_request"
    );
}
/** Bound target edges cannot cycle or point outside the heap, and bound values cannot dangle. */
#[test]
fn bound_cycles_and_dangling_owned_values_are_rejected_before_execution() {
    let source = "function f(x){return x;}const bound=f.bind({n:1},{});function run(){}";
    let original = wire(source);
    let id = original["globals"]["bound"]["value"]["o"].as_u64().unwrap();
    for (field, forged) in [
        ("target", json!(id)),
        ("target", json!(999)),
        ("this", json!({"o": 999})),
        ("args", json!([{"o": 999}])),
    ] {
        let mut value = original.clone();
        let objects = value["heap"][0].as_array_mut().unwrap();
        let entry = objects.iter_mut().find(|entry| entry[0] == id).unwrap();
        entry[1]["c"]["function"]["f"]["bound"][field] = forged;
        rejected(source, value);
    }
}
/**
 * Calling a forged bound function whose target is itself must stop at the call depth or the
 * deadline rather than spinning past SCRIPT_DEADLINE.
 */
#[test]
fn forged_self_bound_calls_stop_within_the_deadline() {
    let source = "function f(x){return x;}const bound=f.bind({n:1},{});function run(){bound();}";
    let mut value = wire(source);
    let id = value["globals"]["bound"]["value"]["o"].as_u64().unwrap();
    let objects = value["heap"][0].as_array_mut().unwrap();
    let entry = objects.iter_mut().find(|entry| entry[0] == id).unwrap();
    entry[1]["c"]["function"]["f"]["bound"]["target"] = json!(id);
    assert!(event(source, "run()", serde_json::from_value(value).unwrap(), &[]).is_err());
}
/** Objects and functions retained exclusively by bound receivers or prefixes survive finish-only collection. */
#[test]
fn bound_receivers_arguments_and_targets_survive_serialized_events() {
    let source = "function make(){const receiver={n:4};const argument=()=>3;return function(cb){this.n=this.n+cb();return this.n;}.bind(receiver,argument);}const bound=make();function run(){document.getElementById('out').textContent=bound();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["7", "10", "13"] {
        let result = next(source, state);
        assert_eq!(result.changes[0].text().unwrap(), expected);
        state = result.state;
    }
}
/** Canonical roots retain mutable prototype properties and every cycle reachable through those values. */
#[test]
fn canonical_prototype_properties_keep_owned_cycles_alive() {
    let source = "function setup(){const box={n:8};const callback=()=>box.n;box.callback=callback;Function.prototype.saved=box;}setup();function run(){document.getElementById('out').textContent=Function.prototype.saved.callback();}";
    let state = load(source, &[]).unwrap().state;
    assert_eq!(next(source, state).changes[0].text().unwrap(), "8");
}
/** Discarded bound records and their expando cycles are reclaimed between events, so the heap does not grow. */
#[test]
fn unreachable_bound_records_and_expando_cycles_are_reclaimed_between_events() {
    let source = "function discard(){const f=function(){return 1;};const bound=f.bind({});bound.self=bound;f.bound=bound;}function run(){discard();}";
    let mut state = load(source, &[]).unwrap().state;
    let objects = |state: &VmState| {
        serde_json::to_value(state).unwrap()["heap"][0]
            .as_array()
            .unwrap()
            .len()
    };
    let baseline = objects(&state);
    for _ in 0..600 {
        state = next(source, state).state;
    }
    assert_eq!(objects(&state), baseline);
}
