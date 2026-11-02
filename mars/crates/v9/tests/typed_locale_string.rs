/** %TypedArray%.prototype.toLocaleString is an own method of length 0 that joins each element's
own locale string with commas, as Array.prototype.toLocaleString does; a missing method would fall
back to Object.prototype and never reach the elements. */
use crate::common;

#[test]
fn typed_array_locale_string_is_own_and_joins_elements() {
    assert_eq!(
        common::eval(
            "const TA = Object.getPrototypeOf(Int8Array.prototype); \
             const d = Object.getOwnPropertyDescriptor(TA, 'toLocaleString'); \
             let r = [typeof d.value, d.value.length, new Int8Array([1, 2, 3]).toLocaleString(), new Uint8Array(0).toLocaleString()].join('|');",
            "r"
        ),
        "function|0|1,2,3|"
    );
}
