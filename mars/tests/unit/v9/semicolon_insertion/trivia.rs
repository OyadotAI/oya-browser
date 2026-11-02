/** Only inter-token trivia contributes semicolon and restricted-production line boundaries. */
use super::*;

/** All JavaScript line terminators split completed statements, with CRLF treated as one line. */
#[test]
fn all_five_line_terminator_forms_allow_insertion() {
    for newline in ["\n", "\r", "\r\n", "\u{2028}", "\u{2029}"] {
        assert_eq!(
            result(&format!("let n=1{newline}n=2"), "n"),
            "2",
            "{newline:?}"
        );
    }
}

/** Line terminators in both comment forms preserve statement boundaries. */
#[test]
fn comment_contained_line_terminators_allow_insertion() {
    for trivia in [
        "// note\n",
        "/* note\n */",
        "/*\r*/",
        "/*\r\n*/",
        "/*\u{2028}*/",
        "/*\u{2029}*/",
    ] {
        assert_eq!(result(&format!("let n=1{trivia}n=2"), "n"), "2");
    }
}

/** Comments and whitespace on one line cannot authorize a missing separator. */
#[test]
fn same_line_trivia_does_not_allow_insertion() {
    for trivia in [" ", "\t", "/* note */", "/* */\t"] {
        syntax(&format!("let n=1{trivia}let m=2"));
    }
}

/** A string's own physical line changes cannot masquerade as following trivia. */
#[test]
fn string_internal_lines_do_not_separate_following_same_line_statements() {
    for literal in [
        "'a\\\nb'",
        "'a\\\r\nb'",
        "'a\u{2028}b'",
        "'a\u{2029}b'",
        "'a\\nb'",
    ] {
        let source = format!("let n={literal} let m=2");
        let error = syntax(&source);
        assert_eq!(
            error.location.unwrap().byte_offset,
            source.find("let m").unwrap()
        );
    }
}

/** Genuine trivia after a multiline literal still authorizes insertion. */
#[test]
fn trivia_after_multiline_string_is_distinct_from_its_content() {
    for literal in ["'a\\\nb'", "'a\u{2028}b'", "'a\u{2029}b'"] {
        assert_eq!(result(&format!("let text={literal}\nlet n=2"), "n"), "2");
    }
}

/** EOF termination accepts trailing complete comments but never repairs an unterminated one. */
#[test]
fn trailing_comments_do_not_swallow_source_eof_and_unclosed_comments_still_fail() {
    for suffix in ["// note", "/* note */", "// note\r\n", "/*\n*/"] {
        load(&format!("let n=1{suffix}"), &[]).unwrap();
    }
    let error = load("let n=1 /* never closed", &[]).unwrap_err();
    assert_eq!(error.code, "script_syntax_error");
    assert!(error.message.contains("unterminated block comment"));
    assert_eq!(error.location.unwrap().byte_offset, "let n=1 ".len());
}
