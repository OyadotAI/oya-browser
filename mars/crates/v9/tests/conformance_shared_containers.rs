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

/** Adding a property outside the declared layout throws; the object keeps its fixed fields. */
#[test]
fn shared_struct_refuses_undeclared_properties() {
    let source = "const T = new SharedStructType(['x']); const o = new T(); let message = ''; try { o.y = 2; } catch (e) { message = e.message; } const r = message;";
    let execution = common::load_with(source, &shared_environment());
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "Cannot add property y, object has fixed layout"
    );
}
