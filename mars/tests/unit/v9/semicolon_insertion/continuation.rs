/** Insertion only follows completed expressions and cannot split valid expression continuations. */
use super::*;

/** Calls continue through whitespace and comments instead of becoming separate statements. */
#[test]
fn newline_before_call_arguments_keeps_one_expression() {
    assert_eq!(
        result("function f(n){return n+1}\nconst n=f\n(2)", "n"),
        "3"
    );
    assert_eq!(
        result("function f(n){return n+1}\nconst n=f/*\n*/(3)", "n"),
        "4"
    );
}

/** Members and indexes remain attached across inter-token line terminators. */
#[test]
fn newline_before_members_and_indexes_keeps_one_expression() {
    assert_eq!(result("const a=[4]\nconst n=a\n[0]", "n"), "4");
    assert_eq!(result("const a={n:5}\nconst n=a\n.n", "n"), "5");
}

/** Binary and assignment operators consume their continuation operands across line breaks. */
#[test]
fn operators_and_assignments_continue_across_lines() {
    assert_eq!(result("let n=1\n+2\n*3", "n"), "7");
    assert_eq!(result("let n=1\nn\n=2\n+3", "n"), "5");
    assert_eq!(result("let n=1\nn\n+=2", "n"), "3");
}

/** A continued expression with a missing RHS remains a syntax failure at the original boundary. */
#[test]
fn continuation_errors_are_not_reinterpreted_as_new_statements() {
    for source in [
        "let n=1\n+",
        "let n=1\nn\n=",
        "let n={a:1}\nn\n.",
        "function f(){return 1\n+}",
    ] {
        syntax(source);
    }
}

/** An implicit complete strict directive validates all later and unreachable bindings. */
#[test]
fn strict_directives_require_complete_exact_literal_statements() {
    for source in [
        "'use strict'\nvar eval=1",
        "function f(arguments){'use strict'\nreturn arguments}",
        "'use strict'\nif(false){eval=1}",
    ] {
        let error = syntax(source);
        assert!(error.message.contains("eval or arguments in strict mode"));
    }
    for source in [
        "function unused(){'use\\x20strict'\nvar eval=1}",
        "function unused(){('use strict')\nvar eval=1}",
        "function unused(){0\n'use strict'\nvar eval=1}",
        "function unused(){if(false){'use strict'\nvar eval=1}}",
    ] {
        load(source, &[]).unwrap();
    }
}

/** A call continuation prevents a preceding string literal from becoming a strict directive. */
#[test]
fn directive_looking_call_continuation_does_not_activate_strict_mode() {
    let source = "function unused(){'use strict'\n(function(){})\nvar eval=1}";
    load(source, &[]).unwrap();
    let source = "'use strict'\n(function(){})\nvar answer=1";
    let error = load(source, &[]).unwrap_err();
    assert_eq!(error.code, "script_execution_error");
    assert_eq!(error.phase.as_deref(), Some("execute"));
}

/** A strict outer source transfers mode to a nested function whose return terminates implicitly. */
#[test]
fn implicit_directives_preserve_nested_function_mode_inheritance() {
    let error = syntax("'use strict'\nfunction unused(){var arguments=1\nreturn arguments}");
    assert!(error.message.contains("eval or arguments in strict mode"));
}
