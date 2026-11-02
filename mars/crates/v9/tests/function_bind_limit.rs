/** Bound-argument admission follows the pinned native semantic limit without changing parameter or resident-storage quotas. */
use crate::common;

/** The accepted boundary is reachable through the real spread-call path and creates a genuine bound capture record. */
#[test]
fn maximum_allowed_bound_argument_count_is_accepted() {
    assert_eq!(
        common::eval(
            "function f(){}const b=f.bind(null,...new Array(65525));const r=[typeof b,b.length,b.name,Object.getPrototypeOf(b)===Object.getPrototypeOf(f)].join();",
            "r"
        ),
        "function,0,bound f,true"
    );
}

/** The first rejected count produces the intrinsic native error and exact pinned message. */
#[test]
fn first_excess_bound_argument_count_throws_intrinsic_range_error() {
    assert_eq!(
        common::eval(
            "const E=RangeError;globalThis.RangeError=function(){throw 'author constructor'};function f(){}let r='missing error';try{f.bind(null,...new Array(65526))}catch(e){r=[e instanceof E,e.message].join(';')}",
            "r"
        ),
        "true;Too many arguments in function call (only 65525 allowed)"
    );
}

/** The target's actual GetPrototypeOf runs before admission, while length and name observations occur only after success. */
#[test]
fn excessive_bound_arguments_are_checked_after_prototype_before_metadata() {
    assert_eq!(
        common::eval(
            "const order=[];const target=new Proxy(function f(){},{getPrototypeOf(t){order.push('prototype');return Reflect.getPrototypeOf(t)},getOwnPropertyDescriptor(t,k){order.push('own:'+k);return Reflect.getOwnPropertyDescriptor(t,k)},get(t,k,r){order.push('get:'+k);return Reflect.get(t,k,r)}});const args=new Array(65527);args[0]=null;let caught=false;try{Reflect.apply(Function.prototype.bind,target,args)}catch(e){caught=e instanceof RangeError}const r=[caught,order.join()].join(';');",
            "r"
        ),
        "true;prototype"
    );
}

/** A thrown prototype callback, revoked proxy or broken nonextensible invariant wins over the later argument-count guard. */
#[test]
fn prototype_failures_keep_their_identity_before_bound_argument_rejection() {
    assert_eq!(
        common::eval(
            "const args=new Array(65527);args[0]=null;const marker={};let same=false,n=0;const throwing=new Proxy(function(){},{getPrototypeOf(){throw marker}});try{Reflect.apply(Function.prototype.bind,throwing,args)}catch(e){same=e===marker}const revoked=Proxy.revocable(function(){},{});revoked.revoke();const invariant=new Proxy(Object.preventExtensions(function(){}),{getPrototypeOf(){return {}}});for(const target of [revoked.proxy,invariant]){try{Reflect.apply(Function.prototype.bind,target,args)}catch(e){if(e instanceof TypeError)n++}}const r=[same,n].join();",
            "r"
        ),
        "true,2"
    );
}

/** Noncallable receivers fail before any target prototype trap, independently of the already evaluated argument count. */
#[test]
fn bind_receiver_callability_precedes_prototype_and_count_checks() {
    assert_eq!(
        common::eval(
            "const args=new Array(65527);let touched=0,n=0;const receiver=new Proxy({}, {getPrototypeOf(){touched++;throw 1}});for(const target of [receiver,null,undefined,3]){try{Reflect.apply(Function.prototype.bind,target,args)}catch(e){if(e instanceof TypeError)n++}}const r=[n,touched].join();",
            "r"
        ),
        "4,0"
    );
}

/** Incoming argument getters finish before the bind builtin starts, so their original exception cannot be replaced by admission. */
#[test]
fn incoming_argument_effects_precede_the_bind_semantic_guard() {
    assert_eq!(
        common::eval(
            "const args=new Array(65527);const marker={};let touched=0;Object.defineProperty(args,'65526',{get(){throw marker}});const target=new Proxy(function(){},{getPrototypeOf(t){touched++;return Reflect.getPrototypeOf(t)}});let same=false;try{Reflect.apply(Function.prototype.bind,target,args)}catch(e){same=e===marker}const r=[same,touched].join();",
            "r"
        ),
        "true,0"
    );
}

/** Each native bound record admits its own argument list; nesting does not invent an aggregate early-admission rule. */
#[test]
fn nested_binding_keeps_the_existing_per_record_limit() {
    assert_eq!(
        common::eval(
            "function f(){}const first=f.bind(null,...new Array(65525));const second=first.bind(null,1);const r=[typeof second,second.length,second.name,Object.getPrototypeOf(second)===Object.getPrototypeOf(f)].join();",
            "r"
        ),
        "function,0,bound bound f,true"
    );
}

/** Count errors belong to the invoked bind method's realm, before native bound allocation would enter the target's context. */
#[cfg(feature = "conformance")]
#[test]
fn bound_argument_errors_preserve_method_realm_and_valid_foreign_binding() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const child=Realm.create();const foreign=Realm.eval(child,'const secret=40;function target(a,b){return secret+a+b}target');const bind=Realm.eval(child,'Function.prototype.bind');const E=Realm.eval(child,'RangeError');const prototype=Realm.eval(child,'Function.prototype');function own(){}const args=new Array(65527);let local=false,other=false;try{Reflect.apply(Function.prototype.bind,foreign,args)}catch(e){local=e instanceof RangeError&&!(e instanceof E)}try{Reflect.apply(bind,own,args)}catch(e){other=e instanceof E&&!(e instanceof RangeError)}const small=Function.prototype.bind.call(foreign,null,1);const r=[local,other,Object.getPrototypeOf(small)===prototype,small(2)].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,43");
}
