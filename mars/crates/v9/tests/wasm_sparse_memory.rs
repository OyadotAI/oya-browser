/** Sparse Wasm memory grows logical length without materializing untouched zero pages. */
mod common;

/** Growth detaches old buffers and lets typed views access distant resident pages. */
#[test]
fn sparse_memory_grows_and_shares_distant_views() {
    let source = "const m=new WebAssembly.Memory({initial:1,maximum:16384});const old=m.buffer;new Uint8Array(old)[0]=23;let r=String(m.grow(16383));const b=m.buffer;const v=new Uint8Array(b,1073741820,4);v[2]=77;const d=new DataView(b);r+=','+[old.detached,old.byteLength,b.byteLength,d.getUint8(0),d.getUint8(1073741822),d.getUint8(50000000)].join()";
    assert_eq!(common::eval(source, "r"), "1,true,0,1073741824,23,77,0");
}

/** Serialized sparse pages restore the same bytes and retain the old detached view. */
#[test]
fn sparse_memory_and_views_survive_state_restoration() {
    let sources = ["const m=new WebAssembly.Memory({initial:1,maximum:16384});const old=m.buffer;new Uint8Array(old)[0]=3;m.grow(16383);const b=m.buffer,v=new Uint8Array(b,1073741820,4);v[2]=77;let r='';setTimeout(()=>{r=[old.detached,b.byteLength,v[2],new DataView(b).getUint8(0),b.maxByteLength].join()},1)".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let state = common::round_trip(loaded.state);
    let result =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 2.0, 1).unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(
        common::global(&result, "r"),
        "true,1073741824,77,3,1073741824"
    );
}

/** Large initial memory supports bounded copies and checks the logical end of views. */
#[test]
fn sparse_initial_memory_slices_and_checks_view_bounds() {
    let source = "const m=new WebAssembly.Memory({initial:16384});const b=m.buffer;const v=new Uint8Array(b,1073741820,4);v[1]=91;const copy=new Uint8Array(b.slice(1073741820,1073741824));let bounds=false;try{new DataView(b).getUint8(1073741824)}catch(e){bounds=e instanceof RangeError}let r=[b.byteLength,copy.join(),bounds].join('|')";
    assert_eq!(common::eval(source, "r"), "1073741824|0,91,0,0|true");
}

/** Memory64 without a declared maximum uses its own implementation bound above 4 GiB. */
#[test]
fn sparse_memory64_without_maximum_grows_past_memory32_limit() {
    let source = "const m=new WebAssembly.Memory({initial:1n,address:'i64'});let r=String(m.grow(65536n))+','+m.buffer.byteLength;new Uint8Array(m.buffer,4294967296,1)[0]=9;r+=','+new Uint8Array(m.buffer,4294967296,1)[0]";
    assert_eq!(common::eval(source, "r"), "1,4295032832,9");
}

/** Atomic read-modify-write accesses use the same sparse bytes as ordinary views. */
#[test]
fn sparse_memory_supports_unshared_atomics() {
    let source = "const m=new WebAssembly.Memory({initial:16384}),a=new Int32Array(m.buffer,1073741820,1);let r=[Atomics.load(a,0),Atomics.add(a,0,7),Atomics.compareExchange(a,0,7,12),a[0]].join()";
    assert_eq!(common::eval(source, "r"), "0,0,7,12");
}

/** Shared sparse growth keeps fixed old windows and atomics operate on the actual distant pages. */
#[test]
fn sparse_shared_memory_growth_and_atomic_views() {
    let source = "const m=new WebAssembly.Memory({initial:1,maximum:16384,shared:true}),old=m.buffer;new Uint8Array(old)[0]=13;const p=m.grow(16383),b=m.buffer,a=new Int32Array(b,1073741820,1);let r=[p,old.byteLength,b.byteLength,new Uint8Array(b)[0],Atomics.add(a,0,7),Atomics.load(a,0),Atomics.wait(a,0,9,0)].join()";
    assert_eq!(
        common::eval(source, "r"),
        "1,65536,1073741824,13,0,7,not-equal"
    );
}

/** Shared sparse aliases and distant bytes remain coherent after inert state restoration. */
#[test]
fn sparse_shared_memory_survives_state_restoration() {
    let sources = ["const m=new WebAssembly.Memory({initial:1,maximum:16384,shared:true});m.grow(16383);const b=m.buffer,v=new Int32Array(b,1073741820,1);Atomics.store(v,0,51);let r='';setTimeout(()=>{r=[b.byteLength,Atomics.load(v,0),m.buffer===b].join()},1)".to_string()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let result = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        2.0,
        1,
    )
    .unwrap();
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    assert_eq!(common::global(&result, "r"), "1073741824,51,true");
}

/** Worker cloning copies sparse pages and preserves aliases without sharing mutable storage. */
#[cfg(feature = "conformance")]
#[test]
fn sparse_buffer_worker_clone_preserves_aliases_and_independence() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let source = "const w=new Worker('onmessage=({data})=>{let v=new Uint8Array(data.b,1073741820,4);v[1]=99;postMessage([data.b===data.v.buffer,v[1],data.b.byteLength].join())}',{type:'string'});const m=new WebAssembly.Memory({initial:16384}),b=m.buffer,v=new Uint8Array(b,1073741820,4);v[1]=42;w.postMessage({b,v});let r=w.getMessage()+','+v[1];w.terminateAndWait()";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,99,1073741824,42");
    session.finish().expect("worker terminates");
}

/** A cloned ordinary sparse buffer can be transferred and detached without affecting its Wasm source. */
#[cfg(feature = "conformance")]
#[test]
fn sparse_worker_clone_can_be_transferred() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let source = "const w=new Worker('onmessage=({data})=>postMessage(data)',{type:'string'});const m=new WebAssembly.Memory({initial:16384}),b=m.buffer;new Uint8Array(b,1073741820,4)[1]=42;w.postMessage(b);const c=w.getMessage();w.postMessage(c,[c]);const d=w.getMessage();let r=[c.detached,d.byteLength,new Uint8Array(d,1073741820,4)[1],b.byteLength].join();w.terminateAndWait()";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true,1073741824,42,1073741824"
    );
    session.finish().expect("worker terminates");
}

/** Shared sparse memory and its buffer retain alias identity and synchronized pages across workers. */
#[cfg(feature = "conformance")]
#[test]
fn sparse_shared_memory_worker_aliases_and_atomics() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let source = "const w=new Worker('onmessage=({data})=>{const a=new Int32Array(data.b,1073741820,1);Atomics.add(a,0,9);postMessage([data.m.buffer===data.b,Atomics.load(a,0)].join())}',{type:'string'});const m=new WebAssembly.Memory({initial:16384,maximum:16384,shared:true}),b=m.buffer,a=new Int32Array(b,1073741820,1);Atomics.store(a,0,4);w.postMessage({m,b});let r=w.getMessage()+','+Atomics.load(a,0);w.terminateAndWait()";
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,13,13");
    session.finish().expect("worker terminates");
}
