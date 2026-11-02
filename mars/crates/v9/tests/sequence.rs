/** Comma (sequence) expressions run every operand in order through the public V9 facade. */
use mars_v9::{JsValue, load};

/** Reads `out` through a lexical global, since script globals expose lexical bindings only. */
fn global(source: &str) -> Option<JsValue> {
    load(&format!("{source}\nconst r = out;"), &[])
        .unwrap()
        .state
        .globals()["r"]
        .value
        .clone()
}

/** Builds a string value for comparisons. */
fn text(value: &str) -> Option<JsValue> {
    Some(JsValue::string(value).unwrap())
}

/** A sequence yields its last operand. */
#[test]
fn sequence_value_is_the_last_operand() {
    assert_eq!(global("var out = (1, 2, 3)"), Some(JsValue::Number(3.0)));
}

/** Operands run left to right, observable through their side effects. */
#[test]
fn operands_evaluate_left_to_right() {
    assert_eq!(
        global("var out = ''; (out += 'a', out += 'b', out += 'c');"),
        text("abc")
    );
}

/** An expression statement with commas runs every operand, including calls. */
#[test]
fn expression_statement_runs_every_operand() {
    let source = "var out = 0; function f() { out = out + 1; } f(), f(), out = out * 10;";
    assert_eq!(global(source), Some(JsValue::Number(20.0)));
}

/** For-loop initializers and updates accept comma expressions. */
#[test]
fn for_loop_clauses_accept_sequences() {
    let source = "var i, j, out = 0; for (i = 0, j = 10; i < j; i++, j--) out++;";
    assert_eq!(global(source), Some(JsValue::Number(5.0)));
}

/** Return, condition and computed-key positions take a full Expression. */
#[test]
fn return_conditions_and_keys_accept_sequences() {
    let source = "var o = { b: 2 }; function f() { return 1, 'b'; } \
                  var out = 0; if (0, 1) out = o[(0, f())];";
    assert_eq!(global(source), Some(JsValue::Number(2.0)));
}

/** A function literal inside a sequence registers and is callable. */
#[test]
fn function_inside_sequence_is_callable() {
    assert_eq!(
        global("var f = (0, function () { return 7; }); var out = f();"),
        Some(JsValue::Number(7.0))
    );
}

/** Strict checks walk every sequence operand. */
#[test]
fn strict_mode_checks_sequence_operands() {
    let error = load("'use strict'; (0, eval = 1);", &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
    let sloppy = load("(0, eval = 1);", &[]).map_err(|error| error.code);
    assert_ne!(sloppy.err().as_deref(), Some("script_syntax_error"));
}

/** A parenthesized parameter list followed by an arrow stays an arrow. */
#[test]
fn parenthesized_pair_before_arrow_is_still_an_arrow() {
    assert_eq!(
        global("var add = (a, b) => a + b; var out = add(2, 3);"),
        Some(JsValue::Number(5.0))
    );
}

/** A sequence is not an assignment target. */
#[test]
fn sequence_assignment_target_is_a_syntax_error() {
    let error = load("var a, b; (a, b) = 1;", &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
    assert_eq!(error.phase.as_deref(), Some("parse"));
}

/** Call arguments keep their own commas rather than forming one sequence. */
#[test]
fn call_arguments_remain_separate() {
    assert_eq!(
        global("function f(a, b) { return b; } var out = f(1, 2);"),
        Some(JsValue::Number(2.0))
    );
}

/** Sequence operands charge the shared work budget, so an endless loop of them still stops. */
#[test]
fn every_operand_charges_the_operation_budget() {
    let error = load("for (;;) (0, 0, 0, 0);", &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(
        error.message.contains("limit exceeded"),
        "{}",
        error.message
    );
}

/** The Google Analytics snippet's comma at line 3, column 43 parses; host APIs may fail later. */
#[test]
fn analytics_shaped_comma_parses() {
    let source = "var i = {}, r = 'ga';\n\n\
                  i['GoogleAnalyticsObject']=r;i[r]=i[r]||function(){},i[r].l=1;";
    let execution = load(&format!("{source}\nconst stamp = i[r].l;"), &[]).unwrap();
    let ga = execution.state.globals()["stamp"].value.clone();
    assert_eq!(ga, Some(JsValue::Number(1.0)));
}
