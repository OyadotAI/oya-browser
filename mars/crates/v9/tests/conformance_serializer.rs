/* Native ValueSerializer compatibility exercises V8 wire bytes and actual receiving JavaScript graphs. */
#![cfg(feature = "conformance")]
mod common;
use mars_v9::PageEnvironment;
/** Every check uses the public d8 host through real VM execution, including author getter behavior. */
fn run(source: &str) -> Vec<String> {
    let execution = common::load_with(
        source,
        &PageEnvironment {
            conformance_d8: true,
            ..common::environment()
        },
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert!(
        execution.effects.conformance_error.is_none(),
        "{:?}",
        execution.effects.conformance_error
    );
    common::console(&execution)
}
#[test]
/** Primitive payloads preserve NaN, signed zero, infinities, UTF-16 surrogates and arbitrary-width BigInts. */
fn primitive_roundtrips_preserve_exact_language_values() {
    assert_eq!(
        run(
            "const values=[undefined,null,true,false,0,-0,-1,42,1.5,Infinity,-Infinity,NaN,'x','λ','\\ud800',1n,-1n,BigInt('0x10000000000000000000000000000000000000000')];console.log(values.every(value=>Object.is(value,d8.serializer.deserialize(d8.serializer.serialize(value)))))"
        ),
        ["true"]
    );
}

#[test]
/** Encoding compact input preserves its backing and emits the actual content. */
fn serialization_does_not_flatten_compact_input() {
    let source = "const s='ab'.repeat(100000);if(d8.serializer.deserialize(d8.serializer.serialize(s))!==s)throw Error('roundtrip');";
    let execution = common::load_with(
        source,
        &PageEnvironment {
            conformance_d8: true,
            ..common::environment()
        },
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert!(
        serde_json::to_string(&execution.state).unwrap().len()
            < common::empty_state_bytes() + 100_000
    );
}
#[test]
/** Known native version-15 records decode through tags and ZigZag rather than a private wrapper format. */
fn native_wire_records_are_accepted() {
    assert_eq!(
        run(
            "const read=bytes=>d8.serializer.deserialize(Uint8Array.from(bytes).buffer);console.log([read([255,15,73,84]),read([255,15,34,2,104,105]),read([255,15,111,34,1,120,73,14,123,1]).x].join(':'))"
        ),
        ["42:hi:7"]
    );
}
#[test]
/** The encoder emits native header/object/string/integer tags and the documented current format version. */
fn native_header_and_payload_tags_are_emitted() {
    assert_eq!(
        run("console.log(Array.from(new Uint8Array(d8.serializer.serialize({x:7}))).join(','))"),
        ["255,16,111,34,1,120,73,14,123,1"]
    );
}
#[test]
/** Object ids preserve both self cycles and references shared between arrays, maps and sets. */
fn composite_identity_and_cycles_survive() {
    assert_eq!(
        run(
            "const a={};const root={a,b:a,map:new Map([[a,a]]),set:new Set([a])};root.self=root;const copy=d8.serializer.deserialize(d8.serializer.serialize(root));console.log([copy.self===copy,copy.a===copy.b,copy.map.get(copy.a)===copy.a,copy.set.has(copy.a)].join(':'))"
        ),
        ["true:true:true:true"]
    );
}
#[test]
/** Array holes and uint32 logical lengths remain sparse and never allocate a dense hole vector. */
fn sparse_arrays_keep_holes_and_logical_lengths() {
    assert_eq!(
        run(
            "const array=new Array(0xffffffff);array[0xfffffffe]=42;const copy=d8.serializer.deserialize(d8.serializer.serialize(array));console.log([copy.length,0 in copy,copy[0xfffffffe],Object.keys(copy).length].join(':'))"
        ),
        ["4294967295:false:42:1"]
    );
}
#[test]
/** Dense native arrays contain genuine holes and are validated against both trailing counts and length. */
fn native_dense_array_records_keep_absent_elements() {
    assert_eq!(
        run(
            "const data=[255,15,65,3,73,2,45,95,36,0,3];const copy=d8.serializer.deserialize(Uint8Array.from(data).buffer);console.log([copy.length,copy[0],1 in copy,2 in copy].join(':'))"
        ),
        ["3:1:false:true"]
    );
}
#[test]
/** Errors can refer to themselves through cause without requiring message or stack properties. */
fn errors_retain_real_prototypes_and_self_causes() {
    assert_eq!(
        run(
            "const error=new TypeError();delete error.stack;error.cause=error;const copy=d8.serializer.deserialize(d8.serializer.serialize(error));console.log([copy instanceof TypeError,copy.cause===copy,Object.hasOwn(copy,'message'),Object.hasOwn(copy,'stack')].join(':'))"
        ),
        ["true:true:false:false"]
    );
}
#[test]
/** Error stacks belong to instances and deletion cannot expose or resurrect a prototype accessor. */
fn error_stack_accessors_are_own_and_deletable() {
    assert_eq!(
        run(
            "const error=new Error('marker');const descriptor=Object.getOwnPropertyDescriptor(error,'stack');delete error.stack;console.log([Object.hasOwn(Error.prototype,'stack'),typeof Error.prototype.stack,typeof descriptor.get,descriptor.get.name,typeof error.stack].join(':'))"
        ),
        ["false:undefined:function:get stack:undefined"]
    );
}
#[test]
/** Error message/cause accessors are ignored while the native stack Get remains observable. */
fn error_property_reads_match_native_descriptor_rules() {
    assert_eq!(
        run(
            "const reads=[];const error=new Error();Object.defineProperty(error,'message',{get(){reads.push('message');return 'bad'},configurable:true});Object.defineProperty(error,'cause',{get(){reads.push('cause');return 1},configurable:true});Object.defineProperty(error,'stack',{get(){reads.push('stack');return 'real stack'},configurable:true});const copy=d8.serializer.deserialize(d8.serializer.serialize(error));console.log([reads.join(','),copy.stack,Object.hasOwn(copy,'message'),Object.hasOwn(copy,'cause')].join(':'))"
        ),
        ["stack:real stack:false:false"]
    );
}
#[test]
/** Buffer views preserve byte aliasing, numeric formats and independent copies of the source bytes. */
fn arraybuffer_views_share_only_the_cloned_backing() {
    assert_eq!(
        run(
            "const buffer=new ArrayBuffer(8),a=new Uint8Array(buffer),b=new DataView(buffer);a[0]=42;const copy=d8.serializer.deserialize(d8.serializer.serialize({a,b,buffer}));copy.a[0]=7;console.log([copy.a.buffer===copy.buffer,copy.b.buffer===copy.buffer,copy.b.getUint8(0),a[0]].join(':'))"
        ),
        ["true:true:7:42"]
    );
}
#[test]
/** Resizable flags and current byte lengths remain coherent after the source buffer has already shrunk. */
fn resizable_view_flags_and_tracking_are_preserved() {
    assert_eq!(
        run(
            "const buffer=new ArrayBuffer(16,{maxByteLength:32}),view=new Uint8Array(buffer);buffer.resize(8);const copy=d8.serializer.deserialize(d8.serializer.serialize(view));const before=copy.length;copy.buffer.resize(12);console.log([before,copy.length,copy.buffer.resizable,copy.buffer.maxByteLength].join(':'))"
        ),
        ["8:12:true:32"]
    );
}
#[test]
/** Corrupted native ids, oversized varints, flags and view alignment fail through the real deserializer Error. */
fn malformed_binary_records_are_rejected_before_exposure() {
    assert_eq!(
        run(
            "const bad=[[255,17,95],[255,15,94,0],[255,15,73,255,255,255,255,255,0],[255,15,66,4,3,5,7,11,86,66,1,2,1],[255,15,66,4,3,5,7,11,86,87,1,2,0],[255,15,111,34,1,120,73,2,123,2]];console.log(bad.every(bytes=>{try{d8.serializer.deserialize(Uint8Array.from(bytes).buffer);return false}catch(error){return error.constructor===Error&&error.message.startsWith('Unable to deserialize cloned data')}}))"
        ),
        ["true"]
    );
}
#[test]
/** Decoding own data cannot invoke inherited setters installed in the destination realm. */
fn decoding_never_invokes_author_property_setters() {
    assert_eq!(
        run(
            "const data=d8.serializer.serialize({x:7});let calls=0;Object.defineProperty(Object.prototype,'x',{set(){calls++},configurable:true});const copy=d8.serializer.deserialize(data);delete Object.prototype.x;console.log([copy.x,calls,Object.hasOwn(copy,'x')].join(':'))"
        ),
        ["7:0:true"]
    );
}
#[test]
/** No delegate exists for native shared capabilities, and ordinary callable values remain unclonable. */
fn standalone_serialization_rejects_delegate_capabilities_and_functions() {
    assert_eq!(
        run(
            "console.log([()=>{},new SharedArrayBuffer(4)].every(value=>{try{d8.serializer.serialize(value);return false}catch(error){return error.constructor===Error}}))"
        ),
        ["true"]
    );
}
#[test]
/** All supplied arguments are serialized even though native deserialize returns only the first stream value. */
fn streams_preserve_cross_argument_identity_and_later_errors() {
    assert_eq!(
        run(
            "const object={};const bytes=new Uint8Array(d8.serializer.serialize(object,object));let rejected=false;try{d8.serializer.serialize(42,()=>{})}catch(error){rejected=true}console.log([d8.serializer.deserialize(d8.serializer.serialize(42,7)),bytes[bytes.length-2]===94,bytes[bytes.length-1]===0,rejected].join(':'))"
        ),
        ["42:true:true:true"]
    );
}
#[test]
/** Forged array-length properties are rejected before native conversion could call a destination prototype hook. */
fn malformed_arrays_cannot_call_author_conversion_hooks() {
    assert_eq!(
        run(
            "let calls=0;Object.prototype.valueOf=function(){calls++;return 0};const bytes=[255,16,97,0,34,6,108,101,110,103,116,104,111,123,0,64,1,0];let rejected=false;try{d8.serializer.deserialize(Uint8Array.from(bytes).buffer)}catch(error){rejected=error.constructor===Error}delete Object.prototype.valueOf;console.log([rejected,calls].join(':'))"
        ),
        ["true:0"]
    );
}
#[test]
/** Native function construction respects new.target when deserialization returns a primitive. */
fn serializer_native_construction_uses_real_receivers() {
    assert_eq!(
        run(
            "class Decoder extends d8.serializer.deserialize{};const value=new Decoder(d8.serializer.serialize(42));console.log([value instanceof Decoder,new d8.serializer.serialize(7) instanceof ArrayBuffer].join(':'))"
        ),
        ["true:true"]
    );
}
#[test]
/** A caller using a foreign intrinsic namespace receives foreign Array prototypes and Error constructors. */
fn foreign_serializer_calls_keep_the_callee_realm() {
    assert_eq!(
        run(
            "const r=Realm.create();const deserialize=Realm.eval(r,'d8.serializer.deserialize');const value=deserialize(d8.serializer.serialize([42]));console.log([value instanceof Array,value instanceof Realm.eval(r,'Array')].join(':'))"
        ),
        ["false:true"]
    );
}
