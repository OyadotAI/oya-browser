/** Newly authored arrow cases verify source-guided grammar and shared lexical closure execution. */
use mars::{
    constants::{EXPRESSION_DEPTH, FUNCTION_NESTING},
    v9::{VmState, event, execute, load},
};
/** An ordinary document proposal exposes expression results without a private evaluation endpoint. */
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
mod grammar;

/** Persisted arrows share captured cells and separate factories, with no initializer replay during events. */
#[test]
fn serialized_arrow_controllers_share_cells_and_keep_factories_independent() {
    let source = "function make(){let n=0;return {inc:()=>{n=n+1;},read:()=>n};}const a=make();const b=make();function run(){a.inc();a.inc();b.inc();document.getElementById('out').textContent=a.read()+' '+b.read();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in ["2 1", "4 2", "6 3"] {
        let persisted: VmState =
            serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        let executed = event(source, "run()", persisted, &[]).unwrap();
        assert_eq!(executed.changes[0].text().unwrap(), expected);
        state = executed.state;
    }
}
/** Header copies retain distinct values when arrow closures escape through continue. */
#[test]
fn arrows_keep_iteration_cells_and_lexical_tdz_const_rules() {
    let source = "const saved=[];for(let i=0;i<3;i=i+1){saved[i]=()=>i;continue;}";
    assert_eq!(
        result(source, "saved[0]()+' '+saved[1]()+' '+saved[2]()"),
        "0 1 2"
    );
    let source =
        "function make(){return ()=>pending;let pending=1;}const f=make();function run(){f();}";
    assert!(
        event(source, "run()", load(source, &[]).unwrap().state, &[])
            .unwrap_err()
            .message
            .contains("before initialization")
    );
    assert!(
        execute("const f=()=>{const x=1;x=2;};f();")
            .unwrap_err()
            .message
            .contains("Assignment to constant variable")
    );
}
/** Arrow blocks have their own return/loop targets and permit ordinary direct nested declarations. */
#[test]
fn arrow_contexts_do_not_inherit_enclosing_loop_targets() {
    for source in [
        "while(false){const f=()=>{break;};}",
        "while(false){const f=()=>{continue;};}",
        "const f=()=>{return 1;};return 2;",
    ] {
        assert_eq!(execute(source).unwrap_err().code, "script_syntax_error");
    }
    assert_eq!(
        result(
            "const f=()=>{return helper();function helper(){return 4;}};",
            "f()"
        ),
        "4"
    );
    assert_eq!(
        result("let n=0;while(n<1){const f=()=>1;n=n+f();break;}", "n"),
        "1"
    );
}
/** Arrows have no arguments of their own: they see the enclosing function's, and none at top level. */
#[test]
fn arrows_see_the_enclosing_function_arguments() {
    assert_eq!(
        result(
            "function make(arguments){return ()=>arguments;}const f=make(7);",
            "f()"
        ),
        "7"
    );
    assert_eq!(result("const f=(arguments)=>arguments+1;", "f(7)"), "8");
    assert_eq!(
        result("const arguments=3;const f=()=>arguments;", "f()"),
        "3"
    );
    assert_eq!(
        result(
            "const arguments=3;function make(){return ()=>arguments.length;}const f=make(9,8);",
            "f()"
        ),
        "2"
    );
    assert_eq!(
        result(
            "function make(){let arguments=3;return ()=>arguments;}const f=make();",
            "f()"
        ),
        "3"
    );
    assert!(
        execute("function f(){arguments=missing;}f();")
            .unwrap_err()
            .message
            .contains("missing is not defined")
    );
    assert!(execute("function caller(){let arguments=3;return global();}const global=()=>arguments;caller();").unwrap_err().message.contains("arguments is not defined"));
}

/** Assigning arguments inside a function (or an arrow within it) rebinds that function's own binding, never an outer one. */
#[test]
fn assigning_arguments_inside_a_function_never_reaches_the_outer_binding() {
    for body in ["arguments=4;", "(()=>{arguments=4;})();"] {
        assert_eq!(
            result(
                &format!("let arguments=3;function make(){{{body}}}make();"),
                "arguments"
            ),
            "3",
            "{body}"
        );
    }
}
/** Inline handlers are ordinary code, so they may define and call arrows. */
#[test]
fn handlers_may_define_and_call_arrows() {
    let source = "let seen=0;function run(f){seen=f(2);}";
    let state = load(source, &[]).unwrap().state;
    for handler in ["run(x=>x*3)", "seen=(()=>6)()"] {
        let executed = event(source, handler, state.clone(), &[]).unwrap();
        assert_eq!(
            executed.state.globals()["seen"].value,
            Some(mars::v9::JsValue::Number(6.0)),
            "{handler}"
        );
    }
}
/** Synthetic concise returns cannot bypass statement counts or nesting/allocation limits. */
#[test]
fn concise_arrows_and_parenthesized_heads_remain_bounded() {
    let nesting = format!("let f={}1;", "x=>".repeat(FUNCTION_NESTING + 1));
    assert_eq!(execute(&nesting).unwrap_err().code, "limit_exceeded");
    let grouping = format!(
        "let f={}x=>x{};",
        "(".repeat(EXPRESSION_DEPTH),
        ")".repeat(EXPRESSION_DEPTH)
    );
    assert_eq!(execute(&grouping).unwrap_err().code, "limit_exceeded");
    let many = (0..=mars::constants::STATEMENTS / 2)
        .map(|n| format!("const f{n}=()=>1;"))
        .collect::<String>();
    assert_eq!(execute(&many).unwrap_err().code, "limit_exceeded");
}
/** Short-lived arrow/object/environment cycles are collected between events, so the heap does not grow. */
#[test]
fn arrows_reclaim_cycles_between_events() {
    let source = "function make(){let n=0;const box={n:0};return ()=>{n=n+1;box.n=box.n+1;document.getElementById('out').textContent=n+' '+box.n;};}const f=make();function run(){const box={};box.f=()=>box;f();}";
    let mut state = load(source, &[]).unwrap().state;
    let mut sizes = Vec::new();
    for expected in 1..=600 {
        let result = event(source, "run()", state, &[]).unwrap();
        assert_eq!(
            result.changes[0].text().unwrap(),
            format!("{expected} {expected}")
        );
        let wire = serde_json::to_value(&result.state).unwrap();
        sizes.push((
            wire["heap"][0].as_array().unwrap().len(),
            wire["heap"][1].as_array().unwrap().len(),
        ));
        state = result.state;
    }
    assert_eq!(sizes[1], sizes[599]);
}
