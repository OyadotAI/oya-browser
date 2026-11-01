/** Implicit punctuation consumes no new source tokens or statement records and raises no budgets. */
use super::*;
use mars::constants::{EXPRESSION_DEPTH, SCRIPT_BYTES, STATEMENT_DEPTH, STATEMENTS, TOKENS};

/** Semicolonless source remains subject to the exact existing byte admission boundary. */
#[test]
fn implicit_termination_preserves_the_source_byte_limit() {
    let prefix = "var n=1/*";
    let source = format!("{prefix}{}*/", "x".repeat(SCRIPT_BYTES - prefix.len() - 2));
    assert_eq!(source.len(), SCRIPT_BYTES);
    load(&source, &[]).unwrap();
    let error = load(&format!("{source} "), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("script bytes"));
}

/** Exactly the same number of real statements fits with implicit and explicit punctuation. */
#[test]
fn inserted_terminators_do_not_consume_statement_budget() {
    for separator in ["\n", ";"] {
        let source = std::iter::repeat_n("0", STATEMENTS)
            .collect::<Vec<_>>()
            .join(separator);
        load(&source, &[]).unwrap();
        let error = load(&format!("{source}{separator}0"), &[]).unwrap_err();
        assert_eq!(error.code, "limit_exceeded");
        assert!(error.message.contains("statements"));
    }
}

/** The End token is free: a script of exactly TOKENS tokens scans, one more does not. */
#[test]
fn implicit_eof_adds_no_program_token_and_keeps_aggregate_end_accounting() {
    // `function f(){return[` and `]}` are 9 tokens; n elements add 2n - 1.
    let count = (TOKENS - 8) / 2;
    let elements = std::iter::repeat_n("1", count)
        .collect::<Vec<_>>()
        .join(",");
    let source = format!("function f(){{return[{elements}]}}");
    load(&source, &[]).unwrap();
    load_scripts(std::slice::from_ref(&source), &[]).unwrap();
    let error = load(&format!("{source};"), &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert_eq!(error.phase.as_deref(), Some("scan"));
}

/** Bare-return and block insertion cannot bypass existing recursive expression admission. */
#[test]
fn implicit_termination_preserves_expression_and_statement_depth_limits() {
    let expression = format!(
        "var n={}1{}",
        "(".repeat(EXPRESSION_DEPTH),
        ")".repeat(EXPRESSION_DEPTH)
    );
    let error = load(&expression, &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("syntax depth"), "{}", error.message);
    let nested = format!(
        "{}0{}",
        "{".repeat(STATEMENT_DEPTH),
        "}".repeat(STATEMENT_DEPTH)
    );
    let error = load(&nested, &[]).unwrap_err();
    assert_eq!(error.code, "limit_exceeded");
    assert!(error.message.contains("statement nesting"));
}
