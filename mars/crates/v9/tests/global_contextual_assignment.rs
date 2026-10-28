/** Contextual global stores preserve V8's prototype and strict-assignment behavior. */
mod common;

/** Strict inherited writable data is readable but cannot become a new global binding. */
#[test]
fn strict_inherited_data_is_not_a_binding() {
    let source = "const obj=Object, ref=ReferenceError, global=globalThis; const proto={x:1}; obj.setPrototypeOf(global,proto);
        let errors=0; function f(){'use strict';try{x=42}catch(e){if(e instanceof ref)errors++}} f();f();
        const r=[errors,x,typeof x,obj.hasOwn(global,'x')].join();";
    assert_eq!(common::eval(source, "r"), "2,1,number,false");
}

/** Inherited setters run and readonly descriptors fail with TypeError under strict assignment. */
#[test]
fn strict_inherited_accessors_and_readonly_properties_keep_set_semantics() {
    let source = "const obj=Object, typ=TypeError; let saved=0; const proto={set s(v){saved=v},get g(){return 1}};
        obj.defineProperty(proto,'n',{value:1,writable:false}); obj.setPrototypeOf(globalThis,proto);
        let a=false,b=false; function f(){'use strict';s=9;try{g=1}catch(e){a=e instanceof typ}try{n=1}catch(e){b=e instanceof typ}}f();
        const r=[saved,a,b].join();";
    assert_eq!(common::eval(source, "r"), "9,true,true");
}

/** Non-strict assignment creates an own property and strict own assignment remains valid. */
#[test]
fn sloppy_and_own_global_assignments_continue_to_work() {
    let source = "const obj=Object, global=globalThis; obj.setPrototypeOf(global,{x:1}); x=3;
        function f(){'use strict';x=4}f();const r=[x,obj.hasOwn(global,'x')].join();";
    assert_eq!(common::eval(source, "r"), "4,true");
}

/** An inherited proxy sees only its set trap, not lookup traps from a preflight. */
#[test]
fn strict_global_store_calls_proxy_set_without_preflight_traps() {
    let source = "const obj=Object, P=Proxy;let seen='';const p=new P({}, {set(t,k,v){seen+='s';return true},has(){seen+='h';return false},getOwnPropertyDescriptor(){seen+='d'}});
        obj.setPrototypeOf(globalThis,p);function f(){'use strict';x=4}f();const r=seen;";
    assert_eq!(common::eval(source, "r"), "s");
}

/** Missing names throw, own properties and built-in Window bindings remain assignable. */
#[test]
fn strict_global_stores_preserve_own_and_window_bindings() {
    let source = "let caught=false;globalThis.ownValue=1;function f(){'use strict';try{missingName=1}catch(e){caught=e instanceof ReferenceError}ownValue=2;fetch=3}f();const r=[caught,ownValue,fetch].join();";
    assert_eq!(common::eval(source, "r"), "true,2,3");
}

/** A rejecting proxy reports the strict write failure rather than silently succeeding. */
#[test]
fn strict_global_store_rejects_a_false_proxy_set() {
    let source = "const obj=Object, P=Proxy, typ=TypeError;let caught=false;obj.setPrototypeOf(globalThis,new P({}, {set(){return false}}));function f(){'use strict';try{x=4}catch(e){caught=e instanceof typ}}f();const r=caught;";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Strict direct and indirect eval follow global contextual stores, while with setters still run. */
#[test]
fn strict_eval_and_with_resolve_their_own_environments() {
    let source = "const obj=Object, ref=ReferenceError;globalThis.eval=eval;obj.setPrototypeOf(globalThis,{inheritedName:1});let direct=false,indirect=false,saved=0;
        try{eval(\"'use strict';inheritedName=2\")}catch(e){direct=e instanceof ref}
        try{(0,eval)(\"'use strict';inheritedName=2\")}catch(e){indirect=e instanceof ref}
        const scope={set target(v){saved=v}};with(scope){target=7}const r=[direct,indirect,saved].join();";
    assert_eq!(common::eval(source, "r"), "true,true,7");
}

/** An exhausted prototype walk stops at the configured bound before creating a global. */
#[test]
fn strict_global_assignment_respects_prototype_chain_limit() {
    let source = "const obj=Object, global=globalThis;let tail={};obj.setPrototypeOf(global,tail);
        for(let i=0;i<4100;i++){let next={};obj.setPrototypeOf(tail,next);tail=next}
        function f(){'use strict';ghost=1}f();";
    let result = common::load(source);
    let failure = result
        .failed
        .first()
        .expect("prototype limit stops the script");
    assert_eq!(failure.error.code, "limit_exceeded");
    assert!(failure.error.message.contains("prototype chain"));
}
