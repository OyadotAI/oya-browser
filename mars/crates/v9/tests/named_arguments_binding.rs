/** A named function expression's self binding never overwrites the activation's own arguments binding. */
mod common;

/** Direct eval reads the local arguments object even when the function expression itself is named arguments. */
#[test]
fn named_arguments_functions_keep_their_local_arguments_object() {
    let source = "const f=function arguments(){return eval('arguments')};const args=f(1,2);const r=[args[0],args[1],args.length,args.callee===f,f.name].join();";
    assert_eq!(common::eval(source, "r"), "1,2,2,true,arguments");
}

/** A sloppy simple parameter remains aliased through the local arguments object and direct eval. */
#[test]
fn named_arguments_functions_preserve_mapped_parameters() {
    let source = "const f=function arguments(value){eval('arguments')[0]=7;const same=arguments===eval('arguments');return [value,arguments[0],same]};const r=f(1).join();";
    assert_eq!(common::eval(source, "r"), "7,7,true");
}

/** An explicit parameter named arguments still shadows both the implicit object and the function's self name. */
#[test]
fn explicit_arguments_parameters_keep_precedence() {
    let source = "const f=function arguments(arguments){return [arguments,eval('arguments')]};const r=f(9).join();";
    assert_eq!(common::eval(source, "r"), "9,9");
}

/** Non-simple parameter expressions see the arguments object before a separate body variable replaces its own binding. */
#[test]
fn default_parameters_keep_arguments_separate_from_body_variables() {
    let source = "const f=function arguments(value=arguments[1]){var arguments='body';return [value,arguments]};const g=function arguments(){var arguments;return arguments[0]};const r=[...f(undefined,7),g(3)].join();";
    assert_eq!(common::eval(source, "r"), "7,body,3");
}

/** Nested arrows inherit the actual invocation's arguments object through direct reads and direct eval. */
#[test]
fn arrows_inherit_named_arguments_functions_actual_arguments() {
    let source = "const f=function arguments(value){return [(()=>arguments[0])(),(()=>eval('arguments[0]'))()]};const r=f(5).join();";
    assert_eq!(common::eval(source, "r"), "5,5");
}

/** A generator keeps its local argument mapping when the activation suspends and resumes. */
#[test]
fn named_arguments_generators_preserve_their_activation_binding() {
    let source = "const f=function* arguments(value){yield eval('arguments[0]');arguments[0]++;yield value};const iterator=f(4);const r=[iterator.next().value,iterator.next().value,iterator.next().done].join();";
    assert_eq!(common::eval(source, "r"), "4,5,true");
}

/** Other expression names remain immutable recursive self bindings, with no effect on the outer variable. */
#[test]
fn ordinary_named_expression_self_bindings_are_preserved() {
    let source = "var self='outer';const f=function self(n){if(n)return n*self(n-1);self=7;return self===f?1:0};const r=[f(4),self].join();";
    assert_eq!(common::eval(source, "r"), "24,outer");
}

/** Strict grammar still rejects arguments as a function name while strict anonymous functions retain unmapped arguments. */
#[test]
fn strict_named_arguments_grammar_is_unchanged() {
    let source = "const out=[];for(const code of ['(function arguments(){\\\"use strict\\\";})','\\\"use strict\\\";(function arguments(){})']){try{eval(code);out.push(false)}catch(error){out.push(error instanceof SyntaxError)}}const f=function(value){'use strict';arguments[0]=9;return value};out.push(f(2));const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,2");
}

/** Captured mapped arguments, parameter setters and lexical arrows retain the same aliases after the public JSON boundary. */
#[test]
fn named_arguments_aliases_survive_state_transport() {
    let sources=vec!["const saved=(function arguments(value){const captured=eval('arguments');return {captured,set(next){value=next},read:()=>arguments[0]}})(2);document.getElementById('go').addEventListener('click',()=>{saved.set(9);console.log(saved.captured[0],saved.read())});".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["9 9"]);
}
