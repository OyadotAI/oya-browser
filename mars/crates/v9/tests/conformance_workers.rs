/* Hermetic d8 worker tests execute real isolated V9 threads and typed graph messages. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::{ConformanceWorkerSession, PageEnvironment};

/** Dropping the session after loading proves all worker threads finish before a test returns. */
fn run(source: &str) -> Vec<String> {
    let session = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(source, &environment);
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
    session
        .finish()
        .expect("worker engine operations complete without native failures");
    common::console(&execution)
}

#[test]
/** Worker function source runs in its own global environment rather than capturing parent locals. */
fn function_workers_are_isolated_and_receive_real_arguments() {
    assert_eq!(
        run(
            "const parentSecret=42; const worker=new Worker(function(value){postMessage(typeof parentSecret+':'+value.answer)}, {type:'function',arguments:[{answer:7}]}); console.log(worker.getMessage());"
        ),
        ["undefined:7"]
    );
}
#[test]
/** String workers call a real message handler and send actual FIFO responses. */
fn string_workers_deliver_messages_in_order() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>{postMessage(data+1);postMessage(data+2)}',{type:'string'});worker.postMessage(4);console.log(worker.getMessage()+':'+worker.getMessage());worker.terminateAndWait();"
        ),
        ["5:6"]
    );
}
#[test]
/** Cyclic and repeated references survive both isolate boundaries while source mutations remain local. */
fn structured_messages_preserve_graph_identity() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(data)',{type:'string'});const child={value:1};const source={a:child,b:child};source.self=source;worker.postMessage(source);child.value=9;const copy=worker.getMessage();console.log([copy.self===copy,copy.a===copy.b,copy.a.value].join(':'));worker.terminateAndWait();"
        ),
        ["true:true:1"]
    );
}
#[test]
/** Maps, sets, holes, negative zero, NaN and BigInts retain structured-clone types and identity. */
fn structured_messages_preserve_supported_data_types() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(data)',{type:'string'});const array=new Array(3);array[2]=1;const key={};const source={key,map:new Map([[key,3]]),set:new Set([key]),array,nan:NaN,negative:-0,big:5n};worker.postMessage(source);const copy=worker.getMessage();console.log([copy.map instanceof Map,copy.map.get(copy.key),copy.set.has(copy.key),copy.array.length,0 in copy.array,Number.isNaN(copy.nan),Object.is(copy.negative,-0),copy.big===5n].join(':'));worker.terminateAndWait();"
        ),
        ["true:3:true:3:false:true:true:true"]
    );
}
#[test]
/** Unsupported functions and symbols reject synchronously, without silently dropping object fields. */
fn function_and_symbol_messages_are_rejected() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});let count=0;for(const value of [()=>{},Symbol(),{nested:()=>{}}]){try{worker.postMessage(value)}catch(error){if(error.name==='DataCloneError')count++}}console.log(count);worker.terminateAndWait();"
        ),
        ["3"]
    );
}
#[test]
/** Transferring a buffer detaches its sender, preserves view aliases and copies actual byte contents. */
fn transferred_buffers_detach_and_preserve_view_aliases() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>{data.view[0]++;postMessage(data)}',{type:'string'});const buffer=new ArrayBuffer(4);const view=new Uint8Array(buffer);view[0]=41;worker.postMessage({buffer,view},[buffer]);const copy=worker.getMessage();console.log([buffer.byteLength,view.length,copy.buffer===copy.view.buffer,copy.view[0]].join(':'));worker.terminateAndWait();"
        ),
        ["0:0:true:42"]
    );
}
#[test]
/** A failed graph clone leaves every transferable attached, so partial cloning cannot destroy data. */
fn failed_serialization_does_not_detach_transferables() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});const buffer=new ArrayBuffer(4);try{worker.postMessage({buffer,callback(){}},[buffer])}catch(error){}console.log(buffer.byteLength);worker.terminateAndWait();"
        ),
        ["4"]
    );
}
#[test]
/** Transfer-list getters run before message serialization, following d8's ValueSerializer setup order. */
fn transfer_list_is_observed_before_message_properties() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(data.value)',{type:'string'});let value=1;const list=[];Object.defineProperty(list,'0',{get(){value=2;return buffer}});list.length=1;const buffer=new ArrayBuffer(0);worker.postMessage({get value(){return value}},list);console.log(worker.getMessage());worker.terminateAndWait();"
        ),
        ["2"]
    );
}
#[test]
/** Error clones keep their native error brand and cyclic cause graph. */
fn error_messages_preserve_brand_and_cause() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(data)',{type:'string'});const cause={};const error=new TypeError('bad',{cause});cause.error=error;worker.postMessage(error);const copy=worker.getMessage();console.log([copy instanceof TypeError,copy.message,copy.cause.error===copy].join(':'));worker.terminateAndWait();"
        ),
        ["true:bad:true"]
    );
}
#[test]
/** Worker timers and promise reactions execute in the real child VM before an awaited message arrives. */
fn child_timers_and_microtasks_execute() {
    assert_eq!(
        run(
            "const worker=new Worker('Promise.resolve().then(()=>setTimeout(()=>postMessage(42),0))',{type:'string'});console.log(worker.getMessage());"
        ),
        ["42"]
    );
}
#[test]
/** close stops further source statements and a finished worker returns undefined after its queue drains. */
fn close_and_finished_workers_end_message_delivery() {
    assert_eq!(
        run(
            "const worker=new Worker('postMessage(1);close();postMessage(2)',{type:'string'});console.log(worker.getMessage()+':'+worker.getMessage());"
        ),
        ["1:undefined"]
    );
}
#[test]
/** An infinite child loop is interrupted and joined by terminateAndWait. */
fn terminate_and_wait_interrupts_running_script() {
    assert_eq!(
        run(
            "const worker=new Worker('postMessage(1);while(true){}',{type:'string'});console.log(worker.getMessage());worker.terminateAndWait();console.log(worker.getMessage());"
        ),
        ["1", "undefined"]
    );
}
#[test]
/** Scope teardown cancels running workers even when the test never explicitly terminates them. */
fn session_scope_joins_running_workers() {
    assert_eq!(
        run(
            "const worker=new Worker('postMessage(1);while(true){}',{type:'string'});console.log(worker.getMessage());"
        ),
        ["1"]
    );
}
#[test]
/** Methods validate the actual receiver rather than accidentally using a captured worker instance. */
fn worker_methods_have_private_receiver_brands() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});let count=0;for(const name of ['getMessage','postMessage','terminate','terminateAndWait']){try{worker[name].call({})}catch(error){if(error instanceof TypeError)count++}}console.log(count);worker.terminateAndWait();"
        ),
        ["4"]
    );
}

#[test]
/** D8 registers transfers before detached checks, so retransferring a detached buffer gives an empty store. */
fn detached_buffers_can_be_registered_for_a_second_transfer() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(data.byteLength)',{type:'string'});const buffer=new ArrayBuffer(4);worker.postMessage(buffer,[buffer]);worker.postMessage(buffer,[buffer]);console.log(worker.getMessage()+':'+worker.getMessage());worker.terminateAndWait();"
        ),
        ["4:0"]
    );
}

#[test]
/** Detached data that was not registered as a transferable remains uncloneable. */
fn detached_buffers_cannot_be_cloned_without_transfer_registration() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});const buffer=new ArrayBuffer(4);worker.postMessage(buffer,[buffer]);let rejected=false;try{worker.postMessage(buffer)}catch(error){rejected=true}console.log(rejected);worker.terminateAndWait();"
        ),
        ["true"]
    );
}

#[test]
/** Ordinary uncaught child exceptions end that worker and preserve d8's undefined receive result. */
fn child_language_errors_end_the_worker_without_becoming_parent_throws() {
    assert_eq!(
        run(
            "const worker=new Worker('throw TypeError(\"child failure\")',{type:'string'});console.log(worker.getMessage());"
        ),
        ["undefined"]
    );
}

#[test]
/** A child engine resource failure cannot silently become a passing test when the parent ignores it. */
fn child_engine_failures_are_reported_when_the_session_finishes() {
    let session = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(
        "const worker=new Worker('\"x\".repeat(100000000)',{type:'string'});worker.getMessage();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let failure = session.finish().unwrap_err();
    assert_eq!(failure.code, "limit_exceeded");
}

#[test]
/** Restoring a Worker object in another session cannot reuse its private endpoint identity. */
fn serialized_worker_brands_do_not_grant_access_to_another_session() {
    let source = "const worker=new Worker('onmessage=()=>{}',{type:'string'});document.getElementById('go').addEventListener('click',()=>{try{worker.getMessage()}catch(error){console.log(error instanceof TypeError)}});";
    let session = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let loaded = common::load_with(source, &environment);
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    session.finish().unwrap();
    let next = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_workers: Some(next.host()),
        ..common::environment()
    };
    let host = common::host();
    let sources = vec![source.to_string()];
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true"]);
    next.finish().unwrap();
}

#[test]
/** A getter deleting a later property makes that property absent in the clone, even with inherited data. */
fn message_property_deletions_are_observed_during_serialization() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(Object.keys(data).join(\",\"))',{type:'string'});const source=Object.create({second:9});Object.defineProperty(source,'first',{get(){delete source.second;return 1},enumerable:true});source.second=2;worker.postMessage(source);console.log(worker.getMessage());worker.terminateAndWait();"
        ),
        ["first"]
    );
}

#[test]
/** Cloned WebAssembly modules are independently validated and instantiated by the child V9 isolate. */
fn wasm_modules_cross_workers_as_real_executable_modules() {
    assert_eq!(
        run(
            "const bytes=new Uint8Array([0,97,115,109,1,0,0,0,1,5,1,96,0,1,127,3,2,1,0,7,10,1,6,97,110,115,119,101,114,0,0,10,6,1,4,0,65,42,11]);const module=new WebAssembly.Module(bytes);const worker=new Worker('onmessage=({data})=>postMessage(new WebAssembly.Instance(data).exports.answer())',{type:'string'});worker.postMessage(module);console.log(worker.getMessage());worker.terminateAndWait();"
        ),
        ["42"]
    );
}

#[test]
/** Worker objects are native capabilities and cannot silently clone as empty ordinary objects. */
fn worker_capabilities_cannot_be_cloned_as_data() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});let rejected=false;try{worker.postMessage(worker)}catch(error){rejected=true}console.log(rejected);worker.terminateAndWait();"
        ),
        ["true"]
    );
}

#[test]
/** Shared buffers retain the same synchronized bytes across both isolate heaps. */
fn shared_buffer_messages_share_actual_bytes() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>{const view=new Int32Array(data);Atomics.add(view,0,1);postMessage(view[0])}',{type:'string'});const buffer=new SharedArrayBuffer(4);const view=new Int32Array(buffer);view[0]=41;worker.postMessage(buffer);console.log(worker.getMessage()+':'+view[0]);worker.terminateAndWait();"
        ),
        ["42:42"]
    );
}

#[test]
/** Growing shared storage in one worker immediately updates a parent's existing tracking view. */
fn growable_shared_buffers_remain_shared_across_messages() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>{data.grow(8);new Int32Array(data)[1]=7;postMessage(data)}',{type:'string'});const buffer=new SharedArrayBuffer(4,{maxByteLength:16});const view=new Int32Array(buffer);worker.postMessage(buffer);const returned=worker.getMessage();console.log([buffer.byteLength,view.length,view[1],returned!==buffer,returned.byteLength].join(':'));worker.terminateAndWait();"
        ),
        ["8:2:7:true:8"]
    );
}

#[test]
/** More completed workers than the concurrency bound retain real responses without consuming live thread slots. */
fn finite_worker_admission_uses_backpressure_without_raising_concurrency() {
    assert_eq!(
        run(
            "const workers=[];for(let i=0;i<80;i++)workers.push(new Worker('postMessage(1)',{type:'string'}));let total=0;for(const worker of workers)total+=worker.getMessage();console.log(total);"
        ),
        ["80"]
    );
}

#[test]
/** Known unimplemented native methods cannot hide as ignored ordinary child exceptions. */
fn missing_child_native_capabilities_fail_the_host_session() {
    let session = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(
        "const worker=new Worker(`new Worker('postMessage(1)',{type:'string',flushDenormals:true})`,{type:'string'});worker.getMessage();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let failure = session.finish().unwrap_err();
    assert_eq!(failure.code, "unsupported_feature");
    assert!(failure.message.contains("flushDenormals"));
}

#[test]
/** Raw JSON records are opaque serializer inputs and must not silently become ordinary data objects. */
fn raw_json_records_cannot_be_sent_to_workers() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});let rejected=false;try{worker.postMessage(JSON.rawJSON('1'))}catch(error){rejected=true}console.log(rejected);worker.terminateAndWait();"
        ),
        ["true"]
    );
}

#[test]
/** A broker grant alone cannot keep garbage-collected buffers alive or consume the session's reservation forever. */
fn shared_grants_release_collected_backing_reservations() {
    assert_eq!(
        run(
            "for(let index=0;index<40;index++){const buffer=new SharedArrayBuffer(1048576);gc();}console.log('released');"
        ),
        ["released"]
    );
}

#[test]
/** Startup executes once while a long-lived child preserves globals across real incoming tasks. */
fn persistent_worker_globals_survive_many_messages() {
    assert_eq!(
        run(
            "const worker=new Worker('let count=0;postMessage(\"ready\");onmessage=()=>postMessage(++count)',{type:'string'});if(worker.getMessage()!=='ready')throw Error('missing startup');for(let index=1;index<=200;index++){worker.postMessage(index);if(worker.getMessage()!==index)throw Error('lost persistent global')}console.log('200');worker.terminateAndWait();"
        ),
        ["200"]
    );
}

#[test]
/** Temporary message roots live through their microtasks and release before the next task's collection. */
fn worker_weak_targets_obey_actual_job_boundaries() {
    assert_eq!(
        run(
            "const worker=new Worker('let weak;onmessage=({data})=>{if(data.start){weak=new WeakRef(data);Promise.resolve().then(()=>{gc();postMessage(weak.deref()!==undefined)})}else{gc();postMessage(weak.deref()===undefined)}}',{type:'string'});worker.postMessage({start:true});const first=worker.getMessage();worker.postMessage({start:false});console.log(first+':'+worker.getMessage());worker.terminateAndWait();"
        ),
        ["true:true"]
    );
}

#[test]
/** An unsupported native capability thrown from a timer retains its native provenance at the host boundary. */
fn asynchronous_native_failures_are_not_ignored_as_child_language_errors() {
    let session = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(
        "const worker=new Worker(`setTimeout(()=>new Worker('',{type:'string',flushDenormals:true}),0)`,{type:'string'});worker.getMessage();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let failure = session.finish().unwrap_err();
    assert_eq!(failure.code, "unsupported_feature");
}

#[test]
/** Unhandled child rejections fail d8's process result even though ordinary child exceptions do not propagate. */
fn unhandled_child_rejections_fail_the_host_session() {
    let session = ConformanceWorkerSession::new();
    let environment = PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(
        "const worker=new Worker('Promise.reject(Error(\"child rejection\"))',{type:'string'});worker.getMessage();",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        session.finish().unwrap_err().code,
        "worker_unhandled_rejection"
    );
}

#[test]
/** A timer's ordinary uncaught JavaScript error ends only its child, preserving d8's receive semantics. */
fn ordinary_child_timer_exceptions_end_only_the_child() {
    assert_eq!(
        run(
            "const worker=new Worker('setTimeout(()=>{throw Error(\"child timer\")},0)',{type:'string'});console.log(worker.getMessage());"
        ),
        ["undefined"]
    );
}

#[test]
/** Joining an isolate removes only its asynchronous futex tickets from a shared backing. */
fn terminating_workers_removes_their_actual_async_waiters() {
    assert_eq!(
        run(
            "const worker=new Worker('onmessage=({data})=>postMessage(Atomics.waitAsync(new Int32Array(data),0,0).async)',{type:'string'});const buffer=new SharedArrayBuffer(4);worker.postMessage(buffer);const waiting=worker.getMessage();worker.terminateAndWait();console.log(waiting+':'+Atomics.notify(new Int32Array(buffer),0));"
        ),
        ["true:0"]
    );
}

#[test]
/** Transferred tracking views retain their real resizable buffer across successive message callbacks. */
fn persistent_workers_retain_resizable_view_metadata() {
    assert_eq!(
        run(
            "const worker=new Worker('let view;onmessage=({data})=>{if(data.view){view=data.view;postMessage(view.length)}else{view.buffer.resize(data.length);postMessage(view.length)}}',{type:'string'});const buffer=new ArrayBuffer(16,{maxByteLength:1024});worker.postMessage({view:new Uint8Array(buffer)},[buffer]);const before=worker.getMessage();worker.postMessage({length:150});console.log(before+':'+worker.getMessage()+':'+buffer.byteLength);worker.terminateAndWait();"
        ),
        ["16:150:0"]
    );
}

#[test]
/** Native Wasm instruction loops observe the same cancellation token as the JavaScript interpreter. */
fn native_instruction_loops_cancel_before_exhausting_the_operation_budget() {
    assert_eq!(
        run(
            "const worker=new Worker('const bytes=new Uint8Array([0,97,115,109,1,0,0,0,1,4,1,96,0,0,3,2,1,0,7,8,1,4,115,112,105,110,0,0,10,9,1,7,0,3,64,12,0,11,11]);const instance=new WebAssembly.Instance(new WebAssembly.Module(bytes));postMessage(1);instance.exports.spin()',{type:'string'});console.log(worker.getMessage());gc();worker.terminateAndWait();console.log(worker.getMessage());"
        ),
        ["1", "undefined"]
    );
}
