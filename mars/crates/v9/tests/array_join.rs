/** Array.join reports a catchable length error before allocating oversized output. */
use crate::common;

/** The data-only empty-separator path rejects an oversized result inside JavaScript. */
#[test]
fn sparse_join_length_overflow_is_catchable() {
    let source = &format!(
        "const huge='x'.repeat({units});let result=false;try{{[huge,huge].join('')}}catch(e){{result=e instanceof RangeError}}const out=result;",
        units = mars_v9::constants::JS_STRING_UNITS
    );
    assert_eq!(common::eval(source, "out"), "true");
}

/** Generic joining still converts the next value before rejecting its oversized append. */
#[test]
fn generic_join_observes_value_conversion_before_length_error() {
    let source = &format!(
        "const huge='x'.repeat({units});let calls=0,kind=false;const value={{toString(){{calls++;return huge}}}};try{{[huge,value].join('')}}catch(e){{kind=e instanceof RangeError}}const out=[kind,calls].join(',');",
        units = mars_v9::constants::JS_STRING_UNITS
    );
    assert_eq!(common::eval(source, "out"), "true,1");
}
