/** Generic array locale conversion and in-place copying retain callback and failure order. */
use crate::common;

/** Locale methods receive the actual element and both formatting options. */
#[test]
fn locale_methods_receive_original_arguments() {
    assert_eq!(
        common::eval(
            "const options={};const value={toLocaleString(locale,arg){return this===value&&locale==='fr'&&arg===options?'ok':'bad'}};const r=[value,null,undefined].toLocaleString('fr',options);",
            "r"
        ),
        "ok,,"
    );
}

/** A recursive locale return uses the same cycle guard as ordinary array joining. */
#[test]
fn locale_cycles_are_bounded() {
    assert_eq!(
        common::eval(
            "const arr=[{toLocaleString(){return [1,arr]}}];const r=[arr.toLocaleString(),arr.toLocaleString()].join('|');",
            "r"
        ),
        "1,|1,"
    );
}

/** Throwing locale getters clean their guard so a later conversion runs normally. */
#[test]
fn locale_failure_does_not_leave_a_cycle_marker() {
    assert_eq!(
        common::eval(
            "const marker={};const arr=[{get toLocaleString(){throw marker}}];let same=false;try{arr.toLocaleString()}catch(e){same=e===marker}arr[0]={toLocaleString(){return 7}};const r=[same,arr.toLocaleString()].join();",
            "r"
        ),
        "true,7"
    );
}

/** Overlapping copies walk backwards and interleave each read and write. */
#[test]
fn copy_within_preserves_overlap_and_operation_order() {
    assert_eq!(
        common::eval(
            "const log=[];const a=[1,2,3,4];const p=new Proxy(a,{get(t,k,r){if(k!=='length')log.push('g'+k);return Reflect.get(t,k,r)},set(t,k,v,r){log.push('s'+k);return Reflect.set(t,k,v,r)}});Array.prototype.copyWithin.call(p,1,0,3);const r=a.join()+';'+log.join();",
            "r"
        ),
        "1,1,2,3;g2,s3,g1,s2,g0,s1"
    );
}

/** A negative has result suppresses Get and requests deletion of the corresponding target. */
#[test]
fn copy_within_does_not_get_absent_sources() {
    assert_eq!(
        common::eval(
            "let gets=0;const p=new Proxy({length:2},{has(){return false},get(t,k){if(k==='length')return 2;gets++;throw 1}});Array.prototype.copyWithin.call(p,1,0,1);const r=gets;",
            "r"
        ),
        "0"
    );
}

/** Boxed string indices beyond the immutable text are ordinary writable own properties. */
#[test]
fn boxed_strings_accept_indices_outside_their_text() {
    assert_eq!(
        common::eval(
            "const s=new String('abc');s[10]='x';const d=Object.getOwnPropertyDescriptor(s,'10');const r=[s[10],d.writable,d.enumerable,d.configurable,s.length].join();",
            "r"
        ),
        "x,true,true,true,3"
    );
}

/** Shift, unshift and splice leave untouched holes intact while moving inherited elements. */
#[test]
fn mutators_preserve_holes_and_materialize_only_moved_elements() {
    assert_eq!(
        common::eval(
            "Array.prototype[3]='p';const a=new Array(6);a.shift();const shift=[Object.hasOwn(a,2),Object.hasOwn(a,3)];a.unshift('x');const unshift=[Object.hasOwn(a,3),a[3]];const b=[0,1,,,4,5];b.splice(4,0,7);const r=[...shift,...unshift,Object.hasOwn(b,2),Object.hasOwn(b,3),b[3],b[4]].join();",
            "r"
        ),
        "true,false,true,p,false,false,p,7"
    );
}

/** A failing first write stops shift before a later getter can run. */
#[test]
fn mutator_failures_preserve_operation_order() {
    assert_eq!(
        common::eval(
            "const log=[];const a={length:3,get 1(){log.push('read1');return 1},get 2(){log.push('read2');return 2}};Object.defineProperty(a,'0',{value:0});try{Array.prototype.shift.call(a)}catch(e){log.push(e.name)}const r=log.join();",
            "r"
        ),
        "read1,TypeError"
    );
}

/** Sorting an array-like never writes the length property it read. */
#[test]
fn sorting_does_not_store_length() {
    assert_eq!(
        common::eval(
            "let stores=0;const a={0:2,1:1,get length(){return 2},set length(v){stores++;throw 1}};Array.prototype.sort.call(a);const r=[a[0],a[1],stores].join();",
            "r"
        ),
        "1,2,0"
    );
}
