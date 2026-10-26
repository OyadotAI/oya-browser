/** Legacy call assignment targets parse, evaluate the call once, then throw a runtime ReferenceError. */
mod common;

/** The left call runs before the invalid reference, while the right side stays unevaluated. */
#[test]
fn call_assignment_throws_after_left_call_without_evaluating_right() {
    assert_eq!(
        common::eval(
            "const log=[];function lhs(){log.push('left')}function rhs(){log.push('right')}let reference=false;try{lhs()=rhs()}catch(e){reference=e instanceof ReferenceError}const r=[log.join(),reference].join('|');",
            "r"
        ),
        "left|true"
    );
}

/** Call targets in unreachable statements or empty for-in loops are never evaluated. */
#[test]
fn unreachable_call_targets_do_not_throw() {
    assert_eq!(
        common::eval(
            "let calls=0;function f(){calls++}if(false){f()=1;f()**=2;f()++}for(f() in []){}const r=calls;",
            "r"
        ),
        "0"
    );
}

/** Logical assignments retain their early error and cannot use the legacy runtime reference. */
#[test]
fn logical_assignment_call_targets_are_syntax_errors() {
    assert_eq!(
        common::eval(
            "let syntax=false;try{eval('function f(){};f() ||= 1')}catch(e){syntax=e instanceof SyntaxError}const r=syntax;",
            "r"
        ),
        "true"
    );
}
