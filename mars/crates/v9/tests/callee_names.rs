/** A call of a value that is not a function names the callee as the source wrote it, as V8 does. */
use crate::common;

/** A member chain is printed dot by dot. */
#[test]
fn member_chain_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval(
            "let r;const o={a:{}};try{o.a.b()}catch(e){r=e.message}",
            "r"
        ),
        "o.a.b is not a function"
    );
}

/** A plain identifier callee is named by its binding. */
#[test]
fn identifier_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval("let r;let q=1;try{q()}catch(e){r=e.message}", "r"),
        "q is not a function"
    );
}

/** A method called on `this` is named with `this`. */
#[test]
fn this_member_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval(
            "let r;function f(){return this.nope()}try{f.call({})}catch(e){r=e.message}",
            "r"
        ),
        "this.nope is not a function"
    );
}

/** Spread calls keep the name too, since they use their own call instruction. */
#[test]
fn spread_call_callee_is_named_in_not_a_function_error() {
    assert_eq!(
        common::eval(
            "let r;const o={};try{o.nope(...[1])}catch(e){r=e.message}",
            "r"
        ),
        "o.nope is not a function"
    );
}

/** A `new` of a value that is not a constructor is named by its binding. */
#[test]
fn identifier_callee_is_named_in_not_a_constructor_error() {
    assert_eq!(
        common::eval("let r;let q=1;try{new q()}catch(e){r=e.message}", "r"),
        "q is not a constructor"
    );
}

/** A literal callee prints as its value, a string in double quotes, as V8's printer writes it. */
#[test]
fn literal_callees_print_as_values() {
    assert_eq!(
        common::eval(
            "let r=[];for(const f of [()=>(1)(),()=>\"s\"(),()=>null(),()=>(-1)()]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "1 is not a function|\"s\" is not a function|null is not a function|-1 is not a function"
    );
}

/** A keyed callee prints its string key after a dot and any other key in brackets. */
#[test]
fn keyed_callees_print_their_keys() {
    assert_eq!(
        common::eval(
            "let r=[];const o={},k='x';for(const f of [()=>o['f'](),()=>o[0](),()=>o[k](),()=>o[Symbol.iterator](),()=>[1][0]()]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "o.f is not a function|o[0] is not a function|o[k] is not a function|o[Symbol.iterator] is not a function|[1][0] is not a function"
    );
}

/** What V8's printer does not render is an intermediate value: literals of objects and
functions, results of `new`, finished optional chains, and each part of a conditional. */
#[test]
fn unprinted_callees_are_intermediate_values() {
    assert_eq!(
        common::eval(
            "let r=[];const a={},b=1;function F(){}for(const f of [()=>({a:1}).a(),()=>new F()(),()=>(a?.b)(),()=>(b?1:2)(),()=>(1n)(),()=>new (()=>1)]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "{(intermediate value)}.a is not a function|(intermediate value) is not a function|(intermediate value) is not a function|(intermediate value)(intermediate value)(intermediate value) is not a function|(intermediate value) is not a function|(intermediate value) is not a constructor"
    );
}

/** Operators print in parentheses, a chain of one operator flat, and a call as `callee(...)`. */
#[test]
fn operator_and_call_callees_print_in_v8_form() {
    assert_eq!(
        common::eval(
            "let r=[];let a=1;function f(){}for(const g of [()=>(a+1+2)(),()=>(typeof a)(),()=>(a++)(),()=>(a,2)(),()=>f()(),()=>(a=1)()]){try{g()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "(a + 1 + 2) is not a function|(typeof a) is not a function|(a++) is not a function|(a , 2) is not a function|f(...) is not a function|a is not a function"
    );
}

/** A template tag is a call like any other and names its tag. */
#[test]
fn template_tag_callee_is_named() {
    assert_eq!(
        common::eval("let r;const x={};try{x.y`t`}catch(e){r=e.message}", "r"),
        "x.y is not a function"
    );
}

/** A call started by native code has no source text: V8 names the value by its type for a
call and by its text for a construct. */
#[test]
fn native_calls_name_the_value_by_type() {
    assert_eq!(
        common::eval(
            "let r=[];for(const f of [()=>Reflect.construct(1,[]),()=>Reflect.construct({},[]),()=>Reflect.construct(function(){},[],()=>1)]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "1 is not a constructor|#<Object> is not a constructor|()=>1 is not a constructor"
    );
}

/** An anonymous class called without `new` has V8's plural wording. */
#[test]
fn anonymous_class_call_uses_plural_wording() {
    assert_eq!(
        common::eval("let r;try{(class{})()}catch(e){r=e.message}", "r"),
        "Class constructors cannot be invoked without 'new'"
    );
}
