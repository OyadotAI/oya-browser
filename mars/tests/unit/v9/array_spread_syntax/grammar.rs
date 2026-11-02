/** Grammar cases keep ellipsis local to array elements and preserve ordinary dot scanning. */
use super::*;

/** Empty, mixed, repeated and nested spreads retain source order and legal trailing commas. */
#[test]
fn array_spread_composes_supported_elements_and_trailing_commas() {
    assert_eq!(
        result("const a=[1,2];", "[...[],0,...a,...[...[3]],4,].length"),
        "5"
    );
    assert_eq!(result("const a=[1,2];", "[...a,...a,][3]"), "2");
    assert_eq!(result("", "[...[],].length"), "0");
}

/** Assignment, conditional and arrow invocation operands reuse assignment-expression grammar. */
#[test]
fn spread_operands_use_assignment_conditional_and_arrow_expressions() {
    assert_eq!(result("let a=[];const b=[...a=[3,4]];", "a[0]+b[1]"), "7");
    assert_eq!(result("", "[...true?[2]:[8]][0]"), "2");
    assert_eq!(result("", "[...(()=>[5])()][0]"), "5");
}

/** Ellipsis recognition does not consume numeric-dot or ordinary member tokens. */
#[test]
fn numeric_and_member_dots_keep_existing_semantics() {
    assert_eq!(
        result(
            "const a={value:[1]};",
            "[.5,...a.value,1.5][0]+a.value.length"
        ),
        "1.5"
    );
    assert_eq!(result("", "[...'.5'][1]"), "5");
}

/** Missing operands are parse errors located at the token right after the ellipsis. */
#[test]
fn missing_spread_operands_are_located_after_the_ellipsis() {
    for source in [
        "const a=[...];",
        "const a=[...,1];",
        "const a=[...",
        "const a=[......[]];",
    ] {
        let error = execute(source).unwrap_err();
        assert_eq!(error.code, "script_syntax_error", "{source}");
        assert_eq!(error.phase.as_deref(), Some("parse"));
        assert_eq!(
            error.location.unwrap().byte_offset,
            source.find("...").unwrap() + 3
        );
    }
}

/** Malformed dot sequences and literal holes cannot acquire accidental array-spread behavior. */
#[test]
fn malformed_ellipsis_separators_and_elisions_remain_syntax_errors() {
    for source in ["const a=[..[]];", "const a=[....[]];", "const a=[...[] 1];"] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}

/** Array holes build sparse arrays: the hole counts toward length but is not an own index. */
#[test]
fn array_holes_build_sparse_arrays() {
    assert_eq!(result("", "[,].length"), "1");
    assert_eq!(
        result("const a=[1,,2];", "a.length+' '+(1 in a)+' '+a[2]"),
        "3 false 2"
    );
    assert_eq!(result("", "[...[], ,1].length"), "2");
}

/** The shared ellipsis also drives call spread, object spread and rest parameters. */
#[test]
fn call_object_and_parameter_spread_follow_javascript() {
    for (source, expression, expected) in [
        ("function f(a,b){return a+b;}", "f(...[1,2])", "3"),
        ("const o={...{x:1},y:2};", "o.x+o.y", "3"),
        ("function f(...a){return a.length;}", "f(1,2,3)", "3"),
        ("const f=(...a)=>a.join('');", "f('a','b')", "ab"),
        ("const f=(a,...b)=>b.length;", "f(1,2,3)", "2"),
    ] {
        assert_eq!(result(source, expression), expected, "{source}");
    }
}
