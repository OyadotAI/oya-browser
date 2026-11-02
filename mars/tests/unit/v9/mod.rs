/** V9 tests verify interpretation without an existing JavaScript engine. */
use mars::constants::{SCRIPT_BYTES, STATEMENTS, TEXT_BYTES};
use mars::v9::{execute, load};
mod abstract_equality;
mod array_selection;
mod array_spread;
mod array_spread_syntax;
mod arrays;
mod arrows;
mod callable_graph;
mod callable_safety;
mod closures;
mod collection_intrinsics;
mod control;
mod dom;
mod expressions;
mod for_loop;
mod function_calls;
mod function_metadata;
mod graph;
mod operators;
mod receivers;
mod semicolon_insertion;
mod strings;
mod var_bindings;

/** The heap object id a committed global names, read from the private state wire. */
fn wire_global_object(wire: &serde_json::Value, name: &str) -> u64 {
    wire["globals"][name]["value"]["o"].as_u64().unwrap()
}

/** One heap object record on the private state wire, for forging hostile states. */
fn wire_object(wire: &mut serde_json::Value, id: u64) -> &mut serde_json::Value {
    let objects = wire["heap"][0].as_array_mut().unwrap();
    let entry = objects.iter_mut().find(|entry| entry[0] == id).unwrap();
    &mut entry[1]
}

/** Variables, escapes and concatenation are evaluated from the parsed AST. */
#[test]
fn own_interpreter_produces_native_text_proposals() {
    let changes = execute("let greeting = 'Héllo'; let suffix = \" 🐙\\n\"; document.getElementById('title').textContent = greeting + suffix;").unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].html_id, "title");
    assert_eq!(changes[0].text, "Héllo 🐙\n");
}

/** Host calls become requests for the app, never direct access; unknown names, syntax and unsupported features fail explicitly. */
#[test]
fn unsupported_language_and_host_access_fail_explicitly() {
    let fetched = load("fetch('http://evil');", &[]).unwrap();
    assert_eq!(fetched.effects.requests.len(), 1);
    for (source, code) in [
        ("process.exit();", "script_execution_error"),
        ("let a = unknown;", "script_execution_error"),
        ("let a = 'x' let b = 'y'", "script_syntax_error"),
        ("let a = 'x'; let a = 'y';", "script_syntax_error"),
        (
            "document.getElementById('a').innerHTML = 'x';",
            "unsupported_feature",
        ),
        (
            "document.getElementById('a').textContent = 'x'; invalid();",
            "script_execution_error",
        ),
    ] {
        assert_eq!(execute(source).unwrap_err().code, code, "{source}");
    }
}

/** Conditional and loop execution obey truthiness and stop at the shared operation budget. */
#[test]
fn conditional_and_loop_statements_execute_with_bounds() {
    let changes = execute("let total=0; while(total<4){total=total+1;} if(total===4){document.getElementById('a').textContent=total;}else{document.getElementById('a').textContent='wrong';}").unwrap();
    assert_eq!(changes[0].text, "4");
    assert_eq!(
        execute("while(true) {}").unwrap_err().code,
        "limit_exceeded"
    );
}

/** A fresh environment for each call prevents cross-document variable leaks. */
#[test]
fn environments_are_per_execution() {
    execute("let previous = 'secret';").unwrap();
    assert_eq!(
        execute("document.getElementById('a').textContent = previous;")
            .unwrap_err()
            .code,
        "script_execution_error"
    );
}

/** Source, statement count and evaluated text have separate work/allocation bounds. */
#[test]
fn language_budgets_are_enforced() {
    assert_eq!(
        execute(&" ".repeat(SCRIPT_BYTES + 1)).unwrap_err().code,
        "limit_exceeded"
    );
    let source: String = (0..=STATEMENTS)
        .map(|id| format!("let n{id} = 'x';"))
        .collect();
    assert_eq!(execute(&source).unwrap_err().code, "limit_exceeded");
    // Text past TEXT_BYTES is refused at the DOM write, an uncatchable limit.
    let source = format!(
        "let a = '{}'; document.getElementById('a').textContent = a + a;",
        "x".repeat(TEXT_BYTES)
    );
    assert_eq!(execute(&source).unwrap_err().code, "limit_exceeded");
}
/** Event handlers read current input data, run as ordinary code over load globals, and recursion still fails. */
#[test]
fn event_handlers_use_current_inputs_and_fresh_function_locals() {
    use mars::v9::{InputValue, event};
    let source = "let prefix='Hello '; function greet() { let name=document.getElementById('name').value; document.getElementById('out').textContent=prefix+name; }";
    let loaded = load(source, &[]).unwrap();
    assert!(loaded.changes.is_empty());
    for name in ["first", "second"] {
        let result = event(
            source,
            "greet()",
            loaded.state.clone(),
            &[InputValue {
                html_id: "name".into(),
                value: name.into(),
            }],
        )
        .unwrap();
        assert_eq!(result.changes[0].text().unwrap(), format!("Hello {name}"));
    }
    let inline = event(
        source,
        "document.getElementById('out').textContent=prefix+'inline'",
        loaded.state.clone(),
        &[],
    )
    .unwrap();
    assert_eq!(inline.changes[0].text().unwrap(), "Hello inline");
    assert!(
        event(
            "function loop() { loop(); }",
            "loop()",
            load("function loop() { loop(); }", &[]).unwrap().state,
            &[]
        )
        .is_err()
    );
}
