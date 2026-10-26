/** Read-only backing capabilities remain native data across species, workers, wire streams and saved requests. */
mod common;

/** A subarray retains readonly aliases while independent copies receive writable storage. */
#[test]
fn readonly_methods_and_copying_methods_preserve_their_distinct_backings() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(new Uint8Array([3,1,2]).buffer.transferToImmutable());const b=a.subarray(1);const c=a.slice();c[0]=9;const r=[b.buffer===a.buffer,b.buffer.immutable,b.join(),a.toReversed().join(),a.toSorted().join(),a.with(0,7).join(),a.map(x=>x+1).buffer.immutable,c.buffer.immutable,a[0]].join(';');",
            "r"
        ),
        "true;true;1,2;2,1,3;1,2,3;7,1,2;false;false;3"
    );
}

/** Write-producing constructors reject read-only targets before mapper calls, including zero-length outputs. */
#[test]
fn typed_species_and_static_constructors_validate_output_writability() {
    assert_eq!(
        common::eval(
            "let errors=0,maps=0;const a=new Uint8Array([1]);for(const length of [0,1]){const target=new Uint8Array(new ArrayBuffer(length).transferToImmutable());function C(){return target}const source=new Uint8Array(length);source.constructor={[Symbol.species]:C};for(const f of [()=>source.map(x=>{maps++;return x}),()=>source.filter(()=>false),()=>source.slice(),()=>Uint8Array.from.call(C,[],()=>{maps++;return 1}),()=>Uint8Array.of.call(C)]){try{f()}catch(e){if(e instanceof TypeError)errors++}}}const r=[errors,maps].join();",
            "r"
        ),
        "10,0"
    );
}

/** Worker records carry real read-only byte buffers and retain all graph-local aliases. */
#[cfg(feature = "conformance")]
#[test]
fn worker_clone_preserves_immutable_bytes_and_reference_identity() {
    assert_eq!(
        worker(
            "const worker=new Worker('onmessage=({data})=>postMessage(data)',{type:'string'});const buffer=new Uint8Array([4,7]).buffer.transferToImmutable();const view=new Uint8Array(buffer);const data={buffer,view,again:view};data.self=data;worker.postMessage(data);const copy=worker.getMessage();let rejected=false;try{copy.view.fill(9)}catch(e){rejected=e instanceof TypeError}console.log([copy.self===copy,copy.view===copy.again,copy.view.buffer===copy.buffer,copy.buffer.immutable,copy.view.join(),rejected].join(';'));worker.terminateAndWait();"
        ),
        ["true;true;true;true;4,7;true"]
    );
}

/** A transfer-list validation failure cannot detach immutable or earlier mutable entries. */
#[cfg(feature = "conformance")]
#[test]
fn worker_transfer_lists_reject_immutable_without_partial_detachment() {
    assert_eq!(
        worker(
            "const worker=new Worker('onmessage=()=>{}',{type:'string'});const a=new ArrayBuffer(1),b=new ArrayBuffer(2).transferToImmutable();let rejected=false;try{worker.postMessage({a,b},[a,b])}catch(e){rejected=e.name==='DataCloneError'}console.log([rejected,a.detached,b.detached,b.byteLength].join());worker.terminateAndWait();"
        ),
        ["true,false,false,2"]
    );
}

/** Native V8's immutable tag is decoded and emitted as a byte record, without application wrappers. */
#[cfg(feature = "conformance")]
#[test]
fn native_serializer_preserves_immutable_tags_and_nested_views() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const read=d8.serializer.deserialize(Uint8Array.from([255,16,67,2,4,8]).buffer);const view=new Uint8Array(read);const copy=d8.serializer.deserialize(d8.serializer.serialize({read,view}));const wire=Array.from(new Uint8Array(d8.serializer.serialize(read)));console.log([read.immutable,view.join(),copy.read.immutable,copy.view.buffer===copy.read,wire.join()].join(';'));",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::console(&execution),
        ["true;4,8;true;true;255,16,67,2,4,8"]
    );
}

/** The browser clone path preserves the same immutable backing rules independently of Worker support. */
#[test]
fn structured_clone_keeps_readonly_backing_and_overlapping_views() {
    assert_eq!(
        common::eval(
            "const buffer=new Uint8Array([5,6,7]).buffer.transferToImmutable();const a=new Uint8Array(buffer),b=new DataView(buffer,1);const copy=structuredClone({buffer,a,b});let rejected=false;try{copy.b.setUint8(0,9)}catch(e){rejected=e instanceof TypeError}const r=[copy.buffer!==buffer,copy.a.buffer===copy.buffer,copy.b.buffer===copy.buffer,copy.buffer.immutable,copy.a.join(),rejected].join(';');",
            "r"
        ),
        "true;true;true;true;5,6,7;true"
    );
}

/** Engine-owned Wasm memory cannot acquire the capability through author transfer operations. */
#[test]
fn wasm_memory_backings_remain_protected_from_immutable_transfer() {
    assert_eq!(
        common::eval(
            "const memory=new WebAssembly.Memory({initial:1,maximum:2});const before=memory.buffer;let rejected=false;try{before.transferToImmutable()}catch(e){rejected=e instanceof TypeError}new Uint8Array(before)[0]=11;const r=[rejected,before.detached,before.immutable,memory.grow(1),new Uint8Array(memory.buffer)[0]].join();",
            "r"
        ),
        "true,false,false,1,11"
    );
}

/** A malformed snapshot cannot combine resizable and immutable internal slots. */
#[test]
fn malformed_immutable_snapshot_is_rejected_before_callbacks() {
    let sources = vec!["globalThis.bad=new ArrayBuffer(2,{maxByteLength:4});setTimeout(()=>{throw 'must not run'},0);".into()];
    let environment = common::environment();
    let host = common::host();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).expect("load");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let mut wire = serde_json::to_value(execution.state).expect("serialize");
    let mut added = false;
    forge_immutable(&mut wire, &mut added);
    assert!(added, "found actual resizable buffer slot");
    let state = serde_json::from_value(wire).expect("inert state decodes");
    assert!(
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1).is_err()
    );
}

/** Add a private immutable slot alongside an actual maximum slot to exercise public state validation. */
fn forge_immutable(value: &mut serde_json::Value, added: &mut bool) {
    match value {
        serde_json::Value::Array(entries) => {
            let maximum =
                serde_json::json!({"p":"array-buffer-maximum".encode_utf16().collect::<Vec<_>>()});
            if !*added && entries.iter().any(|entry| entry.get(0) == Some(&maximum)) {
                let mut entry = entries
                    .iter()
                    .find(|entry| entry.get(0) == Some(&maximum))
                    .expect("maximum entry")
                    .clone();
                entry[0] = serde_json::json!({"p":"array-buffer-immutable".encode_utf16().collect::<Vec<_>>()});
                entry[1] =
                    serde_json::to_value(mars_v9::JsValue::Boolean(true)).expect("bool value");
                entries.push(entry);
                *added = true;
            }
            for entry in entries {
                forge_immutable(entry, added);
            }
        }
        serde_json::Value::Object(fields) => {
            for entry in fields.values_mut() {
                forge_immutable(entry, added);
            }
        }
        _ => {}
    }
}

/** Worker lifetimes belong to one hermetic session and all native child failures are checked. */
#[cfg(feature = "conformance")]
fn worker(source: &str) -> Vec<String> {
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
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
    session.finish().expect("worker native operations succeed");
    common::console(&execution)
}
