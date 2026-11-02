/** Every native write path observes the same immutable backing capability, including empty operations. */
use crate::common;

/** Read-only indexed descriptors accept SameValue only and never convert a proposed replacement. */
#[test]
fn immutable_indices_have_readonly_descriptors_and_same_value_definitions() {
    assert_eq!(
        common::eval(
            "const input=new Float64Array([NaN,-0]);const a=new Float64Array(input.buffer.transferToImmutable());let touched=0;const d=Object.getOwnPropertyDescriptor(a,'0');const same=Reflect.defineProperty(a,'0',{value:NaN,writable:false,configurable:false,enumerable:true});const changed=Reflect.defineProperty(a,'1',{value:0});const cast=Reflect.defineProperty(a,'0',{value:{valueOf(){touched++;return NaN}}});a.note=7;const r=[d.writable,d.configurable,d.enumerable,same,changed,cast,touched,a.note,Object.is(a[1],-0)].join();",
            "r"
        ),
        "false,false,true,true,false,false,0,7,true"
    );
}

/** Ordinary assignment is silent in sloppy code while strict Set and Reflect retain their contracts. */
#[test]
fn immutable_index_assignment_never_changes_bytes() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(new Uint8Array([6]).buffer.transferToImmutable());a[0]=9;let strict=false;try{(function(){'use strict';a[0]=8})()}catch(e){strict=e instanceof TypeError}const reflected=Reflect.set(a,'0',7);const r=[a[0],strict,reflected].join();",
            "r"
        ),
        "6,true,false"
    );
}

/** Typed write validation happens even when an algorithm would otherwise perform no element store. */
#[test]
fn typed_mutators_reject_immutable_even_for_empty_targets() {
    assert_eq!(
        common::eval(
            "let n=0;for(const length of [0,2]){const a=new Uint8Array(new ArrayBuffer(length).transferToImmutable());for(const f of [()=>a.set([]),()=>a.fill(2),()=>a.copyWithin(0,0,0),()=>a.reverse(),()=>a.sort()]){try{f()}catch(e){if(e instanceof TypeError)n++}}}const r=n;",
            "r"
        ),
        "10"
    );
}

/** DataView's read-only check precedes both offset and value conversion. */
#[test]
fn data_view_setters_reject_before_observing_arguments() {
    assert_eq!(
        common::eval(
            "const v=new DataView(new ArrayBuffer(16).transferToImmutable());let n=0,touched=0;const x={valueOf(){touched++;throw 1}};for(const name of ['setInt8','setUint8','setInt16','setUint16','setInt32','setUint32','setFloat16','setFloat32','setFloat64','setBigInt64','setBigUint64']){try{v[name](x,x)}catch(e){if(e instanceof TypeError)n++}}const r=[n,touched,v.getUint8(0)].join();",
            "r"
        ),
        "11,0,0"
    );
}

/** Encoding remains readable while decoding refuses even empty immutable destinations. */
#[test]
fn byte_decoders_reject_before_input_and_options_but_encoders_read() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(new Uint8Array([102,111,111]).buffer.transferToImmutable());let n=0,touched=0;const opt={get alphabet(){touched++;throw 1}};for(const v of [a,new Uint8Array(new ArrayBuffer(0).transferToImmutable())]){for(const f of [()=>v.setFromHex('x'),()=>v.setFromBase64(null,opt)]){try{f()}catch(e){if(e instanceof TypeError)n++}}}const r=[n,touched,a.toHex(),a.toBase64()].join();",
            "r"
        ),
        "4,0,666f6f,Zm9v"
    );
}

/** Atomic read access is valid on ordinary immutable buffers; all RMW and store operations reject them. */
#[test]
fn atomics_preserve_read_access_without_modifying_immutable_bytes() {
    assert_eq!(
        common::eval(
            "const a=new Int32Array(new Int32Array([3]).buffer.transferToImmutable());let n=0,touched=0;const index={valueOf(){touched++;return 0}};for(const name of ['store','add','sub','and','or','xor','exchange','compareExchange']){try{Atomics[name](a,index,7,9)}catch(e){if(e instanceof TypeError)n++}}const r=[n,touched,Atomics.load(a,0),a[0]].join();",
            "r"
        ),
        "8,0,3,3"
    );
}

/** Host byte writers must fail instead of reporting unwritten random bytes or encoded character counts. */
#[test]
fn native_byte_writers_do_not_report_success_on_immutable_storage() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(new ArrayBuffer(4).transferToImmutable());let n=0;for(const f of [()=>crypto.getRandomValues(a),()=>new TextEncoder().encodeInto('test',a)]){try{f()}catch(e){if(e instanceof TypeError)n++}}const r=[n,a.join()].join(';');",
            "r"
        ),
        "2;0,0,0,0"
    );
}

/** Freezing immutable elements leaves named object properties governed by ordinary integrity rules. */
#[test]
fn immutable_view_integrity_does_not_reconfigure_virtual_indices() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array(new Uint8Array([4,5]).buffer.transferToImmutable());a.note=1;Object.freeze(a);const d=Object.getOwnPropertyDescriptor(a,'note');const r=[Object.isFrozen(a),Object.isSealed(a),Object.isExtensible(a),d.writable,d.configurable,a.join()].join(';');",
            "r"
        ),
        "true;true;false;false;false;4,5"
    );
}
