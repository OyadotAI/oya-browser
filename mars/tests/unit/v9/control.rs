/** New conformance cases pin primitive, lexical and completion semantics at V9's public boundary. */
use mars::{
    constants::STATEMENT_DEPTH,
    v9::{BindingKind, JsValue, event, event_scripts, execute, load, load_scripts},
};
use serde_json::json;

/** Text proposals expose expression values, converted with String() because textContent turns null into empty text. */
fn evaluate(expression: &str) -> String {
    execute(&format!(
        "document.getElementById('out').textContent=String({expression});"
    ))
    .unwrap()
    .remove(0)
    .text
}
/** Every supported primitive uses ECMAScript truthiness and scalar conversions. */
#[test]
fn primitive_conversions_and_logical_results_preserve_types() {
    for (source, expected) in [
        ("true", "true"),
        ("false", "false"),
        ("null", "null"),
        ("undefined", "undefined"),
        ("+true", "1"),
        ("+false", "0"),
        ("+null", "0"),
        ("+undefined", "NaN"),
        ("'x'+true+null+undefined", "xtruenullundefined"),
        ("!0", "true"),
        ("!-0", "true"),
        ("!NaN", "true"),
        ("!null", "true"),
        ("!undefined", "true"),
        ("!''", "true"),
        ("!'0'", "false"),
        ("!Infinity", "false"),
        ("false&&missing", "false"),
        ("true||missing", "true"),
        ("''&&missing", ""),
        ("'kept'||missing", "kept"),
        ("0||7", "7"),
        ("'left'&&9", "9"),
        ("false||true&&'right'", "right"),
        ("true||false&&missing", "true"),
        ("!{}", "false"),
        ("![]", "false"),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
}
/** Strict equality does not coerce, and every unordered relational comparison is false. */
#[test]
fn strict_and_relational_comparisons_follow_number_and_utf16_rules() {
    for (source, expected) in [
        ("NaN===NaN", "false"),
        ("NaN!==NaN", "true"),
        ("0===-0", "true"),
        ("true===1", "false"),
        ("null===undefined", "false"),
        ("undefined===undefined", "true"),
        ("'2'===2", "false"),
        ("'a'==='a'", "true"),
        ("'2'<'10'", "false"),
        ("'2'<10", "true"),
        ("null<=false", "true"),
        ("'🦀'<'\\ue000'", "true"),
        ("'\\ud800'<'\\ud801'", "true"),
        ("NaN<1", "false"),
        ("NaN<=1", "false"),
        ("1>NaN", "false"),
        ("1>=NaN", "false"),
        ("undefined<=0", "false"),
        ("1+2*3===7&&4<5", "true"),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
}
/** The supplied primitive tag is exact and cannot smuggle extra data into a scalar. */
#[test]
fn primitive_wire_round_trips_without_null_undefined_ambiguity() {
    for value in [
        JsValue::Boolean(true),
        JsValue::Boolean(false),
        JsValue::Null,
        JsValue::Undefined,
    ] {
        let wire = serde_json::to_value(&value).unwrap();
        assert_eq!(serde_json::from_value::<JsValue>(wire).unwrap(), value);
    }
    assert_ne!(
        serde_json::to_value(JsValue::Null).unwrap(),
        serde_json::to_value(JsValue::Undefined).unwrap()
    );
    for wire in [
        json!({"type":"boolean","value":1}),
        json!({"type":"null","value":null}),
        json!({"type":"undefined","units":[]}),
    ] {
        assert!(serde_json::from_value::<JsValue>(wire).is_err());
    }
}
/** Direct declarations instantiate before execution and blocks never leak their cells. */
#[test]
fn lexical_initialization_shadowing_and_const_rules_are_enforced() {
    let result = execute("let x=1; {let x=2; x=x+3; document.getElementById('inner').textContent=x;} x=x+1; document.getElementById('outer').textContent=x; let y; document.getElementById('empty').textContent=y;").unwrap();
    assert_eq!(
        result.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        ["5", "2", ""]
    );
    for source in [
        "let x=x;",
        "let x=1; {let y=x; let x=2;}",
        "let x=1; {x=2;let x=3;}",
        "const x=1;x=2;",
        "'use strict';missing=1;",
        "{let hidden=1;}let x=hidden;",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_execution_error",
            "{source}"
        );
    }
    for source in ["const x;", "if(true)let x=1;"] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}
/** Two lexical declarations, or a lexical and a function declaration, of one name in one scope are early errors. */
#[test]
fn duplicate_declarations_in_one_scope_are_early_errors() {
    for source in [
        "if(false){let x=1;const x=2;}",
        "function never(){let x=1;let x=2;}",
        "let f=1;function f(){}",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}
/** Top-level functions capture the global environment rather than their caller's blocks. */
#[test]
fn named_functions_skip_caller_locals_and_blocks_for_reads_and_writes() {
    let changes=execute("let x=1;function update(){x=x+1;document.getElementById('read').textContent=x;}function caller(){let x=99;{let x=100;update();}document.getElementById('local').textContent=x;}caller();document.getElementById('global').textContent=x;").unwrap();
    assert_eq!(
        changes.iter().map(|c| c.text.as_str()).collect::<Vec<_>>(),
        ["2", "99", "2"]
    );
    let error=execute("function read(){document.getElementById('out').textContent=secret;}function caller(){let secret='private';read();}caller();").unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(!error.message.contains("private"));
}
/** A scalar or TDZ binding shadows a same-named function without changing the function table. */
#[test]
fn function_calls_observe_local_shadowing_and_tdz() {
    execute(
        "function f(){}function g(){let f=1;document.getElementById('out').textContent=f;}g();",
    )
    .unwrap();
    for source in [
        "function f(){}function g(){let f=1;f();}g();",
        "function f(){}function g(){f();let f=1;}g();",
        "function f(){}function g(f){f();}g(1);",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_execution_error",
            "{source}"
        );
    }
}
/** A TDZ callee is read before arguments, while const assignment checks occur after its RHS. */
#[test]
fn reference_checks_preserve_call_and_assignment_evaluation_order() {
    let error = execute("function f(){}function g(){f(missing);let f=1;}g();").unwrap_err();
    assert!(error.message.contains("before initialization"));
    let error = execute("const x=1;x=missing;").unwrap_err();
    assert!(error.message.contains("missing is not defined"));
    let changes=execute("let total=1;function change(){total=5;return 2;}total=total+change();document.getElementById('out').textContent=total;").unwrap();
    assert_eq!(changes[0].text, "3");
}
/** undefined may be shadowed by a local lexical binding, whose TDZ covers the whole block. */
#[test]
fn builtin_scalar_names_are_resolved_after_lexical_cells() {
    assert_eq!(
        execute("function f(){let undefined=7;return undefined;}document.getElementById('out').textContent=f();").unwrap()
            [0]
        .text,
        "7"
    );
    let error = execute("{document.getElementById('out').textContent=undefined;let undefined=7;}")
        .unwrap_err();
    assert!(error.message.contains("before initialization"));
}
/** A global lexical declaration may not shadow a non-configurable global property (undefined, NaN, document...). */
#[test]
fn global_lexical_declarations_of_restricted_globals_are_syntax_errors() {
    for name in [
        "undefined",
        "NaN",
        "Infinity",
        "document",
        "window",
        "location",
    ] {
        let error = execute(&format!("let {name}=7;")).unwrap_err();
        assert!(
            error.message.contains("SyntaxError"),
            "{name}: {}",
            error.message
        );
    }
}
/** Ordered loading and event reconstruction reject both directions of lexical/function conflict. */
#[test]
fn cross_script_function_and_lexical_conflicts_share_the_same_rules() {
    for sources in [
        vec!["function f(){}".into(), "let f=1;".into()],
        vec!["const f=1;".into(), "function f(){}".into()],
    ] {
        let error = load_scripts(&sources, &[]).unwrap_err();
        assert!(error.message.contains("SyntaxError"), "{}", error.message);
        assert_eq!(error.location.unwrap().script_index, Some(2));
    }
    let sources=vec!["function read(){return 1;}".into(),"function read(){return 2;}function show(){document.getElementById('out').textContent=read();}".into()];
    let state = load_scripts(&sources, &[]).unwrap().state;
    assert_eq!(
        event_scripts(&sources, "show()", state, &[])
            .unwrap()
            .changes[0]
            .text()
            .unwrap(),
        "2"
    );
}
/** Each block entry is fresh; inner loops consume their own completion and else binds nearest if. */
#[test]
fn nested_loop_completions_and_branch_selection_are_lexical() {
    let changes=execute("let i=0;let out='';while(i<4){i=i+1;let fresh=0;fresh=fresh+1;if(i===2){continue;}let j=0;while(true){j=j+1;if(j===2){break;}out=out+i+fresh;}if(i===3){break;}}if(true)if(false)out='wrong';else out=out+'ok';document.getElementById('out').textContent=out;").unwrap();
    assert_eq!(changes[0].text, "1131ok");
    for source in ["break;", "if(false){continue;}", "function f(){break;}"] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}
/** Literal conditions, empty statements and continue cannot bypass the shared operation budget. */
#[test]
fn empty_loops_and_deep_control_chains_fail_with_limits() {
    for source in ["while(true){}", "while(true);", "while(true){continue;}"] {
        let error = execute(source).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert_eq!(error.phase.as_deref(), Some("execute"));
    }
    for source in [
        format!("{};", "if(true)".repeat(STATEMENT_DEPTH + 1)),
        format!(
            "{}{}",
            "{".repeat(STATEMENT_DEPTH + 1),
            "}".repeat(STATEMENT_DEPTH + 1)
        ),
    ] {
        assert_eq!(execute(&source).unwrap_err().code, "limit_exceeded");
    }
}
/** Events preserve mutable state, const metadata and all-or-nothing failure candidates. */
#[test]
fn repeated_counter_events_commit_bindings_and_text_atomically() {
    let source = "let count=0;const target=3;function add(){count=count+1;if(count>=target){document.getElementById('branch').textContent='reached';}else{document.getElementById('branch').textContent='keep';}document.getElementById('count').textContent=count;}function reset(){count=0;}function fail(){count=99;document.getElementById('count').textContent='partial';target=5;}";
    let mut state = load(source, &[]).unwrap().state;
    for count in 1..=4 {
        let result = event(source, "add()", state, &[]).unwrap();
        assert_eq!(result.changes[1].text().unwrap(), count.to_string());
        assert_eq!(
            result.changes[0].text().unwrap(),
            if count >= 3 { "reached" } else { "keep" }
        );
        state = serde_json::from_value(serde_json::to_value(result.state).unwrap()).unwrap();
    }
    let before = serde_json::to_value(&state).unwrap();
    assert!(event(source, "fail()", state.clone(), &[]).is_err());
    assert_eq!(serde_json::to_value(&state).unwrap(), before);
    assert_eq!(state.globals()["target"].kind, BindingKind::Const);
    assert_eq!(
        event(source, "reset()", state, &[])
            .unwrap()
            .state
            .globals()["count"]
            .value,
        Some(JsValue::Number(0.0))
    );
}
/** Private event state must match declarations from the same successfully loaded scripts. */
#[test]
fn supplied_event_bindings_cannot_change_declaration_shape_or_mutability() {
    let sources = vec![
        "let count=1;".into(),
        "const target=3;function show(){document.getElementById('out').textContent=count+target;}"
            .into(),
    ];
    let state = load_scripts(&sources, &[]).unwrap().state;
    for mutation in ["missing", "extra", "kind", "uninitialized"] {
        let mut wire = serde_json::to_value(&state).unwrap();
        match mutation {
            "missing" => {
                wire["globals"].as_object_mut().unwrap().remove("count");
            }
            "extra" => {
                wire["globals"]["extra"] = wire["globals"]["count"].clone();
            }
            "kind" => wire["globals"]["target"]["kind"] = json!("let"),
            _ => {
                wire["globals"]["count"]
                    .as_object_mut()
                    .unwrap()
                    .remove("value");
            }
        }
        let supplied = serde_json::from_value(wire).unwrap();
        assert!(
            event_scripts(&sources, "show()", supplied, &[]).is_err(),
            "{mutation}"
        );
    }
}

/** Direct function redeclarations share their var cell and the final declaration initializes it. */
#[test]
fn duplicate_direct_function_declarations_keep_the_last_hoisted_value() {
    let source = "function f(){return 1;}function f(){return 2;}function outer(){function inner(){return 3;}function inner(){return 4;}return inner();}document.getElementById('out').textContent=f()+' '+outer();";
    assert_eq!(execute(source).unwrap()[0].text, "2 4");
}

/** var arguments keeps the arguments object unless replaced; arrows have none, and strict code refuses the name. */
#[test]
fn var_arguments_preserves_explicit_parameters_and_direct_function_replacements() {
    let source = "function parameter(arguments){var arguments;return arguments;}function declared(){var arguments;function arguments(){return 3;}return arguments();}const arrow=()=>{var arguments=5;return arguments;};document.getElementById('out').textContent=parameter(7)+' '+declared()+' '+arrow();";
    assert_eq!(execute(source).unwrap()[0].text, "7 3 5");
    let source = "let arguments=11;const arrow=()=>{var arguments;return arguments;};document.getElementById('out').textContent=String(arrow());";
    assert_eq!(execute(source).unwrap()[0].text, "undefined");
    let source = "function f(){var arguments;return arguments.length;}function g(){if(false)var arguments;return arguments.length;}document.getElementById('out').textContent=f(1,2)+' '+g(3);";
    assert_eq!(execute(source).unwrap()[0].text, "2 1");
    assert_eq!(
        execute("function never(){'use strict';var arguments;}")
            .unwrap_err()
            .code,
        "script_syntax_error"
    );
}

/** Var conflicts cross intervening blocks but stop at function boundaries and independent lexical siblings. */
#[test]
fn unreachable_var_conflicts_respect_lexical_scopes_and_function_boundaries() {
    for source in [
        "if(false){let x;{var x;}}",
        "function never(){let x;if(false)var x;}",
        "for(let x=0;false;){if(false)var x;}",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
    let source = "let x=1;function never(){var x=2;}{let sibling=3;}var sibling=4;function outer(){const inner=()=>{var arguments=9;return arguments;};return inner();}document.getElementById('out').textContent=x+' '+sibling+' '+outer();";
    assert_eq!(execute(source).unwrap()[0].text, "1 4 9");
}

/** Uncaught error messages end in exactly one period, even when the JavaScript message has its own. */
#[test]
fn uncaught_error_messages_end_with_one_period() {
    let error = execute("const c=1;c=2;").unwrap_err();
    assert!(error.message.contains("Assignment to constant variable"));
    assert!(!error.message.ends_with(".."), "{}", error.message);
}
