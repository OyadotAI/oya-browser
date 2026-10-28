/** Typed array traversal observes real view length, branding, and detachment boundaries. */
mod common;

/** Typed traversal uses its internal witness even when an own property shadows length. */
#[test]
fn typed_search_reduce_and_join_ignore_shadow_length() {
    let source = "const a=new Uint8Array([1,2]);Object.defineProperty(a,'length',{value:1});let sum=0;a.find(x=>{sum+=x;return false});const r=[sum,a.findIndex(x=>x===2),a.reduce((x,y)=>x+y),a.join(),a.at(-1),Array.prototype.join.call(a),a.toString===Array.prototype.toString].join('|');";
    assert_eq!(common::eval(source, "r"), "3|1|3|1,2|2|1|true");
}

/** Typed traversal rejects incompatible receivers and validates detachment before user coercion. */
#[test]
fn typed_methods_reject_bad_receivers_and_detached_buffers() {
    let source = "const out=[];for(const name of ['at','find','findIndex','reduce','join','values'])try{Uint8Array.prototype[name].call({},()=>0)}catch(e){out.push(e instanceof TypeError)}const a=new Uint8Array([1]);const i=a.values();a.buffer.transfer();for(const f of [()=>a.at({valueOf(){throw Error('coercion')}}),()=>a.find(()=>true),()=>i.next(),()=>Uint8Array.from(a)])try{f()}catch(e){out.push(e instanceof TypeError)}const r=out.join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true,true,true,true"
    );
}

/** Search retains its original bound while detached elements become undefined during callbacks. */
#[test]
fn typed_find_reads_fresh_values_after_callback_detachment() {
    let source = "const a=new Uint8Array([1,2]);const seen=[];a.find((value,index)=>{seen.push(value);if(index===0)a.buffer.transfer();return false});const r=[seen.length,seen[0],seen[1]===undefined].join();";
    assert_eq!(common::eval(source, "r"), "2,1,true");
}

/** Callback methods use the view's real length and continue after backing detachment. */
#[test]
fn typed_callback_methods_keep_the_initial_length() {
    let source = "const a=new Uint8Array([1,2,3]);Object.defineProperty(a,'length',{value:1});let seen=[];a.forEach((v,i)=>{seen.push(v);if(i===0)a.buffer.transfer()});let errors=0;for(const m of ['forEach','every','some'])try{Uint8Array.prototype[m].call([],()=>true)}catch(e){errors+=e instanceof TypeError}const b=new Uint8Array([1,2]);Object.defineProperty(b,'length',{value:1});const r=[seen.length,seen[1]===undefined,errors,b.some(x=>x===2),b.every(x=>x>0)].join();";
    assert_eq!(common::eval(source, "r"), "3,true,3,true,true");
}

/** Index searches use native bounds, strict equality and the correct explicit-undefined start. */
#[test]
fn typed_index_searches_use_real_length_and_strict_equality() {
    let source = "const a=new Float32Array([1,NaN,1]);Object.defineProperty(a,'length',{value:1});let bad=0;for(const m of ['indexOf','lastIndexOf'])try{Uint8Array.prototype[m].call([],1)}catch(e){bad+=e instanceof TypeError}const r=[a.indexOf(1,1),a.indexOf(NaN),a.lastIndexOf(1),a.lastIndexOf(1,undefined),bad].join();";
    assert_eq!(common::eval(source, "r"), "2,-1,2,0,2");
}

/** Fill converts once and uses internal length, while copyWithin preserves overlapping bytes. */
#[test]
fn typed_mutators_use_single_conversion_and_real_length() {
    let source = "let calls=0;const a=new Uint8Array([1,2,3,4]);Object.defineProperty(a,'length',{value:1});a.fill({valueOf(){calls++;return 9}},1,3);a.copyWithin(1,0,3);const r=[calls,...a].join();";
    assert_eq!(common::eval(source, "r"), "1,1,1,9,9");
}

/** Detachment from an index conversion stops copyWithin before mutation. */
#[test]
fn typed_copy_within_rechecks_after_index_conversion() {
    let source = "const a=new Uint8Array([1,2]);let r=false;try{a.copyWithin({valueOf(){a.buffer.transfer();return 0}})}catch(e){r=e instanceof TypeError}";
    assert_eq!(common::eval(source, "r"), "true");
}

/** with retains the initial output length while checking its index against the resized source. */
#[test]
fn typed_with_uses_initial_result_length_after_value_conversion() {
    let source = "const b=new ArrayBuffer(2,{maxByteLength:6}),a=new Uint8Array(b);a.set([1,2]);const x=a.with(-1,{valueOf(){b.resize(4);return 9}});const y=a.with(5,{valueOf(){b.resize(6);return 8}});const r=[x.length,x.join(),y.length,y.join()].join('|');";
    assert_eq!(common::eval(source, "r"), "2|1,9|4|1,2,0,0");
}
