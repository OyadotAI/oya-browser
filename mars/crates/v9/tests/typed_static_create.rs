/** TypedArray.from/of construct generic receivers and preserve iterator-versus-array-like observation order. */
use crate::common;

/** A subclass constructor is invoked once with length and determines the resulting native view prototype. */
#[test]
fn static_creation_invokes_the_actual_constructor() {
    assert_eq!(
        common::eval(
            "let hits=0;class Bag extends Uint16Array{constructor(n){super(n);hits++;}}const a=Bag.of(5,6);const target=new Uint8Array(2);function C(n){hits+=n;return target}const b=Float64Array.of.call(C,7,8);const r=[hits,a instanceof Bag,a.join(),b===target,b.join(),Object.hasOwn(Uint8Array,'of')].join('|');",
            "r"
        ),
        "3|true|5,6|true|7,8|false"
    );
}

/** Iterables are consumed before construction, while array-like getters run after construction. */
#[test]
fn source_observation_order_depends_on_iterability() {
    assert_eq!(
        common::eval(
            "const log=[];const iterable={get [Symbol.iterator](){log.push('method');return function*(){log.push('yield');yield 3}}};function C(n){log.push('construct');return new Uint8Array(n)}Uint8Array.from.call(C,iterable);const arraylike={length:1,get 0(){log.push('get');return 4}};Uint8Array.from.call(C,arraylike);const r=log.join();",
            "r"
        ),
        "method,yield,construct,construct,get"
    );
}

/** Null mapping functions are rejected before opening iterators, even when the source is empty. */
#[test]
fn invalid_mappers_are_checked_before_source_getters() {
    assert_eq!(
        common::eval(
            "let gets=0,type=false;const source={get [Symbol.iterator](){gets++;throw 7}};try{Uint8Array.from(source,null)}catch(e){type=e instanceof TypeError}const r=[type,gets].join();",
            "r"
        ),
        "true,0"
    );
}

/** The initial target witness must fit, but later mapper-induced shrink still runs the remaining mappers. */
#[test]
fn target_validation_precedes_mapping_and_later_stores_recheck_bounds() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8});const a=new Uint8Array(b);function C(){return a}let calls=0;Uint8Array.from.call(C,[1,2],v=>{calls++;b.resize(0);return v});let type=false;try{Uint8Array.of.call(C,1)}catch(e){type=e instanceof TypeError}const r=[calls,type,a.length].join();",
            "r"
        ),
        "2,true,0"
    );
}

/** TypedArray construction calls the captured iterator method without reading its getter twice. */
#[test]
fn typed_array_constructor_reads_iterator_method_once() {
    let source = "let reads=0;const source={get [Symbol.iterator](){reads++;if(reads>1)throw Error('re-read');return function*(){yield 3;yield 5}}};const out=new Uint8Array(source);let r=[reads,out.join()].join('|');";
    assert_eq!(common::eval(source, "r"), "1|3,5");
}

/** A non-callable iterator method fails before any array-like access. */
#[test]
fn typed_array_constructor_rejects_noncallable_iterator_before_length() {
    let source = "let reads=0;const source={get [Symbol.iterator](){return 1},get length(){reads++;return 1}};let error=false;try{new Uint8Array(source)}catch(e){error=e instanceof TypeError}let r=[error,reads].join();";
    assert_eq!(common::eval(source, "r"), "true,0");
}
