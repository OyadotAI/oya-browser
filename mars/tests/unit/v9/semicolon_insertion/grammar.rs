/** Completed supported statements terminate independently at source and block boundaries. */
use super::*;

/** Declarations and assignment statements do not require a final punctuation token. */
#[test]
fn eof_completes_supported_bindings_and_assignments() {
    for source in ["var n", "var n=1", "let n=1", "const n=1", "let n=0;n=1"] {
        load(source, &[]).unwrap();
    }
    assert_eq!(result("var n", "n===undefined"), "true");
    assert_eq!(result("var n=1\nn=2", "n"), "2");
}

/** Calls and the legacy literal DOM path both admit ordinary source EOF. */
#[test]
fn eof_completes_calls_and_literal_dom_writes() {
    let source = "function show(){document.getElementById('result').textContent='done'}show()";
    assert_eq!(execute(source).unwrap()[0].text, "done");
    assert_eq!(
        execute("document.getElementById('result').textContent='done'").unwrap()[0].text,
        "done"
    );
}

/** A closing brace terminates supported statement tails without becoming an empty statement. */
#[test]
fn closing_braces_complete_declarations_calls_and_updates() {
    for source in [
        "function f(){var n=2}f()",
        "function f(){let n=2}f()",
        "function f(){const n=2}f()",
        "function f(){var n=1;n=2}f()",
        "function f(){g()}function g(){}f()",
        "if(true){document.getElementById('result').textContent='done'}",
    ] {
        load(source, &[]).unwrap();
    }
    assert_eq!(result("function f(){return 3}", "f()"), "3");
}

/** Newline insertion and explicit punctuation produce identical statement ordering. */
#[test]
fn newline_statements_preserve_execution_order() {
    assert_eq!(result("let n=1\nn=n+2\nn=n*3", "n"), "9");
    assert_eq!(result("let n=1;n=n+2;n=n*3;", "n"), "9");
}

/** Same-line adjacent statements require a separator; the error names the following original token. */
#[test]
fn same_line_missing_separator_points_at_the_next_statement() {
    let source = "let n=1 let m=2";
    let error = syntax(source);
    assert!(error.message.contains("Unexpected"), "{}", error.message);
    assert_eq!(
        error.location.unwrap().byte_offset,
        source.find("let m").unwrap()
    );
}

/** Implicit punctuation cannot supply operands, delimiters or a const initializer. */
#[test]
fn incomplete_productions_are_not_repaired_at_eof_or_braces() {
    for source in [
        "const n",
        "let n=",
        "let n=1+",
        "function f(){return 1+}",
        "f(1",
        "let n=[1",
        "let n={a:1",
    ] {
        syntax(source);
    }
}

/** Explicit for separators remain mandatory despite trivia line terminators. */
#[test]
fn standard_for_headers_require_both_explicit_semicolons() {
    for source in [
        "for(var n=0\nn<2;n++){}",
        "for(var n=0;n<2\nn++){}",
        "for(var n=0/*\n*/n<2;n++){}",
        "for(var n=0;n<2/*\n*/n++){}",
    ] {
        let error = syntax(source);
        assert!(error.message.contains("Unexpected"), "{}", error.message);
    }
    assert_eq!(
        result("let total=0\nfor(var n=0;n<3;n++){total=total+n}", "total"),
        "3"
    );
}

/** Missing control bodies cannot be manufactured as empty statements by insertion. */
#[test]
fn missing_if_and_while_bodies_remain_errors() {
    for source in [
        "while(false)\n",
        "if(true)\n",
        "if(true)\nelse {}",
        "if(true){while(false)\n}",
    ] {
        syntax(source);
    }
    load("while(false);if(false);", &[]).unwrap();
}
