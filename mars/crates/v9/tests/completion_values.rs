/** Script and eval completion values (spec UpdateEmpty): every statement kind that turns an
empty completion into undefined resets the value, and finally keeps the try's value. */
mod common;
use common::eval;

/** Each case is `source => expected String(eval(source))`. */
const CASES: &[(&str, &str)] = &[
    ("1; if (true) {}", "undefined"),
    ("1; if (false) 2;", "undefined"),
    ("1; while (false) 2;", "undefined"),
    ("1; do { 2; break; } while (false)", "2"),
    ("1; do { 2; if (true) break; } while (false)", "undefined"),
    ("1; for (var i = 0; i < 2; i++) { if (i) 3; }", "3"),
    ("1; for (var k in {}) 2;", "undefined"),
    ("1; for (var v of []) 2;", "undefined"),
    ("1; try { 2; } finally { 3; }", "2"),
    ("1; try { throw 0; } catch (e) {}", "undefined"),
    ("1; switch (0) {}", "undefined"),
    ("1; with ({}) {}", "undefined"),
    ("1; with ({}) 4;", "4"),
    ("1; var x = 2;", "1"),
    ("1; L: { 2; break L; }", "2"),
];

/** Every statement kind produces the completion value the spec gives it. */
#[test]
fn every_statement_kind_has_the_spec_completion_value() {
    for (source, expected) in CASES {
        let program = format!("const out = String(eval({source:?}));");
        assert_eq!(&eval(&program, "out"), expected, "{source}");
    }
}
