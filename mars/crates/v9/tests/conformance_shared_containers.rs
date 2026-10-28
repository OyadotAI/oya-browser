/* SharedArray and SharedStructType under --harmony-struct: shared storage accepts only shareable values. */
#![cfg(feature = "conformance")]
mod common;

/** Loads a page with the harmony shared containers enabled. */
fn shared_environment() -> mars_v9::PageEnvironment {
    let mut environment = common::environment();
    environment.conformance_harmony_struct = true;
    environment
}

/** A SharedArray starts with undefined elements and never grows: a store past its length is refused. */
#[test]
fn shared_array_elements_start_undefined_and_length_is_fixed() {
    let source = "const a = new SharedArray(2); a[5] = 1; const r = [a.length, a[0] === undefined, a[5] === undefined].join();";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "2,true,true");
}

/** Storing a plain object into a SharedArray throws, as V8's Object::Share does. */
#[test]
fn shared_array_refuses_unshareable_values() {
    let source = "const a = new SharedArray(1); let message = ''; try { a[0] = {}; } catch (e) { message = e instanceof TypeError ? e.message : 'other'; } const r = message;";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "#<Object> cannot be shared"
    );
}

/** A negative length is a RangeError, not an array with a wrapped length. */
#[test]
fn shared_array_rejects_invalid_length() {
    let source =
        "let kind = ''; try { new SharedArray(-1); } catch (e) { kind = e.name; } const r = kind;";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "RangeError");
}

/** A shared struct keeps its declared field and accepts a store to it. */
#[test]
fn shared_struct_accepts_declared_fields() {
    let source =
        "const T = new SharedStructType(['x']); const o = new T(); o.x = 4; const r = o.x;";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "4");
}

/** An undeclared property is ignored in sloppy code and rejected in strict code. */
#[test]
fn shared_struct_refuses_undeclared_properties() {
    let source = "const T = new SharedStructType(['x']); const o = new T(); o.y=2; let strict=false;try{(function(){'use strict';o.z=3})()}catch(e){strict=e instanceof TypeError}const r=[o.y===undefined,o.z===undefined,strict].join();";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "true,true,true");
}

/** Shared objects reject private fields while ordinary sealed objects retain ECMAScript behavior. */
#[test]
fn private_fields_are_refused_only_for_shared_objects() {
    let source = "function stamp(o){class B{constructor(){return o}}class C extends B{#x=1}try{new C;return true}catch(e){return false}}const T=SharedStructType(['x']);const r=[stamp(new SharedArray(1)),stamp(T()),stamp(Object.seal({}))].join();";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "false,false,true");
}

/** Writable shared fields cannot be frozen or redefined as read-only. */
#[test]
fn shared_fields_remain_writable_after_rejected_freeze() {
    let source = "const T=SharedStructType(['x']);const s=T();let r='';try{Object.defineProperty(s,'x',{writable:false})}catch(e){r+='d'}try{Object.freeze(s)}catch(e){r+='f'}s.x=42;r+=s.x;";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(common::global(&execution, "r"), "df42");
}

/** Shared containers have null prototypes while struct instance checks retain type identity. */
#[test]
fn shared_container_prototypes_and_type_identity_are_distinct() {
    let source = "const A=SharedStructType(['x']),B=SharedStructType(['x']),a=A(),array=SharedArray(1);const r=[A.prototype===null,SharedArray.prototype===null,Object.getPrototypeOf(a)===null,Object.getPrototypeOf(array)===null,a instanceof A,a instanceof B,array instanceof SharedArray].join();";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "true,true,true,true,true,false,true"
    );
}
