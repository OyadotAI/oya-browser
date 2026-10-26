/** JSON.stringify performs observable wrapper conversions in specification order. */
mod common;

/** Number wrappers in the replacer list use ToString rather than their numeric payload. */
#[test]
fn boxed_replacer_names_use_string_conversion() {
    let source = "let calls=[];const name=new Number(9);\
        name.toString=function(){calls.push('string');return 'x'};\
        name.valueOf=function(){throw Error('wrong hint')};\
        let r=JSON.stringify({x:7,y:8},[name,name])+':'+calls.join(',');";
    assert_eq!(common::eval(source, "r"), "{\"x\":7}:string,string");
}

/** Replacer getters run before the boxed space value is converted. */
#[test]
fn replacer_getters_precede_boxed_spacing_conversion() {
    let source = "let calls=[];const names=Object.defineProperty([],0,{get(){calls.push('name')}});\
        const gap=new String();gap.toString=function(){calls.push('space');return '.'};\
        const text=JSON.stringify({x:7},names,gap);let r=calls.join(',')+':'+text;";
    assert_eq!(common::eval(source, "r"), "name,space:{}");
}

/** Numeric space wrappers use ToNumber and still obey the ten-character indentation bound. */
#[test]
fn boxed_number_spacing_uses_numeric_conversion() {
    let source = "const gap=new Number(1);gap.valueOf=()=>2;\
        gap.toString=()=>{throw Error('wrong hint')};let r=JSON.stringify({x:7},null,gap);";
    assert_eq!(common::eval(source, "r"), "{\n  \"x\": 7\n}");
}

/** BigInt primitives consult BigInt.prototype.toJSON before rejecting unconverted BigInts. */
#[test]
fn bigint_values_consult_their_to_json_method() {
    let source = "BigInt.prototype.toJSON=function(key){return key+':'+String(this)};\
        let r=JSON.stringify({x:7n});";
    assert_eq!(common::eval(source, "r"), "{\"x\":\"x:7\"}");
}

/** A proxy over an array remains an array for serialization and replacer lists. */
#[test]
fn json_recognizes_proxy_arrays() {
    let source = "const values=new Proxy(new Proxy([1,2],{}),{});\
        const names=new Proxy(['b'],{});let r=JSON.stringify(values)+':'+JSON.stringify({a:1,b:2},names);";
    assert_eq!(common::eval(source, "r"), "[1,2]:{\"b\":2}");
}

/** IsArray follows targets without invoking traps and rejects a revoked proxy. */
#[test]
fn array_detection_follows_proxies_and_rejects_revocation() {
    let source = "const handler={get(){throw Error('trap called')}};\
        const array=new Proxy(new Proxy([],handler),handler);\
        const {proxy,revoke}=Proxy.revocable([],{});revoke();\
        let r=String(Array.isArray(array))+','+Array.isArray(new Proxy({},{}));\
        try{Array.isArray(proxy)}catch(e){r+=','+e.name}";
    assert_eq!(common::eval(source, "r"), "true,false,TypeError");
}

/** Concat and flatten use IsArray for nested proxies instead of exposing the wrapper object. */
#[test]
fn concat_and_flatten_spread_proxy_arrays() {
    let source = "const p=new Proxy([1,2],{});let r=[].concat(p).join()+':'+[p].flat().join();";
    assert_eq!(common::eval(source, "r"), "1,2:1,2");
}

/** An impossible output length is a catchable allocation error without changing string limits. */
#[test]
fn impossible_proxy_array_output_throws_a_range_error() {
    let source = "const a=new Proxy([],{get(target,key){return key==='length'?1e40:0}});\
        let r='returned';try{JSON.stringify(a)}catch(e){r=e.name}";
    assert_eq!(common::eval(source, "r"), "RangeError");
}

/** A huge logical length does not preempt the first indexed getter's own exception. */
#[test]
fn huge_array_lengths_preserve_index_getter_failure_order() {
    let source = "const sentinel={};const a=new Proxy([],{get(target,key){\
        if(key==='length')return 1e40;if(key==='0')throw sentinel}});\
        let r=false;try{JSON.stringify(a)}catch(e){r=e===sentinel}";
    assert_eq!(common::eval(source, "r"), "true");
}
