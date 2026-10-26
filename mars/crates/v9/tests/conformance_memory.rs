/* Memory measurements execute real collection tasks and account the caller's permitted resident data. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{ConformanceWorkerSession, PageEnvironment};

/** Async tests must produce their final console result through actual promise/task delivery. */
const TASK_LIMIT: usize = 128;
/** Every state round uses the same source and session; no callback or collector is mocked. */
fn run(source: &str) -> Vec<String> {
    let session = ConformanceWorkerSession::new();
    let sources = vec![source.to_string()];
    let host = common::host();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let mut execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut console = Vec::new();
    for _ in 0..TASK_LIMIT {
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        assert!(
            execution.effects.conformance_error.is_none(),
            "{:?}",
            execution.effects.conformance_error
        );
        console.extend(common::console(&execution));
        let Some(due) = execution.effects.next_timer_ms else {
            session.finish().unwrap();
            return console;
        };
        execution = mars_v9::run_timers(
            common::input(&sources, &host, &environment),
            execution.state,
            due,
            1,
        )
        .unwrap();
    }
    panic!("Measurement tasks exceeded their host bound");
}

#[test]
/** Summary results contain real finite byte ranges without including detailed context fields. */
fn summary_measurements_have_bounded_numeric_fields() {
    assert_eq!(
        run(
            "performance.measureMemory().then(result=>console.log([Object.keys(result).join(','),typeof result.total.jsMemoryEstimate,Number.isFinite(result.total.jsMemoryEstimate),result.total.jsMemoryRange.length,result.total.jsMemoryRange[0]<=result.total.jsMemoryEstimate,result.total.jsMemoryEstimate<=result.total.jsMemoryRange[1],result.total.jsMemoryEstimate>0].join(':')));"
        ),
        ["total:number:true:2:true:true:true"]
    );
}
#[test]
/** Option getters happen synchronously, promise reactions precede the queued measurement task. */
fn measurements_run_after_the_current_microtask_checkpoint() {
    assert_eq!(
        run(
            "const order=[];const promise=performance.measureMemory({get detailed(){order.push('getter');return false}});order.push('sync');Promise.resolve().then(()=>order.push('microtask'));promise.then(()=>{order.push('measurement');console.log(order.join(':'))});"
        ),
        ["getter:sync:microtask:measurement"]
    );
}
#[test]
/** Detailed mode requires primitive true and cannot invoke arbitrary boolean conversion hooks. */
fn detailed_options_are_not_coerced() {
    assert_eq!(
        run(
            "performance.measureMemory({detailed:{valueOf(){throw Error('coerced')}}}).then(result=>console.log('current' in result));"
        ),
        ["false"]
    );
}
#[test]
/** A thrown detailed getter remains the same synchronous thrown value before a promise is created. */
fn option_getter_errors_preserve_identity() {
    assert_eq!(
        run(
            "const marker={};try{performance.measureMemory({get detailed(){throw marker}})}catch(error){console.log(error===marker)}"
        ),
        ["true"]
    );
}
#[test]
/** The estimate grows with actual resident values and releases unreachable storage after collection. */
fn resident_allocation_and_collection_change_the_estimate() {
    assert_eq!(
        run(
            "let held;(async()=>{const before=(await performance.measureMemory()).total.jsMemoryEstimate;held=new Array(8192).fill(3.1);const allocated=(await performance.measureMemory()).total.jsMemoryEstimate;held=null;const released=(await performance.measureMemory()).total.jsMemoryEstimate;console.log([allocated>before+65536,released<allocated].join(':'))})();"
        ),
        ["true:true"]
    );
}
#[test]
/** Logical holes at the uint32 boundary never inflate the estimate by billions of nonexistent values. */
fn sparse_logical_lengths_do_not_count_as_resident_memory() {
    assert_eq!(
        run(
            "let held;(async()=>{const before=(await performance.measureMemory()).total.jsMemoryEstimate;held=new Array(4294967295);const after=(await performance.measureMemory()).total.jsMemoryEstimate;console.log(after-before<1048576)})();"
        ),
        ["true"]
    );
}
#[test]
/** Shared backing data is counted once even when many wrappers and views retain it. */
fn shared_backing_payloads_are_deduplicated() {
    assert_eq!(
        run(
            "const backing=new SharedArrayBuffer(65536);const aliases=[];(async()=>{const before=(await performance.measureMemory()).total.jsMemoryEstimate;for(let index=0;index<64;index++)aliases.push(structuredClone(backing));const after=(await performance.measureMemory()).total.jsMemoryEstimate;console.log(after-before<32768)})();"
        ),
        ["true"]
    );
}
#[test]
/** Only contexts sharing the requesting realm's security token appear in detailed results. */
fn foreign_security_tokens_are_excluded() {
    assert_eq!(
        run(
            "const visible=Realm.createAllowCrossRealmAccess();const hidden=Realm.create();Realm.eval(visible,'globalThis.held=new Array(1024).fill(1)');Realm.eval(hidden,'globalThis.held=new Array(1024).fill(1)');performance.measureMemory({detailed:true}).then(result=>console.log([result.other.length,result.current.jsMemoryEstimate>0,result.other[0].jsMemoryEstimate>0].join(':')));"
        ),
        ["1:true:true"]
    );
}
#[test]
/** Disposed contexts vanish from the result once no object or callback retains them. */
fn disposed_unreachable_contexts_are_not_measured() {
    assert_eq!(
        run(
            "const other=Realm.createAllowCrossRealmAccess();Realm.eval(other,'globalThis.held=new Array(1024).fill(1)');(async()=>{const before=await performance.measureMemory({detailed:true});Realm.dispose(other);const after=await performance.measureMemory({detailed:true});console.log(before.other.length+':'+after.other.length)})();"
        ),
        ["1:0"]
    );
}
#[test]
/** Promise resolution reads the result's inherited then getter exactly once. */
fn measurement_resolution_observes_then_getters() {
    assert_eq!(
        run(
            "let reads=0;const promise=performance.measureMemory();Object.defineProperty(Object.prototype,'then',{get(){reads++;return undefined},configurable:true});promise.then(()=>{delete Object.prototype.then;console.log(reads)});"
        ),
        ["1"]
    );
}
#[test]
/** A then getter that throws rejects the measurement promise through normal PromiseResolve semantics. */
fn throwing_then_getters_reject_the_measurement() {
    assert_eq!(
        run(
            "const marker={};const promise=performance.measureMemory();Object.defineProperty(Object.prototype,'then',{get(){throw marker},configurable:true});promise.then(()=>{throw Error('unexpected fulfillment')},error=>{delete Object.prototype.then;console.log(error===marker)});"
        ),
        ["true"]
    );
}
#[test]
/** Worker measurements collect and resolve in their real isolate before posting the resulting number. */
fn workers_measure_their_own_live_heap() {
    assert_eq!(
        run(
            "const worker=new Worker('performance.measureMemory().then(result=>postMessage(result.total.jsMemoryEstimate>0))',{type:'string'});console.log(worker.getMessage());worker.terminateAndWait();"
        ),
        ["true"]
    );
}

#[test]
/** Native two-byte preparation allocates independent backing bytes instead of aliasing the input's Arc. */
fn two_byte_preparation_owns_a_fresh_native_backing() {
    assert_eq!(
        run(
            "const original='a'.repeat(32768);let copy;(async()=>{const before=(await performance.measureMemory()).total.jsMemoryEstimate;copy=createExternalizableTwoByteString(original);const after=(await performance.measureMemory()).total.jsMemoryEstimate;console.log([copy===original,after>before+32768].join(':'))})();"
        ),
        ["true:true"]
    );
}

#[test]
/** Two-byte copies preserve unmatched surrogates and reject invalid inputs without invoking coercion hooks. */
fn two_byte_preparation_preserves_code_units_without_coercion() {
    assert_eq!(
        run(
            r"const copy=createExternalizableTwoByteString('\ud800a');let coerced=false;let rejected=false;try{createExternalizableTwoByteString({toString(){coerced=true;return 'x'}})}catch(error){rejected=true}let constructible=true;try{new createExternalizableTwoByteString('x')}catch(error){constructible=false}console.log([copy.length,copy.charCodeAt(0),coerced,rejected,constructible].join(':'));"
        ),
        ["2:55296:false:true:false"]
    );
}
