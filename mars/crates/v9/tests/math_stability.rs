/** Stable Math operations handle extreme magnitudes, signed zero and argument conversion without intermediate overflow. */
mod common;

/** Scaling protects both ends of binary64, and infinity has priority only after every conversion succeeds. */
#[test]
fn hypot_preserves_magnitude_and_conversion_order() {
    assert_eq!(
        common::eval(
            "let calls=0;const v={valueOf(){calls++;return 4}};const r=[Math.hypot(3e300,4e300)/1e300,Math.hypot(3e-300,4e-300)/1e-300,Math.hypot(Infinity,NaN,v),calls,Object.is(Math.hypot(-0),0)].join();",
            "r"
        ),
        "5,5,Infinity,1,true"
    );
}

/** Negative half ties round toward positive infinity while keeping the sign when the result is zero. */
#[test]
fn round_preserves_negative_zero_and_large_integers() {
    assert_eq!(
        common::eval(
            "const r=[Object.is(Math.round(-0.5),-0),Object.is(Math.round(-0.1),-0),Math.round(-1.5),Math.round(4503599627370497),Math.round(0.49999999999999994)].join();",
            "r"
        ),
        "true,true,-1,4503599627370497,0"
    );
}

/** The inverse hyperbolic tangent is odd even next to the two singularities. */
#[test]
fn atanh_is_symmetric_near_the_poles() {
    assert_eq!(
        common::eval(
            "const x=1-2**-53;const r=[Object.is(Math.atanh(-0),-0),Math.atanh(x)===-Math.atanh(-x),Math.atanh(1),Math.atanh(-1),Number.isNaN(Math.atanh(2))].join();",
            "r"
        ),
        "true,true,Infinity,-Infinity,true"
    );
}
