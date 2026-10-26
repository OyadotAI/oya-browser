/** Annex B copies ordinary block functions into eligible var bindings without leaking generators or async functions. */
mod common;

/** Generator and async declarations remain lexical in scripts and nested function bodies. */
#[test]
fn generator_and_async_block_functions_do_not_leak() {
    let source = "const out=[];{function* gen(){yield 2}async function task(){return 2}async function* both(){yield 2}out.push(typeof gen,typeof task,typeof both)}out.push(typeof gen,typeof task,typeof both);(function(){function* gen(){yield 1}let value=0;async function task(){value=1}{function* gen(){yield 2}async function task(){value=2}}out.push(gen().next().value);task();out.push(value)})();const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "function,function,function,undefined,undefined,undefined,1,1"
    );
}

/** An implicit arguments object is an eligible var binding, unlike an actual formal parameter. */
#[test]
fn implicit_arguments_binding_accepts_the_block_function_copy() {
    let source = "const out=[];(function(x){out.push(arguments[0]);{function arguments(){return 2}}out.push(typeof arguments,arguments())})(1);(function(arguments){{function arguments(){return 3}}out.push(arguments)})(4);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "1,function,2,4");
}

/** Parameters and intervening lexical bindings still suppress the ordinary Annex B copy. */
#[test]
fn block_functions_preserve_parameter_and_lexical_barriers() {
    let source = "const out=[];(function(x){ {function x(){}}out.push(x)})(1);(function(x=2){{function x(){}}out.push(x)})();(function(){var value=3;{let value=4;{function value(){}}out.push(value)}out.push(value)})();const r=out.join();";
    assert_eq!(common::eval(source, "r"), "1,2,4,3");
}

/** A nested block function can update an existing outer var through another block function binding. */
#[test]
fn nested_block_functions_copy_to_the_existing_var() {
    let source = "const out=[];(function(){out.push(typeof f);{function f(){return 4}{function f(){return 5}out.push(f())}out.push(f())}out.push(f())})();const r=out.join();";
    assert_eq!(common::eval(source, "r"), "undefined,5,4,5");
}

/** Eval suppresses optional Annex B hoists that collide with a previous script's lexical global. */
#[test]
fn eval_block_functions_do_not_redeclare_global_lexicals() {
    let source = "let lexical;let threw=false;try{eval('{function lexical(){}}')}catch(e){threw=true}const r=[threw,typeof lexical,Object.hasOwn(globalThis,'lexical')].join();";
    assert_eq!(common::eval(source, "r"), "false,undefined,false");
}

/** A failed optional global creation still reports a real eval declaration error without a property. */
#[test]
fn frozen_global_rejects_annex_eval_creation() {
    let source = "let threw=false;Object.preventExtensions(globalThis);try{eval('{function missing(){}}')}catch(e){threw=e instanceof TypeError}console.log(threw,Object.hasOwn(globalThis,'missing'));";
    let loaded = common::load(source);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::console(&loaded), ["true false"]);
}
