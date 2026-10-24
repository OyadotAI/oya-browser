/** Saved closure graphs preserve cycles and reject hostile identities before publication. */
use super::closures::next;
use mars::v9::{VmState, event, load};
use serde_json::{Value, json};

/** One named way to corrupt a saved state wire, given the forged closure's object id. */
type Forgery = (&'static str, fn(&mut Value, u64));

/** Heap objects and environments on the private state wire. */
fn heap_sizes(state: &VmState) -> (usize, usize) {
    let wire = serde_json::to_value(state).unwrap();
    (
        wire["heap"][0].as_array().unwrap().len(),
        wire["heap"][1].as_array().unwrap().len(),
    )
}

/** Boundary tracing reclaims unreachable value cycles, so many events leave the heap the same size. */
#[test]
fn unreachable_cycles_and_temporary_callbacks_are_reclaimed_between_events() {
    let source = "function discard(){const box={};const f=function(){return box;};box.f=f;}function run(){discard();let f=function(){return 1;};f();}";
    let mut state = load(source, &[]).unwrap().state;
    let baseline = heap_sizes(&state);
    for _ in 0..600 {
        state = next(source, state).state;
    }
    assert_eq!(heap_sizes(&state), baseline);
}
/** Genuine object/function/environment cycles remain reachable and finite on the private wire. */
#[test]
fn reachable_mixed_cycles_round_trip_and_keep_identity() {
    let source = "function make(){const box={};const f=function(){return box;};box.f=f;return box;}const box=make();function run(){document.getElementById('out').textContent=box.f()===box;}";
    let state = load(source, &[]).unwrap().state;
    assert_eq!(next(source, state).changes[0].text().unwrap(), "true");
}
/** Forged references, code sites, environment edges and identity counters are refused before execution. */
#[test]
fn forged_graph_edges_fail_closed() {
    let source = "function make(){let n=0;return function(){let m=1;return ()=>n+m;};}const f=make()();function run(){document.getElementById('out').textContent=f();}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let function = crate::v9::wire_global_object(&original, "f");
    let cases: [Forgery; 6] = [
        ("dangling global", |wire, _| {
            wire["globals"]["f"]["value"] = json!({"o": 999})
        }),
        ("unknown code site", |wire, function| {
            crate::v9::wire_object(wire, function)["c"]["function"]["f"]["closure"]["site"] =
                json!(999_999)
        }),
        ("missing environment", |wire, function| {
            crate::v9::wire_object(wire, function)["c"]["function"]["f"]["closure"]["env"] =
                json!(999)
        }),
        ("environment parent cycle", |wire, _| {
            let env = &mut wire["heap"][1][1];
            env[1]["p"] = env[0].clone();
        }),
        ("dangling environment slot", |wire, _| {
            wire["heap"][1][0][1]["s"][0] = json!({"o": 999})
        }),
        ("rewound identity counter", |wire, _| {
            wire["heap"][2] = json!(0)
        }),
    ];
    for (name, forge) in cases {
        let mut wire = original.clone();
        forge(&mut wire, function);
        let state = serde_json::from_value(wire).unwrap();
        assert_eq!(
            event(source, "run()", state, &[]).unwrap_err().code,
            "invalid_request",
            "{name}"
        );
    }
}
/** An unreachable hostile record must fail validation rather than being silently collected. */
#[test]
fn unreachable_hostile_records_are_not_silently_collected() {
    let source = "const f=function self(){return self;};function run(){}";
    let mut wire = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let next = wire["heap"][2].as_u64().unwrap();
    wire["heap"][0].as_array_mut().unwrap().push(json!([
        next,
        {"c":{"function":{"f":{"closure":{"site":99_999}}}},"p":1_073_741_825}
    ]));
    wire["heap"][2] = json!(next + 1);
    assert_eq!(
        event(source, "run()", serde_json::from_value(wire).unwrap(), &[])
            .unwrap_err()
            .code,
        "invalid_request"
    );
}
