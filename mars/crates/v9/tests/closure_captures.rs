/** Mixed register and environment locals retain every lexical dependency across deferred compilation. */
use crate::common;

/** Escaped sibling closures share writes through one environment after their creator returns. */
#[test]
fn sibling_closures_share_captured_mutations() {
    assert_eq!(
        common::eval(
            "const held=(()=>{let value=1;const temporary=17;return {read:()=>value,write:v=>({value}=v)}})();held.write({value:42});const r=held.read();",
            "r"
        ),
        "42"
    );
}

/** A function may pass a lexical dependency through another closure that has no binding of its own. */
#[test]
fn transitive_closures_keep_outer_bindings() {
    assert_eq!(
        common::eval(
            "const held=(()=>{const value=42;return ()=>()=>({value})})();const r=held()().value;",
            "r"
        ),
        "42"
    );
}

/** Direct eval in a descendant may name any enclosing local, including one absent from its syntax. */
#[test]
fn nested_eval_keeps_dynamic_outer_bindings() {
    assert_eq!(
        common::eval(
            "const held=(()=>{let unseen=41;return ()=>()=>eval('++unseen')})();const r=held()();",
            "r"
        ),
        "42"
    );
}

/** Methods, accessors, field initializers and static initializers all capture external values. */
#[test]
fn class_members_keep_outer_bindings() {
    assert_eq!(
        common::eval(
            "const C=(()=>{const value=42;return class {field=value;get answer(){return value}static answer=value;method(){return value}}})();const c=new C();const r=[c.field,c.answer,C.answer,c.method()].join();",
            "r"
        ),
        "42,42,42,42"
    );
}

/** Parameter defaults keep their own environment while body bindings remain separate. */
#[test]
fn parameter_defaults_and_body_captures_stay_separate() {
    assert_eq!(
        common::eval(
            "function make(value,{read=()=>value}={}){const body=41;return ()=>[read(),body]}const r=make(42)().join();",
            "r"
        ),
        "42,41"
    );
}

/** A function declaration reads the enclosing mutable binding even after it is replaced. */
#[test]
fn function_declarations_observe_rebinding() {
    assert_eq!(
        common::eval(
            "const held=(()=>{function original(){return original}const saved=original;original=42;return saved})();const r=held();",
            "r"
        ),
        "42"
    );
}

/** Lazy methods still resolve captured data after a resident copy is serialized and restored. */
#[test]
fn captures_survive_transport_before_first_call() {
    let sources = vec!["const held=(()=>{let value=40;return ()=>++value})();let r=0;setTimeout(()=>{r=held()+held()},1);".to_owned()];
    let environment = common::environment();
    let host = common::host();
    let initial = common::load_with(&sources[0], &environment);
    let next = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(initial.state),
        1.0,
        1,
    )
    .unwrap();
    assert_eq!(common::failure(&next), None);
    assert_eq!(common::global(&next, "r"), "83");
}
