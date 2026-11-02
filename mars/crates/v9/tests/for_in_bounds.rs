/* for-in over an oversized typed array: browser builds enumerate every key, conformance builds refuse with RangeError. */
use crate::common;
use mars_v9::constants::VM_ARRAY_LENGTH;

/** Browser path: the key snapshot is unbounded, so for-in visits every index and completes. Release only: the walk exceeds the debug script deadline. */
#[cfg(not(feature = "conformance"))]
#[test]
#[cfg_attr(
    debug_assertions,
    ignore = "walks VM_ARRAY_LENGTH keys; exceeds the debug script deadline"
)]
fn browser_path_for_in_over_oversized_typed_array_is_unchanged() {
    let source = format!(
        "const a = new Float32Array({}); let n = 0; for (const k in a) n++; const r = String(n);",
        VM_ARRAY_LENGTH + 1
    );
    let execution = common::load(&source);
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        (VM_ARRAY_LENGTH + 1).to_string()
    );
}

/** Conformance path: V8's fixed-array bound applies, so the enumeration throws RangeError. */
#[cfg(feature = "conformance")]
#[test]
fn conformance_for_in_over_oversized_typed_array_throws_range_error() {
    let source = format!(
        "const a = new Float32Array({}); for (const k in a) {{}}",
        VM_ARRAY_LENGTH + 1
    );
    let execution = common::load(&source);
    let message = common::failure(&execution).unwrap_or_default();
    assert!(message.contains("Invalid array length"), "{message}");
}
