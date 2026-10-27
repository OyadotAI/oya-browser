/** Duplicate named groups (ES2025): one name may label groups in different alternatives, and the parser refuses names that can both take part in one match. */
mod common;

/** A name may be declared once in each alternative of a disjunction; the capture that participated is the one reported. */
#[test]
fn duplicate_names_are_allowed_across_alternatives() {
    assert_eq!(
        common::eval(
            "const m=/(?<a>x)|(?<a>y)/.exec('y');const r=[m[0],String(m[1]),m[2],m.groups.a].join();",
            "r"
        ),
        "y,undefined,y,y"
    );
}

/** A name that repeats within one sequence, or inside a group that carries it, is an early SyntaxError whatever the surrounding alternatives. */
#[test]
fn duplicate_names_in_one_sequence_are_early_errors() {
    assert_eq!(
        common::eval(
            "const out=[];for(const src of ['/(?<a>.)(?<a>.)/','/(?<a>(?<a>.)|.)/','/((?<a>.)|(?<b>.))(?<a>.)/','/(?<a>.)|(?<b>.)(?:(?<b>.)|(?<c>.))/']){try{new Function(src);out.push('parsed')}catch(e){out.push(e instanceof SyntaxError)}}const r=out.join();",
            "r"
        ),
        "true,true,true,true"
    );
}

/** A name shared across alternatives still compiles when a later sequence in the same disjunction repeats it, as long as the carriers never meet in one path. */
#[test]
fn duplicate_names_in_separate_alternatives_still_compile() {
    assert_eq!(
        common::eval(
            "const r=[new RegExp('(?<a>.)|(?<a>.)').test('x'),new RegExp('((?<a>.)|((?<b>.)|(?<c>.))(?<a>.))').test('x'),new RegExp('(?<a>.(?<b>.))|(?<a>.)|(?<b>.)').test('x')].join();",
            "r"
        ),
        "true,true,true"
    );
}

/** A named back-reference reads the carrier that participated, so an iteration of a repeated alternation matches what it captured. */
#[test]
fn named_backreference_reads_the_participating_carrier() {
    assert_eq!(
        common::eval(
            "const m=/(?:(?:(?<a>x)|(?<a>y))\\k<a>){2}/.exec('xxyy');const r=[m[0],String(m[1]),m[2],/(?:(?<a>x)|(?<a>y))\\k<a>/.test('yy'),/(?:(?<a>x)|(?<a>y))\\k<a>/.test('xy')].join();",
            "r"
        ),
        "xxyy,undefined,y,true,false"
    );
}

/** The groups object lists each name once, in source order, and holds whichever carrier matched. */
#[test]
fn groups_object_reports_the_defined_carrier_in_source_order() {
    assert_eq!(
        common::eval(
            "const g=/(?<b>x)(?<a>x)|(?<a>y)(?<b>y)/.exec('yy').groups;const r=[Object.keys(g).join(),g.a,g.b].join();",
            "r"
        ),
        "b,a,y,y"
    );
}

/** A regular expression literal that cannot compile is an early SyntaxError when the script is parsed, not when the literal runs. */
#[test]
fn regex_literal_errors_are_early_syntax_errors() {
    assert_eq!(
        common::eval(
            "let r;try{new Function('/a{2,1}/');r='parsed'}catch(e){r=e instanceof SyntaxError}const ok=new Function('/(?<a>.)|(?<a>.)/');r=[r,typeof ok].join();",
            "r"
        ),
        "true,function"
    );
}
