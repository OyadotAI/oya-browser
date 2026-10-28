/** Array.join reports a catchable length error before allocating oversized output. */
mod common;

/** The data-only empty-separator path rejects an oversized result inside JavaScript. */
#[test]
fn sparse_join_length_overflow_is_catchable() {
    let source = "const huge='x'.repeat(1024*1024);let result=false;try{[huge,huge].join('')}catch(e){result=e instanceof RangeError}const out=result;";
    assert_eq!(common::eval(source, "out"), "true");
}

/** Generic joining still converts the next value before rejecting its oversized append. */
#[test]
fn generic_join_observes_value_conversion_before_length_error() {
    let source = "const huge='x'.repeat(1024*1024);let calls=0,kind=false;const value={toString(){calls++;return huge}};try{[huge,value].join('')}catch(e){kind=e instanceof RangeError}const out=[kind,calls].join(',');";
    assert_eq!(common::eval(source, "out"), "true,1");
}
