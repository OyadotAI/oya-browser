/** Template literals freeze their arrays and preserve realm/parsed-source identity without retaining dead dynamic programs. */
mod common;

/** Evaluate language behavior through the public page API. */
fn eval(source: &str) -> String {
    common::eval(source, "r")
}

/** Cooked and raw arrays freeze every element and virtual length, plus the cooked raw link. */
#[test]
fn template_arrays_are_fully_frozen() {
    assert_eq!(
        eval(
            "let s;(x=>s=x)`a${1}b`;let r=JSON.stringify([Object.isFrozen(s),Object.isFrozen(s.raw),...['0','length','raw'].map(k=>{const d=Object.getOwnPropertyDescriptor(s,k);return [d.writable,d.configurable,d.enumerable]})]);"
        ),
        "[true,true,[false,false,true],[false,false,false],[false,false,false]]"
    );
}

/** Frozen template indices and length reject strict writes rather than only preventing extensions. */
#[test]
fn frozen_template_writes_throw_and_preserve_values() {
    assert_eq!(
        eval(
            "let s;(x=>s=x)`x`;let errors=0;for(const write of [()=>{'use strict';s[0]='bad'},()=>{'use strict';s.raw.length=0},()=>s.push('bad'),()=>Object.defineProperty(s,'raw',{value:[]})]){try{write()}catch(e){if(e instanceof TypeError)errors++}}let r=errors+','+s[0]+','+s.raw.length;"
        ),
        "4,x,1"
    );
}

/** Different closures from one source parse share the literal slot, regardless of substitutions or tag identity. */
#[test]
fn one_parsed_site_reuses_its_object_across_calls_and_closures() {
    assert_eq!(
        eval(
            "function make(){return function(tag,value){return tag`a${value}b`}}const a=make(),b=make();const first=a(s=>s,1);let r=(first===a(s=>s,2))+','+(first===b(s=>s,3));"
        ),
        "true,true"
    );
}

/** Equal text in different literal expressions never aliases a call-site object. */
#[test]
fn distinct_parse_sites_remain_distinct() {
    assert_eq!(eval("const tag=s=>s;let r=tag`x`!==tag`x`;"), "true");
}

/** New Function invocations are distinct parses, including a literal in a deeper nested function. */
#[test]
fn separately_constructed_functions_do_not_share_template_sites() {
    assert_eq!(
        eval(
            "const source='return function(){return (s=>s)`x`}';const a=Function(source)(),b=Function(source)();let r=(a()===a())+','+(a()!==b());"
        ),
        "true,true"
    );
}

/** Each eval parse has its own literal while repeated execution within one eval preserves the site. */
#[test]
fn eval_parse_identity_is_separate_from_repeated_execution() {
    assert_eq!(
        eval(
            "const source='(()=>{const a=[];for(let i=0;i<2;i++)a.push((s=>s)`x`);return a})()';const a=eval(source),b=eval(source),c=(0,eval)(source);let r=(a[0]===a[1])+','+(b[0]===b[1])+','+(a[0]!==b[0])+','+(a[0]!==c[0]);"
        ),
        "true,true,true,true"
    );
}

/** Class field initializer closures use the class's parsed template identity across instances. */
#[test]
fn class_field_initializers_reuse_their_parsed_site() {
    assert_eq!(
        eval("class C{value=(s=>s)`x`}let r=(new C).value===(new C).value;"),
        "true"
    );
}

/** A syntax-invalid cooked chunk remains undefined while the frozen raw chunk keeps its source units. */
#[test]
fn invalid_tagged_escapes_have_undefined_frozen_cooked_values() {
    assert_eq!(
        eval(
            "const s=(x=>x)`\\u{invalid}`;let r=(s[0]===undefined)+','+Object.isFrozen(s)+','+Object.isFrozen(s.raw)+','+s.raw[0];"
        ),
        "true,true,true,\\u{invalid}"
    );
}

/** Committed cache identities remain stable after canonical state transport to another public request. */
#[test]
fn cache_identity_survives_state_transport() {
    let sources=vec!["const memo=new WeakMap;function tag(s){if(!memo.has(s))memo.set(s,{});return memo.get(s)}function site(){return tag`x`}const expected=site();document.getElementById('go').addEventListener('click',()=>console.log(site()===expected));".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(input(), common::round_trip(loaded.state), 3).unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["true"]);
}

/** A forged literal slot is rejected before a later handler can consume cache metadata. */
#[test]
fn saved_cache_rejects_forged_template_slots() {
    let sources=vec!["function site(){return (s=>s)`x`}site();document.getElementById('go').addEventListener('click',()=>site());".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    let mut value = serde_json::to_value(loaded.state).unwrap();
    value["templates"][0][3] = serde_json::json!(u32::MAX);
    let state = serde_json::from_value(value).unwrap();
    let error = mars_v9::click_page(input(), state, 3).err().unwrap();
    assert_eq!(error.code, "invalid_request");
}

/** Duplicate cache keys fail during incremental decoding, before an oversized metadata map can form. */
#[test]
fn saved_cache_rejects_duplicate_site_keys() {
    let loaded = common::load("(s=>s)`x`;");
    let mut value = serde_json::to_value(loaded.state).unwrap();
    let entries = value["templates"].as_array_mut().unwrap();
    entries.push(entries[0].clone());
    assert!(serde_json::from_value::<mars_v9::VmState>(value).is_err());
}

/** Native language GC and conformance contexts are exercised only when that genuine host surface is enabled. */
#[cfg(feature = "conformance")]
fn conformance(source: &str) -> mars_v9::Execution {
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let result = common::load_with(source, &environment);
    assert!(result.failed.is_empty(), "{:?}", common::failure(&result));
    result
}

/** A live parsed script keeps literal keys and their ephemeron values alive across actual major collection. */
#[cfg(feature = "conformance")]
#[test]
fn gc_preserves_weak_template_keys_for_live_code() {
    let ran = conformance(
        "const weak=new WeakMap;function tag(s){if(!weak.has(s))weak.set(s,{});return weak.get(s)}function site(){return tag`x`}const first=site();gc();gc();let r=first===site();",
    );
    assert_eq!(common::global(&ran, "r"), "true");
}

/** Source ownership discovered through a weak-map value joins the same tracing fixed point as template keys. */
#[cfg(feature = "conformance")]
#[test]
fn weakly_reached_function_keeps_its_template_source_alive() {
    let ran = conformance(
        "const memo=new WeakMap;function tag(s){if(!memo.has(s))memo.set(s,{});return memo.get(s)}const holder={},owners=new WeakMap;let f=Function('return tag`x`');owners.set(holder,f);const expected=f();f=null;gc();gc();let r=owners.get(holder)()===expected;",
    );
    assert_eq!(common::global(&ran, "r"), "true");
}

/** Dropping a dynamic program releases its cache's only strong edge to the literal array after the task ends. */
#[cfg(feature = "conformance")]
#[test]
fn dead_dynamic_program_does_not_root_template_arrays() {
    let sources=vec!["let f=Function('return (s=>s)`dynamic`');const weak=new WeakRef(f());f=null;setTimeout(()=>{gc();gc();console.log(weak.deref()===undefined)},0);".to_owned()];
    let host = common::host();
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let input = || common::input(&sources, &host, &environment);
    let loaded = mars_v9::load_page(input()).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let ran = mars_v9::run_timers(input(), loaded.state, 10.0, 10).unwrap();
    assert!(ran.failed.is_empty(), "{:?}", common::failure(&ran));
    assert_eq!(common::console(&ran), ["true"]);
}

/** Foreign literal arrays preserve their creation realm's prototypes and remain cached there. */
#[cfg(feature = "conformance")]
#[test]
fn template_objects_belong_to_the_execution_realm() {
    let ran = conformance(
        "const bank=Realm.create();const get=Realm.eval(bank,'(function(){return (s=>s)`x`})');const a=get(),b=get(),proto=Realm.eval(bank,'Array.prototype');let r=(a===b)+','+(Object.getPrototypeOf(a)===proto)+','+(Object.getPrototypeOf(a.raw)===proto)+','+(proto!==Array.prototype);",
    );
    assert_eq!(common::global(&ran, "r"), "true,true,true,true");
}
