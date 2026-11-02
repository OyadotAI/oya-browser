/* The d8 test-object API uses real native receiver brands and prototype inheritance without browser DOM authority. */
#![cfg(feature = "conformance")]
use crate::common;

/** The five native templates form instance inheritance only; only EventTarget and Div are namespace properties. */
#[test]
fn native_test_objects_expose_the_pinned_template_hierarchy() {
    assert_eq!(
        run(
            "const D=d8.dom.Div,E=d8.dom.EventTarget;const levels=[];let p=D.prototype;for(let i=0;i<5;i++){levels.push(p);p=Object.getPrototypeOf(p)}const d=new D;const r=[Object.keys(d8.dom).sort().join(),levels.map(p=>p.constructor.name).join(':'),levels[4]===E.prototype,p===Object.prototype,levels.every(p=>Object.getPrototypeOf(p.constructor)===Function.prototype),d instanceof D,d instanceof E,Object.getOwnPropertyDescriptor(D,'prototype').writable].join(';');"
        ),
        "Div,EventTarget;Div::::EventTarget;true;true;true;true;true;true"
    );
}

/** NodeTypeCallback has the actual accessor flags and only accepts native Node-derived receivers. */
#[test]
fn node_type_descriptor_and_receiver_signature_match_the_native_api() {
    assert_eq!(
        run(
            "const div=new d8.dom.Div;const node=Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(d8.dom.Div.prototype)));const descriptor=Object.getOwnPropertyDescriptor(node,'nodeType');let n=0;for(const value of [{},Object.create(div),Object.create(d8.dom.Div.prototype),new d8.dom.EventTarget,new Proxy(div,{}),null,undefined]){try{descriptor.get.call(value)}catch(e){if(e instanceof TypeError)n++}}let construct=false;try{new descriptor.get}catch(e){construct=e instanceof TypeError}const r=[div.nodeType,descriptor.get.name,descriptor.get.length,descriptor.set===undefined,descriptor.enumerable,descriptor.configurable,construct,n].join(';');"
        ),
        "1;;0;true;true;true;true;7"
    );
}

/** API calls without new return converted receivers and do not turn arbitrary objects into native nodes. */
#[test]
fn null_native_callbacks_return_receivers_without_granting_brands() {
    assert_eq!(
        run(
            "const node=Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(d8.dom.Div.prototype)));const getter=Object.getOwnPropertyDescriptor(node,'nodeType').get;const plain={};const result=d8.dom.Div.call(plain,{toString(){throw 1}});let rejected=false;try{getter.call(plain)}catch(e){rejected=e instanceof TypeError}const boxed=d8.dom.EventTarget.call(7);const r=[result===plain,rejected,d8.dom.Div()===d8.dom,d8.dom.Div.call(null)===globalThis,boxed.valueOf(),Object.getPrototypeOf(boxed)===Number.prototype].join();"
        ),
        "true,true,true,true,7,true"
    );
}

/** Subclasses and Reflect.construct use the real newTarget prototype while retaining the native allocation's brand. */
#[test]
fn subclasses_and_new_target_keep_native_state_and_prototype_selection() {
    assert_eq!(
        run(
            "class C extends d8.dom.Div{}const a=new C;function Target(){}const selected=Target.prototype;let reads=0;const target=new Proxy(Target,{get(t,k,r){if(k==='prototype')reads++;return Reflect.get(t,k,r)}});const b=Reflect.construct(d8.dom.Div,[],target);const node=Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(d8.dom.Div.prototype)));const getter=Object.getOwnPropertyDescriptor(node,'nodeType').get;Target.prototype=3;const c=Reflect.construct(d8.dom.Div,[],Target);const r=[a instanceof C,a instanceof d8.dom.Div,a.nodeType,Object.getPrototypeOf(b)===selected,getter.call(b),reads,Object.getPrototypeOf(c)===Object.prototype,getter.call(c)].join();"
        ),
        "true,true,1,true,1,1,true,1"
    );
}

/** Changing JavaScript-visible constructor, tag and prototype state cannot forge or erase an allocation signature. */
#[test]
fn native_brands_are_independent_of_mutable_public_properties() {
    assert_eq!(
        run(
            "const div=new d8.dom.Div;const node=Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(d8.dom.Div.prototype)));const getter=Object.getOwnPropertyDescriptor(node,'nodeType').get;div.constructor=null;div[Symbol.toStringTag]='Number';Object.setPrototypeOf(div,null);const copy=Object.assign({},div);let rejected=false;try{getter.call(copy)}catch(e){rejected=e instanceof TypeError}const r=[getter.call(div),rejected,Reflect.ownKeys(div).includes('d8-dom-native-type')].join();"
        ),
        "1,true,false"
    );
}

/** Native test objects are not structured data, so neither browser nor shell serializers can manufacture cloned brands. */
#[test]
fn native_test_objects_are_rejected_by_clone_and_worker_transports() {
    let session = mars_v9::ConformanceWorkerSession::new();
    let environment = mars_v9::PageEnvironment {
        conformance_workers: Some(session.host()),
        ..environment()
    };
    let execution = common::load_with(
        r#"const div=new d8.dom.Div;
        const worker=new Worker('onmessage=()=>{}',{type:'string'});
        let n=0;
        for(const value of [div,new d8.dom.EventTarget]){
            for(const operation of [()=>structuredClone(value),()=>worker.postMessage(value)]){
                try{operation()}catch(e){if(e.name==='DataCloneError')n++}
            }
        }
        let shell=false;
        try{d8.serializer.serialize(div)}catch(e){shell=e instanceof Error}
        worker.terminateAndWait();
        const r=[n,shell].join();"#,
        &environment,
    );
    check(&execution);
    session.finish().expect("worker exits");
    assert_eq!(common::global(&execution, "r"), "4,true");
}

/** Real isolate snapshots preserve native allocation state and reject malformed private type values before executing callbacks. */
#[test]
fn saved_native_brands_round_trip_and_malformed_types_are_rejected() {
    let sources = vec!["const div=new d8.dom.Div;let r;setTimeout(()=>{r=div.nodeType},0);".into()];
    let environment = environment();
    let host = common::host();
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("page loads");
    check(&execution);
    let mut wire = serde_json::to_value(&execution.state).expect("state serializes");
    let restored = mars_v9::run_timers(
        common::input(&sources, &host, &environment),
        common::round_trip(execution.state),
        0.0,
        1,
    )
    .expect("restored callback");
    check(&restored);
    assert_eq!(common::global(&restored, "r"), "1");
    assert!(forge_type(&mut wire));
    let malformed = serde_json::from_value(wire).expect("inert state decodes");
    assert!(
        mars_v9::run_timers(
            common::input(&sources, &host, &environment),
            malformed,
            0.0,
            1
        )
        .is_err()
    );
}

/** Each fresh shell context creates its own native templates; borrowing another template's signature cannot admit a foreign object. */
#[test]
fn native_template_signatures_belong_to_their_creation_context() {
    assert_eq!(
        run(
            "const child=Realm.create();const foreign=Realm.eval(child,'new d8.dom.Div');const own=new d8.dom.Div;const node=Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(d8.dom.Div.prototype)));const get=Object.getOwnPropertyDescriptor(node,'nodeType').get;const foreignGet=Realm.eval(child,\"Object.getOwnPropertyDescriptor(Object.getPrototypeOf(Object.getPrototypeOf(Object.getPrototypeOf(d8.dom.Div.prototype))),'nodeType').get\");const foreignError=Realm.eval(child,'TypeError');let n=0;for(const [f,v,E] of [[get,foreign,TypeError],[foreignGet,own,foreignError]]){try{f.call(v)}catch(e){if(e instanceof E&&Object.getPrototypeOf(e)===E.prototype&&e.message==='Illegal invocation')n++}}const r=[get.call(own),foreignGet.call(foreign),n].join();"
        ),
        "1,1,2"
    );
}

/** Host test extensions are enabled solely by the build capability and actual shell environment. */
fn environment() -> mars_v9::PageEnvironment {
    mars_v9::PageEnvironment {
        conformance_d8: true,
        ..common::environment()
    }
}

/** Every test observes native execution failures instead of accepting a missing feature as success. */
fn check(execution: &mars_v9::Execution) {
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(execution)
    );
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
}

/** Evaluate through the same public page boundary used by the actual conformance host. */
fn run(source: &str) -> String {
    let execution = common::load_with(source, &environment());
    check(&execution);
    common::global(&execution, "r")
}

/** Corrupt an existing number field without changing accounting or granting a new private property. */
fn forge_type(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(entries) => {
            let brand =
                serde_json::json!({"p":"d8-dom-native-type".encode_utf16().collect::<Vec<_>>()});
            if let Some(entry) = entries
                .iter_mut()
                .find(|entry| entry.get(0) == Some(&brand))
            {
                entry[1] = serde_json::to_value(mars_v9::JsValue::Number(999.0))
                    .expect("invalid discriminator");
                return true;
            }
            entries.iter_mut().any(forge_type)
        }
        serde_json::Value::Object(fields) => fields.values_mut().any(forge_type),
        _ => false,
    }
}
