/** Prototype membership reports real budget exhaustion without treating repeated stateful proxy results as a cycle. */
mod common;

/** Revisiting the same proxy remains valid when a later trap result reaches the requested prototype or null. */
#[test]
fn finite_stateful_repeated_prototypes_can_terminate_normally() {
    assert_eq!(
        common::eval(
            "function C(){}let a=0,b=0,c=0;const first=new Proxy({}, {getPrototypeOf(){return ++a===7?C.prototype:first}});const second=new Proxy({}, {getPrototypeOf(){return ++b===9?C.prototype:second}});const missing=new Proxy({}, {getPrototypeOf(){return ++c===5?null:missing}});const r=[first instanceof C,Object.prototype.isPrototypeOf.call(C.prototype,second),Object.prototype.isPrototypeOf.call(C.prototype,missing),a,b,c].join();",
            "r"
        ),
        "true,true,false,7,9,5"
    );
}

/** Actual exhausted walks throw an intrinsic RangeError at the existing bound instead of claiming a missing prototype. */
#[test]
fn exhausted_prototype_walks_throw_without_increasing_the_budget() {
    assert_eq!(
        common::eval(
            "const E=RangeError;globalThis.RangeError=function(){throw 'author constructor'};function C(){}const values=[];for(const method of ['instanceof','isPrototypeOf']){let steps=0;const p=new Proxy({}, {getPrototypeOf(){steps++;return p}});let caught=false,message=false;try{if(method==='instanceof')p instanceof C;else Object.prototype.isPrototypeOf.call(C.prototype,p)}catch(e){caught=e instanceof E;message=e.message==='Prototype traversal exceeds the limit of 4096'}values.push(caught,message,steps)}const r=values.join();",
            "r"
        ),
        "true,true,4096,true,true,4096"
    );
}

/** A stateful trap's own exception wins before exhaustion, even after several repeated results. */
#[test]
fn repeated_prototype_traps_preserve_author_exception_identity() {
    assert_eq!(
        common::eval(
            "function C(){}const marker={};let n=0;for(const method of ['instanceof','isPrototypeOf']){let steps=0;const p=new Proxy({}, {getPrototypeOf(){if(++steps===4)throw marker;return p}});try{if(method==='instanceof')p instanceof C;else Object.prototype.isPrototypeOf.call(C.prototype,p)}catch(e){if(e===marker&&steps===4)n++}}const r=n;",
            "r"
        ),
        "2"
    );
}

/** Normal chain termination and primitive-input fast returns retain their original behavior. */
#[test]
fn non_exhausted_walks_keep_ordinary_membership_and_receiver_order() {
    assert_eq!(
        common::eval(
            "function C(){}const object=Object.create(C.prototype);const none=Object.create(null);const f=Object.prototype.isPrototypeOf;const r=[object instanceof C,C.prototype.isPrototypeOf(object),none instanceof C,f.call(C.prototype,none),f.call(null,1),f.call(undefined,null)].join();",
            "r"
        ),
        "true,true,false,false,false,false"
    );
}
