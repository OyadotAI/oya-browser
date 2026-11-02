/** The canonical String prototype has real empty StringData before and after lazy heap materialization. */
use crate::common;

/** Every ordinary observation runs through the public library and actual VM. */
fn result(source: &str) -> String {
    common::eval(source, "r")
}

/** The pristine prototype is an empty native wrapper, including when generic string methods convert it. */
#[test]
fn pristine_string_prototype_has_empty_native_string_data() {
    let actual = result(
        "const p=String.prototype;const r=[p.valueOf()==='',p.toString()==='',String(p)==='',p.trim()==='',p.padStart(3,'.'),p.charAt(0)==='',Object.prototype.toString.call(p)].join('|');",
    );
    assert_eq!(actual, "true|true|true|true|...|true|[object String]");
}

/** Expando writes and prototype replacement materialize ordinary attributes without changing native StringData. */
#[test]
fn prototype_mutation_keeps_the_empty_wrapper_brand() {
    let actual = result(
        "const p=String.prototype;p.extra={value:7};p[0]='ordinary';Object.setPrototypeOf(p,null);const r=[p.valueOf()==='',p.toString()==='',p.extra.value,p[0],p.length,Object.getPrototypeOf(p)===null,Object.prototype.toString.call(p)].join('|');",
    );
    assert_eq!(actual, "true|true|7|ordinary|0|true|[object String]");
}

/** Inheriting from a string wrapper or forging its public tag/constructor never grants the native slot. */
#[test]
fn inherited_and_proxy_receivers_cannot_forge_string_data_or_run_get_traps() {
    let actual = result(
        "const p=String.prototype;let errors=0,gets=0;const fake={constructor:String,[Symbol.toStringTag]:'String'};const proxy=new Proxy(p,{get(){gets++;throw 1}});for(const method of [p.valueOf,p.toString])for(const receiver of [Object.create(p),fake,proxy,{},null,undefined]){try{method.call(receiver)}catch(e){if(e instanceof TypeError)errors++}}const r=[errors,gets,p.valueOf.call('raw')].join('|');",
    );
    assert_eq!(actual, "12|0|raw");
}

/** Empty StringData has no character indices, and adding an ordinary index does not change its fixed length. */
#[test]
fn empty_string_prototype_preserves_length_and_index_descriptors() {
    let actual = result(
        "const p=String.prototype;const before=Object.getOwnPropertyDescriptor(p,'length');let absent=!Object.hasOwn(p,'0');Object.defineProperty(p,'0',{value:'own',configurable:true,writable:true,enumerable:true});let rejected=false;try{Object.defineProperty(p,'length',{value:1})}catch(e){rejected=e instanceof TypeError}const r=[before.value,before.writable,before.enumerable,before.configurable,absent,p[0],p.charAt(0)==='',p.valueOf()==='',rejected].join();",
    );
    assert_eq!(actual, "0,false,false,false,true,own,true,true,true");
}

/** Foreign canonical prototypes have their own native identity and cross-borrowed String methods accept the same real slot. */
#[cfg(feature = "conformance")]
#[test]
fn foreign_string_prototypes_and_methods_preserve_real_native_brands() {
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "const id=Realm.create();const p=Realm.eval(id,'String.prototype');const f=Realm.eval(id,'String.prototype.valueOf');const foreignType=Realm.eval(id,'TypeError');let foreignError=false;try{f.call({})}catch(e){foreignError=e.constructor===foreignType}const r=[String.prototype.valueOf.call(p)==='',f.call(String.prototype)==='',Object.prototype.toString.call(p),p!==String.prototype,foreignError].join('|');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true|true|[object String]|true|true"
    );
}

/** Real materialized empty payloads survive collection and transport, while a forged replacement payload is rejected. */
#[test]
fn string_prototype_payload_is_immutable_across_state_transport() {
    let sources=vec!["const p=String.prototype;p.extra={value:9};let r;setTimeout(()=>{r=[p.valueOf()==='',p.extra.value,Object.prototype.toString.call(p)].join('|')},0);".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let first = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(first.failed.is_empty(), "{:?}", common::failure(&first));
    let mut forged = serde_json::to_value(&first.state).unwrap();
    let entry = forged["heap"][0]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|entry| {
            let payload = &entry[1]["c"]["string"];
            payload == "" || payload["k"][1] == ""
        })
        .expect("actual materialized empty wrapper in compact heap record");
    entry[1]["c"]["string"] = serde_json::json!("forged payload");
    let forged = serde_json::from_value(forged).unwrap();
    let error = mars_v9::run_timers(common::input(&sources, &host, &environment), forged, 0.0, 1)
        .unwrap_err();
    assert!(error.message.contains("String prototype"), "{error:?}");
    let execution = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(first.state),
        0.0,
        1,
    )
    .unwrap();
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true|9|[object String]");
}

/** Structured clone serializes the actual empty wrapper and ignores ordinary properties as it does for boxed strings. */
#[test]
fn pristine_and_materialized_prototypes_clone_as_real_empty_string_wrappers() {
    let actual = result(
        "const p=String.prototype;const pristine=structuredClone(p);let reads=0;Object.defineProperty(p,'extra',{enumerable:true,get(){reads++;throw 1}});const pair=structuredClone([p,p]);const r=[pristine!==p,pristine.valueOf()==='',Object.getPrototypeOf(pristine)===p,pair[0]===pair[1],pair[0].valueOf()==='',Object.hasOwn(pair[0],'extra'),reads].join();",
    );
    assert_eq!(actual, "true,true,true,true,true,false,0");
}

/** Serializer and Worker cloning consume the same real internal slot without exposing prototype functions or native identities. */
#[cfg(feature = "conformance")]
#[test]
fn native_graph_codecs_preserve_the_empty_wrapper_and_graph_aliases() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_d8: true,
        conformance_workers: Some(session.host()),
        ..common::environment()
    };
    let execution = common::load_with(
        "const p=String.prototype;const copy=d8.serializer.deserialize(d8.serializer.serialize(p));const w=new Worker(\"onmessage=({data})=>postMessage([typeof data[0],data[0].valueOf()==='',data[0]===data[1],Object.prototype.toString.call(data[0])].join('|'))\",{type:'string'});w.postMessage([p,p]);const remote=w.getMessage();w.terminateAndWait();const r=[copy!==p,copy.valueOf()==='',Object.prototype.toString.call(copy),remote].join(';');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    session.finish().unwrap();
    assert_eq!(
        common::global(&execution, "r"),
        "true;true;[object String];object|true|true|[object String]"
    );
}
