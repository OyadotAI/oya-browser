/** Pooled argument vectors: a call sees exactly its own arguments, never those of an earlier call. */
use crate::common;

/** A call with fewer arguments than parameters sees undefined, not the previous call's values. */
#[test]
fn missing_arguments_do_not_inherit_a_previous_calls_values() {
    let source = "function pair(a,b){return [a,b].join('|')}const first=pair('x','y');const second=pair('z');const r=first+';'+second;";
    assert_eq!(common::eval(source, "r"), "x|y;z|");
}

/** `arguments` reflects the length of the call it belongs to, even after a longer call returned. */
#[test]
fn arguments_length_matches_its_own_call_after_a_longer_call() {
    let source = "function count(){return arguments.length}const r=[count(1,2,3,4),count(),count(9)].join();";
    assert_eq!(common::eval(source, "r"), "4,0,1");
}

/** A native call's arguments are copied, so a callee that keeps them is unaffected by later calls. */
#[test]
fn native_call_arguments_are_not_reused_by_a_later_call() {
    let source = "const kept=[];kept.push(Array.prototype.concat.call([],1,2));const other=Math.max(5,6,7);const r=[kept[0].join(),other].join(';');";
    assert_eq!(common::eval(source, "r"), "1,2;7");
}

/** A constructor's arguments are its own even when the pool hands back a vector used by a call. */
#[test]
fn constructor_arguments_are_its_own_after_pooled_calls() {
    let source = "function Point(x,y){this.x=x;this.y=y}function id(v){return v}id(1,2,3);const p=new Point(4);const r=[p.x,p.y,id(7)].join();";
    assert_eq!(common::eval(source, "r"), "4,,7");
}
