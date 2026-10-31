/** Function source retains original grammar and trivia without consulting mutable public metadata. */
use crate::common;

/** Declarations, arrows and methods return exactly the parsed function span. */
#[test]
fn functions_preserve_exact_source() {
    assert_eq!(
        common::eval(
            "function /*a*/ f (x) { return x; }const a=(x /*b*/)=> x+1;const o={get /*c*/ value () {return 1},[ 'm' ] (x) {return x}};const r=[f.toString(),a.toString(),Object.getOwnPropertyDescriptor(o,'value').get.toString(),o.m.toString()].join('|');",
            "r"
        ),
        "function /*a*/ f (x) { return x; }|(x /*b*/)=> x+1|get /*c*/ value () {return 1}|[ 'm' ] (x) {return x}"
    );
}

/** A class constructor reports the class body rather than just its constructor method. */
#[test]
fn classes_preserve_their_entire_source() {
    assert_eq!(
        common::eval(
            "class /*a*/ A { constructor(x) {this.x=x} method(){} }const B=class extends A { method() {return 2} };const r=[A.toString(),B.toString()].join('|');",
            "r"
        ),
        "class /*a*/ A { constructor(x) {this.x=x} method(){} }|class extends A { method() {return 2} }"
    );
}

/** Dynamic names are observable metadata without introducing an anonymous lexical binding. */
#[test]
fn dynamic_functions_report_standard_anonymous_source() {
    assert_eq!(
        common::eval(
            "const f=Function('a','return a');const g=Function('return typeof anonymous');const r=[f.toString(),g(),f(3)].join('|');",
            "r"
        ),
        "function anonymous(a\n) {\nreturn a\n}|undefined|3"
    );
}

/** Public name getters cannot run while obtaining either script or native source. */
#[test]
fn source_does_not_read_public_name_properties() {
    assert_eq!(
        common::eval(
            "function f(){}const bound=f.bind(null);Object.defineProperty(f,'name',{get(){throw 1}});Object.defineProperty(Array.prototype.push,'name',{get(){throw 2}});const r=[f.toString(),Array.prototype.push.toString(),bound.toString(),new Proxy(f,{}).toString()].join('|');",
            "r"
        ),
        "function f(){}|function push() { [native code] }|function () { [native code] }|function () { [native code] }"
    );
}

/** Exact code sharing preserves separate callable identities without accumulating duplicate source. */
#[test]
fn repeated_dynamic_functions_share_source_but_remain_distinct() {
    assert_eq!(
        common::eval(
            "const body='/*'+'x'.repeat(3000)+'*/return 1';const functions=[];for(let i=0;i<1000;i++)functions.push(Function(body));const r=[functions[0]!==functions[999],functions.every(f=>f()===1),functions[0].toString()===functions[999].toString()].join();",
            "r"
        ),
        "true,true,true"
    );
}
