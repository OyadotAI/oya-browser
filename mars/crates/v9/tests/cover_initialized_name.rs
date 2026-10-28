/** Cover initialized names are accepted in patterns but rejected before object expressions execute. */
mod common;

/** An invalid cover initialized name must throw SyntaxError before resolving its callee. */
#[test]
fn initialized_shorthand_requires_a_destructuring_pattern() {
    let source = "let n=0;for(const text of ['missing({x=1})','false && ({x=1})','function f(){return {x=1}}'])try{eval(text)}catch(e){n+=e instanceof SyntaxError}let x;({x=3}={});const f=({y=4})=>y;const r=[n,x,f({})].join();";
    assert_eq!(common::eval(source, "r"), "3,3,4");
}
