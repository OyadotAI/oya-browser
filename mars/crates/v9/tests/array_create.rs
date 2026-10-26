/** Array static construction preserves generic targets, single iterator lookup and live mapping order. */
mod common;

/** The explicit shell flag uses intrinsic arrays without consulting authored constructors or species getters. */
#[cfg(feature = "conformance")]
#[test]
fn shell_subclassing_flag_selects_intrinsic_arrays() {
    let environment = mars_v9::PageEnvironment {
        conformance_builtin_subclassing: false,
        ..common::environment()
    };
    let execution = common::load_with(
        "class A extends Array{};const a=A.from([1]),b=A.of(2);const input=[3];Object.defineProperty(input,'constructor',{get(){throw 7}});const c=input.map(x=>x);const r=[a.constructor===Array,b.constructor===Array,c.constructor===Array,c[0]].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true,3");
}

/** Mapping mutations affect the next native iterator step instead of an earlier materialized list. */
#[test]
fn array_from_interleaves_mapping_and_live_iteration() {
    assert_eq!(
        common::eval(
            "const source=[1,2,3];let calls=0;const a=Array.from(source,(v,i)=>{calls++;source.length=1;return v+i});const r=[a.join(),calls].join('|');",
            "r"
        ),
        "1|1"
    );
}

/** The iterator getter precedes construction, but the captured method runs afterwards and is never re-read. */
#[test]
fn array_from_captures_the_iterator_method_once() {
    assert_eq!(
        common::eval(
            "const log=[];const source={get [Symbol.iterator](){log.push('get');return function*(){log.push('open');yield 3}}};function C(){log.push('construct');return {}}const a=Array.from.call(C,source);const r=[log.join(),a[0],a.length].join('|');",
            "r"
        ),
        "get,construct,open|3|1"
    );
}

/** A subclass receives its normal construction argument, while a nonconstructor receiver uses the intrinsic Array. */
#[test]
fn static_array_methods_honor_generic_constructors() {
    assert_eq!(
        common::eval(
            "let n=-1;class A extends Array{constructor(...args){super(...args);n=args.length}}const a=A.from([1,2]);const fromArgs=n;const b=A.of(3,4);const r=[a instanceof A,fromArgs,b instanceof A,n,b.join(),Array.isArray(Array.of.call(null,5))].join('|');",
            "r"
        ),
        "true|0|true|1|3,4|true"
    );
}

/** Callback failures close acquired iterators, while invalid callbacks are rejected before any iterator access. */
#[test]
fn mapper_validation_and_abrupt_close_follow_the_protocol() {
    assert_eq!(
        common::eval(
            "let closed=0,gets=0,same=false,type=false;const source={*[Symbol.iterator](){try{yield 1;yield 2}finally{closed++}}};try{Array.from(source,()=>{throw 7})}catch(e){same=e===7}try{Array.from({get [Symbol.iterator](){gets++;throw 8}},null)}catch(e){type=e instanceof TypeError}const r=[closed,gets,same,type].join();",
            "r"
        ),
        "1,0,true,true"
    );
}

/** Authored constructor targets observe indexed definitions before the final length assignment. */
#[test]
fn authored_targets_observe_definitions_and_final_length() {
    assert_eq!(
        common::eval(
            "const log=[];function C(){return new Proxy({},{defineProperty(t,k,d){log.push('define:'+k);return Reflect.defineProperty(t,k,d)},set(t,k,v){log.push('set:'+k);return Reflect.set(t,k,v)}})}const a=Array.from.call(C,[4,5]);const r=[a[0],a[1],a.length,log.join()].join('|');",
            "r"
        ),
        "4|5|2|define:0,define:1,set:length"
    );
}
