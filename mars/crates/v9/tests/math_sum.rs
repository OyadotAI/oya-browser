/** Exact summation exercises rounding, cancellation, special values and iterator closing. */
use crate::common;

/** Cancellation recovers bits discarded by a running floating point sum, regardless of order. */
#[test]
fn cancellation_and_ties_round_once() {
    assert_eq!(
        common::eval(
            "const r=[Math.sumPrecise([1e16,1,-1e16]),Math.sumPrecise([1,2**-53]),Math.sumPrecise([1,2**-53,2**-54]),Math.sumPrecise([1e308,1e308,-1e308]),Math.sumPrecise([Number.MIN_VALUE,Number.MIN_VALUE])].join();",
            "r"
        ),
        "1,1,1.0000000000000002,1e+308,1e-323"
    );
}

/** Empty and negative-zero-only sums retain negative zero; cancellation produces positive zero. */
#[test]
fn signed_zero_and_infinities_follow_ieee_rules() {
    assert_eq!(
        common::eval(
            "const r=[Object.is(Math.sumPrecise([]),-0),Object.is(Math.sumPrecise([-0,-0]),-0),Object.is(Math.sumPrecise([-0,0]),0),Object.is(Math.sumPrecise([1,-1]),0),Math.sumPrecise([Infinity,1]),Number.isNaN(Math.sumPrecise([Infinity,-Infinity]))].join();",
            "r"
        ),
        "true,true,true,true,Infinity,true"
    );
}

/** Invalid elements close custom and native iterators without calling numeric coercion. */
#[test]
fn invalid_values_close_the_iterator_and_preserve_the_type_error() {
    assert_eq!(
        common::eval(
            "let closes=0,coercions=0;const bad={valueOf(){coercions++;return 2}};const i=[1,bad,3].values();i.return=()=>{closes++;throw 7};let type=false;try{Math.sumPrecise(i)}catch(e){type=e instanceof TypeError}const r=[closes,coercions,type,i.next().done].join();",
            "r"
        ),
        "1,0,true,true"
    );
}

/** NaN does not suppress later protocol reads or validation of non-number elements. */
#[test]
fn nonfinite_results_still_exhaust_and_validate() {
    assert_eq!(
        common::eval(
            "let count=0;const iterable={*[Symbol.iterator](){count++;yield NaN;count++;yield 1;count++;yield 'bad'}};let type=false;try{Math.sumPrecise(iterable)}catch(e){type=e instanceof TypeError}const r=[count,type].join();",
            "r"
        ),
        "3,true"
    );
}
