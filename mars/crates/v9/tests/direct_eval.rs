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
