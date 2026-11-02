/** Source-guided scalar cases exercise only the public V9 load/event/DOM proposal boundary. */
use mars::{
    constants::{EXPRESSION_DEPTH, JS_ENVIRONMENT_UNITS, JS_STRING_UNITS},
    v9::{JsString, JsValue, event, execute, load},
};
use serde_json::json;

/** An object identity no page heap reaches: below the intrinsic range, far past any allocation,
so a forged reference stays dangling however much the platform bootstrap allocates. */
const DANGLING: u32 = 0x3FFF_FFF0;

/** DOM conversion is observable without adding a test-only evaluate API. */
fn evaluate(expression: &str) -> String {
    execute(&format!(
        "document.getElementById('result').textContent={expression};"
    ))
    .unwrap()
    .remove(0)
    .text
}
/** New conformance cases pin precedence, grouping and left association. */
#[test]
fn arithmetic_precedence_and_associativity_match_javascript() {
    for (source, expected) in [
        ("2+3*4", "14"),
        ("(2+3)*4", "20"),
        ("20/5/2", "2"),
        ("10-3-2", "5"),
        ("-2*-3+4", "10"),
        ("7%-4", "3"),
        ("-7%4", "-3"),
        ("+' 12.5e1 ' + 1", "126"),
        ("'2'+3*4", "212"),
        ("2+3+'4'", "54"),
        ("'2'-'1'", "1"),
        ("'  '\u{feff}-0", "0"),
        ("'x'-1", "NaN"),
        ("1/0", "Infinity"),
        ("-1/0", "-Infinity"),
        ("0/0", "NaN"),
        ("1/-0", "-Infinity"),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
}
/** Adapted from V8 number-tostring.js decimal cases, Copyright 2008 V8 project authors. */
#[test]
fn adapted_v8_decimal_formatting_cases_keep_ecmascript_thresholds() {
    for (source, expected) in [
        ("0", "0"),
        ("-0", "0"),
        ("NaN", "NaN"),
        ("Infinity", "Infinity"),
        ("-Infinity", "-Infinity"),
        ("0.000001", "0.000001"),
        ("0.0000001", "1e-7"),
        ("1000000000000000000000", "1e+21"),
        ("111111111111111111111", "111111111111111110000"),
        ("1.1e21", "1.1e+21"),
        ("1e20", "100000000000000000000"),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
}
/** New lexical cases follow the reviewed V8 comment/string escape routines. */
#[test]
fn comments_numeric_literals_and_separators_preserve_token_boundaries() {
    for (source, expected) in [
        ("/* before */ 1 + // line\r\n2", "3"),
        ("0xff + 0b10 + 0o10", "265"),
        ("1_000 + .5e2", "1050"),
        ("0x1_0 + 0b1_0", "18"),
        ("1/* split */+2", "3"),
        ("0x1000000000000081", "1152921504606847200"),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
    for source in [
        "1_",
        "1__0",
        "1_.0",
        "1._0",
        "0x_1",
        "0b102",
        "1e+",
        "1e_2",
        "12abc",
        "1++2",
        "1--2",
        "/* never closed",
    ] {
        let error = execute(&format!("let n={source};")).unwrap_err();
        assert_eq!(error.code, "script_syntax_error", "{source}");
        assert!(error.location.is_some());
    }
    for source in ["077", "'\\1'", "`text`"] {
        execute(&format!("let n={source};")).unwrap();
    }
    assert_eq!(evaluate("2**10"), "1024");
    execute("let n=1n;").unwrap();
    // Past 128 bits a BigInt widens to limbs rather than being refused.
    assert_eq!(
        evaluate("String(1000000000000000000000000000000000000000n+1n)"),
        "1000000000000000000000000000000000000001"
    );
}
/** JavaScript identity escapes and UTF-16 pairs replace the prototype's invented escape restrictions. */
#[test]
fn string_escapes_and_line_continuations_follow_javascript_rules() {
    for (source, expected) in [
        ("'\\q'", "q"),
        ("'\\x41\\u0042\\u{1f419}'", "AB🐙"),
        ("'\\ud83d\\udc19'", "🐙"),
        ("'one\\\r\ntwo'", "onetwo"),
        ("'a\\\u{2028}b'", "ab"),
        ("'\\b\\f\\v\\0'", "\u{8}\u{c}\u{b}\0"),
        ("'a\u{2028}b'", "a\u{2028}b"),
    ] {
        assert_eq!(evaluate(source), expected, "{source}");
    }
    for source in [
        "'\\x0'",
        "'\\u{}'",
        "'\\u{110000}'",
        "'\\u12'",
        "'line\nbreak'",
    ] {
        assert_eq!(
            execute(&format!("let s={source};")).unwrap_err().code,
            "script_syntax_error"
        );
    }
}
/** Negative zero, infinities, NaN and lone surrogates survive the state wire; the DOM write replaces the surrogate. */
#[test]
fn typed_wire_preserves_nonfinite_signed_zero_and_surrogate_units() {
    let loaded = load("let n=-0; let inf=1/0; let nan=0/0; let s='\\ud800';", &[]).unwrap();
    let wire = serde_json::to_string(&loaded).unwrap();
    let restored: mars::v9::Execution = serde_json::from_str(&wire).unwrap();
    let globals = restored.state.globals();
    assert!(matches!(globals["inf"].value, Some(JsValue::Number(n)) if n == f64::INFINITY));
    assert!(matches!(globals["nan"].value, Some(JsValue::Number(n)) if n.is_nan()));
    assert!(
        matches!(globals["n"].value, Some(JsValue::Number(n)) if n == 0.0 && n.is_sign_negative())
    );
    assert!(matches!(&globals["s"].value, Some(JsValue::String(s)) if s.as_units() == [0xd800]));
    let changes = execute("document.getElementById('x').textContent='\\ud800';").unwrap();
    assert_eq!(changes[0].text, "\u{fffd}");
    for value in [json!({"n":"bogus"}), json!({"w":vec![0;JS_STRING_UNITS+1]})] {
        assert!(
            serde_json::from_value::<JsValue>(value.clone()).is_err(),
            "{value}"
        );
    }
}
/** A global holding null keeps that value across the state wire instead of falling back into its dead zone. */
#[test]
fn null_globals_survive_the_state_wire() {
    use mars::v9::{VmState, event};
    let source = "let saved=null;const fixed=null;function show(){document.getElementById('out').textContent=String(saved)+' '+String(fixed);}";
    let loaded = load(source, &[]).unwrap();
    let state: VmState =
        serde_json::from_value(serde_json::to_value(&loaded.state).unwrap()).unwrap();
    assert_eq!(
        event(source, "show()", state, &[]).unwrap().changes[0]
            .text()
            .unwrap(),
        "null null"
    );
}
/** Located errors never include the source, literals or field values. */
#[test]
fn script_locations_count_utf16_columns_and_crlf_once() {
    let error = execute("let a='🐙'; let n=; private-secret").unwrap_err();
    let location = error.location.unwrap();
    assert_eq!(location.line, 1);
    assert_eq!(location.column, 19);
    assert!(!error.message.contains("private-secret"));
    let error = execute("// comment\r\nlet n=; ").unwrap_err();
    assert_eq!(error.location.unwrap().line, 2);
    assert_eq!(error.input_kind.as_deref(), Some("script"));
    assert_eq!(
        execute("\u{85}let a=1;").unwrap_err().code,
        "script_syntax_error"
    );
}
/** Candidate environments and proposals never escape when an event fails after a valid write. */
#[test]
fn typed_globals_survive_event_round_trips_and_failed_candidates() {
    let source = "let total=2+3*4; function calculate(){document.getElementById('result').textContent=total+1;} function fail(){document.getElementById('result').textContent='partial';missing();}";
    let loaded = load(source, &[]).unwrap();
    let wire = serde_json::to_string(&loaded.state).unwrap();
    let state = serde_json::from_str(&wire).unwrap();
    let result = event(source, "calculate()", state, &[]).unwrap();
    assert_eq!(result.changes[0].text().unwrap(), "15");
    assert!(event(source, "fail()", loaded.state.clone(), &[]).is_err());
    assert_eq!(
        loaded.state.globals()["total"].value,
        Some(JsValue::Number(14.0))
    );
}

/** Object identity, array properties, function frames and returns survive service serialization. */
#[test]
fn objects_arrays_parameters_and_returns_flow_through_native_events() {
    use mars::v9::event;
    let source = "const model={items:[2,3]}; let score=0; function update(a,b){model.items[0]=a+b; if(model.items[0]>10){score=model.items[0];}else{score=0;} return score+model.items.length;} function show(){document.getElementById('result').textContent=update(4,7);}";
    let loaded = load(source, &[]).unwrap();
    let wire = serde_json::to_vec(&loaded.state).unwrap();
    let state: mars::v9::VmState = serde_json::from_slice(&wire)
        .unwrap_or_else(|error| panic!("{error}: {}", String::from_utf8_lossy(&wire)));
    let result = event(source, "show()", state, &[])
        .unwrap_or_else(|error| panic!("{error:?}: {}", String::from_utf8_lossy(&wire)));
    assert_eq!(result.changes[0].text().unwrap(), "13");
    assert!(matches!(
        result.state.globals()["model"].value,
        Some(JsValue::Object(_))
    ));
}

/** Heap aliases and cycles use stable arena IDs rather than recursive wire structures. */
#[test]
fn object_aliases_and_cycles_keep_identity_across_events() {
    use mars::v9::event;
    let source = "let first={}; let alias=first; first.self=first; function show(){document.getElementById('result').textContent=(first===alias)+' '+(first===first.self);}";
    let loaded = load(source, &[]).unwrap();
    let wire = serde_json::to_vec(&loaded.state).unwrap();
    let state = serde_json::from_slice(&wire).unwrap();
    let result = event(source, "show()", state, &[]).unwrap();
    assert_eq!(result.changes[0].text().unwrap(), "true true");
}

/** Failed object and DOM writes remain confined to the discarded event candidate. */
#[test]
fn failed_heap_event_discards_mutations_and_dom_proposals() {
    use mars::v9::event;
    let source = "let state={value:1}; function fail(){state.value=9; document.getElementById('result').textContent='partial';missing();}";
    let loaded = load(source, &[]).unwrap();
    let before = serde_json::to_vec(&loaded.state).unwrap();
    let result = event(source, "fail()", loaded.state.clone(), &[]);
    assert!(result.is_err());
    assert_eq!(before, serde_json::to_vec(&loaded.state).unwrap());
}

/** Deserialized heap references are checked before an event evaluates page code. */
#[test]
fn dangling_heap_references_are_rejected_at_the_state_boundary() {
    use mars::v9::{VmState, event};
    let source =
        "let state={value:1}; function show(){document.getElementById('result').textContent=1;}";
    let loaded = load(source, &[]).unwrap();
    let mut wire = serde_json::to_value(&loaded.state).unwrap();
    let id = crate::v9::wire_global_object(&wire, "state");
    crate::v9::wire_object(&mut wire, id)["k"][0][1] = json!({"o": DANGLING});
    let state: VmState = serde_json::from_value(wire).unwrap();
    assert_eq!(
        event(source, "show()", state, &[]).unwrap_err().code,
        "invalid_request"
    );
}

/** Block declarations shadow outer names throughout their temporal dead zone. */
#[test]
fn block_temporal_dead_zone_prevents_outer_value_reads() {
    use mars::v9::event;
    let source = "let value=1; function show(){{document.getElementById('result').textContent=value; let value=2;}}";
    let loaded = load(source, &[]).unwrap();
    let error = event(source, "show()", loaded.state, &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("before initialization"));
}

/** Assignment cannot initialize a lexical cell before its declaration executes. */
#[test]
fn block_temporal_dead_zone_prevents_assignment_and_discards_proposals() {
    use mars::v9::event;
    let source = "let outer=1; function show(){{outer=9; document.getElementById('result').textContent='partial'; let outer=2;}}";
    let loaded = load(source, &[]).unwrap();
    let before = serde_json::to_vec(&loaded.state).unwrap();
    let error = event(source, "show()", loaded.state.clone(), &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert!(error.message.contains("before initialization"));
    assert_eq!(before, serde_json::to_vec(&loaded.state).unwrap());
}

/** Function call depth counts calls, not lexical block scopes. */
#[test]
fn nested_blocks_do_not_consume_function_call_depth() {
    use mars::v9::event;
    let mut nested = String::from("function show(){");
    for _ in 0..20 {
        nested.push('{');
    }
    nested.push_str("document.getElementById('result').textContent=7;");
    for _ in 0..20 {
        nested.push('}');
    }
    nested.push('}');
    let loaded = load(&nested, &[]).unwrap();
    let result = event(&nested, "show()", loaded.state, &[]).unwrap();
    assert_eq!(result.changes[0].text().unwrap(), "7");
}

/** Parameter names cannot redeclare lexical bindings in their function body. */
#[test]
fn function_parameters_conflict_with_body_declarations_during_parse() {
    let error = load("function show(value){let value=2;}", &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
}

/** Page scripts share globals and execute in document order within one VM budget. */
#[test]
fn ordered_page_scripts_share_state_and_event_functions() {
    use mars::v9::{event_scripts, load_scripts};
    let sources = vec![
        "let value=3; function show(){document.getElementById('result').textContent=value;} // tag boundary".into(),
        "value=value+4;".into(),
    ];
    let loaded = load_scripts(&sources, &[]).unwrap();
    let result = event_scripts(&sources, "show()", loaded.state, &[]).unwrap();
    assert_eq!(result.changes[0].text().unwrap(), "7");
}

/** Storage calls mutate bounded candidates and remain visible to ordered event execution. */
#[test]
fn local_and_session_storage_methods_return_candidates_for_events() {
    use mars::v9::{StorageState, event_scripts_with_storage, load_scripts_with_storage};
    let sources = vec![
        "window.localStorage.setItem('local', 'persisted'); window.sessionStorage.setItem('session', 'tab'); function show(){document.getElementById('result').textContent=window.localStorage.getItem('local')+' '+window.sessionStorage.getItem('session');}".into(),
    ];
    let loaded = load_scripts_with_storage(&sources, &[], StorageState::default()).unwrap();
    assert_eq!(loaded.storage.local.get("local").unwrap(), "persisted");
    assert_eq!(loaded.storage.session.get("session").unwrap(), "tab");
    let result =
        event_scripts_with_storage(&sources, "show()", loaded.state, &[], loaded.storage).unwrap();
    assert_eq!(result.changes[0].text().unwrap(), "persisted tab");
}

/** Missing keys are null, and removeItem/clear affect only their candidate area. */
#[test]
fn storage_missing_remove_and_clear_follow_web_storage_rules() {
    use mars::v9::{StorageState, load_scripts_with_storage};
    let sources = vec!["window.localStorage.setItem('a','1'); window.localStorage.removeItem('a'); window.sessionStorage.setItem('b','2'); window.sessionStorage.clear(); document.getElementById('result').textContent=(window.localStorage.getItem('a')===null)+' '+(window.sessionStorage.getItem('b')===null);".into()];
    let result = load_scripts_with_storage(&sources, &[], StorageState::default()).unwrap();
    assert_eq!(result.storage.local.get("a"), None);
    assert_eq!(result.storage.session.get("b"), None);
    assert_eq!(result.changes[0].text().unwrap(), "true true");
}

/** A write past the storage quota throws a catchable QuotaExceededError and leaves the area unchanged. */
#[test]
fn storage_quota_failure_throws_and_keeps_the_area() {
    use mars::{
        constants::STORAGE_AREA_BYTES,
        v9::{StorageState, load_scripts_with_storage},
    };
    const ENTRIES_TO_FILL: usize = 32;
    let local: std::collections::BTreeMap<String, String> = (0..ENTRIES_TO_FILL)
        .map(|index| {
            let key = format!("k{index}");
            let value = "x".repeat(STORAGE_AREA_BYTES / ENTRIES_TO_FILL - key.len());
            (key, value)
        })
        .collect();
    let storage = StorageState {
        local: local.clone(),
        session: Default::default(),
    };
    let sources = vec!["let caught='';try{window.localStorage.setItem('new','x');}catch(error){caught=error.name;}".into()];
    let loaded = load_scripts_with_storage(&sources, &[], storage).unwrap();
    assert_eq!(
        loaded.state.globals()["caught"].value,
        Some(JsValue::String(
            JsString::from_utf8("QuotaExceededError").unwrap()
        ))
    );
    assert_eq!(loaded.storage.local, local);
}

/** Later classic scripts do not hoist their functions into earlier execution. */
#[test]
fn later_page_script_functions_are_not_visible_earlier() {
    use mars::v9::load_scripts;
    let sources = vec![
        "show();".into(),
        "function show(){document.getElementById('result').textContent='later';}".into(),
    ];
    let error = load_scripts(&sources, &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert_eq!(error.location.unwrap().script_index, Some(1));
}

/** A later duplicate reports its declaration's own source line and script index. */
#[test]
fn duplicate_page_declarations_point_to_the_conflicting_declaration() {
    use mars::v9::load_scripts;
    let sources = vec![
        "let value=1;".into(),
        "document.getElementById('result').textContent='candidate';\nlet value=2;".into(),
    ];
    let location = load_scripts(&sources, &[]).unwrap_err().location.unwrap();
    assert_eq!(location.script_index, Some(2));
    assert_eq!(location.line, 2);
}

/** Later function declarations cannot shadow earlier page lexical bindings. */
#[test]
fn page_functions_cannot_conflict_with_prior_script_bindings() {
    use mars::v9::load_scripts;
    let sources = vec!["let show=3;".into(), "function show(){return 4;}".into()];
    let location = load_scripts(&sources, &[]).unwrap_err().location.unwrap();
    assert_eq!(location.script_index, Some(2));
    assert_eq!(location.line, 1);
}

/** A runtime failure in a later script identifies that script and returns no candidate. */
#[test]
fn later_page_script_errors_include_source_order_index() {
    use mars::v9::load_scripts;
    let sources = vec![
        "document.getElementById('result').textContent='candidate';".into(),
        "let value=missing;".into(),
    ];
    let error = load_scripts(&sources, &[]).unwrap_err();
    assert_eq!(
        error
            .location
            .as_deref()
            .and_then(|location| location.script_index),
        Some(2)
    );
    assert_eq!(error.code, "script_execution_error");
}

/** Scanner and parser failures keep per-script source positions after ordered token merging. */
#[test]
fn later_script_syntax_errors_keep_their_script_index_and_line() {
    use mars::v9::load_scripts;
    let sources = vec!["let value=1;".into(), "let broken=;".into()];
    let error = load_scripts(&sources, &[]).unwrap_err();
    let location = error.location.unwrap();
    assert_eq!(location.script_index, Some(2));
    assert_eq!(location.line, 1);
}
/** A handler's optional terminator is inserted after scanning without rewriting source locations. */
#[test]
fn trailing_handler_comments_do_not_swallow_the_optional_semicolon() {
    let source = "function f(){document.getElementById('result').textContent=2+3*4;}";
    for handler in [
        "f()",
        "f() // note",
        "f(); // note",
        "f() /* note */",
        "/*🐙*/\r\nf() // note",
        "f() // note\u{2028}",
    ] {
        let state = load(source, &[]).unwrap().state;
        let result = event(source, handler, state, &[]).unwrap();
        assert_eq!(result.changes[0].text().unwrap(), "14", "{handler}");
    }
    let error = event(
        source,
        "/*🐙*/\r\nmissing() // note",
        load(source, &[]).unwrap().state,
        &[],
    )
    .unwrap_err();
    let location = error.location.unwrap();
    assert_eq!(
        (location.line, location.column, location.byte_offset),
        (2, 1, "/*🐙*/\r\n".len())
    );
    assert_eq!(error.input_kind.as_deref(), Some("handler"));
}
/** AST depth bounds explicit nesting, and committed strings are bounded separately in aggregate. */
#[test]
fn expression_and_environment_budgets_are_independent() {
    let nested = format!(
        "{}1{}",
        "(".repeat(EXPRESSION_DEPTH),
        ")".repeat(EXPRESSION_DEPTH)
    );
    assert_eq!(
        execute(&format!("let n={nested};")).unwrap_err().code,
        "limit_exceeded"
    );
    let value = JsValue::String(JsString::from_utf8(&"x".repeat(JS_STRING_UNITS)).unwrap());
    let globals = (0..=JS_ENVIRONMENT_UNITS / JS_STRING_UNITS)
        .map(|i| (format!("n{i}"), value.clone()))
        .map(|(name, value)| {
            (
                name,
                mars::v9::Binding {
                    kind: mars::v9::BindingKind::Let,
                    value: Some(value),
                },
            )
        })
        .collect();
    assert_eq!(
        event("", "0", mars::v9::VmState::from_globals(globals), &[])
            .unwrap_err()
            .code,
        "limit_exceeded"
    );
}
