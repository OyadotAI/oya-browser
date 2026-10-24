/** Arrow-head and concise-body grammar cases preserve assignment precedence and early errors. */
use super::result;
use mars::v9::execute;

/** Every supported head is assignment grammar, and concise bodies have implicit return completion. */
#[test]
fn simple_heads_expression_and_block_bodies_return_values() {
    let source = "const single=x=>x+1;const pair=(a,b)=>a*b;const empty=()=>7;const trailing=(a,)=>a;const block=x=>{let n=x+2;return n;};const noReturn=()=>{};";
    assert_eq!(
        result(
            source,
            "single(2)+' '+pair(3,4)+' '+empty()+' '+trailing(8)+' '+block(1)+' '+(noReturn()===undefined)"
        ),
        "3 12 7 8 3 true"
    );
    assert_eq!(result("const f=()=>({value:4});", "f().value"), "4");
    assert_eq!(result("const f=async=>async+1;", "f(2)"), "3");
}
/** Nested concise arrows associate to the right and capture the defining parameters. */
#[test]
fn arrows_pass_return_and_store_through_supported_callee_forms() {
    let source = "function apply(f,x){return f(x);}const make=x=>y=>x+y;const add=make(4);const box={f:add};const list=[x=>x*2];";
    assert_eq!(
        result(
            source,
            "apply(add,3)+' '+box.f(2)+' '+list[0](5)+' '+((x)=>x+1)(8)"
        ),
        "7 6 10 9"
    );
    assert_eq!(
        result("const factory=()=>function(){return 3;};", "factory()()"),
        "3"
    );
}
/** Parentheses disambiguate grouping, calls and parameters without admitting arrows as binary operands. */
#[test]
fn grouping_and_assignment_precedence_are_distinct() {
    assert_eq!(result("const f=(x=>x+1);", "f((2+3))"), "6");
    assert_eq!(result("const f=false||(x=>x+2);", "f(2)"), "4");
    assert_eq!(result("let f;f=x=>x+3;", "f(2)"), "5");
    execute("x=>x;(x,y)=>x+y;").unwrap();
    for source in [
        "let f=1+x=>x;",
        "let f=false||x=>x;",
        "let f=(x+1)=>x;",
        "let f=((x))=>x;",
        "let f=(f())=>x;",
        "let f=()=>{}.x;",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}
/** Arrow parameter duplicates and malformed heads fail even if their source branch is never entered. */
#[test]
fn invalid_and_duplicate_heads_are_early_errors() {
    for source in [
        "if(false){const f=(x,x)=>x;}",
        "const f=(,)=>1;",
        "const f=(x,,y)=>x;",
        "const f=(1)=>1;",
        "const f=(true)=>1;",
    ] {
        assert_eq!(
            execute(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
}

/** Defaults, rest, destructuring and async heads are ordinary arrow parameters. */
#[test]
fn default_rest_destructuring_and_async_heads_run() {
    assert_eq!(
        result(
            "const f=(x=1)=>x,g=(...x)=>x.length,h=([x])=>x,i=({x})=>x;",
            "[f(),g(1,2),h([3]),i({x:4})].join()"
        ),
        "1,2,3,4"
    );
    assert_eq!(
        result(
            "const f=async x=>x,g=async(x)=>x;",
            "typeof f(1).then+typeof g(1).then"
        ),
        "functionfunction"
    );
}

/** All JavaScript line terminators and multiline comments participate in the no-line-break arrow rule. */
#[test]
fn line_breaks_before_arrow_are_forbidden_and_after_arrow_are_allowed() {
    for separator in [
        "\n", "\r", "\r\n", "\u{2028}", "\u{2029}", "/*\n*/", "//note\n",
    ] {
        for head in ["x", "(x)", "()"] {
            let source = format!("const f={head}{separator}=>1;");
            let error = execute(&source).unwrap_err();
            assert_eq!(error.code, "script_syntax_error", "{source:?}");
            assert_eq!(error.phase.as_deref(), Some("parse"), "{source:?}");
        }
    }
    assert_eq!(result("const f=x/*same line*/=>\nx+1;", "f(3)"), "4");
    assert_eq!(result("const f=(\nx\n)=>x+1;", "f(3)"), "4");
}
