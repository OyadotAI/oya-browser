/** Map and Set constructor families observe each yielded value and close abrupt insertions without eager materialization. */
use crate::common;

/** A sparse source larger than resident-vector bounds rejects its first bad weak entry without exhausting holes. */
#[test]
fn weak_map_rejects_a_sparse_initial_entry_before_materializing_the_source() {
    assert_eq!(
        common::eval(
            "const source=new Array(0x80001);source[0]={};source[0x80000]=1;let closed=0;const iterator=source[Symbol.iterator]();iterator.return=function(){closed++;return {}};const iterable={[Symbol.iterator](){return iterator}};let rejected=false;try{new WeakMap(iterable)}catch(e){rejected=e instanceof TypeError}const r=[rejected,closed].join();",
            "r"
        ),
        "true,1"
    );
}

/** Null and undefined inputs do not read a subclass adder or require any iterator. */
#[test]
fn nullish_collection_sources_skip_adder_acquisition() {
    assert_eq!(
        common::eval(
            "let n=0;for(const Base of [Map,Set,WeakMap,WeakSet]){class C extends Base{}const name=Base===Map||Base===WeakMap?'set':'add';Object.defineProperty(C.prototype,name,{get(){throw 1}});for(const source of [null,undefined]){if(new C(source) instanceof C)n++}}const r=n;",
            "r"
        ),
        "8"
    );
}

/** Adder callability is established before the source can observe Symbol.iterator access. */
#[test]
fn non_callable_collection_adders_reject_before_getting_the_iterator() {
    assert_eq!(
        common::eval(
            "const order=[];let n=0;for(const Base of [Map,Set,WeakMap,WeakSet]){class C extends Base{}Object.defineProperty(C.prototype,Base===Map||Base===WeakMap?'set':'add',{get(){order.push('adder');return 1}});const source={get [Symbol.iterator](){order.push('iterator');throw 2}};try{new C(source)}catch(e){if(e instanceof TypeError)n++}}const r=[n,order.join(':')].join(';');",
            "r"
        ),
        "4;adder:adder:adder:adder"
    );
}

/** Entry properties and adder side effects interleave before the next pull rather than after iterator exhaustion. */
#[test]
fn map_entry_reads_and_additions_interleave_with_iteration() {
    assert_eq!(
        common::eval(
            "const order=[];class C extends Map{}let original;Object.defineProperty(C.prototype,'set',{get(){order.push('adder');return original=function(k,v){order.push('add:'+k);return Map.prototype.set.call(this,k,v)}}});const iterator={i:0,get next(){order.push('next getter');return function(){order.push('next:'+this.i);if(this.i===2)return{done:true};const i=this.i++;return{value:{get 0(){order.push('key:'+i);return i},get 1(){order.push('value:'+i);return i+10}}}}}};const source={get [Symbol.iterator](){order.push('iterator getter');return function(){order.push('iterator call');return iterator}}};const result=new C(source);const r=[order.join(','),result.get(0),result.get(1)].join(';');",
            "r"
        ),
        "adder,iterator getter,iterator call,next getter,next:0,key:0,value:0,add:0,next:1,key:1,value:1,add:1,next:2;10;11"
    );
}

/** Abrupt entry reads and adders close once, preserving original errors even when cleanup throws or returns a primitive. */
#[test]
fn constructor_abrupt_insertions_close_once_and_preserve_the_original_error() {
    assert_eq!(
        common::eval(
            "let n=0;for(const mode of ['key','value','adder']){for(const cleanup of ['throw','primitive','getter']){const marker={};let next=0,closed=0;class C extends Map{set(k,v){if(mode==='adder')throw marker;return super.set(k,v)}}const entry={get 0(){if(mode==='key')throw marker;return 1},get 1(){if(mode==='value')throw marker;return 2}};const iterator={next(){next++;return{value:entry}},get return(){closed++;if(cleanup==='getter')throw 'cleanup getter';return function(){if(cleanup==='throw')throw 'cleanup call';return 1}}};try{new C({[Symbol.iterator](){return iterator}})}catch(e){if(e===marker&&closed===1&&next===1)n++}}}const r=n;",
            "r"
        ),
        "9"
    );
}

/** IteratorStepValue failures are propagated directly; constructor insertion never began and must not close. */
#[test]
fn failing_next_done_or_value_does_not_trigger_constructor_iterator_close() {
    assert_eq!(
        common::eval(
            "let n=0;for(const Base of [Map,Set,WeakMap,WeakSet]){for(const mode of ['next','done','value']){const marker={};let closed=0;const iterator={next(){if(mode==='next')throw marker;return{get done(){if(mode==='done')throw marker;return false},get value(){throw marker}}},return(){closed++;return{}}};try{new Base({[Symbol.iterator](){return iterator}})}catch(e){if(e===marker&&closed===0)n++}}}const r=n;",
            "r"
        ),
        "12"
    );
}

/** Each family invokes its captured native-compatible adder with the correct receiver and value arity. */
#[test]
fn collection_constructor_families_keep_their_brands_and_captured_adder() {
    assert_eq!(
        common::eval(
            "let n=0;for(const Base of [Map,Set,WeakMap,WeakSet]){const mapped=Base===Map||Base===WeakMap;const name=mapped?'set':'add',key={},second={};const original=Base.prototype[name];class C extends Base{}const calls=[];C.prototype[name]=function(...args){calls.push([this instanceof C,args.length]);C.prototype[name]=function(){throw 'must use captured adder'};return original.apply(this,args)};const source=mapped?[[key,7],[second,9]]:[key,second];const result=new C(source);if(result.has(key)&&result.has(second)&&calls.length===2&&calls.every(c=>c[0]&&c[1]===(mapped?2:1)))n++}const r=n;",
            "r"
        ),
        "4"
    );
}

/** Key getter results and original exceptions remain live while later property and cleanup callbacks collect. */
#[cfg(feature = "conformance")]
#[test]
fn streaming_constructor_roots_key_values_through_gc_callbacks() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const entry={get 0(){return {alive:7}},get 1(){gc();return 9}};const map=new Map([entry]);const key=map.keys().next().value;let same=false;class C extends Map{set(){throw {alive:13}}}const iterator={next(){return{value:[1,2]}},return(){gc();return {}}};try{new C({[Symbol.iterator](){return iterator}})}catch(e){same=e.alive===13}const r=[map.get(key),key.alive,same].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "9,7,true");
}
