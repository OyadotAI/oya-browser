/** The `with` statement (spec 14.11) and its object environment record (spec 9.1.1.2). */
mod common;
use common::eval;

/** Names in a with body read and write the object's properties first, else the outer scope. */
#[test]
fn with_resolves_names_on_the_object() {
    let source = "var x=1,y=2;var o={x:10};with(o){x=x+1;y=y+1;var z=x}var r=[o.x,x,y,z].join();const out=r;";
    assert_eq!(eval(source, "out"), "11,1,3,11");
}

/** A function called through a with object gets the object as `this`. */
#[test]
fn with_calls_pass_the_object_as_this() {
    let source = "var o={v:7,f:function(){return this.v}};var r;with(o){r=f()}const out=r;";
    assert_eq!(eval(source, "out"), "7");
}

/** Closures made inside a with body keep seeing the object, live. */
#[test]
fn with_closures_see_the_object() {
    let source = "function mk(o){with(o){return function(){return a}}}var o={a:1};var g=mk(o);o.a=5;var r=g();const out=r;";
    assert_eq!(eval(source, "out"), "5");
}

/** Symbol.unscopables hides a property from the with scope. */
#[test]
fn with_honors_unscopables() {
    let source = "var v='outer';var o={v:'inner'};o[Symbol.unscopables]={v:true};var r;with(o){r=v}const out=r;";
    assert_eq!(eval(source, "out"), "outer");
}

/** `delete name` in a with body deletes the object's property; typeof sees it. */
#[test]
fn with_delete_and_typeof() {
    let source = "var o={p:1};var r;with(o){r=[typeof p,delete p,typeof p].join()}r+=','+('p' in o);const out=r;";
    assert_eq!(eval(source, "out"), "number,true,undefined,false");
}

/** with(null) throws a TypeError; primitives are wrapped. */
#[test]
fn with_converts_with_to_object() {
    let source = "var r;try{with(null){}}catch(e){r=e instanceof TypeError}with('abc'){r=r+','+length}const out=r;";
    assert_eq!(eval(source, "out"), "true,3");
}

/** Strict eval code inherits strictness, so a `with` in it is a SyntaxError. */
#[test]
fn with_in_strict_eval_is_a_syntax_error() {
    let source = "var r;(function(){'use strict';try{eval('with({}){}')}catch(e){r=e instanceof SyntaxError}})();const out=r;";
    assert_eq!(eval(source, "out"), "true");
}

/** In a with body `let` then a new line is a name, not a declaration. */
#[test]
fn with_let_newline_is_a_name() {
    let source = "var r='ok';with({let:1})let\n{}const out=r;";
    assert_eq!(eval(source, "out"), "ok");
}

/** B.3.4: a function declared as an `if` branch hoists like a block function. */
#[test]
fn annex_b_if_branch_function_hoists() {
    let source =
        "var before=typeof f;if(true)function f(){return 'yes'}var r=before+','+f();const out=r;";
    assert_eq!(eval(source, "out"), "undefined,yes");
}

/** B.3.2.1: a block function named like a parameter or `arguments` leaves it alone. */
#[test]
fn annex_b_block_function_skips_parameters() {
    let source =
        "var r=(function(f){{function f(){}}return typeof f+','+typeof arguments})(1);const out=r;";
    assert_eq!(eval(source, "out"), "number,object");
}

/** B.3.5: sloppy `for (var x = init in obj)` runs the initializer once, before obj. */
#[test]
fn annex_b_for_in_initializer_runs() {
    let source = "var n=0,s;for(var a=(++n,7) in s=a,{p:1}){}var r=[n,s].join();const out=r;";
    assert_eq!(eval(source, "out"), "1,7");
}

/** Nested labels may still label a sloppy function declaration. */
#[test]
fn annex_b_nested_labeled_function() {
    let source = "a: b: function f(){return 3}var r=f();const out=r;";
    assert_eq!(eval(source, "out"), "3");
}
