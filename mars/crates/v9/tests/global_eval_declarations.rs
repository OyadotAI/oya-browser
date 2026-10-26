/** Global eval validates every declaration before mutation and defines functions without invoking accessors or changing protected attributes. */
mod common;

/** A global function made nonwritable and nonconfigurable cannot be silently replaced by sloppy eval. */
#[test]
fn readonly_global_functions_reject_eval_redeclaration() {
    let source = "function locked(){}Object.defineProperty(globalThis,'locked',{value:1,writable:false});let caught=false,ran=false;try{eval('function locked(){};ran=true')}catch(error){caught=error instanceof TypeError}const descriptor=Object.getOwnPropertyDescriptor(globalThis,'locked');const r=[caught,ran,locked,descriptor.writable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "true,false,1,false,false");
}

/** Configurable read-only properties can become writable enumerable eval functions with configurable attributes. */
#[test]
fn configurable_globals_are_replaced_with_function_data_descriptors() {
    let source = "Object.defineProperty(globalThis,'replaceable',{value:1,writable:false,enumerable:false,configurable:true});let initial;eval('initial=replaceable;function replaceable(){return 345}');const descriptor=Object.getOwnPropertyDescriptor(globalThis,'replaceable');const r=[typeof initial,initial(),descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "function,345,true,true,true");
}

/** Admissible nonconfigurable data properties retain their exact attributes during function initialization. */
#[test]
fn nonconfigurable_writable_enumerable_globals_keep_attributes() {
    let source = "Object.defineProperty(globalThis,'existing',{value:1,writable:true,enumerable:true,configurable:false});let initial;(0,eval)('initial=existing;function existing(){return 2222}');const descriptor=Object.getOwnPropertyDescriptor(globalThis,'existing');const r=[initial(),existing(),descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "2222,2222,true,true,false");
}

/** Function declarations replace configurable accessors without consulting their getter or setter. */
#[test]
fn function_initialization_never_calls_replaced_accessors() {
    let source = "let reads=0,writes=0;Object.defineProperty(globalThis,'accessorFunction',{get(){reads++;throw 'get'},set(value){writes++;throw 'set'},enumerable:false,configurable:true});eval('function accessorFunction(){return 7}');const descriptor=Object.getOwnPropertyDescriptor(globalThis,'accessorFunction');const r=[reads,writes,accessorFunction(),typeof descriptor.value,descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "0,0,7,function,true,true,true");
}

/** Nonconfigurable accessors, nonenumerable data and read-only data all fail admission without author accessors. */
#[test]
fn protected_global_descriptors_reject_function_declarations() {
    let source = "let touched=0;Object.defineProperty(globalThis,'blockedAccessor',{get(){touched++;return 1},set(v){touched++},enumerable:true,configurable:false});Object.defineProperty(globalThis,'blockedHidden',{value:1,writable:true,enumerable:false,configurable:false});Object.defineProperty(globalThis,'blockedReadonly',{value:1,writable:false,enumerable:true,configurable:false});const out=[];for(const name of ['blockedAccessor','blockedHidden','blockedReadonly']){try{eval('function '+name+'(){}');out.push(false)}catch(error){out.push(error instanceof TypeError)}}out.push(touched);const r=out.join();";
    assert_eq!(common::eval(source, "r"), "true,true,true,0");
}

/** A forbidden function prevents every unrelated variable and function from being created in either eval entry form. */
#[test]
fn global_function_preflight_is_atomic_before_any_binding_creation() {
    let source = "Object.defineProperty(globalThis,'preflightLocked',{value:1,writable:false,configurable:false});const out=[];const code='var preflightVar;function preflightFunction(){}function preflightLocked(){}';for(const indirect of [false,true]){try{if(indirect)(0,eval)(code);else eval(code);out.push(false)}catch(error){out.push(error instanceof TypeError)}out.push(Object.hasOwn(globalThis,'preflightVar'),Object.hasOwn(globalThis,'preflightFunction'))}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,false,false,true,false,false"
    );
}

/** Own setters still observe var initializers, while declaration-only eval leaves their descriptors and values untouched. */
#[test]
fn var_initialization_preserves_own_accessor_binding_behavior() {
    let source = "let values=[];Object.defineProperty(globalThis,'varAccessor',{set(value){values.push(value)},configurable:true});eval('var varAccessor');eval('var varAccessor=1');eval(\"with({}){eval('var varAccessor=2')}\");const descriptor=Object.getOwnPropertyDescriptor(globalThis,'varAccessor');const r=[values.join(':'),'value' in descriptor,typeof descriptor.set].join();";
    assert_eq!(common::eval(source, "r"), "1:2,false,function");
}

/** An inherited property does not satisfy CreateGlobalVarBinding and its setter does not run for ordinary global initialization. */
#[test]
fn plain_global_var_declarations_shadow_inherited_setters() {
    let source = "let called=0;Object.defineProperty(Object.prototype,'inheritedGlobalVar',{set(value){called++},configurable:true});eval('var inheritedGlobalVar=7');const descriptor=Object.getOwnPropertyDescriptor(globalThis,'inheritedGlobalVar');delete Object.prototype.inheritedGlobalVar;const r=[called,descriptor.value,descriptor.writable,descriptor.enumerable,descriptor.configurable].join();";
    assert_eq!(common::eval(source, "r"), "0,7,true,true,true");
}

/** A with environment still intercepts the initializer through HasProperty, even after eval creates the separate global binding. */
#[test]
fn nested_with_eval_keeps_inherited_setter_resolution() {
    let source = "let called=0,value;Object.defineProperty(Object.prototype,'withInheritedVar',{set(next){called++;value=next},configurable:true});eval(\"with({}){eval('var withInheritedVar=2')}\");const descriptor=Object.getOwnPropertyDescriptor(globalThis,'withInheritedVar');delete Object.prototype.withInheritedVar;const r=[called,value,!!descriptor,descriptor.value===undefined].join();";
    assert_eq!(common::eval(source, "r"), "1,2,true,true");
}

/** Existing globals remain usable when extensibility is removed, but new eval declarations fail before installing any function. */
#[test]
fn nonextensible_global_var_preflight_prevents_partial_function_creation() {
    let source = "Object.defineProperty(globalThis,'existingExtensibleCase',{value:1,writable:true,enumerable:true,configurable:false});Object.preventExtensions(globalThis);let caught=false;try{eval('function existingExtensibleCase(){return 2};var missingGlobalVar')}catch(error){caught=error instanceof TypeError}const r=[caught,existingExtensibleCase,Object.hasOwn(globalThis,'missingGlobalVar')].join();";
    assert_eq!(common::eval(source, "r"), "true,1,false");
}

/** Optional script block-function hoists remain harmless on a frozen global, while required declarations still fail before bodies. */
#[test]
fn optional_annex_script_hoists_do_not_become_required_declaration_errors() {
    let sources=vec!["let observed=0;Object.preventExtensions(globalThis);".into(),"{function optionalAnnex(){}}observed++;".into(),"var requiredShouldFail=1;{function optionalShouldNotExist(){}}observed++;".into(),"const r=[observed,Object.hasOwn(globalThis,'optionalAnnex'),Object.hasOwn(globalThis,'requiredShouldFail'),Object.hasOwn(globalThis,'optionalShouldNotExist')].join();".into()];
    let host = common::host();
    let environment = common::environment();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(execution.failed.len(), 1, "{:?}", execution.failed);
    assert!(common::failure(&execution).unwrap().contains("SyntaxError"));
    assert_eq!(common::global(&execution, "r"), "1,false,false,false");
}

/** Script instantiation uses SyntaxError and leaves other declarations absent when a prior script protected the function name. */
#[test]
fn script_redeclaration_preflight_throws_syntax_error_before_mutation() {
    let sources = vec![
        "var scriptLocked=1;Object.defineProperty(globalThis,'scriptLocked',{writable:false});"
            .into(),
        "var scriptShouldNotExist=2;function scriptLocked(){}".into(),
        "const r=[scriptLocked,Object.hasOwn(globalThis,'scriptShouldNotExist')].join();".into(),
    ];
    let host = common::host();
    let environment = common::environment();
    let execution = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert_eq!(execution.failed.len(), 1);
    assert!(common::failure(&execution).unwrap().contains("SyntaxError"));
    assert_eq!(common::global(&execution, "r"), "1,false");
}

/** Function initialization preserves the final declaration and nonconfigurable flags after public state transport. */
#[test]
fn global_function_descriptors_survive_state_transport() {
    let sources=vec!["Object.defineProperty(globalThis,'transportFunction',{value:1,writable:true,enumerable:true,configurable:false});eval('function transportFunction(){return 1}function transportFunction(){return 7}');document.getElementById('go').addEventListener('click',()=>{const d=Object.getOwnPropertyDescriptor(globalThis,'transportFunction');console.log(transportFunction(),d.writable,d.enumerable,d.configurable)});".into()];
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
    assert_eq!(common::console(&clicked), ["7 true true false"]);
}
