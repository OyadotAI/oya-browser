/** BigInt literal keys retain exact decimal names even outside the runtime arithmetic bound. */
mod common;

/** Each radix produces the same canonical decimal property name. */
#[test]
fn bigint_keys_use_decimal_names_in_objects_and_patterns() {
    assert_eq!(
        common::eval(
            "const o={0b0n:1,0O7n:2,0Xffn:3};const {255n:x}=o;const r=[Object.keys(o).join(),x].join('|');",
            "r"
        ),
        "0,7,255|3"
    );
}

/** Wide literals need only exact decimal conversion when the grammar uses them as names. */
#[test]
fn wide_bigint_keys_do_not_require_runtime_bigint_values() {
    assert_eq!(
        common::eval(
            "const a={0xffffffffffffffffffffffffffffffffffffffffn:1};const b={12345678901234567890123456789012345678901234567890n:2};const r=[Object.keys(a)[0],Object.keys(b)[0]].join('|');",
            "r"
        ),
        "1461501637330902918203684832716283019655932542975|12345678901234567890123456789012345678901234567890"
    );
}
