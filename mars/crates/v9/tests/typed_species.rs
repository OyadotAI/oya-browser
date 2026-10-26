/** Buffer and typed-array species use actual constructors with live native storage validation and observable callback order. */
mod common;

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
