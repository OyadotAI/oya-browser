/** Quantified single atoms match long inputs without the recursion depth of the general loop. */
mod common;
use common::eval;

/** Greedy loops over a class, a dot and a literal run over hundreds of thousands of characters. */
#[test]
fn greedy_single_atom_loops_scale_to_long_inputs() {
    let source = "const a='a'.repeat(400000);const astral='\\u{10400}'.repeat(300000);\
        const r=[/^[a-z]+$/.test(a),/^\\p{Alphabetic}+$/u.test(astral),/^.+$/.test(a),/^a+$/.test(a),\
        /^[^x]*x$/.test(a+'x'),/^\\P{Alphabetic}+$/u.test('!'.repeat(200000))].join();";
    assert_eq!(eval(source, "r"), "true,true,true,true,true,true");
}

/** Bounds, laziness, backtracking into the continuation and captures around the loop still hold. */
#[test]
fn flat_loops_keep_quantifier_semantics() {
    let source = "const r=[/^a{2,3}$/.test('aaaa'),/^a{2,3}$/.test('aaa'),/^a{2,}b$/.test('ab'),\
        'aaab'.match(/a+?b/)[0],'aaab'.match(/a*?/)[0],'xaaay'.match(/a+/)[0],\
        'aaa'.match(/^(a+)a$/)[1],'ab\\ncd'.match(/^.+/)[0],'ab\\ncd'.match(/^.+/s)[0],\
        /(?<=a+)b/.test('aaab'),'aaab'.replace(/a{0,2}/,'x'),'AAA'.match(/a+/i)[0]].join();";
    assert_eq!(
        eval(source, "r"),
        "false,true,false,aaab,,aaa,aa,ab,ab\ncd,true,xab,AAA"
    );
}

/** A loop whose atom holds a capture keeps the per-iteration capture reset of the general path. */
#[test]
fn capturing_loops_still_reset_captures_per_iteration() {
    let source = "const m='ab'.match(/(?:(a)|(b))+/);const r=[m[0],m[1],m[2]].join();";
    assert_eq!(eval(source, "r"), "ab,,b");
}

/** Wall time a 300k-character `\p{Alphabetic}` scan may take in a release build. */
const PROPERTY_SCAN_MS: u128 = 200;

/** Class membership binary searches its merged ranges instead of scanning Alphabetic's hundreds
of ranges for every character, which was 4 to 17 times slower on long inputs. */
#[test]
fn property_class_scans_stay_fast_on_long_inputs() {
    let source =
        "const astral='\\u{10400}'.repeat(300000);const r=/^\\p{Alphabetic}+$/u.test(astral);";
    let started = std::time::Instant::now();
    assert_eq!(eval(source, "r"), "true");
    let elapsed = started.elapsed().as_millis();
    if !cfg!(debug_assertions) {
        assert!(elapsed < PROPERTY_SCAN_MS, "{elapsed} ms");
    }
}

/** Merged ranges keep overlapping, adjacent, unordered and case-folded members matching. */
#[test]
fn merged_class_ranges_keep_membership() {
    let source = "const r=[/^[z-za-cb-yA]+$/.test('abczyA'),/^[^d-fa-c]$/.test('g'),/^[^d-fa-c]$/.test('e'),\
        /^[\\d0-5x]+$/.test('09x'),/^[k-m]+$/i.test('KLm'),/[\\u{1F600}-\\u{1F64F}a]/u.test('\\u{1F610}'),\
        /^[\\p{Lu}\\p{Ll}]+$/u.test('aZ'),/^[\\p{L}--[a-z]]+$/v.test('AB'),/^[\\p{L}--[a-z]]+$/v.test('Ab')].join();";
    assert_eq!(
        eval(source, "r"),
        "true,true,false,true,true,true,true,true,false"
    );
}
