/** Set algebra uses live internal data and the complete observable set-like argument protocol. */
use crate::common;

/** Algebra retains insertion order, avoids species construction, and treats NaN and zeros as set keys. */
#[test]
fn algebra_preserves_membership_and_insertion_order() {
    assert_eq!(
        common::eval(
            "const a=new Set([1,2,NaN,-0]);const b=new Set([2,3,NaN]);a.constructor={get [Symbol.species](){throw 1}};const r=[Array.from(a.union(b)).join(),Array.from(a.intersection(b)).join(),Array.from(a.difference(b)).join(),Array.from(a.symmetricDifference(b)).join()].join('|');",
            "r"
        ),
        "1,2,NaN,0,3|2,NaN|1,0|1,0,3"
    );
}

/** Size, has and keys getters run once and in order even for an early size answer. */
#[test]
fn set_like_record_is_validated_before_size_shortcuts() {
    assert_eq!(
        common::eval(
            "const log=[];const other={get size(){log.push('size');return 0},get has(){log.push('has');return ()=>false},get keys(){log.push('keys');return ()=>({next(){return {done:true}}})}};const value=new Set([1]).isSubsetOf(other);let invalid=false;try{new Set().union({size:NaN,has(){},keys(){}})}catch(e){invalid=e instanceof TypeError}const r=[value,log.join(),invalid].join('|');",
            "r"
        ),
        "false|size,has,keys|true"
    );
}

/** A has callback may grow the receiver; the selected live walk observes its new insertion. */
#[test]
fn intersection_observes_receiver_mutations() {
    assert_eq!(
        common::eval(
            "const a=new Set([1]);const other={size:10,has(x){if(x===1){a.delete(1);a.add(2)}return true},keys(){throw 1}};const r=Array.from(a.intersection(other)).join();",
            "r"
        ),
        "1,2"
    );
}

/** Keys iterators need no iterator symbol, and a normal early exit validates return's object result. */
#[test]
fn predicates_close_keys_iterators_on_early_exit() {
    assert_eq!(
        common::eval(
            "let closed=0;const other={size:1,has(){return false},keys(){return {next(){return {done:false,value:1}},return(){closed++;return 7}}}};let type=false;try{new Set([1,2]).isDisjointFrom(other)}catch(e){type=e instanceof TypeError}const r=[closed,type].join();",
            "r"
        ),
        "1,true"
    );
}
