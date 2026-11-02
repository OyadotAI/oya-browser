/** Async iteration requires an of clause and never falls through to classic for or property enumeration grammar. */
use crate::common;

/** Empty, expression and declaration heads cannot turn a for-await statement into a classic semicolon loop. */
#[test]
fn for_await_rejects_classic_for_headers() {
    for head in [
        "",
        "value",
        "value=0",
        "var value",
        "var value=0",
        "let value=0",
        "const value=0",
        "var a,b",
    ] {
        let source = format!("async function f(){{for await({head};;){{}}}}");
        let error = mars_v9::check(&source).unwrap_err();
        assert_eq!(error.code, "script_syntax_error", "{head}: {error:?}");
    }
}

/** Async iteration cannot silently discard await and enumerate properties through for-in. */
#[test]
fn for_await_rejects_for_in_headers() {
    for head in [
        "value",
        "var value",
        "let value",
        "const value",
        "var value=0",
        "[value]",
        "{value}",
    ] {
        let source = format!("async function f(){{for await({head} in {{}}){{}}}}");
        let error = mars_v9::check(&source).unwrap_err();
        assert_eq!(error.code, "script_syntax_error", "{head}: {error:?}");
    }
}

/** Supported declaration, assignment, destructuring and resource heads retain their actual async-of grammar. */
#[test]
fn for_await_of_keeps_its_valid_binding_forms() {
    for head in [
        "value",
        "var value",
        "let value",
        "const value",
        "const [value]",
        "const {value}",
        "using value",
        "await using value",
    ] {
        let source = format!("async function f(){{for await({head} of []){{}}}}");
        assert!(mars_v9::check(&source).is_ok(), "{head}");
    }
}

/** Ordinary classic, property and sync-iterator loops remain valid inside async functions. */
#[test]
fn ordinary_for_families_stay_valid_in_async_functions() {
    let source = "async function f(){let total=0;for(let i=0;i<2;i++)total++;for(const key in {a:1,b:2})total++;for(const value of [1,2])total+=value;return total}f().then(value=>console.log(value));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["7"]);
}

/** The admitted async-of form still awaits sync-iterator values and completes through native promise jobs. */
#[test]
fn valid_for_await_of_executes_actual_async_iteration() {
    let source = "async function f(){const values=[];for await(const value of [Promise.resolve(1),2])values.push(value);return values.join(':')}f().then(value=>console.log(value));";
    let execution = common::load(source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["1:2"]);
}

/** Dynamic async and async-generator constructors apply the same early grammar rules before creating a function. */
#[test]
fn dynamic_async_constructors_reject_non_of_headers() {
    let source = "const A=(async()=>{}).constructor;const G=(async function*(){}).constructor;const out=[];for(const C of [A,G])for(const body of ['for await(;;){}','for await(var value;;){}','for await(var value in {}){}']){try{C(body);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,true,true,true");
}

/** Parsing rejects an invalid nested async function before any earlier expression in its eval source can run. */
#[test]
fn invalid_for_await_is_an_early_error_without_side_effects() {
    let source = "let side=0;let syntax=false;try{eval('side++;async()=>{for await(var value;;){}}')}catch(error){syntax=error instanceof SyntaxError}const r=[syntax,side].join();";
    assert_eq!(common::eval(source, "r"), "true,0");
}

/** Awaited iteration does not become legal in classic scripts or nonasync function bodies. */
#[test]
fn for_await_still_requires_an_async_context() {
    for source in [
        "for await(const value of []){}",
        "function f(){for await(var value of []){}}",
        "function* f(){for await(var value of []){}}",
    ] {
        assert_eq!(
            mars_v9::check(source).unwrap_err().code,
            "script_syntax_error"
        );
    }
}
