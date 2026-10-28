/** Exponential formatting rounds exact decimal ties up and prints zero without a sign. */
mod common;

/** An exact tie picks the larger magnitude, for both signs; Rust's formatter rounds to even. */
#[test]
fn exponential_ties_round_away_from_zero() {
    let source = "const r=[(1.25).toExponential(1),(-1.25).toExponential(1),(9.5).toExponential(0),\
        (0.5).toExponential(0),(123.456).toExponential(3)].join();";
    assert_eq!(common::eval(source, "r"), "1.3e+0,-1.3e+0,1e+1,5e-1,1.235e+2");
}

/** Negative zero prints as zero with or without a fraction digit count. */
#[test]
fn negative_zero_prints_unsigned() {
    let source = "const r=[(-0).toExponential(),(-0).toExponential(2),(0).toExponential(1)].join();";
    assert_eq!(common::eval(source, "r"), "0e+0,0.00e+0,0.0e+0");
}

/** Long fraction counts print the exact binary expansion, and a carry bumps the exponent. */
#[test]
fn long_fractions_are_exact_and_carries_move_the_exponent() {
    let source = "const r=[(123.456).toExponential(20),(0.0001).toExponential(16),\
        (9.99).toExponential(1),(1.1e-32).toExponential()].join();";
    assert_eq!(
        common::eval(source, "r"),
        "1.23456000000000003070e+2,1.0000000000000000e-4,1.0e+1,1.1e-32"
    );
}

/** toPrecision shares the tie rule in both its exponential and fixed forms. */
#[test]
fn precision_ties_round_away_from_zero() {
    let source = "const r=[(125).toPrecision(2),(12.5).toPrecision(2),(99.5).toPrecision(2),\
        (1.25).toPrecision(2),(-125).toPrecision(2)].join();";
    assert_eq!(common::eval(source, "r"), "1.3e+2,13,1.0e+2,1.3,-1.3e+2");
}
