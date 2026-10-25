/** Var declarations preserve function ownership and declaration instantiation across native VM requests. */
use mars::{
    constants::VM_BINDINGS,
    v9::{VmState, event, event_scripts, execute, load, load_scripts},
};
use serde_json::json;

/** Ordinary document proposals reveal JavaScript values without a privileged evaluation endpoint. */
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

/** Hoisting scans every supported statement branch without executing unreachable initializers. */
#[test]
fn script_vars_are_initialized_before_reads_and_hoist_out_of_blocks_and_unreachable_branches() {
    let source =
        "const before=x;var x=1;{var block=2;}if(false){var hidden=3;}while(false){var never=4;}";
    assert_eq!(
        result(
            source,
            "(before===undefined)+' '+x+' '+block+' '+(hidden===undefined)+' '+(never===undefined)"
        ),
        "true 1 2 true true"
    );
}

/** A function's own hoisted cell shadows outer bindings even when its declaration is after return. */
#[test]
fn function_var_reads_before_declaration_do_not_fall_back_to_outer_bindings() {
    let source = "var x=9;function f(){const before=x;return before;var x=2;}function g(){if(false){var x=3;}return x;}";
    assert_eq!(
        result(source, "(f()===undefined)+' '+(g()===undefined)+' '+x"),
        "true true 9"
    );
}

/** Multiple declarators instantiate together and execute initializers in their original left-to-right order. */
#[test]
fn var_lists_allow_missing_initializers_and_name_only_direct_anonymous_values() {
    let source = "var first=later,later=3,empty,callback=function(a){},arrow=x=>x;var later;";
    assert_eq!(
        result(
            source,
            "(first===undefined)+' '+later+' '+(empty===undefined)+' '+callback.name+' '+callback.length+' '+arrow.name"
        ),
        "true 3 true callback 1 arrow"
    );
}

/** Repeated declarations without an initializer cannot erase a parameter or earlier assignment. */
#[test]
fn repeated_var_declarations_preserve_parameter_and_initialized_values() {
    let source = "var value=4;var value;function preserve(value){var value;{var value;}return value;}function replace(value){var value=8;return value;}";
    assert_eq!(
        result(source, "value+' '+preserve(7)+' '+replace(7)"),
        "4 7 8"
    );
}

/** Direct function declarations initialize before execution and later duplicates supply the callable value. */
#[test]
fn duplicate_functions_and_var_redeclarations_follow_declaration_instantiation_order() {
    let source = "const early=f();function f(){return 1;}var f;function f(){return 2;}const before=f();var f=3;function local(value){var value;function value(){return 5;}return value();}";
    assert_eq!(
        result(source, "early+' '+before+' '+f+' '+local(9)"),
        "2 2 3 5"
    );
}

/** Global var stores and page-global properties share cells while function locals stay private. */
#[test]
fn global_var_identifier_and_property_writes_remain_coherent() {
    let source = "var count=1;const first=this.count;this.count=2;const second=count;count=3;function local(){var count=7;return count;}";
    assert_eq!(
        result(
            source,
            "first+' '+second+' '+this.count+' '+local()+' '+count"
        ),
        "1 2 3 7 3"
    );
    assert!(
        execute("function local(){var privateValue=1;}local();const outside=privateValue;")
            .unwrap_err()
            .message
            .contains("is not defined")
    );
}

/** Lexical declarations conflict with vars crossing their own scope, including unreachable bodies and headers. */
#[test]
fn var_hoisting_preserves_lexical_early_errors_and_tdz() {
    for source in [
        "let x;var x;",
        "var x;const x=1;",
        "function f(){let x;{var x;}}",
        "function f(){{let x;if(false){var x;}}}",
        "for(let i=0;false;){var i;}",
        "function f(value){let value;}",
    ] {
        let error = execute(source).unwrap_err();
        assert!(
            error.message.contains("SyntaxError"),
            "{source}: {}",
            error.message
        );
    }
    execute("function f(){{let x;}var x;function nested(){var x;}}f();").unwrap();
    assert!(
        execute("var x=1;{const before=x;let x=2;}")
            .unwrap_err()
            .message
            .contains("before initialization")
    );
}

/** Var headers use the function cell on every iteration; lexical headers retain distinct cells. */
#[test]
fn for_var_closures_share_the_terminal_cell_while_for_let_closures_remain_distinct() {
    let source = "const shared=[];for(var i=0,j=1;i<3;i=i+1){shared[i]=()=>i;}const separate=[];for(let k=0;k<3;k=k+1){separate[k]=()=>k;}";
    assert_eq!(
        result(
            source,
            "shared[0]()+' '+shared[1]()+' '+shared[2]()+' '+i+' '+j+' '+separate[0]()+' '+separate[1]()+' '+separate[2]()"
        ),
        "3 3 3 3 1 0 1 2"
    );
}

/** Later classic scripts instantiate only when reached, and repeat declarations retain committed earlier values. */
#[test]
fn ordered_scripts_do_not_hoist_later_program_vars_into_earlier_execution() {
    let sources = vec!["const observed=later;".into(), "var later=1;".into()];
    let error = load_scripts(&sources, &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert_eq!(error.location.unwrap().script_index, Some(1));
    let sources = vec![
        "var count=1;".into(),
        "var count;count=count+1;document.getElementById('out').textContent=count;".into(),
    ];
    assert_eq!(
        load_scripts(&sources, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "2"
    );
    for sources in [
        ["let x;", "var x;"],
        ["var x;", "let x;"],
        ["function f(){}", "const f=1;"],
        ["let f;", "function f(){}"],
    ] {
        let error = load_scripts(&sources.map(String::from), &[]).unwrap_err();
        assert!(
            error.message.contains("SyntaxError"),
            "{sources:?}: {}",
            error.message
        );
    }
}

/** Source strictness checks every var name, even within branches that never run. */
#[test]
fn strict_var_names_are_rejected_before_execution() {
    for source in [
        "'use strict';var eval;",
        "function f(){'use strict';if(false){var arguments;}}",
        "'use strict';for(var eval=0;false;){}",
        "function f(){'use strict';var public;}",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}

/** var arguments keeps the parameter when one is named arguments, and the arguments object otherwise. */
#[test]
fn var_arguments_preserves_explicit_parameters_and_the_arguments_object() {
    assert_eq!(
        result(
            "function f(arguments){var arguments;return arguments;}",
            "f(7)"
        ),
        "7"
    );
    assert_eq!(
        result(
            "function f(){var arguments;return arguments.length;}",
            "f(1,2)"
        ),
        "2"
    );
}

/** Several lexical declarators in one statement, var lists and sequences are ordinary syntax. */
#[test]
fn multiple_declarators_and_sequences_run() {
    assert_eq!(
        result(
            "let a=1,b=2;const c=3,d=4;let n=0;for(let i=0,j=1;i<1;i++){n=n+j;}var x=1;x=1,2;const values=(1,2);for(var k=0;k<1;k=k+1,k=k+1){}",
            "[a,b,c,d,n,x,values,k].join()"
        ),
        "1,2,3,4,1,1,2,2"
    );
}

/** Reconstructed events retain var state without replaying load initializers or erasing duplicate declarations. */
#[test]
fn serialized_var_counters_persist_and_failed_callbacks_keep_the_committed_value() {
    let source = "var count=0;function run(){var next=count+1;var next;count=next;document.getElementById('out').textContent=count;}function bad(){count=count+100;missing();}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in 1..=3 {
        assert!(event(source, "bad()", state.clone(), &[]).is_err());
        let persisted: VmState =
            serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
        let executed = event(source, "run()", persisted, &[]).unwrap();
        assert_eq!(executed.changes[0].text().unwrap(), expected.to_string());
        state = executed.state;
    }
}

/** A single declaration statement cannot bypass the page's declared-variable allocation limit. */
#[test]
fn many_var_declarators_share_the_existing_variable_quota() {
    // The page's declared-variable quota is VM_BINDINGS; one statement naming more must fail.
    let names = (0..=VM_BINDINGS)
        .map(|index| format!("v{index}"))
        .collect::<Vec<_>>()
        .join(",");
    assert_eq!(
        execute(&format!("var {names};")).unwrap_err().code,
        "limit_exceeded"
    );
}

/** Vars a sloppy direct eval declares count against the same quota as global vars: half the
quota in globals and a little over half in eval vars together cross it, neither alone does. */
#[test]
fn eval_declared_vars_share_the_variable_quota_with_global_vars() {
    let half = VM_BINDINGS / 2;
    // Short base-36 names keep the eval sources inside the page's dynamic code allowance; the
    // punctuation prefixes keep them clear of reserved words.
    let names = |prefix: &str, range: std::ops::Range<usize>| {
        range
            .map(|index| format!("{prefix}{}", base36(index)))
            .collect::<Vec<_>>()
            .join(",")
    };
    let globals = format!("var {};", names("$", 0..half));
    // The surplus outlasts the interpreter's periodic bound check while f is still running.
    let evals = half + half / 16;
    // One eval string stays under the string literal bound, so the eval vars arrive in parts.
    let parts = 4;
    let calls = (0..parts)
        .map(|part| {
            let range = part * evals / parts..(part + 1) * evals / parts;
            format!("eval('var {}');", names("_", range))
        })
        .collect::<String>();
    let eval = format!("function f(){{{calls}}}f();");
    assert!(execute(&globals).is_ok());
    assert!(execute(&eval).is_ok());
    assert_eq!(
        execute(&format!("{globals}{eval}")).unwrap_err().code,
        "limit_exceeded"
    );
}

/** A number's base-36 digits, for short distinct identifiers. */
fn base36(mut value: usize) -> String {
    let mut digits = Vec::new();
    loop {
        digits.push(char::from_digit((value % 36) as u32, 36).unwrap());
        value /= 36;
        if value == 0 {
            return digits.into_iter().rev().collect();
        }
    }
}

/** Initialized Undefined remains a legitimate stored var value after later assignments and service messages. */
#[test]
fn assigning_undefined_to_a_var_preserves_valid_initialized_event_state() {
    let source = "var count=0;function clear(){count=undefined;}function run(){document.getElementById('out').textContent=(count===undefined)+' '+(this.count===undefined);}";
    let state = event(source, "clear()", load(source, &[]).unwrap().state, &[])
        .unwrap()
        .state;
    let persisted = serde_json::from_value(serde_json::to_value(state).unwrap()).unwrap();
    assert_eq!(
        event(source, "run()", persisted, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "true true"
    );
}

/** A later global var declaration preserves an earlier page property and migrates it into one canonical cell. */
#[test]
fn an_existing_global_expando_becomes_a_coherent_var_cell_in_a_later_script() {
    let sources=vec!["this.count=4;".into(),"var count;const prior=count;count=count+1;this.count=this.count+1;document.getElementById('out').textContent=prior+' '+count;function run(){count=count+1;document.getElementById('out').textContent=this.count;}".into()];
    let loaded = load_scripts(&sources, &[]).unwrap();
    assert_eq!(loaded.changes[0].text().unwrap(), "4 6");
    let persisted = serde_json::from_value(serde_json::to_value(loaded.state).unwrap()).unwrap();
    assert_eq!(
        event_scripts(&sources, "run()", persisted, &[])
            .unwrap()
            .changes[0]
            .text()
            .unwrap(),
        "7"
    );
}

/** Declaring a builtin without an initializer preserves its canonical callable rather than replacing it with Undefined. */
#[test]
fn var_function_preserves_the_supported_builtin_function_identity() {
    assert_eq!(
        result(
            "const original=Function;var Function;",
            "(Function===original)+' '+Function.name+' '+Function.length+' '+(Function.prototype.bind===original.prototype.bind)"
        ),
        "true Function 1 true"
    );
}

/** Redeclaring a platform global with var keeps its value, since the global property already exists. */
#[test]
fn var_redeclarations_of_platform_globals_keep_their_values() {
    assert_eq!(
        result(
            "var globalThis;var undefined;var NaN;var fetch;var localStorage;var window;var document;",
            "[typeof globalThis,typeof undefined,typeof NaN,typeof fetch,typeof localStorage,typeof window,typeof document].join()"
        ),
        "object,undefined,number,function,object,object,object"
    );
}

/** The actual desktop fixture observes a hoisted undefined controller before constructing its saved callback. */
#[test]
fn desktop_fixture_exercises_var_hoisting_before_controller_initialization() {
    let html = include_str!("../../../fixtures/desktop.html");
    let source = html
        .split_once("<script>")
        .unwrap()
        .1
        .split_once("</script>")
        .unwrap()
        .0;
    let document = mars::document::parse(html).unwrap();
    let projection = mars::document::script_document(&document, "fixture-page").unwrap();
    let probe = "document.getElementById('result').textContent=typeof controllerBeforeDeclaration+' '+typeof greetingController+' '+greetingController.name;";
    let executed = mars::v9::load_document_scripts(
        &[source.into(), probe.into()],
        projection,
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        executed.changes.last().unwrap().text().unwrap(),
        "undefined function bound greet"
    );
}

/** Retained activation vars and parameters cannot be forged back into a lexical temporal dead zone. */
#[test]
#[ignore = "known V9 gap: environment records carry no code site, so a hole in a var or parameter slot is not rejected on load"]
fn retained_var_and_parameter_cells_must_remain_initialized_before_events() {
    let source = "function make(parameter){var saved=3;return ()=>saved+parameter;}const callback=make(4);function run(){document.getElementById('out').textContent=callback();}";
    let original = serde_json::to_value(load(source, &[]).unwrap().state).unwrap();
    let slots = original["heap"][1][0][1]["s"].as_array().unwrap().len();
    for slot in 0..slots {
        let mut wire = original.clone();
        wire["heap"][1][0][1]["s"][slot] = json!({"h": 0});
        let state: VmState = serde_json::from_value(wire).unwrap();
        assert_eq!(
            event(source, "run()", state, &[]).unwrap_err().code,
            "invalid_request",
            "slot {slot}"
        );
    }
    let state: VmState = serde_json::from_value(original).unwrap();
    assert_eq!(
        event(source, "run()", state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "7"
    );
}
