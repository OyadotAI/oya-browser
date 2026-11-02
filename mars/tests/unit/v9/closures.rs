/** Public V9 closure tests pin captured identity, invocation order, persistence and bounded graph ownership. */
use mars::v9::{Execution, VmState, event, execute, load};

/** One visible proposal observes expressions without exposing interpreter implementation helpers. */
fn result(source: &str, expression: &str) -> String {
    let source = format!("{source} document.getElementById('out').textContent={expression};");
    execute(&source).unwrap()[0].text.clone()
}
/** Events round-trip the actual private wire representation between otherwise stateless executions. */
pub(super) fn next(source: &str, state: VmState) -> Execution {
    let bytes = serde_json::to_vec(&state).unwrap();
    event(
        source,
        "run()",
        serde_json::from_slice(&bytes).unwrap(),
        &[],
    )
    .unwrap()
}
/** Returned closures retain mutable private cells after their factory activation has returned. */
#[test]
fn private_counters_persist_across_serialized_events() {
    let source = "function factory(){let count=0;return function(){count=count+1;return count;};}const counter=factory();function run(){document.getElementById('out').textContent=counter();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in 1..=4 {
        let event = next(source, state);
        assert_eq!(event.changes[0].text().unwrap(), expected.to_string());
        state = event.state;
    }
}
/** Absolute code sites stay distinct across ordered scripts and root declaration cells remain mutable. */
#[test]
fn ordered_scripts_preserve_code_identity_and_reassigned_function_cells() {
    use mars::v9::{event_scripts, load_scripts};
    let sources=vec![
        "function make(){let n=0;return function(){n=n+1;return n;};}const counter=make();function value(){return 7;}const old=value;".into(),
        "function value(){return 8;}const other=function(){return 9;};function run(){document.getElementById('out').textContent=counter()+' '+old()+' '+value()+' '+other();value=function(){return 10;};}".into(),
    ];
    let state = load_scripts(&sources, &[]).unwrap().state;
    let first = event_scripts(&sources, "run()", state, &[]).unwrap();
    assert_eq!(first.changes[0].text().unwrap(), "1 7 8 9");
    let second = event_scripts(&sources, "run()", first.state, &[]).unwrap();
    assert_eq!(second.changes[0].text().unwrap(), "2 7 10 9");
}
/** Siblings share cells while separate factory invocations never share their mutable activation. */
#[test]
fn sibling_closures_share_cells_and_factories_are_independent() {
    let source = "function factory(){let n=0;return {inc:function(){n=n+1;},get:function(){return n;}};}const a=factory();const b=factory();a.inc();a.inc();b.inc();";
    assert_eq!(result(source, "a.get()+' '+b.get()"), "2 1");
}
/** Function identity survives assignment, object/array storage, callbacks and returned expression callees. */
#[test]
fn functions_are_values_in_every_supported_call_position() {
    let source = "function apply(f,v){return f(v);}function factory(){return function(n){return n+1;};}let f=factory();const alias=f;const box={f:f};const callbacks=[f];let x=apply(f,2)+box.f(3)+callbacks[0](4)+factory()(5)+(function(n){return n+2;})(5);f=function(n){return n+10;};";
    assert_eq!(
        result(
            source,
            "x+' '+(alias===box.f)+' '+(alias===f)+' '+(!f)+' '+(f(1))"
        ),
        "25 true false false 11"
    );
    assert_eq!(result("const f=function(){};", "(f||false)===f"), "true");
}
/** Callee evaluation happens once before left-to-right argument callbacks, even if they replace its cell. */
#[test]
fn invocation_retains_the_callee_before_argument_side_effects() {
    let source = "let trace='';let target=function(a,b){trace=trace+'C';return a+b;};function pick(){trace=trace+'P';return target;}function arg(n){trace=trace+n;target=function(){return 99;};return n;}let value=pick()(arg(1),arg(2));";
    assert_eq!(result(source, "trace+' '+value+' '+target()"), "P12C 3 99");
    let error = execute("const n=3;function fail(){return missing;}n(fail());").unwrap_err();
    assert!(error.message.contains("missing is not defined"));
    let error = execute("const n=3;n(1);").unwrap_err();
    assert!(error.message.contains("is not a function"));
}
/** Nested declarations hoist into their defining activation after parameters have been initialized. */
#[test]
fn direct_nested_declarations_hoist_and_override_parameters() {
    let source = "function outer(helper){return helper();function helper(){return 7;}}";
    assert_eq!(result(source, "outer(99)"), "7");
    let source = "function make(){return helper;function helper(){return 9;}}const saved=make();";
    assert_eq!(result(source, "saved()"), "9");
    // Sloppy block function declarations also bind in the enclosing scope (Annex B).
    assert!(execute("function outer(){if(true){function blocked(){}}}").is_ok());
    assert_eq!(
        result("if(true){function blocked(){return 4;}}", "blocked()"),
        "4"
    );
    assert!(execute("function outer(){while(true){const f=function(){break;};}}").is_err());
}
/** Named expression recursion uses a private immutable name outside the parameter activation. */
#[test]
fn named_expressions_keep_their_self_binding_private() {
    let source = "const f=function self(n){if(n===0){return 1;}return n*self(n-1);};const g=function self(self){return self;};";
    assert_eq!(result(source, "f(5)+' '+g(8)"), "120 8");
    assert!(
        execute("const f=function hidden(){};hidden();")
            .unwrap_err()
            .message
            .contains("hidden is not defined")
    );
}

/** A named expression's own name is immutable: sloppy writes are ignored and strict writes throw. */
#[test]
fn named_expression_self_bindings_are_immutable() {
    assert_eq!(
        result(
            "const f=function self(){self=1;return typeof self;};",
            "f()"
        ),
        "function"
    );
    let error = execute("const f=function self(){'use strict';self=1;};f();").unwrap_err();
    assert!(error.message.contains("TypeError"), "{}", error.message);
}
/** Captured lookup observes live TDZ/const cells and block shadowing rather than copied values. */
#[test]
fn captured_cells_preserve_tdz_const_and_block_shadowing() {
    let source = "let n=99;function make(){let f;{let n=4;f=function(){n=n+1;return n;};}return f;}const f=make();";
    assert_eq!(result(source, "f()+' '+f()+' '+n"), "5 6 99");
    let source = "function make(){return function(){return pending;};let pending=1;}const f=make();function run(){f();}";
    let state = load(source, &[]).unwrap().state;
    assert!(
        event(source, "run()", state, &[])
            .unwrap_err()
            .message
            .contains("before initialization")
    );
    assert!(
        execute("function make(){const n=1;return function(){n=2;};}make()();")
            .unwrap_err()
            .message
            .contains("Assignment to constant variable")
    );
    assert_eq!(
        result(
            "function make(){const box={n:1};return function(){box.n=box.n+1;return box.n;};}const f=make();",
            "f()+' '+f()"
        ),
        "2 3"
    );
}
/** For-let iterations allocate distinct cells and continue copies them before update evaluation. */
#[test]
fn iteration_closures_retain_distinct_bindings_and_shared_object_aliases() {
    let source =
        "const callbacks=[];for(let i=0;i<3;i=i+1){callbacks[i]=function(){return i;};continue;}";
    assert_eq!(
        result(
            source,
            "callbacks[0]()+' '+callbacks[1]()+' '+callbacks[2]()"
        ),
        "0 1 2"
    );
    let source = "const callbacks=[];for(const box={n:0};box.n<3;box.n=box.n+1){callbacks[box.n]=function(){return box.n;};}";
    assert_eq!(
        result(
            source,
            "callbacks[0]()+' '+callbacks[1]()+' '+callbacks[2]()"
        ),
        "3 3 3"
    );
    let source = "const callbacks=[];for(let i=0;i<2;callbacks[2]=function(){return i;}){callbacks[i]=function(){return i;};i=i+1;}";
    assert_eq!(
        result(
            source,
            "callbacks[0]()+' '+callbacks[1]()+' '+callbacks[2]()"
        ),
        "1 2 2"
    );
}
/** A failed candidate cannot mutate the prior snapshot's captured cells, heap or DOM proposals. */
#[test]
fn failed_callbacks_discard_captured_mutations_and_proposals() {
    let source = "function make(){let n=0;const box={n:0};return function(fail){n=n+1;box.n=box.n+1;document.getElementById('out').textContent=n+' '+box.n;if(fail){missing();}};}const f=make();function bad(){f(true);}function run(){f(false);}";
    let state = load(source, &[]).unwrap().state;
    let before = serde_json::to_value(&state).unwrap();
    assert!(event(source, "bad()", state.clone(), &[]).is_err());
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    assert_eq!(next(source, state).changes[0].text().unwrap(), "1 1");
}
/** Inline handlers are ordinary script: they may pass function code and run several statements. */
#[test]
fn inline_handlers_run_ordinary_code() {
    let source = "let calls=0;function run(f){calls=calls+1;if(f){f();}}";
    let state = load(source, &[]).unwrap().state;
    for (handler, expected) in [
        ("run(function(){calls=calls+10;})", 11.0),
        ("(function(){calls=5;})()", 5.0),
        ("run.call()", 1.0),
        ("run();run();", 2.0),
    ] {
        let executed = event(source, handler, state.clone(), &[]).unwrap();
        assert_eq!(
            executed.state.globals()["calls"].value,
            Some(mars::v9::JsValue::Number(expected)),
            "{handler}"
        );
    }
}
/** Functions convert like any object: ToNumber gives NaN and + concatenates a string. */
#[test]
fn functions_convert_through_to_primitive() {
    assert_eq!(
        result("const f=function(){};", "(+f)+' '+(f<1)+' '+(typeof (f+1))"),
        "NaN false string"
    );
}
/** Closure allocation is bounded only by live functions, and runaway recursion throws a catchable RangeError. */
#[test]
fn closure_garbage_is_free_and_recursion_hits_the_call_depth() {
    assert_eq!(
        result("let i=0;while(i<20000){(function(){});i=i+1;}", "i"),
        "20000"
    );
    let error = execute("const f=function self(){self();};f();").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("RangeError"), "{}", error.message);
}
