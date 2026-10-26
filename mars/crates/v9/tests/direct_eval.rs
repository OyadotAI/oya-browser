/** Direct eval (spec PerformEval, EvalDeclarationInstantiation): caller scopes, vars, this. */
mod common;
use common::eval;

/** Direct eval reads and writes the caller's locals, block lexicals and parameters. */
#[test]
fn direct_eval_sees_caller_scopes() {
    let source =
        "function f(a){let b=2;{const c=3;eval('a=a*10');return eval('a+b+c')}}const r=f(1);";
    assert_eq!(eval(source, "r"), "15");
}

/** Direct eval sees the caller's `this`; an aliased eval is indirect and global. */
#[test]
fn direct_eval_sees_this_and_alias_is_indirect() {
    let source = "var x='global';function f(){var x='local';var e=eval;return [eval('this.k'),eval('x'),e('x')].join()}\
        const r=f.call({k:'obj'});";
    assert_eq!(eval(source, "r"), "obj,local,global");
}

/** A sloppy eval's `var` and function land in the caller function's scope, not the global. */
#[test]
fn sloppy_eval_vars_hoist_into_the_function() {
    let source = "var v='g';function f(){eval('var v=1;function h(){return v+1}');eval('v=v+1');return [v,h(),typeof v].join()}\
        const r=[f(),v,typeof h].join();";
    assert_eq!(eval(source, "r"), "2,3,number,g,undefined");
}

/** A strict eval keeps its vars to itself. */
#[test]
fn strict_eval_vars_stay_inside() {
    let source = "function f(){'use strict';eval('var q=1');return typeof q}\
        function g(){eval('\"use strict\";var q=1');return typeof q}const r=[f(),g()].join();";
    assert_eq!(eval(source, "r"), "undefined,undefined");
}

/** A sloppy eval's var may not redeclare a caller's let; a global eval var is deletable. */
#[test]
fn eval_var_conflicts_and_global_vars() {
    let source = "let out=[];function f(){let z;try{eval('var z')}catch(e){out.push(e.constructor.name)}}f();\
        eval('var gv=1');out.push(Object.getOwnPropertyDescriptor(globalThis,'gv').configurable);\
        out.push(eval(5));const r=out.join();";
    assert_eq!(eval(source, "r"), "SyntaxError,true,5");
}

/** A sloppy eval in a parameter initializer may not declare `arguments`. */
#[test]
fn parameter_eval_cannot_declare_arguments() {
    let source = "let r;const f=(p=eval('var arguments=1'))=>{};try{f()}catch(e){r=e.constructor.name}\
        function g(p=eval('var other=2')){return other}r=r+','+g();";
    assert_eq!(
        eval(&format!("{source};const out=r;"), "out"),
        "SyntaxError,2"
    );
}

/** Indirect strict eval has private var and function bindings instead of changing the global. */
#[test]
fn indirect_strict_eval_keeps_declarations_private() {
    let source = "var value=1;const evil=eval;\
        const closure=evil('\"use strict\";var value=2;function local(){return value}local');\
        const r=[value,closure(),typeof local].join();";
    assert_eq!(eval(source, "r"), "1,2,undefined");
}

/** Eval lexicals are private in either mode, while sloppy global vars remain deletable. */
#[test]
fn indirect_eval_lexicals_do_not_escape() {
    let source = "const evil=eval;evil('let lexical=1;const immutable=2;var removable=3');\
        const r=[typeof lexical,typeof immutable,removable,delete globalThis.removable].join();";
    assert_eq!(eval(source, "r"), "undefined,undefined,3,true");
}

/** A closure created by strict indirect eval retains its private bindings through state transport. */
#[test]
fn indirect_eval_closures_survive_state_transport() {
    let sources = vec![
        "const evil=eval;const closure=evil('\"use strict\";var value=7;()=>++value');\
        document.getElementById('go').addEventListener('click',()=>console.log(closure()));"
            .to_string(),
    ];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty());
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert_eq!(common::console(&clicked), ["8"]);
}

/** A strict caller's direct eval rejects restricted identifiers and delete grammar immediately. */
#[test]
fn direct_eval_inherits_strict_early_errors() {
    let source = "function f(){'use strict';const out=[];\
        for(const code of ['var eval','var yield','delete missing','function g(eval){}']){\
        try{eval(code);out.push(false)}catch(e){out.push(e instanceof SyntaxError)}}return out.join()}const r=f();";
    assert_eq!(eval(source, "r"), "true,true,true,true");
}
