/** Typed-array integrity and searches follow their native brands and live backing-store witnesses. */
mod common;

/** Integer-indexed elements remain writable after a failed freeze or seal, but extension has stopped. */
#[test]
fn nonempty_typed_arrays_reject_integrity_reconfiguration_after_preventing_extensions() {
    assert_eq!(
        common::eval(
            "let count=0;for(const C of [Uint8Array,Int8Array,Float32Array,BigInt64Array]){for(const op of [Object.freeze,Object.seal]){const a=new C(1);a.note=1;let threw=false;try{op(a)}catch(e){threw=e instanceof TypeError}const value=typeof a[0]==='bigint'?7n:7;const changed=Reflect.defineProperty(a,'0',{value});const named=Object.getOwnPropertyDescriptor(a,'note');if(threw&&!Object.isExtensible(a)&&!Object.isFrozen(a)&&Object.isSealed(a)&&!named.configurable&&changed&&a[0]===value&&Object.seal(a)===a)count++}}const r=String(count);",
            "r"
        ),
        "8"
    );
}

/** Zero-length and detached typed arrays have no indices; DataView bytes are not ordinary properties. */
#[test]
fn empty_detached_and_data_view_integrity_operates_on_ordinary_properties() {
    assert_eq!(
        common::eval(
            "const z=new Uint8Array(0);z.x=1;Object.freeze(z);const b=new ArrayBuffer(2);const a=new Uint8Array(b);b.transfer();a.x=2;Object.freeze(a);const v=new DataView(new ArrayBuffer(2));v.x=3;Object.freeze(v);v.setUint8(0,9);const r=[Object.isFrozen(z),Object.isFrozen(a),Object.isFrozen(v),v.getUint8(0),Object.getOwnPropertyDescriptor(z,'x').writable,Object.getOwnPropertyDescriptor(a,'x').configurable].join();",
            "r"
        ),
        "true,true,true,9,false,false"
    );
}

/** Typed-array includes rejects incompatible objects before any length or fromIndex user code runs. */
#[test]
fn typed_includes_requires_a_real_view_before_observing_arguments() {
    assert_eq!(
        common::eval(
            "const f=Uint8Array.prototype.includes;let errors=0,touched=0;const o={get length(){touched++;throw 1}};const from={valueOf(){touched++;return 0}};for(const value of [o,[1,2],null,undefined,new DataView(new ArrayBuffer(2)),new Proxy(new Uint8Array([1]),{}),Object.create(Uint8Array.prototype)]){try{f.call(value,1,from)}catch(e){if(e instanceof TypeError)errors++}}const r=[errors,touched,f.length,f.name,Array.prototype.includes.call({0:1,length:1},1)].join();",
            "r"
        ),
        "7,0,1,includes,true"
    );
}

/** An own length expando cannot replace the typed array's internal element count. */
#[test]
fn typed_includes_does_not_read_a_shadowing_length_getter() {
    assert_eq!(
        common::eval(
            "const a=new Uint8Array([1,2,3]);Object.defineProperty(a,'length',{get(){throw 1}});const r=[a.includes(2),a.includes(3,2),a.includes(1,1)].join();",
            "r"
        ),
        "true,true,false"
    );
}

/** Search elements are compared with SameValueZero rather than converted into the element type. */
#[test]
fn typed_includes_preserves_same_value_zero_and_bigint_content_identity() {
    assert_eq!(
        common::eval(
            "const a=new Float64Array([NaN,-0,Infinity]);const b=new BigInt64Array([1n]);let converted=0;const obj={valueOf(){converted++;return 1}};const r=[a.includes(NaN),a.includes(0),a.includes(-Infinity),b.includes(1),b.includes(1n),b.includes(obj),converted].join();",
            "r"
        ),
        "true,true,false,false,true,false,0"
    );
}

/** Empty views skip fromIndex conversion; nonempty views propagate its original exception. */
#[test]
fn typed_includes_orders_empty_length_and_from_index_coercion() {
    assert_eq!(
        common::eval(
            "let calls=0;const marker={};const from={valueOf(){calls++;throw marker}};const a=new Uint8Array([1]);let same=false,symbol=false;try{a.includes(1,from)}catch(e){same=e===marker}try{a.includes(1,Symbol())}catch(e){symbol=e instanceof TypeError}const empty=new Uint8Array(0);const r=[same,symbol,empty.includes(1,from),calls].join();",
            "r"
        ),
        "true,true,false,1"
    );
}

/** Initial detachment throws, while detachment during coercion makes the captured range read undefined. */
#[test]
fn typed_includes_distinguishes_initial_and_during_coercion_detachment() {
    assert_eq!(
        common::eval(
            "let touched=0;const b=new ArrayBuffer(2);const a=new Uint8Array(b);b.transfer();let threw=false;try{a.includes(undefined,{valueOf(){touched++;return 0}})}catch(e){threw=e instanceof TypeError}function search(value){const b=new ArrayBuffer(2);const a=new Uint8Array(b);a[0]=2;return a.includes(value,{valueOf(){b.transfer();return 0}})}const r=[threw,touched,search(undefined),search(2)].join();",
            "r"
        ),
        "true,0,true,false"
    );
}

/** The search snapshots length before coercion but reads the current bytes after shrink or growth. */
#[test]
fn typed_includes_uses_live_reads_with_the_original_length_snapshot() {
    assert_eq!(
        common::eval(
            "const b=new ArrayBuffer(4,{maxByteLength:8});const a=new Uint8Array(b);a.set([1,2,3,4]);const shrunk=a.includes(undefined,{valueOf(){b.resize(1);return 0}});b.resize(4);const grown=a.includes(4,{valueOf(){b.resize(8);a[7]=4;return 0}});const fixed=new Uint8Array(b,4,4);b.resize(3);let out=false;try{fixed.includes(0)}catch(e){out=e instanceof TypeError}const r=[shrunk,grown,out].join();",
            "r"
        ),
        "true,false,true"
    );
}
