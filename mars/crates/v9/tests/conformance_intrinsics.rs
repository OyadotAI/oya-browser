//! The opt-in shell native grammar executes only implemented operations on genuine VM state.
#![cfg(feature = "conformance")]
mod common;

/** V8's numeric hole-NaN test value preserves its bits without becoming a V9 missing element. */
#[test]
fn native_hole_nan_preserves_bits_without_becoming_a_hole() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let n=%GetHoleNaN(),a=[n],v=new DataView(new ArrayBuffer(8));v.setFloat64(0,n,false);let r=[Number.isNaN(a[0]),0 in a,v.getUint32(0,false)===%GetHoleNaNUpper(),v.getUint32(4,false)===%GetHoleNaNLower(),%IsSmi(n)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,true,false");
}

/** Internalized-string tests inspect actual backing and distinguish thin aliases. */
#[test]
fn native_internalized_string_test_reads_real_representation() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let interned=%ConstructInternalizedString('internalized-example'),thin=%ConstructThinString('thin-string-long-enough');let r=[%IsInternalizedString(interned),%IsInternalizedString(thin),%IsInternalizedString('literal'),%IsInternalizedString({})].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,false,true,false");
}

/** ConstructDouble preserves exact IEEE-754 bits and uses actual Smi or heap-number backing. */
#[test]
fn native_construct_double_preserves_bits_and_numeric_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let min=%ConstructDouble(0,1),zero=%ConstructDouble(0,0),negative=%ConstructDouble(0x80000000,0),nan=%ConstructDouble(0x7ff80000,123);let view=new DataView(new ArrayBuffer(8));view.setFloat64(0,nan,false);let r=[min===Number.MIN_VALUE,%IsSmi(zero),%IsSmi(negative),Object.is(negative,-0),Number.isNaN(nan),view.getUint32(4,false)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,false,true,true,123");
}

/** Native conversion helpers use ordinary coercion and preserve existing numeric identity. */
#[test]
fn native_conversion_helpers_preserve_coercion_and_number_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let order='';let x={[Symbol.toPrimitive](hint){order+=hint;return 7n}};let n=%AllocateHeapNumberWithValue(1.5);let same=%IsSameHeapObject(n,%ToNumber(n))&&%IsSameHeapObject(n,%ToNumeric(n));let bad=false;try{%ToNumber(x)}catch(e){bad=e instanceof TypeError}let r=[same,bad,%ToNumeric(x),%ToString({toString(){return 'ok'}}),order].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,7,ok,numbernumber");
}

/** Native Call uses normal call semantics and evaluates its operands once in order. */
#[test]
fn native_call_preserves_receiver_arguments_and_evaluation_order() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let order='';const receiver={x:10};function f(a,b){'use strict';return this.x+a+b}let r=%Call((order+='f',f),(order+='t',receiver),(order+='a',2),(order+='b',3));let bad=false;try{%Call(1,0)}catch(e){bad=e instanceof TypeError}r=[r,order,bad].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "15,ftab,true");
}

/** Comparison intrinsics order real UTF-16 code units without coercing their operands. */
#[test]
fn native_string_comparisons_follow_utf16_order() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let a='abc\\u0102',b='abc\\u0201';let r=[%StringLessThan(a,b),%StringLessThanOrEqual(a,a),%StringGreaterThan(b,a),%StringGreaterThanOrEqual(a,a),%StringEqual(a,a),%StringCompare(a,b),%StringCompare(a,a),%StringCompare(b,a)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,true,true,-1,0,1");
}

/** Compact large spreads retain the canonical character identity across GC and saved state. */
#[test]
fn large_string_spread_roots_survive_gc_and_snapshot() {
    let sources = vec!["let a=[...'ab'.repeat(150000)];let same=%IsSameHeapObject(a[0],'a')&&%IsSameHeapObject(a[1],'b');gc();setTimeout(()=>console.log(a.length,same,%IsSameHeapObject(a[299998],'a')),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 1.0, 10).unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::console(&resumed), ["300000 true true"]);
}

/** One-byte iterator results use an isolate-owned read-only cell that survives saved state. */
#[test]
fn one_byte_iterator_roots_keep_identity_through_gc_and_snapshot() {
    let sources = vec!["let a='ab'[Symbol.iterator]().next().value,b='ac'[Symbol.iterator]().next().value;let same=[b,'a','ab'[0],Object('ab')[0],String.fromCharCode(97),String.fromCodePoint(97),'ab'.at(0),'ab'.charAt(0),'ab'.slice(0,1),'ab'.substring(0,1),'ab'.substr(0,1),'a b'.split(' ')[0],' a '.trim()].every(x=>%IsSameHeapObject(a,x));gc();setTimeout(()=>console.log(same,%IsSameHeapObject(a,'ax'[Symbol.iterator]().next().value),%InYoungGeneration(a)),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 1.0, 10).unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::console(&resumed), ["true true false"]);
}

/** Saved character roots cannot be relabeled to grant a forged read-only identity. */
#[test]
fn one_byte_iterator_roots_reject_forged_unit() {
    let sources = vec!["let a='ab'[Symbol.iterator]().next().value;setTimeout(()=>{},0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut state = serde_json::to_value(loaded.state).unwrap();
    let roots = state["page"]["character_roots"].as_object_mut().unwrap();
    let root = roots.remove("97").unwrap();
    roots.insert("98".into(), root);
    let forged = serde_json::from_value(state).unwrap();
    assert!(mars_v9::run_timers(common::input(&sources, &host, &environment), forged, 1.0, 1).is_err());
}

/** Shell arguments remain strings and retain the same mutable array through state transport. */
#[test]
fn shell_arguments_survive_state_transport() {
    let sources = vec!["arguments.push('later');setTimeout(()=>console.log(arguments.join(',')),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_script_arguments = vec!["1".into(), "--literal".into()];
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 1.0, 10).unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::console(&resumed), ["1,--literal,later"]);
}

/** A pending reaction retains its captured isolate value across collection and state transport. */
#[test]
fn continuation_data_survives_gc_and_pending_reaction_snapshot() {
    let sources = vec!["const {getContinuationPreservedEmbedderData:get,setContinuationPreservedEmbedderData:set}=d8.getExtrasBindingObject();let resolve;const p=new Promise(r=>resolve=r);let obj={value:42};set(obj);p.then(()=>console.log(get().value));set('outer');obj=null;gc();setTimeout(()=>{resolve();console.log(get())},0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 1.0, 10).unwrap();
    assert!(resumed.failed.is_empty(), "{:?}", common::failure(&resumed));
    assert_eq!(common::console(&resumed), ["outer", "42"]);
}

/** Queueing a callback captures its data, including undefined, and restores its caller afterward. */
#[test]
fn continuation_data_follows_queued_microtasks() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const {getContinuationPreservedEmbedderData:get,setContinuationPreservedEmbedderData:set}=d8.getExtrasBindingObject();let values=[];queueMicrotask(()=>values.push(get()));set('captured');queueMicrotask(()=>{values.push(get());set('inside')});set('outer');%PerformMicrotaskCheckpoint();let r=[values[0]===undefined,values[1],get()].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,captured,outer");
}

/** A checkpoint drains reactions queued by async callbacks before returning to script. */
#[test]
fn checkpoint_drains_async_rejections_created_by_reactions() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let order=[];async function first(){try{await Promise.reject(1)}catch(e){order.push('first')}}first();async function f(){order.push('start');try{await Promise.reject(1)}catch(e){order.push('caught')}}Promise.resolve().then(f);%PerformMicrotaskCheckpoint();let r=order.join(',');";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "first,start,caught");
}

/** Native detachment updates real buffers and views, including repeated detachment. */
#[test]
fn detach_intrinsic_invalidates_real_buffer_views() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let execution = common::load_with(
        "const b=new ArrayBuffer(4),a=new Uint8Array(b);a[0]=7;%ArrayBufferDetach(b);%ArrayBufferDetach(b);const r=[b.byteLength,a.byteLength,a[0],%Is64Bit()].join();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        format!("0,0,,{}", cfg!(target_pointer_width = "64"))
    );
}

/** The optional detach key is checked before modifying the actual buffer. */
#[test]
fn detach_intrinsic_checks_optional_key_before_mutation() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const b=new ArrayBuffer(4);let r='';try{%ArrayBufferDetach(b,'wrong')}catch(e){r=[e instanceof TypeError,b.byteLength].join()}%ArrayBufferDetach(b,undefined);r+=','+b.byteLength;";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,4,0");
}

/** Ordinary pages reject native grammar, including when the host supplies an index without shell mode. */
#[test]
fn native_grammar_requires_shell_and_explicit_script_index() {
    let source = "const result=%Is64Bit();";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    assert!(common::load_with(source, &environment).failed.is_empty());
    environment.conformance_d8 = false;
    environment.conformance_native_main_source = Some(0);
    assert!(!common::load_with(source, &environment).failed.is_empty());
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = None;
    assert!(!common::load_with(source, &environment).failed.is_empty());
}

/** A real unknown intrinsic fails rather than being guessed at. */
#[test]
fn unknown_intrinsics_fail_explicitly() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let failure = common::load_with("%HasSmiElements([]);", &environment);
    assert!(!failure.failed.is_empty());
}

/** Optimizer hints evaluate their arguments, answer undefined, and the status reports V8's own
never-optimize bits (kIsFunction | kNeverOptimize) so mjsunit's assertUnoptimized holds. */
#[test]
fn optimizer_hints_are_inert_and_status_never_optimizes() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let calls=0;function f(){calls++;return f}const out=[%PrepareFunctionForOptimization(f()),%OptimizeFunctionOnNextCall(f(),'concurrent'),%DeoptimizeNow(),%GetOptimizationStatus(f),%GetOptimizationStatus(undefined,''),%IsMaglevEnabled(),%IsBeingInterpreted(),%DebugPrint(7),calls];const r=out.join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), ",,,3,2,false,true,7,2");
}

/** Inputs other than detachable ordinary buffers cannot grant detachment authority. */
#[test]
fn detach_rejects_non_buffers_and_protected_wasm_memory() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const out=[];for(const value of [{},new SharedArrayBuffer(2),new WebAssembly.Memory({initial:1}).buffer]){try{%ArrayBufferDetach(value);out.push('bad')}catch(e){out.push(e instanceof TypeError)}}const r=out.join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true");
}

/** ToLength executes real numeric coercion, clamping, and failure propagation. */
#[test]
fn to_length_observes_conversion_and_bounds() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let hint;const value={[Symbol.toPrimitive](h){hint=h;return 12.9}};const out=[%ToLength(value),hint,%ToLength(-1),%ToLength(Infinity)];try{%ToLength(Symbol())}catch(e){out.push(e instanceof TypeError)}const r=out.join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "12,number,0,9007199254740991,true"
    );
}

/** The waiter intrinsic reads live mutex and condition queues and rejects foreign values. */
#[test]
fn synchronization_waiter_intrinsic_reads_real_branded_queues() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const m=new Atomics.Mutex(),c=new Atomics.Condition();let r=[%AtomicsSynchronizationPrimitiveNumWaitersForTesting(m),%AtomicsSynchronizationPrimitiveNumWaitersForTesting(c)].join();try{%AtomicsSynchronizationPrimitiveNumWaitersForTesting({})}catch(e){r+=','+(e instanceof TypeError)}";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0,0,true");
}

/** Waiter counts distinguish queued addresses from notified promises awaiting delivery. */
#[test]
fn atomics_waiter_intrinsics_report_actual_queue_state() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const a=new Int32Array(new SharedArrayBuffer(16));const wait=Atomics.waitAsync(a,1,0);let r=[%AtomicsNumWaitersForTesting(a,0),%AtomicsNumWaitersForTesting(a,1),%AtomicsNumUnresolvedAsyncPromisesForTesting(a,1)].join();Atomics.notify(a,1);r+=','+[%AtomicsNumWaitersForTesting(a,1),%AtomicsNumUnresolvedAsyncPromisesForTesting(a,1)].join();try{%AtomicsNumWaitersForTesting({},0)}catch(e){r+=','+(e instanceof TypeError)}";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0,1,0,0,1,true");
}

/** Disallowing waits persists across state transport and can be explicitly reversed. */
#[test]
fn atomics_wait_permission_survives_state_transport() {
    let sources = vec!["const a=new Int32Array(new SharedArrayBuffer(4));%SetAllowAtomicsWait(false);setTimeout(()=>{let denied=false;try{Atomics.wait(a,0,0,0)}catch(e){denied=e instanceof TypeError}let before=%IsAtomicsWaitAllowed();%SetAllowAtomicsWait(true);console.log(denied,before,%IsAtomicsWaitAllowed(),Atomics.wait(a,0,0,0))},0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        10,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true false true timed-out"]);
}

/** Asynchronous GC removes an unreachable symbol while the outer weak-map key stays live. */
#[test]
fn weak_collection_size_tracks_async_symbol_collection() {
    let sources = vec!["let r='pending';(()=>{const map=new WeakMap(),outer=Symbol('outer');map.set(outer,1);(()=>{const inner=Symbol('inner');map.set(inner,1);if(!map.has(inner)||map.get(inner)!==1)throw Error('missing')})();(async()=>{await gc({type:'major',execution:'async'});r=[map.has(outer),%GetWeakCollectionSize(map)].join()})()})();".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let delivered = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        10,
    )
    .unwrap();
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::global(&delivered, "r"), "true,1");
}

/** Live weak keys survive, while unreachable ephemeron cycles and set keys are removed after a snapshot. */
#[test]
fn weak_collection_sweep_preserves_live_keys_and_drops_cycles() {
    let sources = vec!["let r='pending';const map=new WeakMap(),set=new WeakSet(),live={};map.set(live,42);set.add(live);(()=>{const key={},value={key};map.set(key,value);set.add(key)})();setTimeout(async()=>{await gc({type:'major',execution:'async'});r=[map.get(live),set.has(live),%GetWeakCollectionSize(map),%GetWeakCollectionSize(set)].join()},0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let started = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        10,
    )
    .unwrap();
    let delivered = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(started.state),
        2.0,
        10,
    )
    .unwrap();
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::global(&delivered, "r"), "42,true,1,1");
}

/** Only the explicit shared-string table shares literal storage; computed strings remain ordinary. */
#[test]
fn shared_literal_provenance_is_gated_and_survives_state_transport() {
    let sources = vec!["const literal='shared literal';const dynamic=['shared',' literal'].join('');setTimeout(()=>console.log(%IsSharedString(literal),%IsSharedString(dynamic)),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    environment.conformance_shared_string_table = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let delivered = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        1.0,
        10,
    )
    .unwrap();
    assert!(
        delivered.failed.is_empty(),
        "{:?}",
        common::failure(&delivered)
    );
    assert_eq!(common::console(&delivered), ["true false"]);
}

/** The shared table is opt-in and equal live shared literals retain actual backing identity. */
#[test]
fn shared_literals_use_real_interning_only_when_enabled() {
    let source = "const a='shared';const b='sha'+'red';const c='shared';const r=[%IsSharedString(a),%IsSharedString(b),%IsSameHeapObject(a,c)].join();";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let ordinary = common::load_with(source, &environment);
    assert!(
        ordinary.failed.is_empty(),
        "{:?}",
        common::failure(&ordinary)
    );
    assert_eq!(common::global(&ordinary, "r"), "false,false,true");
    environment.conformance_shared_string_table = true;
    let shared = common::load_with(source, &environment);
    assert!(shared.failed.is_empty(), "{:?}", common::failure(&shared));
    assert_eq!(common::global(&shared, "r"), "true,false,true");
}

/** V9 does not pretend unboxed numbers have V8 heap object identity. */
#[test]
fn heap_identity_rejects_unboxed_primitive_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let execution = common::load_with("%IsSameHeapObject(1,1);", &environment);
    assert!(!execution.failed.is_empty());
}

/** A shared collection really sweeps unreachable weak keys while preserving live shared strings. */
#[test]
fn shared_gc_runs_real_local_collection_without_workers() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    environment.conformance_shared_string_table = true;
    let source = "const map=new WeakMap();(()=>{const key={};map.set(key,1)})();const s='shared';const ret=%SharedGC();const r=[%GetWeakCollectionSize(map),%IsSharedString(s),ret===undefined].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0,true,true");
}

/** Initializer inspection returns the actual own field function and does not inherit a base's initializer. */
#[test]
fn class_initializer_intrinsic_exposes_own_actual_function() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "class A{x=1}class B extends A{}class C{}const f=%GetInitializerFunction(A);const r=[typeof f,f===%GetInitializerFunction(A),%GetInitializerFunction(B),%GetInitializerFunction(C),new A().x].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "function,true,,,1");
}

/** The maximum-length intrinsic reports V9's actual UTF-16 string bound. */
#[test]
fn string_max_length_intrinsic_reports_the_engine_limit() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let execution = common::load_with("const r=%StringMaxLength();", &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        mars_v9::constants::JS_STRING_UNITS.to_string()
    );
}

/** Forced Wasm detachment replaces backing without losing bytes or resizable metadata. */
#[test]
fn force_detach_preserves_wasm_memory_and_invalidates_old_views() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const m=new WebAssembly.Memory({initial:1,maximum:3}),old=m.toResizableBuffer(),view=new Uint8Array(old);view[0]=42;%ArrayBufferDetachForceWasm(old);const b=m.buffer;const r=[old.detached,view.length,b!==old,b.resizable,b.maxByteLength,new Uint8Array(b)[0],m.grow(1),new Uint8Array(m.buffer)[0]].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true,0,true,true,196608,42,1,42"
    );
}

/** Forced detachment refuses shared storage and incompatible receivers. */
#[test]
fn force_detach_rejects_shared_buffers_and_non_buffers() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let errors=0;for(const value of [{},new SharedArrayBuffer(8)])try{%ArrayBufferDetachForceWasm(value)}catch(e){errors+=e instanceof TypeError}const r=errors;";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "2");
}

/** Wasm exception introspection preserves the real raw scalar encoding and reference identity. */
#[test]
fn wasm_exception_values_use_real_payload_bits() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const tag=new WebAssembly.Tag({parameters:['i32','i64','f32','f64','externref']}),ref={};const e=new WebAssembly.Exception(tag,[424242,-1n,5,5,ref]);const a=%GetWasmExceptionValues(e);const r=[a.slice(0,12).join(':'),a[12]===ref,a!==%GetWasmExceptionValues(e)].join('|');";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "6:31026:65535:65535:65535:65535:16544:0:16404:0:0:0|true|true"
    );
}

/** Raw NaN payload bits are not fabricated when V9 cannot recover their original representation. */
#[test]
fn wasm_exception_raw_nan_bits_report_unsupported() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const tag=new WebAssembly.Tag({parameters:['f32']});const e=new WebAssembly.Exception(tag,[NaN]);%GetWasmExceptionValues(e);";
    let execution = common::load_with(source, &environment);
    assert!(!execution.effects.unsupported.is_empty() || !execution.failed.is_empty());
}

/** Only the exact Wasm exception fixture receives native grammar, and only when explicitly enabled. */
#[test]
fn wasm_exception_fixture_native_grammar_is_explicitly_gated() {
    let host = common::host();
    for (path, enabled, succeeds) in [
        ("test/mjsunit/wasm/exceptions-utils.js", true, true),
        ("test/mjsunit/wasm/exceptions-utils.js", false, false),
        ("test/mjsunit/wasm/other.js", true, false),
    ] {
        let source = "globalThis.r=%Is64Bit();";
        let modules = mars_v9::ModuleInput {
            sources: vec![mars_v9::ModuleSource {
                url: path.into(),
                source: Some(source.into()),
                classic: true,
            }],
            ..Default::default()
        };
        let mut environment = common::environment();
        environment.conformance_d8 = true;
        environment.conformance_native_wasm_exceptions_helper = enabled;
        environment
            .conformance_sources
            .insert(path.into(), source.into());
        let sources = vec![format!("d8.file.execute('{path}');")];
        let execution = mars_v9::load_page(mars_v9::PageInput {
            modules: &modules,
            ..common::input(&sources, &host, &environment)
        })
        .unwrap();
        assert_eq!(
            execution.failed.is_empty(),
            succeeds,
            "{:?}",
            common::failure(&execution)
        );
    }
}

/** Internal V8 predicates inspect actual object kinds and do not invoke proxy traps. */
#[test]
fn object_predicates_do_not_unwrap_proxies() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const p=Proxy.revocable([],{});p.revoke();const r=[%IsJSReceiver(p.proxy),%IsJSReceiver(1),%IsArray([]),%IsArray(Array.prototype),%IsArray(p.proxy),%ToBoolean(0),%ToBoolean('x')].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true,false,true,true,false,false,true"
    );
}

/** Native typeof, property-key conversion and iterator result creation reuse actual language semantics. */
#[test]
fn native_conversion_and_iterator_results_keep_value_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const symbol=Symbol(),item={};let hint;const key={[Symbol.toPrimitive](h){hint=h;return symbol}},result=%CreateIterResultObject(item,2);const r=[%Typeof(2n),%ToName(key)===symbol,hint,result.value===item,result.done,Object.keys(result).join(':')].join();";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "bigint,true,string,true,true,value:done"
    );
}

/** Weak-collection sizes report the real live table, including deletions. */
#[test]
fn weak_collection_size_counts_live_entries() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const key={},map=new WeakMap(),set=new WeakSet();map.set(key,1);set.add(key);let r=[%GetWeakCollectionSize(map),%GetWeakCollectionSize(set)].join();map.delete(key);r+=','+%GetWeakCollectionSize(map);try{%GetWeakCollectionSize(new Map())}catch(e){r+=','+(e instanceof TypeError)}";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "1,1,0,true");
}


#[test]
fn shared_number_behaves_as_a_number() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(1.5);let r=[typeof x,x===1.5,x==1.5,x+2,x*2,Object.is(x,1.5),Number.isFinite(x),Number.isNaN(x),JSON.stringify(x),new Map([[x,7]]).get(1.5),%IsInWritableSharedSpace(x)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "number,true,true,3.5,3,true,true,false,1.5,7,true");
}

#[test]
fn shared_number_boxing_preserves_its_real_backing() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(1.5), a=Object(x),b=new Number(x);gc();let r=[%IsSameHeapObject(x,Number(x)),%IsSameHeapObject(x,Number(a)),%IsSameHeapObject(x,a.valueOf()),%IsSameHeapObject(x,b.valueOf()),a.valueOf()===1.5].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,true,true");
}

#[test]
fn share_object_rejects_unshareable_values_with_type_error() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let r=[];for(const value of [{},[],1n,Symbol('s')]){try{%ShareObject(value);r.push(false)}catch(e){r.push(e instanceof TypeError)}}r=r.join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,true");
}

#[test]
fn share_object_fuzzing_handles_unshareable_values_and_extra_arguments() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_fuzzing = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let side=0;let r=[%ShareObject()===undefined,%ShareObject({})===undefined,%ShareObject(1,++side)===1,side].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,1");
}

#[test]
fn explicit_shared_strings_shares_values_without_interning_all_literals() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const plain='share me',shared=%ShareObject(plain);let r=[%IsSharedString(plain),%IsSharedString(shared),plain===shared].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "false,true,true");
}

#[test]
fn shared_number_pressure_reclaims_only_unreachable_backings() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const keep=%ShareObject(1.5);for(let i=0;i<5000;i++){let temporary=%ShareObject(i+0.5)}let r=[keep,%IsInWritableSharedSpace(keep)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "1.5,true");
}

#[test]
fn shared_number_works_across_native_boundaries() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(1.5);let r=[x.toFixed(1),Math.max(x,2),new Uint8Array([x])[0],JSON.stringify({v:x}),[0,1,2].indexOf(x),Number.isInteger(x),x<2,Boolean(x),String(x)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "1.5,2,1,{\"v\":1.5},-1,false,true,true,1.5");
}

#[test]
fn shared_number_is_accepted_by_numeric_replacer_and_function_length() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(2.0+0.5),f=()=>{};Object.defineProperty(f,'length',{value:x});let r=[f.bind(null).length,JSON.stringify({'2.5':7,a:1},[x]),JSON.stringify({a:1},null,x)].join('|');";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "2|{\"2.5\":7}|{\n  \"a\": 1\n}");
}

#[test]
fn shared_number_obeys_single_number_array_constructor_rules() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(1.5);let r=false;try{new Array(x)}catch(e){r=e instanceof RangeError}";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true");
}

#[test]
fn shared_number_converts_to_bigint_and_exact_sum() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(4294967296);let r=[BigInt(x),Math.sumPrecise([x,1]),+x===x].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "4294967296,4294967297,true");
}

#[test]
fn shared_number_survives_shared_container_and_atomic_comparison() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(1.5),a=new SharedArray(1);a[0]=x;let r=[%IsSameHeapObject(a[0],x),Atomics.compareExchange(a,0,1.5,2),a[0]].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,1.5,2");
}

#[test]
fn shared_number_atomic_comparison_uses_numeric_equality() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const a=new SharedArray(1),n=%ShareObject(NaN),z=%ShareObject(-0),x=%ShareObject(1.5),y=%ShareObject(1.5);a[0]=n;Atomics.compareExchange(a,0,n,9);let r=Number.isNaN(a[0]);a[0]=z;Atomics.compareExchange(a,0,0,7);r+=','+a[0];a[0]=x;Atomics.compareExchange(a,0,y,8);r+=','+a[0];";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,7,8");
}

#[test]
fn shared_number_root_survives_collection_and_store_reads() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%ShareObject(1.5),a=new SharedArray(1);a[0]=x;gc();%SharedGC();let r=[x,a[0],%IsSameHeapObject(x,a[0])].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "1.5,1.5,true");
}

#[test]
fn shared_number_preserves_nan_zero_and_collection_semantics() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const n=%ShareObject(NaN),z=%ShareObject(-0),i=%ShareObject(Infinity);let r=[n===n,Number.isNaN(n),Object.is(n,NaN),Object.is(z,-0),Object.is(z,0),1/z,JSON.stringify([n,z,i]),new Set([n,NaN,z,0]).size,z==0n,z===0,Object.is(new Map([[z,1]]).keys().next().value,0)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "false,true,true,true,false,-Infinity,[null,0,null],2,true,true,true");
}

#[test]
fn shared_number_preserves_identity_across_state_transport() {
    let sources = vec!["const x=%ShareObject(1.5), y=x,box=new Number(x);setTimeout(()=>console.log(%IsSameHeapObject(x,y),%IsSameHeapObject(x,box.valueOf())),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let delivered = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 1.0, 10).unwrap();
    assert!(delivered.failed.is_empty(), "{:?}", common::failure(&delivered));
    assert_eq!(common::console(&delivered), ["true true"]);
}

#[test]
fn shared_number_rejects_forged_snapshot_payload() {
    fn tamper(value: &mut serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Array(fields)) = map.get_mut("q") {
                    if fields.len() == 2 { fields[0] = serde_json::json!(0); return true; }
                }
                map.values_mut().any(tamper)
            }
            serde_json::Value::Array(values) => values.iter_mut().any(tamper),
            _ => false,
        }
    }
    let sources = vec!["const x=%ShareObject(1.5);setTimeout(()=>console.log(x),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_shared_strings = true;
    environment.conformance_native_main_source = Some(0);
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    assert!(tamper(&mut wire));
    let state = serde_json::from_value(wire).unwrap();
    assert!(mars_v9::run_timers(common::input(&sources, &host, &environment), state, 1.0, 10).is_err());
}


#[test]
fn external_string_resources_preserve_aliases_encodings_and_snapshots() {
    let source = "const one=createExternalizableString('abc'),two=createExternalizableTwoByteString('abc'),alias=two;externalizeString(one);externalizeString(two);externalizeString(alias);let r=[isOneByteString(one),isOneByteString(two),two===alias,two].join();setTimeout(()=>console.log(isOneByteString(two),two===alias),0);";
    let sources = vec![source.to_string()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_expose_externalize_string = true;
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    assert_eq!(common::global(&loaded, "r"), "true,false,true,abc");
    let delivered = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), 1.0, 10).unwrap();
    assert!(delivered.failed.is_empty(), "{:?}", common::failure(&delivered));
    assert_eq!(common::console(&delivered), ["false true"]);
}

#[test]
fn local_heap_number_identity_and_numeric_semantics() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%AllocateHeapNumber(), y=x, z=%AllocateHeapNumber();let r=[%IsSameHeapObject(x,y),%IsSameHeapObject(x,z),typeof x,x===0,x==0,x+2,x*3,Object.is(x,0),JSON.stringify(x),new Map([[x,7]]).get(0),%InYoungGeneration(x)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,false,number,true,true,2,0,true,0,7,true");
}

#[test]
fn local_heap_number_identity_survives_collection_and_snapshot() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["const x=%AllocateHeapNumber(),y=x;gc();let r='';setTimeout(()=>r=[%IsSameHeapObject(x,y),%InYoungGeneration(x),x+1].join(),0);".to_string()];
    let host = common::host();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(execution.state), execution.effects.next_timer_ms.unwrap(), 1).unwrap();
    assert_eq!(common::global(&resumed, "r"), "true,false,1");
}

#[test]
fn local_heap_number_numeric_consumers_and_boxing() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const x=%AllocateHeapNumber(),a=Object(x),b=new Number(x);let r=[%IsSameHeapObject(x,Number(x)),%IsSameHeapObject(x,a.valueOf()),%IsSameHeapObject(x,b.valueOf()),x.toFixed(1),Math.max(x,2),new Uint8Array([x])[0],JSON.stringify({v:x}),Number.isFinite(x),Number.isInteger(x),x<2,Boolean(x),String(x),BigInt(x)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,0.0,2,0,{\"v\":0},true,true,true,false,0,0");
}

/** A saved local-number identity cannot be forged to point at another heap object. */
#[test]
fn local_heap_number_rejects_forged_snapshot() {
    fn tamper(value: &mut serde_json::Value) -> bool {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(serde_json::Value::Array(fields)) = map.get_mut("Q") {
                    if fields.len() == 2 { fields[0] = serde_json::json!(1); return true; }
                }
                map.values_mut().any(tamper)
            }
            serde_json::Value::Array(values) => values.iter_mut().any(tamper),
            _ => false,
        }
    }
    let sources = vec!["const x=%AllocateHeapNumber();setTimeout(()=>console.log(x),0);".into()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut wire = serde_json::to_value(execution.state).unwrap();
    assert!(tamper(&mut wire));
    let state = serde_json::from_value(wire).unwrap();
    assert!(mars_v9::run_timers(common::input(&sources, &host, &environment), state, 1.0, 1).is_err());
}

#[test]
fn shared_slot_smi_tag_survives_numeric_operations_and_snapshot() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["const a=new SharedArray(2);a[0]=0;a[1]=-7;let r='';setTimeout(()=>{const x=a[0],y=a[1];r=[%IsSmi(x),%IsSmi(y),x===0,y===-7,x+2,y*2,Object.is(x,-0),Object.is(x,0),Number.isInteger(y),new Map([[x,7]]).get(0),JSON.stringify([x,y])].join('|')},0);".to_string()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), loaded.effects.next_timer_ms.unwrap(), 1).unwrap();
    assert_eq!(common::global(&resumed, "r"), "true|true|true|true|2|-14|false|true|true|7|[0,-7]");
}

#[test]
fn tagged_smi_behaves_as_number_at_native_boundaries() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const a=SharedArray(2);a[0]=0;a[1]=2;const x=a[0],y=a[1];let r=[typeof x,x.toFixed(1),Math.max(x,3),new Uint8Array([y])[0],Number.isFinite(y),new Array(y).length,BigInt(y),JSON.stringify({v:x}),JSON.stringify({0:1,2:3},[y]),x==0n,x===0,Object.is(x,-0),[y].indexOf(2),({2:'yes'})[y]].join('|');";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "number|0.0|3|2|true|2|2|{\"v\":0}|{\"2\":3}|true|true|false|0|yes");
}

#[test]
fn shared_atomic_number_replacements_are_published_with_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const a=SharedArray(1),n=%AllocateHeapNumber();a[0]=2;const old=Atomics.exchange(a,0,1.5),x=a[0];let r=[old===2,x===1.5,%IsSameHeapObject(x,a[0]),Atomics.compareExchange(a,0,1.5,n)===1.5,a[0]===0,%IsSameHeapObject(a[0],n),%IsSameHeapObject(a[0],a[0])].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,true,true,false,true");
}

#[test]
fn tagged_smi_snapshot_is_stable_and_rejects_out_of_range() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_harmony_struct = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["const a=SharedArray(1);a[0]=7;setTimeout(()=>0,0);".to_string()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let bytes = serde_json::to_vec(&loaded.state).unwrap();
    let restored: mars_v9::VmState = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(bytes, serde_json::to_vec(&restored).unwrap());
    assert!(serde_json::from_str::<mars_v9::JsValue>(r#"{"M":1073741824}"#).is_err());
    assert!(serde_json::to_value(mars_v9::JsValue::Smi(1_073_741_824.0)).is_err());
}

#[test]
fn tagged_small_integer_arithmetic_keeps_fast_path_provenance() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let x=1,y=2;let r=[%IsSmi(x+y),%IsSmi(y-x),%IsSmi(x*y),%IsSmi(-x),%IsSmi(~x),%IsSmi(++y),%IsSmi(y/x),%IsSmi(y%x),%IsSmi(y&x),%IsSmi(y<<x),%IsSmi(0*-1),Object.is(0*-1,-0),%IsSmi(1073741823+1)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,true,true,true,true,true,true,true,true,false,true,false");
}

/** Heap number creation keeps real aliases and runs authored numeric conversion exactly once. */
#[test]
fn heap_number_with_value_preserves_identity_and_numeric_coercion() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_fuzzing = true;
    environment.conformance_native_main_source = Some(0);
    let source = "let gets=0;const x=%AllocateHeapNumberWithValue(42),y=%AllocateHeapNumberWithValue(x),z=%AllocateHeapNumberWithValue({valueOf(){gets++;return '123'}}),nan=%AllocateHeapNumberWithValue('not a number');let bad=false;try{%AllocateHeapNumberWithValue(Symbol.iterator)}catch(e){bad=e instanceof TypeError}let side=0;const absent=%AllocateHeapNumberWithValue(),extra=%AllocateHeapNumberWithValue(1,++side);let r=[x===42,%IsSameHeapObject(x,y),%IsSmi(x),z===123,gets,Number.isNaN(nan),bad,absent===undefined,extra===undefined,side].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,false,true,1,true,true,true,true,1");
}

/** Non-finite boxed values survive the same public state boundary as ordinary numbers. */
#[test]
fn heap_number_with_nonfinite_value_survives_snapshot() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["const x=%AllocateHeapNumberWithValue('NaN');let r='';setTimeout(()=>r=[Number.isNaN(x),%IsSameHeapObject(x,x)].join(),0);".to_string()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), loaded.effects.next_timer_ms.unwrap(), 1).unwrap();
    assert_eq!(common::global(&resumed, "r"), "true,true");
}

/** An untracked f64 cannot be mistaken for a heap allocation with known identity. */
#[test]
fn heap_number_with_value_refuses_untracked_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let execution = common::load_with("%AllocateHeapNumberWithValue(Math.sqrt(2));", &environment);
    assert!(common::failure(&execution).is_some_and(|message| message.contains("HeapNumber identity for untracked number")));
}


/** Bytecode number constants retain actual per-code allocation identity across GC and transport. */
#[test]
fn heap_number_literals_keep_per_code_constant_identity() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["function f(){return 1.5}function g(){return 1.5}const a=f(),b=f(),c=g(),d=%AllocateHeapNumberWithValue(a);gc();let r=0;setTimeout(()=>r=[%IsSameHeapObject(a,b),%IsSameHeapObject(a,c),%IsSameHeapObject(a,d),%InYoungGeneration(a),a+1].join(),0);".to_string()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), loaded.effects.next_timer_ms.unwrap(), 1).unwrap();
    assert_eq!(common::global(&resumed, "r"), "true,false,true,false,2.5");
}


/** Reused dynamic evaluation sites cannot alias old constants from different source text. */
#[test]
fn numeric_constant_sites_follow_dynamic_source_changes() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["const a=eval(\"1.5\"),b=eval(\"2.5\"),c=eval(\"1.5\");let r=0;setTimeout(()=>r=[a,b,c,%IsSameHeapObject(a,a),%IsSameHeapObject(b,b)].join(),0);".to_string()];
    let host = common::host();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(loaded.state), loaded.effects.next_timer_ms.unwrap(), 1).unwrap();
    assert_eq!(common::global(&resumed, "r"), "1.5,2.5,1.5,true,true");
}

/** Equal numeric literals within the same bytecode pool share one real old HeapNumber. */
#[test]
fn equal_heap_number_literals_share_their_function_pool() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "function f(){return [1.5,1.5]}const [a,b]=f();let r=[%IsSameHeapObject(a,b),%IsSameHeapObject(a,%AllocateHeapNumberWithValue(a)),%InYoungGeneration(a)].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "true,true,false");
}

/** Serialized number pools cannot claim a different compiled source fingerprint. */
#[test]
fn forged_numeric_constant_source_is_rejected() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["const x=1.5;setTimeout(()=>console.log(x),0);".to_string()];
    let host = common::host();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let summary = serde_json::to_value(execution.state.summary()).unwrap();
    assert!(summary["page"].get("number_constants").is_none());
    let mut wire = serde_json::to_value(execution.state).unwrap();
    let pools = wire["page"]["number_constants"].as_object_mut().unwrap();
    let pool = pools.values_mut().next().unwrap();
    let original = pool["source"][0].as_u64().unwrap();
    pool["source"][0] = serde_json::json!(original ^ 1);
    let state = serde_json::from_value::<mars_v9::VmState>(wire).unwrap();
    assert!(mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1).is_err());
}

/** Negative heap-number bit patterns remain exact through JSON map keys and state transport. */
#[test]
fn negative_heap_number_constant_survives_snapshot() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let sources = vec!["function f(){return -1.5}const x=f();let r=0;setTimeout(()=>r=[x,%IsSameHeapObject(x,f())].join(),0);".to_string()];
    let host = common::host();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let resumed = mars_v9::run_timers(common::input(&sources, &host, &environment), common::round_trip(execution.state), execution.effects.next_timer_ms.unwrap(), 1).unwrap();
    assert_eq!(common::global(&resumed, "r"), "-1.5,true");
}

/** Native limits report actual tagged and backing allocation boundaries without changing them. */
#[test]
fn native_numeric_and_buffer_limits_match_real_boundaries() {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_native_main_source = Some(0);
    let source = "const n=%MaxSmi(),cap=%ArrayBufferMaxByteLength();let ordinary=false,shared=false;try{new ArrayBuffer(cap+1)}catch(e){ordinary=e instanceof RangeError}try{new SharedArrayBuffer(cap+1)}catch(e){shared=e instanceof RangeError}const a=new ArrayBuffer(cap),b=new SharedArrayBuffer(cap);let r=[n,%IsSmi(n),%IsSmi(n+1),%IsSmi(cap),a.byteLength,b.byteLength,ordinary,shared].join();";
    let execution = common::load_with(source, &environment);
    assert!(execution.failed.is_empty(), "{:?}", common::failure(&execution));
    assert_eq!(common::global(&execution, "r"), "1073741823,true,false,true,2097152,2097152,true,true");
}
