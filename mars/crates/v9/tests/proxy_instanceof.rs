/** Instance checks observe genuine proxy prototype operations, constructor ordering and native GC roots. */
use crate::common;

/** Direct, explicit hasInstance and inherited proxy prototype checks all use the same actual trap result. */
#[test]
fn instance_checks_follow_proxy_get_prototype_of_traps() {
    assert_eq!(
        common::eval(
            "function C(){}let calls=0;const p=new Proxy({}, {getPrototypeOf(){calls++;return C.prototype}});const o=Object.create(p);const r=[p instanceof C,p instanceof C,C[Symbol.hasInstance](p),C[Symbol.hasInstance](p),o instanceof C,o instanceof C,calls].join();",
            "r"
        ),
        "true,true,true,true,true,true,6"
    );
}

/** A proxy can change its reported prototype between calls; ordinary checks cannot cache the target's raw chain. */
#[test]
fn later_instance_checks_observe_changed_trap_results() {
    assert_eq!(
        common::eval(
            "function C(){}let current=C.prototype;const p=new Proxy({}, {getPrototypeOf(){return current}});const first=p instanceof C;current=null;const second=p instanceof C;current=C.prototype;const third=p instanceof C;const r=[first,second,third].join();",
            "r"
        ),
        "true,false,true"
    );
}

/** Revocation, thrown values and nonextensible-target invariants retain ordinary proxy semantics during traversal. */
#[test]
fn instance_traversal_propagates_proxy_errors_and_invariants() {
    assert_eq!(
        common::eval(
            "function C(){}const marker={};let same=false,n=0;const revoked=Proxy.revocable({},{});revoked.revoke();const invalid=new Proxy(Object.preventExtensions({}),{getPrototypeOf(){return C.prototype}});for(const value of [revoked.proxy,invalid,new Proxy({}, {getPrototypeOf(){return 1}})]){try{value instanceof C}catch(e){if(e instanceof TypeError)n++}}try{new Proxy({}, {getPrototypeOf(){throw marker}}) instanceof C}catch(e){same=e===marker}const r=[n,same].join();",
            "r"
        ),
        "3,true"
    );
}

/** OrdinaryHasInstance returns false for primitives before observing the constructor's prototype property. */
#[test]
fn primitive_inputs_skip_the_constructor_prototype_getter() {
    assert_eq!(
        common::eval(
            "let reads=0;const C=new Proxy(function(){},{get(t,k,r){if(k==='prototype'){reads++;throw 1}return Reflect.get(t,k,r)}});const h=Function.prototype[Symbol.hasInstance];const values=[undefined,null,1,'x',Symbol(),1n];const r=[values.every(v=>h.call(C,v)===false),reads].join();",
            "r"
        ),
        "true,0"
    );
}

/** A custom method precedes callability/prototype traversal, while a bound target retains its own custom method. */
#[test]
fn custom_and_bound_has_instance_dispatch_precedes_prototype_walks() {
    assert_eq!(
        common::eval(
            "let calls=0;const value=new Proxy({}, {getPrototypeOf(){throw 1}});const custom={[Symbol.hasInstance](v){calls++;return v===value}};function C(){}Object.defineProperty(C,Symbol.hasInstance,{value(v){calls++;return v===7}});const bound=C.bind(null);const r=[value instanceof custom,7 instanceof bound,calls].join();",
            "r"
        ),
        "true,true,2"
    );
}

/** The inherited hasInstance lookup occurs once before the constructor prototype read and value-side trap. */
#[test]
fn instance_check_observation_order_is_stable_and_single_read() {
    assert_eq!(
        common::eval(
            "const order=[];function C(){}const ctor=new Proxy(C,{get(t,k,r){if(k===Symbol.hasInstance)order.push('method');if(k==='prototype')order.push('constructor prototype');return Reflect.get(t,k,r)}});const value=new Proxy({}, {getPrototypeOf(){order.push('value prototype');return C.prototype}});const result=value instanceof ctor;const r=[result,order.join(':')].join(';');",
            "r"
        ),
        "true;method:constructor prototype:value prototype"
    );
}

/** Transparent nested proxies and ordinary intermediate nodes preserve their target's actual prototype operations. */
#[test]
fn nested_transparent_proxies_preserve_each_observable_traversal() {
    assert_eq!(
        common::eval(
            "function C(){}let n=0;const leaf=new Proxy({}, {getPrototypeOf(){n++;return C.prototype}});const nested=new Proxy(new Proxy(leaf,{}),{});const middle=Object.create(nested);const r=[middle instanceof C,C[Symbol.hasInstance](middle),n].join();",
            "r"
        ),
        "true,true,2"
    );
}

/** A fresh intermediate proxy remains live while a later getPrototypeOf trap runs the precise collector. */
#[cfg(feature = "conformance")]
#[test]
fn ephemeral_prototype_cursor_is_rooted_across_trap_collection() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "function C(){}let steps=0;const value=new Proxy({}, {getPrototypeOf(){steps++;return new Proxy({}, {getPrototypeOf(){gc();steps++;return C.prototype}})}});const r=[value instanceof C,steps].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,2");
}
