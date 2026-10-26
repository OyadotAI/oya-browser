/** Number fixed formatting uses exact binary64 rounding, preserving the sign of rounded values. */
mod common;

/** A negative nonzero value stays negative when its formatted magnitude rounds to zero. */
#[test]
fn fixed_preserves_negative_rounded_zero() {
    assert_eq!(
        common::eval("const r=[(-0.001).toFixed(2),(-0).toFixed(2)].join();", "r"),
        "-0.00,0.00"
    );
}

/** Decimal scaling cannot manufacture a tie for a binary value slightly below its midpoint. */
#[test]
fn fixed_does_not_double_round() {
    assert_eq!(
        common::eval(
            "const r=[(1.005).toFixed(2),(2.55).toFixed(1),(-1.005).toFixed(2)].join();",
            "r"
        ),
        "1.00,2.5,-1.00"
    );
}

/** Both even and odd lower neighbors round exact decimal midpoints away from zero. */
#[test]
fn fixed_rounds_exact_ties_away_from_zero() {
    assert_eq!(
        common::eval(
            "const r=[(0.125).toFixed(2),(0.375).toFixed(2),(-0.125).toFixed(2),(2.5).toFixed(0),(3.5).toFixed(0)].join();",
            "r"
        ),
        "0.13,0.38,-0.13,3,4"
    );
}

/** Number parser aliases share the globals and retain nonenumerable writable properties. */
#[test]
fn number_parser_aliases_share_global_identity() {
    assert_eq!(
        common::eval(
            "const d=Object.getOwnPropertyDescriptor(Number,'parseInt');const r=[Number.parseInt===parseInt,Number.parseFloat===parseFloat,Number.parseInt('ff',16),d.enumerable,d.writable,d.configurable].join();",
            "r"
        ),
        "true,true,255,false,true,true"
    );
}

/** A nondecimal expansion rounds at the next-double interval instead of printing 52 digits. */
#[test]
fn radix_fraction_uses_binary64_precision() {
    assert_eq!(
        common::eval(
            "const r=[(1.5).toString(3),(0.1).toString(16)].join();",
            "r"
        ),
        "1.111111111111111111111111111111112,0.1999999999999a"
    );
}

/** The smallest subnormal retains all 1,074 binary places and its final significant bit. */
#[test]
fn radix_fraction_retains_subnormal_digits() {
    assert_eq!(
        common::eval(
            "const s=Number.MIN_VALUE.toString(2);const r=s==='0.'+'0'.repeat(1073)+'1';",
            "r"
        ),
        "true"
    );
}

/** Large integer radix strings omit unrepresented low digits rather than amplifying rounding. */
#[test]
fn radix_integers_zero_unrepresented_digits() {
    assert_eq!(
        common::eval("const r=(1.2345e27).toString(5);", "r"),
        "314404114120101444444424000000000000000"
    );
}
