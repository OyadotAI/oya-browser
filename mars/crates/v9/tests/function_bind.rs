/** Bound-function metadata is captured eagerly and respects the target's exotic operations. */
mod common;
use common::eval;

/** A bound function's name and length stay independent of subsequent target mutations. */
#[test]
fn metadata_is_captured_at_bind_time() {
    let source = "function f(a,b){}const bound=f.bind(null,1);\
        Object.defineProperty(f,'name',{value:'changed'});Object.defineProperty(f,'length',{value:9});\
        const r=[bound.name,bound.length].join();";
    assert_eq!(eval(source, "r"), "bound f,1");
}

/** Finite lengths truncate before subtraction; only numeric own lengths contribute. */
#[test]
fn length_converts_numeric_values_before_subtracting() {
    let source = "function f(){}const lengths=[4.2,-2,NaN,Infinity,'4'];const out=[];\
        for(const length of lengths){Object.defineProperty(f,'length',{value:length,configurable:true});out.push(f.bind(null,1).length)}\
        const r=out.join();";
    assert_eq!(eval(source, "r"), "3,0,0,Infinity,0");
}

/** Proxy traps execute at bind time in the order required by BoundFunctionCreate and bind. */
#[test]
fn proxy_metadata_traps_run_once_in_order() {
    let source = "const log=[];const proto={};function f(a,b){}\
        const proxy=new Proxy(f,{getPrototypeOf(t){log.push('proto');return proto},\
        getOwnPropertyDescriptor(t,k){log.push('own:'+k);return Reflect.getOwnPropertyDescriptor(t,k)},\
        get(t,k,r){log.push('get:'+k);return Reflect.get(t,k,r)}});\
        const bound=Function.prototype.bind.call(proxy,null,1);\
        const r=[log.join('|'),bound.name,bound.length,Object.getPrototypeOf(bound)===proto,log.length].join(';');";
    assert_eq!(
        eval(source, "r"),
        "proto|own:length|get:length|get:name;bound f;1;true;4"
    );
}

/** Metadata getters and prototype traps propagate errors immediately from the bind call. */
#[test]
fn binding_propagates_prototype_and_metadata_errors() {
    let source = "const out=[];const marker={};\
        for(const traps of [{getPrototypeOf(){throw marker}},{getOwnPropertyDescriptor(){throw marker}},{get(){throw marker}}]){\
        try{Function.prototype.bind.call(new Proxy(function(){},traps),null);out.push(false)}catch(e){out.push(e===marker)}}\
        const r=out.join();";
    assert_eq!(eval(source, "r"), "true,true,true");
}

/** Deleting captured metadata leaves it absent rather than computing it again from the target. */
#[test]
fn metadata_deletion_is_permanent() {
    let source = "function f(a){}const bound=f.bind(null);delete bound.name;delete bound.length;\
        const r=[Object.hasOwn(bound,'name'),Object.hasOwn(bound,'length'),bound.name,bound.length].join();";
    assert_eq!(eval(source, "r"), "false,false,,0");
}

/** Native source text for bound and proxy functions does not invoke a user-controlled name getter. */
#[test]
fn native_function_source_does_not_read_the_name() {
    let source = "function f(){}const bound=f.bind(null);let reads=0;\
        Object.defineProperty(bound,'name',{get(){reads++;return 'trap'}});\
        const proxy=new Proxy(f,{get(){reads++;throw 1}});\
        const r=[Function.prototype.toString.call(bound),Function.prototype.toString.call(proxy),reads].join('|');";
    assert_eq!(
        eval(source, "r"),
        "function () { [native code] }|function () { [native code] }|0"
    );
}
