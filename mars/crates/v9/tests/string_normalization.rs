/** Unicode normalization uses scalar algorithms without replacing JavaScript's lone surrogates. */
use crate::common;

/** Canonical and compatibility forms compose and decompose the actual Unicode sequences. */
#[test]
fn all_normalization_forms_are_distinct() {
    assert_eq!(
        common::eval(
            "const r=['A\\u030a'.normalize(), '\\u00c5'.normalize('NFD'), '\\ufb03'.normalize('NFKC'), '\\u212b'.normalize('NFKD')].join('|');",
            "r"
        ),
        "Å|A\u{30a}|ffi|A\u{30a}"
    );
}

/** Unpaired UTF-16 code units are preserved exactly and separate combining sequences. */
#[test]
fn lone_surrogates_are_preserved_as_normalization_boundaries() {
    assert_eq!(
        common::eval(
            "const s='A\\ud800\\u030aA\\u030a\\udc00';const n=s.normalize();const r=Array.from({length:n.length},(_,i)=>n.charCodeAt(i)).join();",
            "r"
        ),
        "65,55296,778,197,56320"
    );
}

/** Normalize is generic and coerces the receiver before reading the requested form. */
#[test]
fn receiver_and_form_coercion_follow_spec_order() {
    assert_eq!(
        common::eval(
            "const log=[];const receiver={toString(){log.push('receiver');return 'A\\u030a'}};const form={toString(){log.push('form');return 'NFD'}};const value=String.prototype.normalize.call(receiver,form);let range=false;try{''.normalize('nfc')}catch(e){range=e instanceof RangeError}const r=[value,log.join(),range,String.prototype.normalize.call(123)].join('|');",
            "r"
        ),
        "A\u{30a}|receiver,form|true|123"
    );
}
