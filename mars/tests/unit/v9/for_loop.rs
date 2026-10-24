/** Newly authored standard-for cases test source-guided execution through the public V9 boundary. */
use mars::{
    constants::STATEMENT_DEPTH,
    v9::{event, execute, load},
};

/** A final DOM proposal exposes ordinary page-script state without a private evaluator hook. */
fn result(source: &str, expression: &str) -> String {
    execute(&format!(
        "{source}document.getElementById('out').textContent={expression};"
    ))
    .unwrap()
    .last()
    .unwrap()
    .text
    .clone()
}
/** Initializer, test, body and update run in their language order, including the final false test. */
#[test]
fn clauses_preserve_order_and_zero_iterations_skip_body_and_update() {
    let source = "let trace='';function init(){trace=trace+'I';return 0;}function test(i){trace=trace+'T';return i<2;}function next(i){trace=trace+'U';return i+1;}for(let i=init();test(i);i=next(i)){trace=trace+'B';}";
    assert_eq!(result(source, "trace"), "ITBUTBUT");
    assert_eq!(
        result("let n=0;for(n=n+1;false;n=n+10){n=n+100;}", "n"),
        "1"
    );
}
/** Every combination of omitted clauses is valid and retains body-controlled termination. */
#[test]
fn each_clause_can_be_omitted_independently() {
    for mask in 0..8 {
        let init = if mask & 1 == 0 { "" } else { "i=0" };
        let test = if mask & 2 == 0 { "" } else { "count<3" };
        let update = if mask & 4 == 0 { "" } else { "i=i+1" };
        let source = format!(
            "let count=0;let i=0;for({init};{test};{update}){{count=count+1;if(count===3)break;}}"
        );
        assert_eq!(result(&source, "count"), "3", "mask {mask}");
    }
    assert_eq!(result("let i=0;for(;i<3;i=i+1);", "i"), "3");
    assert_eq!(result("for(17;false;19);", "1"), "1");
}
/** Continue proceeds through update; break and return leave before it. */
#[test]
fn completion_controls_update_and_function_return() {
    assert_eq!(
        result(
            "let sum=0;for(let i=0;i<5;i=i+1){if(i===2)continue;sum=sum+i;}",
            "sum"
        ),
        "8"
    );
    assert_eq!(
        result("let updates=0;for(;;updates=updates+1){break;}", "updates"),
        "0"
    );
    assert_eq!(
        result(
            "let updates=0;function choose(){for(;;updates=updates+1){return 7;}}let answer=choose();",
            "answer+' '+updates"
        ),
        "7 0"
    );
}
/** Each loop consumes only its own break/continue completion. */
#[test]
fn nested_for_and_while_loops_keep_their_own_targets() {
    assert_eq!(
        result(
            "let sum=0;for(let i=0;i<3;i=i+1){if(i===1)continue;let j=0;while(j<3){j=j+1;if(j===2)continue;if(j===3)break;sum=sum+i;}}",
            "sum"
        ),
        "2"
    );
    assert_eq!(
        result(
            "let i=0;let sum=0;while(i<3){i=i+1;for(let j=0;j<3;j=j+1){if(j===1)continue;if(j===2)break;sum=sum+i;}}",
            "sum"
        ),
        "6"
    );
}
/** Lexical headers shadow outer bindings for all clauses and disappear on completion. */
#[test]
fn lexical_headers_preserve_tdz_const_and_lifetime_rules() {
    assert_eq!(
        result(
            "let i=9;let sum=0;for(let i=0;i<3;i=i+1){let i=100;sum=sum+i;}",
            "i+' '+sum"
        ),
        "9 300"
    );
    assert_eq!(
        result(
            "let total=0;for(let i=0;i<3;i=i+1){let fresh=0;fresh=fresh+1;total=total+fresh;}",
            "total"
        ),
        "3"
    );
    assert_eq!(result("for(let i;false;);", "1"), "1");
    for source in [
        "let i=9;for(let i=i;false;);",
        "for(let i=0;false;);let gone=i;",
        "for(const i=0;i<1;i=1);",
        "for(const i=0;i<1;){i=1;}",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_execution_error",
            "{source}"
        );
    }
}
/** Shallow per-iteration cells keep shared heap references rather than copying their objects. */
#[test]
fn header_objects_keep_alias_identity_and_function_calls_skip_loop_scopes() {
    assert_eq!(
        result(
            "const original={n:0};for(let box=original;box.n<3;box.n=box.n+1){box.value=box.n;}",
            "original.n+' '+original.value"
        ),
        "3 2"
    );
    assert_eq!(
        result(
            "let total=0;for(const box={n:0};box.n<3;box.n=box.n+1){total=total+box.n;}",
            "total"
        ),
        "3"
    );
    assert_eq!(
        result(
            "let i=9;let seen=0;function read(){return i;}for(let i=0;i<2;i=i+1){seen=seen+read();}",
            "seen"
        ),
        "18"
    );
}
/** Omitted or true tests cannot bypass operation limits and nesting remains bounded before execution. */
#[test]
fn infinite_and_deep_loops_fail_with_structured_limits() {
    for source in ["for(;;);", "for(;;){continue;}", "for(let i=0;;i=i+1){}"] {
        let error = execute(source).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(error.message.contains("limit"));
        assert_eq!(error.phase.as_deref(), Some("execute"));
    }
    let source = format!("{};", "for(;;)".repeat(STATEMENT_DEPTH + 1));
    assert_eq!(execute(&source).unwrap_err().code, "limit_exceeded");
}
/** A const header without an initializer is an early error, as for any const declaration. */
#[test]
fn const_headers_require_an_initializer() {
    assert_eq!(
        execute("for(const i;false;);").unwrap_err().code,
        "script_syntax_error"
    );
}
/** for-of, for-in, bare-target heads and several header declarators run as JavaScript; for await needs an async function. */
#[test]
fn other_header_families_follow_javascript() {
    assert_eq!(
        result(
            "let a='';for(let x of [1,2])a+=x;for(const x in {p:1,q:2})a+=x;let y;for(y of [3])a+=y;for(let i=0,j=5;i<2;i=i+1){a+=j;}",
            "a"
        ),
        "12pq355"
    );
    assert_eq!(
        execute("for await(x of [1]){}").unwrap_err().code,
        "unsupported_feature"
    );
    assert_eq!(
        execute("for(1=2;false;);").unwrap_err().code,
        "script_syntax_error"
    );
    execute("let i=0;for(i=0;i<2;i++){}").unwrap();
    execute("let i=0;let j=0;for(i=j=0;i<2;i=i+1){}").unwrap();
}
/** Header names never escape into the page snapshot or the next event's declaration contract. */
#[test]
fn loop_globals_round_trip_without_exporting_header_bindings() {
    let source = "let count=0;function add(){for(let i=0;i<3;i=i+1){count=count+1;}document.getElementById('out').textContent=count;}";
    let state = load(source, &[]).unwrap().state;
    assert_eq!(
        state.globals().keys().cloned().collect::<Vec<_>>(),
        ["count"]
    );
    let first = event(source, "add()", state, &[]).unwrap();
    assert_eq!(first.changes[0].text().unwrap(), "3");
    assert_eq!(
        event(source, "add()", first.state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "6"
    );
}
/** An error in update discards body proposals and every candidate global change. */
#[test]
fn failed_update_discards_the_entire_event_candidate() {
    let source = "let count=0;function fail(){for(let i=0;i<1;i=missing){count=count+1;document.getElementById('out').textContent='partial';}}function show(){document.getElementById('out').textContent=count;}";
    let state = load(source, &[]).unwrap().state;
    assert!(event(source, "fail()", state.clone(), &[]).is_err());
    assert_eq!(
        event(source, "show()", state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "0"
    );
}
