/** Legacy function.caller follows active invocations without revealing strict or native callers. */
mod common;
use common::eval;

/** A function's immediate caller stays visible through deeper helper calls inspecting it. */
#[test]
fn caller_uses_the_requested_active_function() {
    let source = "function target(){return inspect()}function inspect(){return target.caller}\
        function outer(){return target()}const r=[outer()===outer,target()===null,target.caller===null].join();";
    assert_eq!(eval(source, "r"), "true,true,true");
}

/** Recursive lookup selects the innermost activation, then its actual immediate caller. */
#[test]
fn recursion_selects_the_latest_activation() {
    let source = "function target(n){if(n)return target(n-1);return target.caller}\
        function outer(){return target(2)}const r=[outer()===target,target.caller===null].join();";
    assert_eq!(eval(source, "r"), "true,true");
}

/** Strict callers are censored even while a nested helper inspects an earlier active function. */
#[test]
fn strict_callers_stay_private() {
    let source = "function target(){return inspect()}function inspect(){return target.caller}\
        function strict(){'use strict';return target()}const r=strict()===null;";
    assert_eq!(eval(source, "r"), "true");
}

/** Native builtin callbacks conceal the outer script function rather than skipping the native. */
#[test]
fn native_callbacks_hide_the_outer_caller() {
    let source = "function target(){return target.caller}function outer(){return [0].map(target)[0]}\
        const r=outer()===null;";
    assert_eq!(eval(source, "r"), "true");
}

/** call, apply and Reflect.apply are transparent forwarding operations, not callback hosts. */
#[test]
fn forwarding_keeps_the_actual_script_caller() {
    let source = "function target(){return target.caller}\
        function a(){return target.call(null)}function b(){return target.apply(null,[])}\
        function c(){return Reflect.apply(target,null,[])}const r=[a()===a,b()===b,c()===c].join();";
    assert_eq!(eval(source, "r"), "true,true,true");
}

/** Eval frames are not function invocations, including a strict eval inside a sloppy caller. */
#[test]
fn eval_frames_do_not_replace_the_actual_caller() {
    let source = "function target(){return target.caller}\
        function a(){return eval('target()')}function b(){return eval('\"use strict\";target()')}\
        function c(){return (0,eval)('target()')}const r=[a()===a,b()===b,c()===c].join();";
    assert_eq!(eval(source, "r"), "true,true,true");
}

/** Resuming a saved generator frame exposes its real code identity while censoring strict code. */
#[test]
fn resumed_generators_keep_caller_privacy() {
    let source = "function target(){return target.caller}function* g(){yield target()}\
        function* strict(){'use strict';yield target()}function outer(){return g().next().value}\
        const r=[outer()===g,strict().next().value===null].join();";
    assert_eq!(eval(source, "r"), "true,true");
}
