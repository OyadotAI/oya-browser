/** Native property traversal and JSON revivers must throw before Rust's stack can overflow. */
use crate::common;

/** Missing traps recurse through the handler's prototype before any trap body is invoked. */
#[test]
fn handler_prototype_cycles_throw_catchable_range_errors() {
    let source = "const operations=[p=>p.x,p=>Reflect.ownKeys(p),p=>Object.isExtensible(p)];\
        let r=[];for(const operation of operations){const h={};const p=new Proxy({},h);\
        h.__proto__=p;try{operation(p);r.push('returned')}catch(e){r.push(e.name)}}\
        r=r.join(',');";
    assert_eq!(
        common::eval(source, "r"),
        "RangeError,RangeError,RangeError"
    );
}

/** A trap inherited through an ordinary prototype remains callable after the guard unwinds. */
#[test]
fn inherited_proxy_traps_keep_their_handler_receiver() {
    let source = "const h=Object.create({get(target,key){return this.prefix+target[key]}});\
        h.prefix='value:';const p=new Proxy({x:7},h);let r=p.x+','+p.x;";
    assert_eq!(common::eval(source, "r"), "value:7,value:7");
}

/** A reviver can make a sibling cyclic after the JSON reader has accepted a shallow input. */
#[test]
fn reviver_created_cycles_throw_catchable_range_errors() {
    let source = "const o={0:11,1:9};let r='returned';\
        try{JSON.parse('[0,0]',function(){this[1]=o})}catch(e){r=e.name}\
        r+=','+JSON.parse('[1,2]',(key,value)=>value)[1];";
    assert_eq!(common::eval(source, "r"), "RangeError,2");
}

/** The guard retains ordinary bottom-up visits and the root reviver's replacement. */
#[test]
fn revivers_visit_children_before_their_holders() {
    let source = "let calls=[];const value=JSON.parse('{\"a\":[1,2]}',function(key,value){\
        calls.push(key);return typeof value==='number'?value+1:value});\
        let r=calls.join('|')+':'+JSON.stringify(value);";
    assert_eq!(common::eval(source, "r"), "0|1|a|:{\"a\":[2,3]}");
}

/** A cyclic array flattened to infinite depth terminates at the ordinary call-depth bound. */
#[test]
fn infinite_flattening_of_cycles_throws_a_range_error() {
    let source = "const a=[];a.push(a);let r='returned';\
        try{a.flat(Infinity)}catch(e){r=e.name}r+=','+[1,[2]].flat().join();";
    assert_eq!(common::eval(source, "r"), "RangeError,1,2");
}

/** Deep acyclic values reach the guard without changing the engine's existing stack budget. */
#[test]
fn stringify_nesting_throws_a_catchable_range_error() {
    let source = "let a=0;for(let i=0;i<1000;i++)a=[a];let r='returned';\
        try{JSON.stringify(a)}catch(e){r=e.name}r+=','+JSON.stringify([1,2]);";
    assert_eq!(common::eval(source, "r"), "RangeError,[1,2]");
}
