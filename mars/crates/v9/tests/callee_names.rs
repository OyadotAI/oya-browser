/** A call of a value that is not a function names the callee as the source wrote it, as V8 does. */
mod common;

/** A member chain is printed dot by dot. */
#[test]
fn member_chain_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval(
            "let r;const o={a:{}};try{o.a.b()}catch(e){r=e.message}",
            "r"
        ),
        "o.a.b is not a function"
    );
}

/** A plain identifier callee is named by its binding. */
#[test]
fn identifier_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval("let r;let q=1;try{q()}catch(e){r=e.message}", "r"),
        "q is not a function"
    );
}

/** A method called on `this` is named with `this`. */
#[test]
fn this_member_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval(
            "let r;function f(){return this.nope()}try{f.call({})}catch(e){r=e.message}",
            "r"
        ),
        "this.nope is not a function"
    );
}

/** Spread calls keep the name too, since they use their own call instruction. */
#[test]
fn spread_call_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval(
            "let r;const o={};try{o.nope(...[1])}catch(e){r=e.message}",
            "r"
        ),
        "o.nope is not a function"
    );
}

/** A `new` of a value that is not a constructor is named by its binding. */
#[test]
fn identifier_callee_is_named_in_not_a_constructor_error() {
    assert_eq!(
        common::eval("let r;let q=1;try{new q()}catch(e){r=e.message}", "r"),
        "q is not a constructor"
    );
}
