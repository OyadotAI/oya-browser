/** Shared backing stores expose distinct brands, coherent views, atomic integers and safe state transport. */
mod common;

/** Shared and ordinary buffer brands reject each other's accessor receivers while views share bytes. */
#[test]
fn shared_storage_has_a_distinct_brand_and_coherent_views() {
    let source = "const b=new SharedArrayBuffer(16),i=new Int32Array(b),d=new DataView(b);i[0]=42;d.setInt32(4,-7,true);let errors=0;for(const [proto,value]of[[ArrayBuffer.prototype,b],[SharedArrayBuffer.prototype,new ArrayBuffer(1)]]){try{Object.getOwnPropertyDescriptor(proto,'byteLength').get.call(value)}catch(e){errors+=e instanceof TypeError}}const r=[b.byteLength,i.length,d.getInt32(0,true),i[1],Object.prototype.toString.call(b),errors].join();";
    assert_eq!(
        common::eval(source, "r"),
        "16,4,42,-7,[object SharedArrayBuffer],2"
    );
}

/** Every integer width wraps exactly and compareExchange observes actual previous bytes. */
#[test]
fn atomic_operations_use_integer_widths_and_bigints() {
    let source = "const b=new SharedArrayBuffer(16),a=new Int8Array(b),big=new BigInt64Array(b,8,1);Atomics.store(a,0,127);const out=[Atomics.add(a,0,1),a[0],Atomics.compareExchange(a,0,-128,7),Atomics.xor(a,0,3),a[0]];Atomics.store(big,0,9223372036854775807n);out.push(Atomics.add(big,0,1n),Atomics.load(big,0),Atomics.exchange(big,0,-1n),big[0]);const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "127,-128,-128,7,4,9223372036854775807,-9223372036854775808,-9223372036854775808,-1"
    );
}

/** Atomics also supports ordinary integer views, with converted store results before truncation. */
#[test]
fn ordinary_atomic_storage_and_coercions_follow_the_same_rules() {
    let source = "const a=new Int16Array(2);const out=[Atomics.store(a,0,65537.9),a[0],Atomics.store(a,1,Infinity),a[1],Atomics.notify(new Int32Array(1),0)];let calls=0;Atomics.add(a,{valueOf(){calls++;return 0}},{valueOf(){calls++;return 2}});out.push(a[0],calls);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "65537,1,Infinity,0,0,3,2");
}

/** Growth appends zero bytes and only omitted-length views track the synchronized current length. */
#[test]
fn shared_growth_preserves_fixed_views_and_tracks_omitted_lengths() {
    let source = "const b=new SharedArrayBuffer(4,{maxByteLength:16}),tracking=new Int32Array(b),fixed=new Int32Array(b,0,1);tracking[0]=7;b.grow(12);const r=[b.growable,b.maxByteLength,b.byteLength,tracking.length,fixed.length,tracking[0],tracking[2]].join();";
    assert_eq!(common::eval(source, "r"), "true,16,12,3,1,7,0");
}

/** Structured cloning makes a new wrapper, and JSON state transport restores local backing aliases. */
#[test]
fn shared_clone_aliases_survive_state_transport() {
    let sources=vec!["const first=new SharedArrayBuffer(4),second=structuredClone(first),a=new Int32Array(first),b=new Int32Array(second);a[0]=7;document.getElementById('go').addEventListener('click',()=>{Atomics.add(b,0,1);console.log(first!==second,a[0],b[0])});".to_string()];
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
    assert_eq!(common::console(&clicked), ["true 8 8"]);
}

/** Immediate wait results arise from actual comparison or timeout, without manufacturing notifications. */
#[test]
fn atomic_wait_distinguishes_comparison_and_elapsed_timeout() {
    let source = "const a=new Int32Array(new SharedArrayBuffer(4));const r=[Atomics.wait(a,0,1),Atomics.wait(a,0,0,0),Atomics.wait(a,0,0,1),Atomics.notify(a,0),Atomics.isLockFree(1),Atomics.isLockFree(8)].join();";
    assert_eq!(
        common::eval(source, "r"),
        "not-equal,timed-out,timed-out,0,true,false"
    );
}

/** Async notification returns a real Promise, resolves later, and consumes exactly one indexed waiter. */
#[test]
fn asynchronous_wait_notification_uses_real_promise_jobs() {
    let source = "const a=new Int32Array(new SharedArrayBuffer(4));const result=Atomics.waitAsync(a,0,0);let resolved=false;result.value.then(value=>{resolved=true;console.log(value)});console.log(result.async,result.value instanceof Promise,Atomics.notify(a,0),resolved);";
    let execution = common::load_with(source, &common::environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::console(&execution), ["true true 1 false", "ok"]);
}

/** Mismatch and zero timeout never register an asynchronous future or expose a promise. */
#[test]
fn asynchronous_wait_can_complete_synchronously() {
    let source = "const a=new Int32Array(new SharedArrayBuffer(4));const mismatch=Atomics.waitAsync(a,0,1),timeout=Atomics.waitAsync(a,0,0,0);const r=[mismatch.async,mismatch.value,timeout.async,timeout.value,Atomics.notify(a,0)].join();";
    assert_eq!(
        common::eval(source, "r"),
        "false,not-equal,false,timed-out,0"
    );
}

/** An out-of-scope async wait survives JSON transport and times out through the actual timer queue. */
#[test]
fn asynchronous_wait_timeout_survives_serialization() {
    let sources=vec!["(()=>{const a=new Int32Array(new SharedArrayBuffer(4));Atomics.waitAsync(a,0,0,3).value.then(value=>console.log(value))})();".to_string()];
    let host = common::host();
    let environment = common::environment();
    let mut execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    for _ in 0..8 {
        assert!(
            execution.failed.is_empty(),
            "{:?}",
            common::failure(&execution)
        );
        if !common::console(&execution).is_empty() {
            assert_eq!(common::console(&execution), ["timed-out"]);
            return;
        }
        let due = execution
            .effects
            .next_timer_ms
            .expect("pending native wait task");
        execution = mars_v9::run_timers(
            common::input(&sources, &host, &environment),
            common::round_trip(execution.state),
            due,
            1,
        )
        .unwrap();
    }
    panic!("async timeout did not resolve");
}

/** An unrelated request restores the real pending ticket before notifying its registered promise. */
#[test]
fn asynchronous_wait_notification_survives_serialization() {
    let sources=vec!["const a=new Int32Array(new SharedArrayBuffer(4));Atomics.waitAsync(a,0,0).value.then(value=>console.log(value));document.getElementById('go').addEventListener('click',()=>console.log(Atomics.notify(a,0)));".to_string()];
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
    assert_eq!(common::console(&clicked), ["1", "ok"]);
}

/** Separate Rust-owned VMs block and wake using the same store; cancellation removes actual pending waits. */
#[cfg(feature = "conformance")]
mod workers {
    use super::*;
    /** Each test owns every worker and joins it before returning. */
    fn run(source: &str) -> Vec<String> {
        let session = mars_v9::ConformanceWorkerSession::new();
        let environment = mars_v9::PageEnvironment {
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
        session.finish().unwrap();
        common::console(&execution)
    }
    /** Notify at another byte offset cannot wake the registered blocking thread. */
    #[test]
    fn workers_block_and_wake_at_the_actual_byte_offset() {
        let source = "const a=new Int32Array(new SharedArrayBuffer(8));const worker=new Worker(function(){onmessage=function({data:buffer}){const a=new Int32Array(buffer);postMessage('ready');postMessage(Atomics.wait(a,0,0));}},{type:'function'});worker.postMessage(a.buffer);worker.getMessage();const wrong=Atomics.notify(a,1);let notified=0;while(notified===0){notified=Atomics.notify(a,0)}console.log(wrong,notified,worker.getMessage());worker.terminate();";
        assert_eq!(run(source), ["0 1 ok"]);
    }
    /** Terminating a blocked worker triggers the VM cancellation check and removes its indexed waiter. */
    #[test]
    fn worker_termination_cancels_blocking_waits() {
        let source = "const a=new Int32Array(new SharedArrayBuffer(4));const worker=new Worker(function(){onmessage=function({data:buffer}){postMessage('ready');Atomics.wait(new Int32Array(buffer),0,0);postMessage('unexpected');}},{type:'function'});worker.postMessage(a.buffer);worker.getMessage();worker.terminateAndWait();console.log(Atomics.notify(a,0));";
        assert_eq!(run(source), ["0"]);
    }
    /** A terminated agent's asynchronous registration must not consume another agent's notification. */
    #[test]
    fn worker_termination_cancels_asynchronous_waits() {
        let source = "const a=new Int32Array(new SharedArrayBuffer(4));const worker=new Worker(function(){onmessage=function({data:buffer}){Atomics.waitAsync(new Int32Array(buffer),0,0);postMessage('ready');}},{type:'function'});worker.postMessage(a.buffer);worker.getMessage();worker.terminateAndWait();console.log(Atomics.notify(a,0));";
        assert_eq!(run(source), ["0"]);
    }
}

/** Serialized backing identities cannot merge inconsistent same-revision byte contents. */
#[test]
fn conflicting_shared_alias_snapshots_are_rejected() {
    let sources=vec!["const a=new SharedArrayBuffer(4),b=structuredClone(a);document.getElementById('go').addEventListener('click',()=>console.log(a===b));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let mut seen = 0;
    rewrite_shared(&mut wire, &mut |store| {
        seen += 1;
        if seen == 2 {
            store.insert("bytes".into(), serde_json::json!("ffffffff"));
        }
    });
    assert_eq!(seen, 2);
    let state = serde_json::from_value(wire).unwrap();
    assert!(mars_v9::click_page(common::input(&sources, &host, &environment), state, 3).is_err());
}

/** A pending future's native completion must be a genuine local callback with the matching backing and ticket. */
#[test]
fn forged_async_waiter_callbacks_are_rejected() {
    let sources=vec!["const a=new Int32Array(new SharedArrayBuffer(4));Atomics.waitAsync(a,0,0);document.getElementById('go').addEventListener('click',()=>console.log('unexpected'));".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    let mut changed = 0;
    rewrite_shared(&mut wire, &mut |store| {
        if let Some(serde_json::Value::Array(rows)) = store.get_mut("waiters") {
            for row in rows {
                row["callback"] = serde_json::json!(u32::MAX);
                changed += 1;
            }
        }
    });
    assert_eq!(changed, 1);
    let state = serde_json::from_value(wire).unwrap();
    assert!(mars_v9::click_page(common::input(&sources, &host, &environment), state, 3).is_err());
}

/** A forged revision cannot place the next native write near integer overflow. */
#[test]
fn shared_snapshot_revision_overflow_is_rejected() {
    let loaded = common::load_with(
        "globalThis.buffer=new SharedArrayBuffer(4)",
        &common::environment(),
    );
    let mut wire = serde_json::to_value(loaded.state).unwrap();
    rewrite_shared(&mut wire, &mut |store| {
        store.insert("revision".into(), serde_json::json!(u64::MAX));
    });
    assert!(serde_json::from_value::<mars_v9::VmState>(wire).is_err());
}

/** Tests corrupt the public state codec while keeping ordinary script-visible properties unrelated. */
fn rewrite_shared(
    value: &mut serde_json::Value,
    change: &mut impl FnMut(&mut serde_json::Map<String, serde_json::Value>),
) {
    match value {
        serde_json::Value::Object(object) => {
            if let Some(serde_json::Value::Object(store)) = object.get_mut("shared_array_buffer") {
                change(store)
            }
            for value in object.values_mut() {
                rewrite_shared(value, change)
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                rewrite_shared(value, change)
            }
        }
        _ => {}
    }
}
