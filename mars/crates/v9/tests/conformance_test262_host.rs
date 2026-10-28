/** The page host's `$262.evalScript` hook runs source as a script, not as an eval. */
#[cfg(feature = "conformance")]
mod common;

/** Script evaluation instantiates declarations like a new script: non-configurable vars, script-scoped lets, collisions as SyntaxError. */
#[cfg(feature = "conformance")]
#[test]
fn eval_script_hook_uses_script_declaration_instantiation() {
    let environment = common::environment();
    let source = "__v9EvalScript('var x = 1; let y = 2; function f() {}');\
        let collided = false; try { __v9EvalScript('let x;'); } catch (e) { collided = e instanceof SyntaxError; }\
        const r = [x, typeof y, Object.getOwnPropertyDescriptor(globalThis, 'x').configurable, typeof f, collided].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "1,number,false,function,true"
    );
}
