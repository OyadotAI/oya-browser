//! Global eval declarations create own bindings before initialization, even over inherited setters.
#![cfg(feature = "conformance")]
use crate::common;

#[test]
fn global_eval_var_shadows_inherited_setter() {
    let source = "var hits=0;Object.getPrototypeOf(globalThis).__defineSetter__('probe',function(){hits++});eval('var probe=1');const r=[hits,Object.hasOwn(globalThis,'probe'),globalThis.probe].join();";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0,true,1");
}

/** Nested sloppy eval must create its global var before resolving the initializer through with. */
#[test]
fn nested_global_eval_var_shadows_inherited_setter() {
    let source = "var hits=0;this.__defineSetter__('a',function(v){});eval('var a=1');eval(\"with({}) { eval('var a=2') }\");this.__defineSetter__('a',function(v){});eval('function a(){}');this.__defineSetter__('b',function(v){});eval('const b=3');try{eval(\"with({}) { eval('const b=23') }\")}catch(e){}this.__defineSetter__('c',function(v){throw 42});try{eval('var c=1')}catch(e){}__proto__.__defineSetter__('aa',function(){hits++});eval('var aa=1');__proto__.__defineSetter__('probe',function(){hits++});eval(\"with({}) { eval('var probe=1') }\");const r=[hits,Object.hasOwn(globalThis,'probe'),globalThis.probe].join();";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "0,true,1");
}

/** Each shell realm gets a distinct prototype above its own Object.prototype. */
#[test]
fn shell_global_prototypes_are_distinct_from_ordinary_objects() {
    let source = "const r=[Object.getPrototypeOf(globalThis)!==Object.prototype,Realm.eval(Realm.create(),'Object.getPrototypeOf(globalThis)!==Object.prototype')].join();";
    let mut environment = common::environment();
    environment.conformance_d8 = true;
    let execution = common::load_with(source, &environment);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true");
}
