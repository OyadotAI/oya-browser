/** For-in respects proxy descriptors and virtual prototypes while retaining a bounded, serializable candidate snapshot. */
use crate::common;

/** Virtual prototype keys are discovered, and enumeration never asks a proxy's has trap. */
#[test]
fn virtual_prototypes_are_enumerated_without_has_traps() {
    let source = "const target=Object.assign(Object.create({physical:1}),{own:1});let calls=0;const proxy=new Proxy(target,{getPrototypeOf(){return {virtual:1}},has(){calls++;return false}});const keys=[];for(const key in proxy)keys.push(key);const r=[keys.join(),calls].join('|');";
    assert_eq!(common::eval(source, "r"), "own,virtual|0");
}

/** Candidate filtering starts from the original receiver, so its descriptor trap can hide virtual inherited names too. */
#[test]
fn proxy_descriptors_filter_inherited_candidates() {
    let source = "let mode=0;const target={own:1};const proxy=new Proxy(target,{getPrototypeOf(){return {virtual:1}},getOwnPropertyDescriptor(target,key){return {enumerable:mode===1||mode===2&&key in target,configurable:true}}});function keys(){const result=[];for(const key in proxy)result.push(key);return result.join()}const result=[keys()];mode=1;result.push(keys());mode=2;result.push(keys());const r=result.join('|');";
    assert_eq!(common::eval(source, "r"), "|own,virtual|own");
}

/** A descriptor-backed virtual string key exists for for-in even when the target and has trap do not acknowledge it. */
#[test]
fn descriptor_backed_ghost_keys_keep_proxy_order_and_exclude_symbols() {
    let source = "const symbol=Symbol('hidden');const proxy=new Proxy({},{ownKeys(){return ['second',symbol,'first']},getOwnPropertyDescriptor(){return {enumerable:true,configurable:true}},has(){throw 'wrong has'},getPrototypeOf(){return null}});const keys=[];for(const key in proxy)keys.push(key);const r=keys.join();";
    assert_eq!(common::eval(source, "r"), "second,first");
}

/** The complete ownKeys/prototype snapshot precedes the first proxy descriptor filter and loop-body execution. */
#[test]
fn proxy_descriptor_filtering_is_deferred_until_after_key_collection() {
    let source = "const log=[];const proxy=new Proxy({x:1},{ownKeys(){log.push('keys');return ['x']},getOwnPropertyDescriptor(){log.push('descriptor');return {enumerable:true,configurable:true}},getPrototypeOf(){log.push('proto');return null},has(){throw 'wrong has'}});for(const key in proxy)log.push('body:'+key);const r=log.join();";
    assert_eq!(common::eval(source, "r"), "keys,proto,descriptor,body:x");
}

/** Revocation, invalid prototype invariants, authored exceptions and virtual cycles retain their genuine errors. */
#[test]
fn proxy_enumeration_preserves_errors_and_bounded_cycles() {
    let source = "const out=[];const thrown=new Proxy({},{getPrototypeOf(){throw 'prototype'}});try{for(const key in thrown){}}catch(e){out.push(e)}const target=Object.preventExtensions({});const invalid=new Proxy(target,{getPrototypeOf(){return {}}});try{for(const key in invalid){}}catch(e){out.push(e.name)}const pair=Proxy.revocable({},{});pair.revoke();try{for(const key in pair.proxy){}}catch(e){out.push(e.name)}let cyclic;cyclic=new Proxy({},{getPrototypeOf(){return cyclic}});try{for(const key in cyclic){}}catch(e){out.push(e.name)}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "prototype,TypeError,TypeError,RangeError"
    );
}

/** Transparent nested proxies preserve the inner virtual descriptor and prototype behavior. */
#[test]
fn nested_proxies_preserve_virtual_enumeration() {
    let source = "const inner=new Proxy({},{ownKeys(){return ['ghost']},getOwnPropertyDescriptor(){return {configurable:true,enumerable:true}},getPrototypeOf(){return {inherited:1}},has(){throw 'wrong has'}});const outer=new Proxy(inner,{});const values=[];for(const key in outer)values.push(key);const r=values.join();";
    assert_eq!(common::eval(source, "r"), "ghost,inherited");
}

/** Ordinary nonenumerable shadows and deletions remain effective, and typed numeric misses stop before prototypes. */
#[test]
fn ordinary_shadowing_deletion_and_typed_index_barriers_are_preserved() {
    let source = "const prototype={deleted:1,hidden:1,tail:1};const object=Object.assign(Object.create(prototype),{first:1,deleted:2});Object.defineProperty(object,'hidden',{value:2,enumerable:false});const keys=[];for(const key in object){keys.push(key);if(key==='first'){delete object.deleted;delete prototype.deleted}}const typed=new Uint8Array(0);Object.setPrototypeOf(typed,{0:1,other:1});const numeric=[];for(const key in typed)numeric.push(key);const r=[keys.join(),numeric.join()].join('|');";
    assert_eq!(common::eval(source, "r"), "first,tail|other");
}

/** Candidate keys, original receiver and next index retain their meaning through a suspended generator's JSON snapshot. */
#[test]
fn suspended_proxy_enumeration_survives_state_transport() {
    let sources=vec!["const proxy=new Proxy({own:1},{getPrototypeOf(){return {virtual:1}},has(){throw 'wrong has'}});function* enumerate(){for(const key in proxy)yield key}const iterator=enumerate();console.log(iterator.next().value);document.getElementById('go').addEventListener('click',()=>console.log(iterator.next().value,iterator.next().done));".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::console(&loaded), ["own"]);
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["virtual true"]);
}
