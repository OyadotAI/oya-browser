/** Reflect invocation validates callable/constructor targets and genuine array-like lists before reading their elements. */
use crate::common;

/** Reflect rejects null argument lists while Function.apply explicitly accepts them as empty lists. */
#[test]
fn reflect_lists_require_objects_and_apply_keeps_its_nullish_rule() {
    assert_eq!(
        common::eval(
            "let count=0;for(const list of [null,undefined,3,'x'])try{Reflect.apply(()=>0,null,list)}catch(e){if(e instanceof TypeError)count++}function f(){return arguments.length}let ctor=false;try{Reflect.construct(function(){},null)}catch(e){ctor=e instanceof TypeError}const r=[count,f.apply(null,null),f.apply(null,undefined),ctor].join();",
            "r"
        ),
        "4,0,0,true"
    );
}

/** A bad invocation target fails before any authored argument-list getter can run. */
#[test]
fn target_validation_precedes_list_access() {
    assert_eq!(
        common::eval(
            "let gets=0,count=0;const list={get length(){gets++;throw 7}};try{Reflect.apply({},null,list)}catch(e){if(e instanceof TypeError)count++}try{Reflect.construct(()=>{},list)}catch(e){if(e instanceof TypeError)count++}try{Reflect.construct(function(){},list,()=>{})}catch(e){if(e instanceof TypeError)count++}const r=[gets,count].join();",
            "r"
        ),
        "0,3"
    );
}

/** Sparse array argument lists observe inherited indexed getters rather than replacing every hole with undefined. */
#[test]
fn argument_lists_read_inherited_holes() {
    assert_eq!(
        common::eval(
            "const a=new Array(1);const p=Object.create(Array.prototype);Object.defineProperty(p,'0',{get(){return 7}});Object.setPrototypeOf(a,p);const r=Reflect.apply(x=>x,null,a);",
            "r"
        ),
        "7"
    );
}
