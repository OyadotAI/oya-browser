/** Real shared GC roots preserve identity, synchronized mutation and typed snapshots across native Workers. */
mod common;
/** Small binary fixtures exercise the public API without any external compiler. */
fn section(bytes: &mut Vec<u8>, kind: u8, payload: &[u8]) {
    bytes.extend([kind, payload.len() as u8]);
    bytes.extend(payload);
}
/** One shared i32 struct exports typed creation, reading and atomic addition. */
fn module() -> Vec<u8> {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            4, 0x65, 0x5f, 1, 0x7f, 1, 0x60, 1, 0x7f, 1, 0x63, 0, 0x60, 1, 0x63, 0, 1, 0x7f, 0x60,
            2, 0x63, 0, 0x7f, 1, 0x7f,
        ],
    );
    section(&mut bytes, 3, &[3, 1, 2, 3]);
    let mut exports = vec![3];
    for (index, name) in ["make", "read", "add"].iter().enumerate() {
        exports.push(name.len() as u8);
        exports.extend(name.bytes());
        exports.extend([0, index as u8]);
    }
    section(&mut bytes, 7, &exports);
    let mut code = vec![3];
    for body in [
        vec![0, 0x20, 0, 0xfb, 0, 0, 0x0b],
        vec![0, 0x20, 0, 0xfb, 2, 0, 0, 0x0b],
        vec![0, 0x20, 0, 0x20, 1, 0xfe, 0x60, 0, 0, 0, 0x0b],
    ] {
        code.push(body.len() as u8);
        code.extend(body);
    }
    section(&mut bytes, 10, &code);
    bytes
}
/** State transport reconstructs one coherent graph shared by all of a VM's wrapper aliases. */
#[test]
fn shared_gc_aliases_survive_json_state_transport() {
    let bytes = module();
    let sources = vec![format!(
        "const bytes=new Uint8Array({bytes:?});const {{make,read,add}}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const node=make(41),alias=node,second=make(9);document.getElementById('go').addEventListener('click',()=>{{console.log(add(alias,1),read(node),alias===node,read(second))}});"
    )];
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
    assert_eq!(common::console(&clicked), ["41 42 true 9"]);
}
/** Two independent native VM threads atomically modify one actual backing record and return its same root. */
#[cfg(feature = "conformance")]
#[test]
fn shared_gc_workers_mutate_one_native_record_and_preserve_identity() {
    let bytes = module();
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});const mod=new WebAssembly.Module(bytes);const {{make,read}}=new WebAssembly.Instance(mod).exports;const node=make(41);const code='onmessage=({{data}})=>{{const {{add}}=new WebAssembly.Instance(data.mod).exports;for(let i=0;i<200;i++)add(data.node,1);postMessage(data.node)}}';const first=new Worker(code,{{type:'string'}}),second=new Worker(code,{{type:'string'}});first.postMessage({{mod,node}});second.postMessage({{mod,node}});const a=first.getMessage(),b=second.getMessage();console.log(read(node),a===node,b===node);first.terminateAndWait();second.terminateAndWait();"
    );
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_wasm_shared: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(&source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    session.finish().unwrap();
    assert_eq!(common::console(&execution), ["441 true true"]);
}
/** A forged field payload is rejected before any event handler can read or mutate shared state. */
#[test]
fn shared_gc_forged_typed_snapshots_fail_before_execution() {
    let bytes = module();
    let sources = vec![format!(
        "const bytes=new Uint8Array({bytes:?});const node=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.make(1);document.getElementById('go').addEventListener('click',()=>console.log('unsafe'));"
    )];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut state = serde_json::to_value(loaded.state).unwrap();
    let object = shared_wire(&mut state).expect("one real shared wrapper");
    object["payload"] = serde_json::Value::String("00".into());
    assert!(serde_json::from_value::<mars_v9::VmState>(state).is_err());
}

/** Locate the public serialized class payload without relying on unrelated heap tuple positions. */
fn shared_wire(value: &mut serde_json::Value) -> Option<&mut serde_json::Value> {
    match value {
        serde_json::Value::Object(map) => {
            if map.contains_key("wasm_shared") {
                return map.get_mut("wasm_shared");
            }
            map.values_mut().find_map(shared_wire)
        }
        serde_json::Value::Array(values) => values.iter_mut().find_map(shared_wire),
        _ => None,
    }
}

/** Arena references do not form Rust Arc cycles: unreachable self-referential arrays release their storage. */
#[cfg(feature = "conformance")]
#[test]
fn shared_gc_collects_native_cycles_under_the_existing_byte_budget() {
    let mut bytes = b"\0asm\x01\0\0\0".to_vec();
    section(
        &mut bytes,
        1,
        &[
            4, 0x65, 0x5e, 0x63, 0, 1, 0x60, 1, 0x7f, 1, 0x63, 0, 0x60, 1, 0x63, 0, 0, 0x60, 1,
            0x63, 0, 1, 0x63, 0,
        ],
    );
    section(&mut bytes, 3, &[3, 1, 2, 3]);
    let mut exports = vec![3];
    for (index, name) in ["make", "cycle", "get"].iter().enumerate() {
        exports.push(name.len() as u8);
        exports.extend(name.bytes());
        exports.extend([0, index as u8]);
    }
    section(&mut bytes, 7, &exports);
    let mut code = vec![3];
    for body in [
        vec![0, 0x20, 0, 0xfb, 7, 0, 0x0b],
        vec![0, 0x20, 0, 0x41, 0, 0x20, 0, 0xfb, 14, 0, 0x0b],
        vec![0, 0x20, 0, 0x41, 0, 0xfb, 11, 0, 0xd4, 0x0b],
    ] {
        code.push(body.len() as u8);
        code.extend(body);
    }
    section(&mut bytes, 10, &code);
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});const {{make,cycle,get}}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const survivor=make(1);cycle(survivor);for(let i=0;i<300;i++){{const node=make(1024);cycle(node);if(i%20===0)gc();}}gc();let r=get(survivor)===survivor;"
    );
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    environment.conformance_wasm_shared = true;
    let execution = common::load_with(&source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true");
}

/** Cloning a shared object preserves its actual shared identity and mutations through either reference. */
#[test]
fn structured_clone_preserves_a_shared_gc_root() {
    let bytes = module();
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});const {{make,read,add}}=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports;const node=make(41),other=structuredClone(node);add(other,1);let r=(node===other)+','+read(node);"
    );
    assert_eq!(common::eval(&source, "r"), "true,42");
}

/** Well-formed wire bytes containing a NaN in an i32 field fail typed validation before callbacks run. */
#[test]
fn shared_gc_typed_snapshot_values_are_revalidated() {
    let bytes = module();
    let sources = vec![format!(
        "const bytes=new Uint8Array({bytes:?});const node=new WebAssembly.Instance(new WebAssembly.Module(bytes)).exports.make(1);document.getElementById('go').addEventListener('click',()=>console.log('unsafe'));"
    )];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let mut state = serde_json::to_value(loaded.state).unwrap();
    let wire = shared_wire(&mut state).unwrap();
    let payload = wire["payload"].as_str().unwrap();
    let mut payload = payload[..payload.len() - 16].to_owned();
    payload.push_str("000000000000f87f");
    wire["payload"] = serde_json::Value::String(payload);
    let state = serde_json::from_value::<mars_v9::VmState>(state).expect("wire syntax is valid");
    assert!(mars_v9::click_page(common::input(&sources, &host, &environment), state, 3).is_err());
}

/** Native memory packets retain growable buffer mode and share page growth with sender memory. */
#[cfg(feature = "conformance")]
#[test]
fn wasm_memory_packets_preserve_growable_mode_and_native_growth() {
    let source = "const memory=new WebAssembly.Memory({initial:1,maximum:3,shared:true}),buffer=memory.toResizableBuffer();const worker=new Worker('onmessage=({data})=>{const buffer=data.buffer;buffer.grow(131072);postMessage([buffer.growable,buffer===data.toResizableBuffer(),buffer.byteLength])}',{type:'string'});worker.postMessage(memory);const response=worker.getMessage();console.log(response.join(),memory.buffer===buffer,buffer.byteLength);worker.terminateAndWait();";
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
    assert_eq!(
        common::console(&execution),
        ["true,true,131072 true 131072"]
    );
}

/** Fixed and growable wrapper metadata remains coherent through JSON state restoration. */
#[test]
fn wasm_memory_buffer_modes_survive_state_transport() {
    let sources=vec!["const memory=new WebAssembly.Memory({initial:1,maximum:4,shared:true}),fixed=memory.buffer,growable=memory.toResizableBuffer(),view=new Uint8Array(growable);view[0]=42;document.getElementById('go').addEventListener('click',()=>{memory.grow(1);console.log(growable===memory.buffer,fixed.byteLength,growable.byteLength,view.length,view[0],growable.maxByteLength)});".to_owned()];
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
    assert_eq!(
        common::console(&clicked),
        ["true 65536 131072 131072 42 262144"]
    );
}

/** A JS shared container retains the actual Wasm node across worker publication and saved state. */
#[cfg(feature = "conformance")]
#[test]
fn js_shared_container_preserves_wasm_node_across_worker_and_snapshot() {
    let bytes = module();
    let source = format!(
        "const bytes=new Uint8Array({bytes:?});const mod=new WebAssembly.Module(bytes);const {{make,read}}=new WebAssembly.Instance(mod).exports;const node=make(41),array=SharedArray(1);array[0]=node;const w=new Worker('onmessage=({{data}})=>{{const {{add}}=new WebAssembly.Instance(data.mod).exports;add(data.array[0],1);postMessage(data.array)}}',{{type:'string'}});w.postMessage({{mod,array}});const returned=w.getMessage();w.terminateAndWait();gc();let r='';setTimeout(()=>r=[returned===array,array[0]===node,read(array[0])].join(),0);"
    );
    let sources = vec![source];
    let host = common::host();
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_harmony_struct: true,
        conformance_wasm_shared: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let resumed = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state.clone()),
        loaded.effects.next_timer_ms.unwrap(),
        1,
    )
    .unwrap();
    assert_eq!(common::global(&resumed, "r"), "true,true,42");
    session.finish().unwrap();
}
