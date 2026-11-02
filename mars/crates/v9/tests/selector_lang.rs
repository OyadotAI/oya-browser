/** :lang() matches an element by its nearest language, using RFC 4647 basic filtering: a range matches itself and its subtags, case-insensitively. */
use crate::common;
use common::eval;

/** An element inherits its language from the nearest ancestor that sets lang. */
#[test]
fn lang_matches_an_inherited_language_by_prefix() {
    let source = "const d=document.createElement('div');d.lang='en-US';\
        const p=document.createElement('p');d.append(p);document.body.append(d);\
        const r=String(p.matches(':lang(en)'));";
    assert_eq!(eval(source, "r"), "true");
}

/** A range that is not a prefix of the language does not match. */
#[test]
fn lang_rejects_a_range_that_is_not_a_prefix() {
    let source = "const p=document.createElement('p');p.lang='en-US';document.body.append(p);\
        const r=String(p.matches(':lang(fr)'));";
    assert_eq!(eval(source, "r"), "false");
}

/** Ranges compare case-insensitively, so en-us matches en-US. */
#[test]
fn lang_compares_case_insensitively() {
    let source = "const p=document.createElement('p');p.lang='en-US';document.body.append(p);\
        const r=String(p.matches(':lang(\"en-us\")'));";
    assert_eq!(eval(source, "r"), "true");
}

/** An empty lang means the language is unknown, so no range matches. */
#[test]
fn lang_matches_nothing_when_the_language_is_unknown() {
    let source = "const p=document.createElement('p');p.lang='';document.body.append(p);\
        const r=String(p.matches(':lang(*)'));";
    assert_eq!(eval(source, "r"), "false");
}
