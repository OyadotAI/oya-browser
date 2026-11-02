/** Numeric array writes preserve inherited setters and readonly properties without reordering key coercion. */
use crate::common;

/** Creating an index calls the inherited setter without creating an own element or extending length. */
#[test]
fn numeric_array_creation_observes_an_inherited_setter() {
    let source = "let total=0;Object.defineProperty(Array.prototype,'0',{get(){return total},set(value){total+=value},configurable:true});const array=[];array[0]=10;const before=array.length;array.push(100);const r=[before,array.length,total,array[0],Object.hasOwn(array,'0')].join();";
    assert_eq!(common::eval(source, "r"), "0,1,110,110,false");
}

/** Deleting an own element exposes a setter subsequently installed on the prototype array. */
#[test]
fn numeric_array_holes_observe_new_prototype_setters() {
    let source = "let count=0;const prototype=[];const array=[,6];Object.setPrototypeOf(array,prototype);array[0]=3;delete array[0];Object.defineProperty(prototype,'0',{set(value){count+=value},configurable:true});array[0]=3;const own=[1,,,2];Object.setPrototypeOf(own,prototype);own[0]=9;const r=[count,array.length,Object.hasOwn(array,'0'),own[0]].join();";
    assert_eq!(common::eval(source, "r"), "3,2,false,9");
}

/** Inherited nonwritable indices reject sloppy creation silently without extending the array length. */
#[test]
fn numeric_array_creation_respects_frozen_inherited_indices() {
    let source = "const prototype=Object.create(Array.prototype);prototype[1]=153;const array=[];Object.setPrototypeOf(array,prototype);array[0]=42;Object.freeze(prototype);array[1]=42;const blocked=array.length;array[2]=42;const r=[blocked,array.length,array[1],Object.hasOwn(array,'1'),array[2]].join();";
    assert_eq!(common::eval(source, "r"), "1,3,153,false,42");
}

/** Strict numeric creation propagates a readonly inherited index failure before any length mutation. */
#[test]
fn strict_numeric_array_creation_throws_for_readonly_inherited_indices() {
    let source = "'use strict';const prototype=Object.create(Array.prototype);Object.defineProperty(prototype,'1',{value:153});const array=[];Object.setPrototypeOf(array,prototype);array[0]=42;let error;try{array[1]=42}catch(caught){error=caught.name}const r=[error,array.length,array[1],Object.hasOwn(array,'1')].join();";
    assert_eq!(common::eval(source, "r"), "TypeError,1,153,false");
}

/** Proxy prototype Set receives new or hole indices; existing own elements remain ordinary overwrites. */
#[test]
fn numeric_array_creation_observes_proxy_prototypes() {
    let source = "let seen='';const prototype=new Proxy(Array.prototype,{set(target,key,value,receiver){seen+=key+':'+value+';';return true}});const array=[1,,];Object.setPrototypeOf(array,prototype);array[0]=9;array[1]=7;array[4]=8;const r=[seen,array.length,array[0],Object.hasOwn(array,'1'),Object.hasOwn(array,'4')].join('|');";
    assert_eq!(common::eval(source, "r"), "1:7;4:8;|2|9|false|false");
}

/** Simple numeric assignment observes a setter installed by its RHS, after evaluating the key expression once. */
#[test]
fn numeric_assignment_rhs_can_install_an_inherited_setter() {
    let source = "let seen='';const array=[];function key(){seen+='key;';return 0}function rhs(){seen+='rhs;';Object.defineProperty(Array.prototype,'0',{set(value){seen+='set:'+value+';'},configurable:true});return 7}array[key()]=rhs();const r=[seen,array.length,Object.hasOwn(array,'0')].join('|');";
    assert_eq!(common::eval(source, "r"), "key;rhs;set:7;|0|false");
}
