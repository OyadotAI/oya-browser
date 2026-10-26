/** Unicode properties use exact UCD categories, aliases and script extensions across the full code-point range. */
mod common;

/** Unicode folding includes every simple equivalent while legacy mode preserves its ASCII and expansion restrictions. */
#[test]
fn case_equivalence_observes_the_unicode_flag() {
    assert_eq!(
        common::eval(
            "const r=[/å/i.test('Å'),/å/ui.test('Å'),/[k]/ui.test('K'),/[ſ]/ui.test('s'),/ß/ui.test('SS'),/ı/i.test('I'),/σ/ui.test('ς'),/\\bſ\\b/ui.test('ſ')].join();",
            "r"
        ),
        "false,true,true,true,false,false,true,true"
    );
}

/** Starting inside a surrogate pair backs up in Unicode mode, while backreferences never consume half of a pair. */
#[test]
fn unicode_positions_preserve_complete_code_points() {
    assert_eq!(
        common::eval(
            "const p=/^./ug;p.lastIndex=1;const m=p.exec('\\ud800\\udc00');const q=/(\\ud800)\\1/u;const r=[m[0].length,p.lastIndex,q.exec('\\ud800\\ud800\\udc00')===null,/(.)\\1/ui.test('\\u{118aa}\\u{118ca}')].join();",
            "r"
        ),
        "2,2,true,true"
    );
}

/** Lookbehind runs quantified groups backwards and cannot begin a match in a surrogate pair's trailing unit. */
#[test]
fn lookbehind_captures_follow_backward_evaluation() {
    assert_eq!(
        common::eval(
            "const m=/(?<=([ab]+)([bc]+))$/.exec('abc');const n=/(?<=([\\udc00-\\udd00]A))B/u.exec('\\ud801\\udc00AB\\udc11AB');const r=[m[1],m[2],n[1]===String.fromCharCode(0xdc11)+'A'].join();",
            "r"
        ),
        "a,bc,true"
    );
}

/** Current supplementary case pairs agree with property data, and complemented class escapes close after folding. */
#[test]
fn new_case_pairs_and_complemented_class_escapes_agree() {
    assert_eq!(
        common::eval(
            "const r=[/\\u{a7ce}/ui.test('\\u{a7cf}'),/\\P{Lu}/ui.test('\\u{a7ce}'),/\\u{16ea0}/ui.test('\\u{16ebb}'),/[\\W]/ui.test('ſ'),/[\\W]/ui.test('K'),/[\\w]/ui.test('ſ')].join();",
            "r"
        ),
        "true,true,true,false,false,true"
    );
}

/** Buffer assertions are opt-in Unicode syntax and remain independent from multiline line anchors. */
#[test]
fn buffer_boundaries_observe_the_explicit_feature_setting() {
    assert_eq!(
        common::eval(
            "let rejected=false;try{new RegExp('\\\\A','u')}catch(e){rejected=e instanceof SyntaxError}const r=[rejected,/\\A/.test('A')].join();",
            "r"
        ),
        "true,true"
    );
    let environment = mars_v9::PageEnvironment {
        regexp_buffer_boundaries: true,
        ..common::environment()
    };
    let execution = common::load_with(
        "let rejected=0;for(const p of ['\\\\A*','[\\\\A]'])try{new RegExp(p,'u')}catch(e){if(e instanceof SyntaxError)rejected++}const r=[/\\Afoo/um.test('\\nfoo'),/foo\\z/um.test('foo\\n'),/foo\\Z/u.test('foo\\r\\n'),'ab\\r\\n'.replace(/\\Z/gu,'|'),rejected,/\\A/.test('A')].join('|');",
        &environment,
    );
    assert!(
        execution.failed.is_empty(),
        "{:?}",
        common::failure(&execution)
    );
    assert_eq!(
        common::global(&execution, "r"),
        "false|false|true|ab|\r|\n||2|true"
    );
}

/** A proven leading anchor avoids scanning impossible start positions without increasing the backtracking budget. */
#[test]
fn grouped_and_lookahead_anchors_bound_failed_searches() {
    assert_eq!(
        common::eval(
            "const text='x'.repeat(400000);const r=[/^y/.test(text),/(^y)/.test(text),/^y|^z/.test(text),/(?=^y)\\w+/.test(text),/^y/m.test('x\\ny')].join();",
            "r"
        ),
        "false,false,false,false,true"
    );
}

/** Scalar properties include astral letters and non-scalar surrogate category membership. */
#[test]
fn properties_cover_astral_points_and_surrogates() {
    assert_eq!(
        common::eval(
            "const r=[/\\p{Ll}/u.test('\\u{1d7be}'),/\\p{Lu}/u.test('\\u{1d5e3}'),/\\p{Cs}/u.test('\\ud800'),/\\p{Any}/u.test('\\udfff'),/\\p{Assigned}/u.test('\\ud800'),/\\p{Cn}/u.test('\\u{10ffff}')].join();",
            "r"
        ),
        "true,true,true,true,true,true"
    );
}

/** Script_Extensions includes common-script characters only for scripts named by their UCD membership. */
#[test]
fn script_extensions_are_distinct_from_script() {
    assert_eq!(
        common::eval(
            "const r=[/\\p{Script=Common}/u.test('\\u30fc'),/\\p{Script=Hiragana}/u.test('\\u30fc'),/\\p{Script_Extensions=Hira}/u.test('\\u30fc'),/\\p{scx=Kana}/u.test('\\u30fc'),/\\p{scx=Latn}/u.test('\\u30fc')].join();",
            "r"
        ),
        "true,false,true,true,false"
    );
}

/** Complemented Unicode properties apply complement before case closure, unlike negated ordinary classes. */
#[test]
fn property_complements_precede_case_closure() {
    assert_eq!(
        common::eval(
            "const r=[/\\P{Lu}/iu.test('A'),/\\P{Lu}/iu.test('a'),/[^A-Z]/iu.test('A'),/[^A-Z]/iu.test('a'),/\\p{ASCII_Hex_Digit}/u.test('F'),/\\p{Math}/u.test('+')].join();",
            "r"
        ),
        "true,true,false,false,true,true"
    );
}

/** Property and value names are case sensitive and script abbreviations require an explicit property. */
#[test]
fn property_aliases_require_exact_spelling() {
    assert_eq!(
        common::eval(
            "let failures=0;for(const name of ['latin','Latin','Block=Latin','script=Latin','gc=letter','Grapheme_Link'])try{new RegExp('\\\\p{'+name+'}','u')}catch(e){if(e instanceof SyntaxError)failures++}const r=[failures,/\\p{gc=L}/u.test('a'),/\\p{sc=Latn}/u.test('a'),/\\p{Alpha}/u.test('a')].join();",
            "r"
        ),
        "6,true,true,true"
    );
}
