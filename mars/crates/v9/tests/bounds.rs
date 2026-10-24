/** V9's bounds still hold with the full platform: every one fails explicitly as a limit. */
mod common;
use common::{failure, load};
use mars_v9::constants::{JS_STRING_UNITS, VM_FUNCTIONS, VM_OBJECTS, VM_PROPERTIES};

/// Objects one loop iteration allocates. The bounds are large enough that one allocation per
/// iteration would meet the script deadline first in a debug build, so each iteration makes a
/// batch and the bound is reached in a few thousand iterations.
const BATCH: usize = 32;

/** An array literal of BATCH empty objects (BATCH + 1 allocations). */
fn batch() -> String {
    format!("[{}]", vec!["{}"; BATCH].join(","))
}

/** Live objects past VM_OBJECTS stop the script; garbage does not count. */
#[test]
fn live_objects_are_bounded_but_garbage_is_collected() {
    let garbage = load(&format!(
        "for(let i=0;i<{};i++){{const o={}}}const r='ok';",
        VM_OBJECTS * 2 / BATCH,
        batch()
    ));
    assert!(garbage.failed.is_empty(), "{:?}", failure(&garbage));
    let live = load(&format!(
        "const keep=[];for(let i=0;i<{};i++)keep.push({});",
        VM_OBJECTS / BATCH + 1,
        batch()
    ));
    assert!(failure(&live).unwrap().contains("V9 heap objects"));
}

/** Live closures past VM_FUNCTIONS stop the script. */
#[test]
fn live_functions_are_bounded() {
    let execution = load(&format!(
        "const keep=[];for(let i=0;i<{};i++)keep.push(()=>i);",
        VM_FUNCTIONS + 10
    ));
    assert!(failure(&execution).unwrap().contains("V9 functions"));
}

/** Own properties past VM_PROPERTIES stop the script. */
#[test]
fn properties_are_bounded() {
    let fields: Vec<String> = (0..BATCH).map(|i| format!("k{i}:i")).collect();
    let execution = load(&format!(
        "const keep=[];for(let i=0;i<{};i++)keep.push({{{}}});",
        VM_PROPERTIES / BATCH + 1,
        fields.join(",")
    ));
    assert!(
        failure(&execution)
            .unwrap()
            .contains("V9 aggregate own properties")
    );
}

/** Strings past JS_STRING_UNITS fail as a limit. */
#[test]
fn string_length_is_bounded() {
    let execution = load(&format!(
        "let s='x'.repeat({});s=s+'yy';",
        JS_STRING_UNITS - 1
    ));
    assert!(failure(&execution).unwrap().contains("UTF-16 units"));
}

/** An infinite loop stops at the operation budget or the deadline. */
#[test]
fn infinite_loops_stop() {
    let execution = load("while(true){}");
    let message = failure(&execution).unwrap();
    assert!(
        message.contains("operation limit") || message.contains("time limit"),
        "{message}"
    );
}

/** Limits are engine errors: try/catch cannot swallow them. */
#[test]
fn limits_are_not_catchable() {
    let execution = load(&format!(
        "try{{const keep=[];for(let i=0;i<{};i++)keep.push({})}}catch(e){{}}",
        VM_OBJECTS / BATCH + 1,
        batch()
    ));
    assert!(failure(&execution).unwrap().contains("V9 heap objects"));
}

/** A failed script rolls back alone; later scripts see only earlier successful ones. */
#[test]
fn failing_scripts_roll_back_alone() {
    let sources = vec![
        "var a=1;".to_string(),
        "var b=2;throw 0;".to_string(),
        "const r=[typeof a,typeof b].join();".to_string(),
    ];
    let environment = common::environment();
    let host = common::host();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(
        execution
            .failed
            .iter()
            .map(|failure| failure.index)
            .collect::<Vec<_>>(),
        [1]
    );
    assert_eq!(common::global(&execution, "r"), "number,undefined");
}

/** Saved state is validated: forged identities and oversized collections are refused. */
#[test]
fn hostile_state_is_refused() {
    let execution = load("const o={a:1};");
    let mut wire = serde_json::to_value(&execution.state).unwrap();
    wire["heap"][2] = serde_json::json!(0);
    let forged: mars_v9::VmState = serde_json::from_value(wire).unwrap();
    let environment = common::environment();
    let host = common::host();
    let sources = vec!["const o={a:1};".to_string()];
    let error = mars_v9::run_timers(common::input(&sources, &host, &environment), forged, 0.0, 1)
        .unwrap_err();
    assert_eq!(error.code, "invalid_request");
}

/** Committed state round-trips through JSON and stays under VM_STATE_BYTES for jQuery-sized pages. */
#[test]
fn state_round_trips_and_is_bounded() {
    let execution = load(
        "class A{constructor(){this.m=new Map([[1,{s:new Set([1])}]])}}const a=new A();const g=(function*(){yield 1})();",
    );
    let bytes = serde_json::to_vec(&execution.state).unwrap();
    assert!(bytes.len() < mars_v9::constants::VM_STATE_BYTES);
    let back: mars_v9::VmState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(serde_json::to_vec(&back).unwrap(), bytes);
}

/** Nesting inside one function body is bounded by EXPRESSION_DEPTH and STATEMENT_DEPTH, and
each function literal starts again so nested library closures do not add up. */
#[test]
fn expression_and_statement_nesting_are_bounded_per_function() {
    use mars_v9::constants::{EXPRESSION_DEPTH, STATEMENT_DEPTH};
    let conditional = format!("{}0", "a?b:".repeat(EXPRESSION_DEPTH + 1));
    assert!(
        mars_v9::check(&conditional)
            .unwrap_err()
            .message
            .contains("expression nesting")
    );
    let statements = format!("{};", "if(x)".repeat(STATEMENT_DEPTH + 1));
    assert!(
        mars_v9::check(&statements)
            .unwrap_err()
            .message
            .contains("statement nesting")
    );
    let half = STATEMENT_DEPTH / 2;
    let split = format!(
        "{}(function(){{{};}});",
        "if(x)".repeat(half),
        "if(x)".repeat(half)
    );
    assert!(mars_v9::check(&split).is_ok());
}
