/** Parameter expression closures retain their own cells while body declarations get distinct bindings. */
use crate::common;

/** A body var copies the initial argument without overwriting the parameter closure's cell. */
#[test]
fn body_vars_do_not_mutate_parameter_expression_closures() {
    assert_eq!(
        common::eval(
            "function f(a=1,g=()=>a){var a=2;return [g(),a]}function h(a=3){var a;return a}const r=[...f(),h()].join();",
            "r"
        ),
        "1,2,3"
    );
}

/** Named expressions expose their self binding to defaults before body vars shadow it. */
#[test]
fn named_function_self_binding_is_visible_to_parameter_defaults() {
    assert_eq!(
        common::eval(
            "const f=function self(a=self){var self;return a};const r=f()===f;",
            "r"
        ),
        "true"
    );
}

/** Function declarations in the body cannot replace the parameter captured by a default closure. */
#[test]
fn body_function_declarations_have_separate_parameter_cells() {
    assert_eq!(
        common::eval(
            "function f(a=1,g=()=>a){function a(){return 2}return [g(),a()]}const r=f().join();",
            "r"
        ),
        "1,2"
    );
}

/** Parameter eval vars are outside the distinct var environment created for the function body. */
#[test]
fn direct_eval_declarations_preserve_parameter_and_body_environments() {
    assert_eq!(
        common::eval(
            "function f(a=eval('var x=1;()=>x')){eval('var x=2');return [a(),x]}const r=f().join();",
            "r"
        ),
        "1,2"
    );
}

/** Generator suspension preserves the original parameter bindings captured before body var initialization. */
#[test]
fn generator_defaults_keep_their_parameter_environment() {
    assert_eq!(
        common::eval(
            "function* f({a=3}={},g=()=>a){var a=4;yield g();yield a}const r=[...f()].join();",
            "r"
        ),
        "3,4"
    );
}

/** Deleting a dynamic eval var reveals the outer name; fixed function bindings remain undeletable. */
#[test]
fn sloppy_eval_vars_are_deletable_and_reveal_outer_bindings() {
    assert_eq!(
        common::eval(
            "let x=9;function f(){eval('var x=2');const before=x;const removed=delete x;var fixed=1;return [before,removed,x,delete fixed]}const r=f().join();",
            "r"
        ),
        "2,true,9,false"
    );
}

/** Separate parameter and body environments remain distinct after public state transport. */
#[test]
fn captured_parameter_cells_survive_state_transport() {
    let sources=vec!["function f(a=1,g=()=>a){var a=2;return()=>[g(),a].join()}const saved=f();document.getElementById('go').addEventListener('click',()=>console.log(saved()));".to_string()];
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
    assert_eq!(common::console(&clicked), ["1,2"]);
}

/** A named expression's immutable self binding is outside sloppy eval's deletable var environment. */
#[test]
fn eval_vars_can_shadow_and_then_reveal_function_self_names() {
    assert_eq!(
        common::eval(
            "const f=function self(){eval('var self=100');self=200;const value=self;delete self;return [value,self===f]};const r=f().join();",
            "r"
        ),
        "200,true"
    );
}

/** Catch defaults capture the initialized catch record, while the catch body has its own lexical record. */
#[test]
fn catch_defaults_capture_catch_names_and_keep_body_lexicals_separate() {
    assert_eq!(
        common::eval(
            "let y=200;function f(){try{throw {}}catch({x=()=>y,y=300}){return x()}}function g(){try{throw {}}catch({x=()=>y}){let y=300;return x()}}const r=[f(),g()].join();",
            "r"
        ),
        "300,200"
    );
}

/** All catch binding names enter their dead zone before any binding initializer executes. */
#[test]
fn catch_binding_initializers_enforce_the_dead_zone() {
    assert_eq!(
        common::eval(
            "let y=200;let r;try{try{throw {}}catch({x=y,y=300}){}}catch(e){r=e.name}",
            "r"
        ),
        "ReferenceError"
    );
}

/** Body var declarations copy the implicit arguments binding along with formal parameter bindings. */
#[test]
fn body_vars_preserve_the_implicit_arguments_object() {
    assert_eq!(
        common::eval(
            "function f(a=1,g=()=>arguments){var arguments;return [arguments[0],g()===arguments]}const r=f(3).join();",
            "r"
        ),
        "3,true"
    );
}

/** Global var hoisting creates an own binding instead of reading an inherited ordinary property. */
#[test]
fn eval_var_hoisting_shadows_inherited_global_properties() {
    assert_eq!(
        common::eval(
            "Object.prototype.inheritedVar=42;const r=eval('const initial=inheritedVar;var inheritedVar=87;[initial,inheritedVar,Object.hasOwn(globalThis,\"inheritedVar\")].join()');delete Object.prototype.inheritedVar;",
            "r"
        ),
        ",87,true"
    );
}

/** Global declaration instantiation does not call an inherited proxy's HasProperty trap. */
#[test]
fn global_var_creation_does_not_probe_inherited_proxy_bindings() {
    let loaded = common::load(
        "const O=Object,P=Proxy,root=globalThis,run=eval;let calls=0;O.setPrototypeOf(root,new P({inheritedVar:42},{has(){calls++;return true}}));run('var inheritedVar');const r=[root.inheritedVar,calls,O.hasOwn(root,'inheritedVar')].join();",
    );
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), ",0,true");
}

/** Lexical for-in/of names are already in their dead zone while the RHS expression evaluates. */
#[test]
fn iteration_rhs_evaluates_inside_the_head_dead_zone() {
    assert_eq!(
        common::eval(
            "let x={};const r=['for(let [x,y] of [x]);','for(let [y,x] of [x]);','for(let [x,y] in {x});','for(let [y,x] in {x});'].map(source=>{try{eval(source);return 'ok'}catch(e){return e.name}}).join();",
            "r"
        ),
        "ReferenceError,ReferenceError,ReferenceError,ReferenceError"
    );
}

/** Fresh destructuring bindings retain their dead zone on every iteration, including register-only scopes. */
#[test]
fn repeated_iteration_bindings_begin_uninitialized() {
    assert_eq!(
        common::eval(
            "let r;try{for(let [x=y,y] of [[1,2],[undefined,3]]){}}catch(e){r=e.name}",
            "r"
        ),
        "ReferenceError"
    );
}

/** Closures created by the RHS retain the temporary head dead zone rather than later iteration values. */
#[test]
fn rhs_closures_retain_the_head_dead_zone() {
    assert_eq!(
        common::eval(
            "let x=7,saved;let r;for(let x of (saved=()=>x,[1])){}try{saved()}catch(e){r=e.name}",
            "r"
        ),
        "ReferenceError"
    );
}

/** Const assignment resolves its initialized state before reporting an immutable binding. */
#[test]
fn immutable_binding_assignment_preserves_dead_zone_error_order() {
    assert_eq!(
        common::eval(
            "const r=['c=1;const c=2','const c=2;c=1','(()=>{c=1;const c=2})()','(()=>{const c=2;return()=>c=1})()()'].map(source=>{try{eval(source);return 'ok'}catch(e){return e.name}}).join();",
            "r"
        ),
        "ReferenceError,TypeError,ReferenceError,TypeError"
    );
}

/** The private class name binding is immutable even in otherwise sloppy source. */
#[test]
fn named_class_expression_self_bindings_are_immutable() {
    assert_eq!(
        common::eval(
            "const C=class Self{method(){Self=1}};let r;try{new C().method()}catch(e){r=e.name}",
            "r"
        ),
        "TypeError"
    );
}

/** Parentheses cannot turn an object or array expression into a destructuring assignment target. */
#[test]
fn parenthesized_destructuring_targets_are_early_errors() {
    assert_eq!(
        common::eval(
            "const r=['for (({x}) of [{x:1}]){}','for (([x]) in {}){}','(({x})={x:1})','for ([({x})] of []){}'].map(source=>{try{eval(source);return 'ok'}catch(e){return e.name}}).join();",
            "r"
        ),
        "SyntaxError,SyntaxError,SyntaxError,SyntaxError"
    );
}

/** Catch bodies cannot redeclare catch names lexically, and only simple catch names admit var redeclarations. */
#[test]
fn catch_binding_redeclarations_are_checked_before_execution() {
    assert_eq!(
        common::eval(
            "const cases=['try{}catch(e){let e}','try{}catch(e){function e(){}}','try{}catch({e}){var e}','try{}catch([e,e]){}','try{}catch(e){for(var e of []){}}','try{}catch({e}){for(var e of []){}}'];const r=cases.map(source=>{try{eval(source);return 'ok'}catch(e){return e.name}}).join();",
            "r"
        ),
        "SyntaxError,SyntaxError,SyntaxError,SyntaxError,ok,SyntaxError"
    );
}
