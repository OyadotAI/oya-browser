/*! Break and continue early errors name the exact V8 reason, so each jump failure is distinguishable. */

/** The source must fail to parse and its SyntaxError text must contain the given reason. */
fn rejects_with(source: &str, reason: &str) {
    let error = mars_v9::check(source).expect_err(source);
    let text = format!("{error:?}");
    assert!(
        text.contains(reason),
        "{source}: expected {reason:?}, got {text}"
    );
}

/** A labelled continue whose label names a non-loop statement is refused with its own reason. */
#[test]
fn continue_to_non_iteration_label_names_the_label() {
    rejects_with(
        "a: { continue a; }",
        "Illegal continue statement: 'a' does not denote an iteration statement",
    );
}

/** A jump to a label that is not in scope is reported as an undefined label, with the label's name. */
#[test]
fn undefined_label_names_the_label() {
    rejects_with(
        "while (true) { break missing; }",
        "Undefined label 'missing'",
    );
    rejects_with(
        "while (true) { continue missing; }",
        "Undefined label 'missing'",
    );
}

/** An unlabelled continue outside any loop says that no iteration statement surrounds it. */
#[test]
fn continue_outside_loop_names_the_missing_loop() {
    rejects_with(
        "continue;",
        "Illegal continue statement: no surrounding iteration statement",
    );
}

/** An unlabelled break outside any breakable statement keeps the plain break reason. */
#[test]
fn break_outside_breakable_keeps_plain_reason() {
    rejects_with("break;", "Illegal break statement");
}

/** A labelled continue to an enclosing loop label is still accepted. */
#[test]
fn continue_to_enclosing_loop_label_is_accepted() {
    assert!(mars_v9::check("outer: while (true) { inner: for (;;) { continue outer; } }").is_ok());
}
