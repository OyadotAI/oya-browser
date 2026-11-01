/** Immutable backing capabilities survive lifecycle changes while preserving exact bytes and native brands. */
use crate::common;

/** Converting fixed and resizable allocations detaches the source and freezes only their bytes. */
#[test]
fn transfer_to_immutable_makes_fixed_backing_and_preserves_zero_filled_bytes() {
    assert_eq!(
        common::eval(
            "let ok=0;for(const options of [undefined,{maxByteLength:12}]){const a=new ArrayBuffer(3,options);new Uint8Array(a).set([2,4,6]);const b=a.transferToImmutable(5);if(a.detached&&b.immutable&&!b.resizable&&b.maxByteLength===5&&new Uint8Array(b).join()==='2,4,6,0,0')ok++}const r=ok;",
            "r"
        ),
        "2"
    );
}

/** Length conversion can run arbitrary script; failed conversions leave the original allocation attached. */
#[test]
fn transfer_conversion_failure_does_not_detach_the_source() {
    assert_eq!(
        common::eval(
            "const a=new ArrayBuffer(3);let same=false;const marker={};try{a.transferToImmutable({valueOf(){throw marker}})}catch(e){same=e===marker}let detached=false;try{a.transferToImmutable({valueOf(){a.transfer();return 1}})}catch(e){detached=e instanceof TypeError}const r=[same,detached,a.detached].join();",
            "r"
        ),
        "true,true,true"
    );
}

/** Buffer accessors cannot be borrowed by shared buffers, proxies, or matching-looking records. */
#[test]
fn immutable_getter_preserves_the_arraybuffer_brand() {
    assert_eq!(
        common::eval(
            "const get=Object.getOwnPropertyDescriptor(ArrayBuffer.prototype,'immutable').get;let n=0;for(const a of [{},new SharedArrayBuffer(0),new Proxy(new ArrayBuffer(0),{})]){try{get.call(a)}catch(e){if(e instanceof TypeError)n++}}const r=[new ArrayBuffer(0).immutable,new ArrayBuffer(0).transferToImmutable().immutable,n].join();",
            "r"
        ),
        "false,true,3"
    );
}

/** Every public transfer variant refuses to invalidate immutable aliases. */
#[test]
fn immutable_buffers_cannot_transfer_or_resize() {
    assert_eq!(
        common::eval(
            "const a=new ArrayBuffer(2).transferToImmutable();let n=0;for(const op of ['transfer','transferToFixedLength','transferToImmutable','resize']){try{a[op](0)}catch(e){if(e instanceof TypeError)n++}}const r=[n,a.detached,a.byteLength,a.immutable].join();",
            "r"
        ),
        "4,false,2,true"
    );
}

/** Immutable slicing always selects the intrinsic constructor without consulting authored species. */
#[test]
fn slice_to_immutable_ignores_species_and_copies_selected_bytes() {
    assert_eq!(
        common::eval(
            "const a=new ArrayBuffer(4);new Uint8Array(a).set([1,3,5,7]);Object.defineProperty(a,'constructor',{get(){throw 1}});const b=a.sliceToImmutable(1,3);const r=[b.immutable,b.resizable,b.byteLength,new Uint8Array(b).join(),a.detached].join(';');",
            "r"
        ),
        "true;false;2;3,5;false"
    );
}

/** Ordinary slicing produces mutable independent bytes and rejects a read-only species result. */
#[test]
fn ordinary_slice_from_immutable_is_mutable_and_checks_species_writability() {
    assert_eq!(
        common::eval(
            "const a=new ArrayBuffer(2).transferToImmutable();const b=a.slice();new Uint8Array(b)[0]=9;const c=new ArrayBuffer(2);c.constructor={[Symbol.species]:function(){return a}};let threw=false;try{c.slice()}catch(e){threw=e instanceof TypeError}const r=[b.immutable,new Uint8Array(b)[0],new Uint8Array(a)[0],threw].join();",
            "r"
        ),
        "false,9,0,true"
    );
}

/** Detachment wins over shrinking when coercion invalidates the selected source range. */
#[test]
fn immutable_slice_rechecks_the_source_after_argument_coercion() {
    assert_eq!(
        common::eval(
            "const a=new ArrayBuffer(4,{maxByteLength:8});let range=false,detached=false;try{a.sliceToImmutable(0,{valueOf(){a.resize(1);return 4}})}catch(e){range=e instanceof RangeError}a.resize(4);try{a.sliceToImmutable(0,{valueOf(){a.resize(1);a.transfer();return 4}})}catch(e){detached=e instanceof TypeError}const r=[range,detached].join();",
            "r"
        ),
        "true,true"
    );
}

/** Stored VM bytes and read-only internal state remain associated after the app's JSON boundary. */
#[test]
fn immutable_state_survives_a_json_round_trip() {
    let sources = vec!["const source=new Uint8Array([4,8]).buffer;globalThis.saved=source.transferToImmutable();let r;setTimeout(()=>{r=[saved.immutable,new Uint8Array(saved).join(),saved.detached].join(';')},0);".into()];
    let environment = common::environment();
    let host = common::host();
    let execution =
        mars_v9::load_page(common::input(&sources, &host, &environment)).expect("page loads");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    let state = common::round_trip(execution.state);
    let execution =
        mars_v9::run_timers(common::input(&sources, &host, &environment), state, 0.0, 1)
            .expect("restored page");
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true;4,8;false");
}
