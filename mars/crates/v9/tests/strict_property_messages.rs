/** A strict-mode property write or delete that fails names the property and the object the way
V8 does: the object as `#<Ctor>`, `[object Tag]`, a function's source or a primitive's text. */
mod common;

/** A read-only property names the receiver by type and text. */
#[test]
fn read_only_writes_name_the_receiver() {
    assert_eq!(
        common::eval(
            "'use strict';let r=[];for(const f of [()=>{Object.freeze({a:1}).a=2},()=>{Object.freeze([1])[0]=2},()=>{'abc'[0]='x'},()=>{(function f(){}).name=1},()=>{class Foo{}const o=new Foo();Object.defineProperty(o,'k',{value:1});o.k=2},()=>{const e=new TypeError('m');Object.defineProperty(e,'k',{value:1});e.k=2}]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "Cannot assign to read only property 'a' of object '#<Object>'|Cannot assign to read only property '0' of object '[object Array]'|Cannot assign to read only property '0' of string 'abc'|Cannot assign to read only property 'name' of function 'function f(){}'|Cannot assign to read only property 'k' of object '#<Foo>'|Cannot assign to read only property 'k' of object 'TypeError: m'"
    );
}

/** A getter without a setter names the object that holds it; a closed object names the property;
a primitive cannot take a property at all. */
#[test]
fn other_write_failures_use_their_own_wording() {
    assert_eq!(
        common::eval(
            "'use strict';let r=[];for(const f of [()=>{({get a(){}}).a=1},()=>{Object.preventExtensions({}).a=1},()=>{(1).x=1},()=>{new Proxy(Object.preventExtensions({}),{}).x=1}]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "Cannot set property a of #<Object> which has only a getter|Cannot add property a, object is not extensible|Cannot create property 'x' on number '1'|Cannot define property x, object is not extensible"
    );
}

/** A refused delete names the object the property lives on. */
#[test]
fn refused_deletes_name_the_object() {
    assert_eq!(
        common::eval(
            "'use strict';let r=[];for(const f of [()=>{delete Object.prototype},()=>{delete [].length},()=>{delete 'abc'[0]}]){try{f()}catch(e){r.push(e.message)}}r=r.join('|')",
            "r"
        ),
        "Cannot delete property 'prototype' of function Object() { [native code] }|Cannot delete property 'length' of [object Array]|Cannot delete property '0' of [object String]"
    );
}

/** A symbol key prints with its description. */
#[test]
fn symbol_keys_print_their_description() {
    assert_eq!(
        common::eval(
            "'use strict';let r;const s=Symbol('k');try{Object.freeze({[s]:1})[s]=2}catch(e){r=e.message}",
            "r"
        ),
        "Cannot assign to read only property 'Symbol(k)' of object '#<Object>'"
    );
}
