/** Serialized legacy events must retain real handler source and refuse forged source markers. */
use mars_v9::{Execution, VmState, event, load};
use serde_json::json;

/** Use the public transport representation between every event to exercise graph validation. */
fn next(source: &str, handler: &str, state: VmState) -> Execution {
    let state = serde_json::from_slice(&serde_json::to_vec(&state).unwrap()).unwrap();
    event(source, handler, state, &[]).unwrap()
}

/** Repeating a handler preserves captured variables without growing its retained source ledger. */
#[test]
fn repeated_handlers_reuse_their_validated_source() {
    let source = "let count=0;function run(){document.getElementById('out').textContent=++count}";
    let mut state = load(source, &[]).unwrap().state;
    for expected in 1..=4 {
        let execution = next(source, "run()", state);
        assert_eq!(execution.changes[0].text().unwrap(), expected.to_string());
        state = execution.state;
    }
    let wire = serde_json::to_value(state).unwrap();
    assert_eq!(wire["page"]["legacy_handlers"].as_array().unwrap().len(), 1);
}

/** A later handler keeps earlier handler closures and top-level lexical declarations executable. */
#[test]
fn changed_handlers_preserve_earlier_closures_and_lexical_globals() {
    let source = "var saved;";
    let loaded = load(source, &[]).unwrap();
    let first = next(
        source,
        "let captured=40;saved=()=>++captured;",
        loaded.state,
    );
    let second = next(
        source,
        "document.getElementById('out').textContent=saved();",
        first.state,
    );
    assert_eq!(second.changes[0].text().unwrap(), "41");
    let third = next(
        source,
        "document.getElementById('out').textContent=saved()+1;",
        second.state,
    );
    assert_eq!(third.changes[0].text().unwrap(), "43");
}

/** Source markers grant no exemption for code sites absent from the validated source ledger. */
#[test]
fn absent_handler_source_markers_are_refused() {
    let source = "var count=0;";
    let loaded = load(source, &[]).unwrap();
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    wire["page"]["legacy_handlers"] = json!([1]);
    let error = event(
        source,
        "count++",
        serde_json::from_value(wire).unwrap(),
        &[],
    )
    .unwrap_err();
    assert_eq!(error.message, "Invalid retained legacy handler.");
}

/** Removing the Script marker cannot authorize its lexical bindings using arbitrary dynamic code. */
#[test]
fn unmarked_handler_lexical_bindings_are_refused() {
    let source = "var saved;";
    let first = next(
        source,
        "let captured=1;saved=()=>captured;",
        load(source, &[]).unwrap().state,
    );
    let mut wire = serde_json::to_value(first.state).unwrap();
    wire["page"]["legacy_handlers"] = json!([]);
    let error = event(
        source,
        "saved()",
        serde_json::from_value(wire).unwrap(),
        &[],
    )
    .unwrap_err();
    assert_eq!(error.message, "Invalid V9 state graph: global binding.");
}

/** Internal immutable offsets must never leak into the source-relative handler diagnostic. */
#[test]
fn handler_errors_keep_source_relative_locations() {
    let source = "var ready=true;";
    for handler in ["missing()", "const = ;"] {
        let error = event(source, handler, load(source, &[]).unwrap().state, &[]).unwrap_err();
        assert_eq!(error.input_kind.as_deref(), Some("handler"));
        let location = error.location.unwrap();
        assert!(location.byte_offset < handler.len());
        assert_eq!(location.script_index, None);
    }
}
