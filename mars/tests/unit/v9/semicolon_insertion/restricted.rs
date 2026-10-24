/** Restricted productions use trivia facts while preserving source-owned control-flow legality. */
use super::*;

/** Return with intervening trivia newline produces undefined rather than reading a later operand. */
#[test]
fn return_line_terminators_and_newline_comments_produce_bare_returns() {
    for trivia in [
        "\n",
        "\r",
        "\r\n",
        "\u{2028}",
        "\u{2029}",
        "/*\n*/",
        "// note\n",
    ] {
        assert_eq!(
            result(
                &format!("function f(){{return{trivia}7}}"),
                "f()===undefined"
            ),
            "true"
        );
    }
    assert_eq!(result("function f(){return}", "f()===undefined"), "true");
}

/** Return expressions beginning on the same line continue through their valid expression grammar. */
#[test]
fn same_line_return_operands_continue_across_newlines() {
    assert_eq!(result("function f(){return /* same */ 1\n+2}", "f()"), "3");
    assert_eq!(result("function f(){return (\n4\n)}", "f()"), "4");
}

/** A bare return never hides malformed source or illegal jumps in unreachable statements. */
#[test]
fn return_does_not_bypass_complete_source_validation() {
    for source in [
        "function f(){return\nlet n=}",
        "function f(){return\nbreak}",
        "return\n1",
    ] {
        syntax(source);
    }
}

/** Legal newline-separated jumps do not consume a following identifier as a label. */
#[test]
fn loop_jumps_terminate_before_newline_separated_identifiers() {
    assert_eq!(result("let n=0\nwhile(true){break\nn=9}", "n"), "0");
    assert_eq!(
        result(
            "let n=0\nlet skipped=0\nwhile(n<3){n++\ncontinue\nskipped++}",
            "n+' '+skipped"
        ),
        "3 0"
    );
    assert_eq!(result("let n=0\nwhile(true){n++\nbreak}", "n"), "1");
}

/** A jump to an undeclared label is a syntax error, as in browsers; a declared one jumps. */
#[test]
fn same_line_jump_labels_follow_javascript() {
    for jump in ["break", "continue"] {
        let error = load(&format!("while(false){{{jump} label}}"), &[]).unwrap_err();
        assert_eq!(error.code, "script_syntax_error");
    }
    load("label:while(true){break label}", &[]).unwrap();
    load("let n=0;label:while(n<3){n++;continue label}", &[]).unwrap();
}

/** Function boundaries and unreachable paths still enforce break and continue ownership. */
#[test]
fn insertion_cannot_make_illegal_or_cross_function_jumps_legal() {
    for source in [
        "break\n",
        "continue\n",
        "if(false){break\n}",
        "while(false){const f=function(){break\n}}",
        "while(false){const f=()=>{continue\n}}",
    ] {
        syntax(source);
    }
}

/** Newline-separated updates become prefix statements rather than modifying the preceding target. */
#[test]
fn postfix_line_breaks_preserve_following_prefix_updates() {
    assert_eq!(result("var a=0,b=0\na\n++b", "a+' '+b"), "0 1");
    assert_eq!(result("var a=2,b=2\na/*\n*/--b", "a+' '+b"), "2 1");
    assert_eq!(result("var n=0\nn/*same*/++", "n"), "1");
    assert_eq!(result("var n=0\n++\nn", "n"), "1");
}

/** Multiline operands do not prohibit a postfix operator adjacent to the completed closing bracket. */
#[test]
fn completed_multiline_member_operands_keep_adjacent_postfix_updates() {
    for key in ["'n'", "'\\\nn'", "'\u{2028}n'"] {
        let source = format!("const box={{}}\nbox[{key}]=1\nbox[\n{key}\n]++");
        assert_eq!(result(&source, &format!("box[{key}]")), "2");
    }
}

/** Arrow heads retain their no-line-terminator restriction: a parse error located no later than the arrow. */
#[test]
fn newline_before_arrow_is_a_located_error() {
    for source in [
        "const f=x\n=>x",
        "const f=(x)/*\n*/=>x",
        "const f=()\u{2029}=>1",
    ] {
        let error = syntax(source);
        assert!(error.location.unwrap().byte_offset <= source.find("=>").unwrap());
    }
}

/** Line breaks inside parameters or after the arrow retain normal supported arrow semantics. */
#[test]
fn arrow_parameter_and_body_newlines_remain_legal() {
    assert_eq!(result("const f=(\nx\n)=>\nx+1", "f(2)"), "3");
    assert_eq!(result("const f=()=>{return\n3}", "f()===undefined"), "true");
}
