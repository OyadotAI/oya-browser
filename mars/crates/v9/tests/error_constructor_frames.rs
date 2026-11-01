/** Error subclasses built through `super()` hide their own constructor frames, while outer callers stay visible. */
use crate::common;

/** Public `stack` text is the observable boundary; the tests never inspect private trace records. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** A derived error's default constructor is not a stack frame, as in V8's ErrorUtils::Construct. */
#[test]
fn default_derived_error_constructor_frame_is_hidden() {
    assert_eq!(
        eval(
            "class MyError extends Error {};let r=String(!new MyError().stack.includes('at MyError'));"
        ),
        "true"
    );
}

/** Frames below the subclass constructor, such as the function that built it, remain in the trace. */
#[test]
fn caller_of_derived_error_constructor_stays_in_trace() {
    assert_eq!(
        eval(
            "class MyError extends Error {};function outerCaller(){return new MyError()};let r=String(outerCaller().stack.includes('at outerCaller'));"
        ),
        "true"
    );
}

/** A direct `new Error()` has no subclass target to skip, so its own caller remains visible. */
#[test]
fn direct_error_construction_keeps_its_caller() {
    assert_eq!(
        eval(
            "function outerCaller(){return new Error()};let r=String(outerCaller().stack.includes('at outerCaller'));"
        ),
        "true"
    );
}
