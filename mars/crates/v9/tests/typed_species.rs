/** Buffer and typed-array species use actual constructors with live native storage validation and observable callback order. */
mod common;

/** Map chooses its target store by element and buffer kind after species construction. */
#[test]
fn map_rechecks_matching_kind_destinations_after_callbacks() {
    let source = "function run(shared,sourceResizable,targetResizable,different){const log=[];const aBuf=new ArrayBuffer(2,sourceResizable?{maxByteLength:4}:undefined);const bBuf=shared?aBuf:new ArrayBuffer(2,targetResizable?{maxByteLength:4}:undefined);const a=new Uint8Array(aBuf);const b=different?new Int8Array(bBuf):new Uint8Array(bBuf);a.constructor={[Symbol.species]:function(){log.push('species');return b}};let error='none';try{a.map((v,i)=>{log.push('cb'+i);if(i===0)bBuf.resize(0);return {valueOf(){log.push('convert');return 9}}})}catch(e){error=e.name}return error+':'+log.join(',')}const r=[run(true,true,true,false),run(false,true,true,false),run(false,false,true,false),run(false,true,true,true)].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "TypeError:species,cb0,convert|TypeError:species,cb0,convert|none:species,cb0,convert,cb1,convert|none:species,cb0,convert,cb1,convert"
    );
}

/** A detached source can continue through a separate valid destination with undefined reads. */
#[test]
fn map_keeps_captured_length_after_source_detaches() {
    let source = "const buf=new ArrayBuffer(2),a=new Uint8Array(buf),seen=[];a.map((v,i)=>{seen.push(String(v));if(i===0)buf.transfer();return 7});const r=seen.join();";
    assert_eq!(common::eval(source, "r"), "0,undefined");
}

/** Conversion can shrink or detach the target, so fast stores validate only afterward. */
#[test]
fn map_rechecks_after_conversion_side_effects() {
    let source = "function run(detach){const log=[],buf=new ArrayBuffer(2,{maxByteLength:4}),a=new Uint8Array(buf);a.constructor={[Symbol.species]:function(){return a}};let error='none';try{a.map(()=>{log.push('callback');return {valueOf(){log.push('valueOf');if(detach)buf.transfer();else buf.resize(0);return 1}}})}catch(e){error=e.name}return error+':'+log.join(',')}const r=[run(false),run(true)].join('|');";
    assert_eq!(
        common::eval(source, "r"),
        "TypeError:callback,valueOf|TypeError:callback,valueOf"
    );
}

/** Conversion exceptions propagate before a later out-of-bounds store can replace them. */
#[test]
fn map_preserves_conversion_errors_after_target_shrink() {
    let source = "const b=new ArrayBuffer(1,{maxByteLength:2}),a=new Uint8Array(b);a.constructor={[Symbol.species]:function(){return a}};let caught='';try{a.map(()=>{b.resize(0);return {valueOf(){throw 'conversion'}}})}catch(e){caught=e}const r=caught;";
    assert_eq!(common::eval(source, "r"), "conversion");
}

/** A target regrown during conversion accepts the converted value at the live index. */
#[test]
fn map_accepts_target_regrown_during_conversion() {
    let source = "const b=new ArrayBuffer(1,{maxByteLength:2}),a=new Uint8Array(b);a.constructor={[Symbol.species]:function(){return a}};const result=a.map(()=>{b.resize(0);return {valueOf(){b.resize(1);return 7}}});const r=[result===a,a[0]].join();";
    assert_eq!(common::eval(source, "r"), "true,7");
}

/** Only length-tracking growable-shared views use V8's resizable element kind. */
#[test]
fn map_distinguishes_tracking_and_fixed_growable_shared_views() {
    let source = "function run(tracking){const shared=new SharedArrayBuffer(2,{maxByteLength:4}),targetBuffer=new ArrayBuffer(2,{maxByteLength:4});const source=tracking?new Uint8Array(shared):new Uint8Array(shared,0,2);const target=new Uint8Array(targetBuffer);source.constructor={[Symbol.species]:function(){return target}};let calls=0,error='none';try{source.map(()=>{calls++;if(calls===1)targetBuffer.resize(0);return 1})}catch(e){error=e.name}return error+':'+calls}const r=[run(true),run(false)].join('|');";
    assert_eq!(common::eval(source, "r"), "TypeError:1|none:2");
}

/** ArrayBuffer subclass construction preserves new.target and slice performs actual species allocation. */
#[test]
fn array_buffer_slice_preserves_subclasses_and_checks_the_real_result() {
    assert_eq!(
        common::eval(
            "class A extends ArrayBuffer{};const a=new A(4);new Uint8Array(a).set([1,2,3,4]);const b=a.slice(1,3);let short=false,alias=false;class S extends ArrayBuffer{constructor(n){super(Math.max(n-1,0))}}try{new S(5).slice(0,4)}catch(e){short=e instanceof TypeError}a.constructor={[Symbol.species]:function(){return a}};try{a.slice()}catch(e){alias=e instanceof TypeError}const r=[a instanceof A,b instanceof A,new Uint8Array(b).join(),short,alias].join('|');",
            "r"
        ),
        "true|true|2,3|true|true"
    );
}

/** Map selects its output before invoking callbacks; filter selects after all predicate calls. */
#[test]
fn map_and_filter_observe_different_species_timing() {
    assert_eq!(
        common::eval(
            "const log=[];class B extends Uint16Array{constructor(n){log.push('construct');super(n)}}class A extends Uint8Array{static get [Symbol.species](){log.push('species');return B}}const a=new A([2,3]);const mapped=a.map(v=>{log.push('map');return v+256});const filtered=a.filter(v=>{log.push('filter');return true});const r=[mapped instanceof B,mapped.join(),filtered instanceof B,filtered.join(),log.join()].join('|');",
            "r"
        ),
        "true|258,259|true|2,3|species,construct,map,map,filter,filter,species,construct"
    );
}

/** Species results are validated even for zero elements and cannot cross the Number/BigInt content boundary. */
#[test]
fn invalid_typed_species_are_rejected_before_mapping() {
    assert_eq!(
        common::eval(
            "let calls=0,types=0;class A extends Uint8Array{static get [Symbol.species](){return Array}}class B extends Uint8Array{static get [Symbol.species](){return BigInt64Array}}for(const a of [new A(0),new B(0)])try{a.map(()=>{calls++;return 1})}catch(e){if(e instanceof TypeError)types++}const r=[types,calls].join();",
            "r"
        ),
        "2,0"
    );
}

/** Subarray passes the real backing store into species and keeps length tracking when no end is supplied. */
#[test]
fn subarray_uses_species_and_preserves_length_tracking() {
    assert_eq!(
        common::eval(
            "class A extends Uint8Array{static get [Symbol.species](){return Uint16Array}}const a=new A(8);const b=a.subarray(2,4);const buffer=new ArrayBuffer(4,{maxByteLength:8});const tracking=new Uint8Array(buffer).subarray(1);buffer.resize(8);const r=[b instanceof Uint16Array,b.buffer===a.buffer,b.byteOffset,b.length,tracking.length].join();",
            "r"
        ),
        "true,true,2,2,7"
    );
}

/** User conversion can detach the source, but a detached buffer can never be read through a stale byte slice. */
#[test]
fn buffer_slice_rechecks_detachment_after_author_code() {
    assert_eq!(
        common::eval(
            "const a=new ArrayBuffer(4);let failed=false;try{a.slice({valueOf(){a.transfer();return 0}})}catch(e){failed=e instanceof TypeError}const r=failed;",
            "r"
        ),
        "true"
    );
}
