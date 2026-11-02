/** Destructuring holes consume iterator steps without observing result values or changing cleanup and native next semantics. */
use crate::common;

/** A successful elision reads next and done, skips value, then closes the still-live record. */
#[test]
fn assignment_elisions_skip_result_value_getters() {
    let source = "const log=[];const iterable={[Symbol.iterator](){return this},next(){log.push('next');return {get done(){log.push('done');return false},get value(){throw 'value'}}},return(){log.push('return');return {}}};const same=([,]=iterable)===iterable;const r=[log.join(':'),same].join();";
    assert_eq!(common::eval(source, "r"), "next:done:return,true");
}

/** Binding declarations and function parameter patterns share the same elision semantics as assignment. */
#[test]
fn declaration_and_parameter_elisions_skip_values() {
    let source = "let stepped=0,closed=0;function iterable(){return {[Symbol.iterator](){return this},next(){stepped++;return {done:false,get value(){throw 'value'}}},return(){closed++;return {}}}}var [,]=iterable();let [,]=iterable();const [,]=iterable();function f([,]){}f(iterable());(([,])=>{})(iterable());const r=[stepped,closed].join();";
    assert_eq!(common::eval(source, "r"), "5,5");
}

/** Real binding elements and rest still observe values, while adjacent holes only advance their positions. */
#[test]
fn mixed_elisions_bindings_and_rest_observe_only_needed_values() {
    let source = "let next=0,done=0,values=0,closed=0;const it={[Symbol.iterator](){return this},next(){const index=next++;return new Proxy({done:index===5,value:index},{get(target,key){if(key==='done'){done++;if(typeof gc==='function')gc()}if(key==='value'){values++;if(target.value===0||target.value===2)throw 'hole'}return target[key]}})},return(){closed++;return {}}};const [,value,,...rest]=it;const r=[value,rest.join(':'),next,done,values,closed].join();";
    assert_eq!(common::eval(source, "r"), "1,3:4,6,6,3,0");
}

/** Exhaustion is remembered across further holes, defaults and rest without another next, value or return access. */
#[test]
fn exhausted_elisions_do_not_advance_or_close_again() {
    let source = "let next=0,done=0;const it={[Symbol.iterator](){return this},next(){next++;return {get done(){done++;return {}},get value(){throw 'value'}}},get return(){throw 'return'}};const [,,,value=7,...rest]=it;const r=[value,rest.length,next,done].join();";
    assert_eq!(common::eval(source, "r"), "7,0,1,1");
}

/** Next, result-object and done failures retain their original errors and never invoke IteratorClose. */
#[test]
fn abrupt_elision_steps_do_not_close_the_iterator() {
    let source = "const out=[];for(const mode of ['next','result','done']){const marker={};let closed=0;const it={[Symbol.iterator](){return this},next(){if(mode==='next')throw marker;if(mode==='result')return 1;return {get done(){throw marker},get value(){throw 'value'}}},return(){closed++;throw 'close'}};try{[,]=it;out.push(false)}catch(error){out.push(mode==='result'?error instanceof TypeError:error===marker,closed)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,0,true,0,true,0");
}

/** A normal partial pattern still exposes return getter, call and nonobject-result errors after its skipped value. */
#[test]
fn successful_elisions_propagate_normal_close_failures() {
    let source = "const out=[];for(const mode of ['getter','call','result']){const marker={};let calls=0;const it={[Symbol.iterator](){return this},next(){return {done:false,get value(){throw 'value'}}},get return(){calls++;if(mode==='getter')throw marker;return function(){if(mode==='call')throw marker;return 1}}};try{[,]=it;out.push(false)}catch(error){out.push(mode==='result'?error instanceof TypeError:error===marker,calls)}}const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,1,true,1,true,1");
}

/** Iterator acquisition caches next once; a changed public method cannot alter subsequent elision steps. */
#[test]
fn elisions_preserve_cached_next_and_its_receiver() {
    let source = "let gets=0,calls=0,closed=0,receivers=true;const it={[Symbol.iterator](){return this},get next(){gets++;return function(){calls++;receivers=receivers&&this===it;Object.defineProperty(it,'next',{value(){throw 'replacement'},configurable:true});return {done:false,get value(){throw 'value'}}}},return(){closed++;return {}}};[,,]=it;const r=[gets,calls,closed,receivers].join();";
    assert_eq!(common::eval(source, "r"), "1,2,1,true");
}

/** The native array iterator's own next operation still reads indexed getters even when the pattern discards the value. */
#[test]
fn native_array_elisions_keep_index_getters_and_abrupt_cleanup_rules() {
    let source = "let reads=0,closed=0;const marker={};const proto=Object.getPrototypeOf([][Symbol.iterator]());const previous=Object.getOwnPropertyDescriptor(proto,'return');Object.defineProperty(proto,'return',{value(){closed++;return {}},configurable:true});const array=[1];Object.defineProperty(array,'0',{get(){reads++;throw marker}});let same=false;try{[,]=array}catch(error){same=error===marker}if(previous)Object.defineProperty(proto,'return',previous);else delete proto.return;const r=[same,reads,closed].join();";
    assert_eq!(common::eval(source, "r"), "true,1,0");
}

/** A successful native next can perform author index reads and still needs normal iterator closure. */
#[test]
fn successful_native_array_elisions_still_close_partial_iteration() {
    let source = "let reads=0,closed=0;const proto=Object.getPrototypeOf([][Symbol.iterator]());const previous=Object.getOwnPropertyDescriptor(proto,'return');Object.defineProperty(proto,'return',{value(){closed++;return {}},configurable:true});const array=[1,2];Object.defineProperty(array,'0',{get(){reads++;return 9}});[,]=array;if(previous)Object.defineProperty(proto,'return',previous);else delete proto.return;const r=[reads,closed].join();";
    assert_eq!(common::eval(source, "r"), "1,1");
}

/** Nested holes consume their inner record and close it before the outer pattern's still-live record. */
#[test]
fn nested_elisions_preserve_inner_then_outer_closure() {
    let source = "const log=[];const inner={[Symbol.iterator](){return this},next(){log.push('inner step');return {done:false,get value(){throw 'value'}}},return(){log.push('inner close');return {}}};const outer={[Symbol.iterator](){return this},next(){log.push('outer step');return {done:false,value:inner}},return(){log.push('outer close');return {}}};const [[,]]=outer;const r=log.join(':');";
    assert_eq!(
        common::eval(source, "r"),
        "outer step:inner step:inner close:outer close"
    );
}

/** A suspended default resumes into subsequent elisions using the same cached record and normal cleanup after JSON transport. */
#[test]
fn elision_steps_survive_generator_state_transport() {
    let sources = vec!["let next=0,closed=0;const it={[Symbol.iterator](){return this},next(){const index=next++;return {done:false,get value(){if(index!==1)throw 'hole';return undefined}}},return(){closed++;return {}}};function* run(){const [,value=yield 'pause',,]=it;return value}const gen=run();gen.next();document.getElementById('go').addEventListener('click',()=>{console.log(gen.next(7).value,next,closed)});".into()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["7 3 1"]);
}
