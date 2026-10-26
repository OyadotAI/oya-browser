/** Annex B leading-zero decimals validate their integer prefix before fraction and exponent separators. */
mod common;

/** A nonoctal leading-zero number keeps the standard fraction and exponent separator grammar. */
#[test]
fn legacy_decimals_allow_separators_after_the_integer_prefix() {
    let source = "const r=[09.1_2,08e1_0,0009.1_2e1_0,077,077.toString(),Number('09.1_2')].join();";
    assert_eq!(
        common::eval(source, "r"),
        "9.12,80000000000,91200000000,63,63,NaN"
    );
}

/** Legacy integer prefixes never admit separators, and octal prefixes never gain decimal suffixes. */
#[test]
fn legacy_prefixes_reject_separators_and_octal_decimal_suffixes() {
    let source = "const inputs=['09_1','09_1.2','07_7','077.1','077e2','00.1','00e2','08.1__2','08e_1'];const r=inputs.every(text=>{try{Function('return '+text);return false}catch(e){return e instanceof SyntaxError}});";
    assert_eq!(common::eval(source, "r"), "true");
}

/** Fractional Annex B spellings remain early errors under a strict directive. */
#[test]
fn strict_code_rejects_legacy_decimal_fractions_and_exponents() {
    let source = "const inputs=['09.1_2','08e1_0','077'];const r=inputs.every(text=>{try{Function('\"use strict\";return '+text);return false}catch(e){return e instanceof SyntaxError}});";
    assert_eq!(common::eval(source, "r"), "true");
}
