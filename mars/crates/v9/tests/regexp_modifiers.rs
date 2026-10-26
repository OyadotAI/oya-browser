/** Scoped RegExp modifiers affect real compiled matching without leaking into surrounding grammar or flags. */
mod common;

/** Alternations restore the enclosing mode instead of contaminating sibling branches. */
#[test]
fn case_modes_do_not_leak_to_sibling_alternatives() {
    assert_eq!(
        common::eval(
            "const r=/(?-i:a)|b/i.test('B')&&!/(?i:a)|b/.test('B')&&/(?i:c)|d/.test('C')&&!/(?i:c)|d/.test('D');",
            "r"
        ),
        "true"
    );
}

/** Repetition and character classes use the scoped ignoreCase mode rather than only global flags. */
#[test]
fn scoped_case_applies_to_classes_and_repetition() {
    assert_eq!(
        common::eval(
            "const r=/(?i:[A-Z]{10})/.test('abcdefghijklmn')&&/(?i:x|[a-f])/.test('F')&&!/(?i:x|[a-f])/.test('G')&&/^(?i:[a-z])(?-i:[a-z])$/i.test('Ab')&&!/^(?i:[a-z])(?-i:[a-z])$/i.test('AB');",
            "r"
        ),
        "true"
    );
}

/** An inner removal restores the immediate enclosing group's mode after the inner group ends. */
#[test]
fn nested_modes_restore_each_enclosing_scope() {
    assert_eq!(
        common::eval(
            "const re=/^F(?i:oo(?-i:b)a)r$/;const r=re.test('FoObAr')&&!re.test('FooBar')&&!re.test('FoobaR')&&/^F(?i:oo(?i:b)a)r$/.test('FOOBAr');",
            "r"
        ),
        "true"
    );
}

/** The outer continuation uses outer flags, while quantified retry reenters the same inner flags. */
#[test]
fn greedy_backtracking_restores_modes_before_each_continuation() {
    assert_eq!(
        common::eval(
            "const re=/^(?i:a+)[A-Z]$/;const r=re.test('aaA')&&!re.test('aaa')&&/^(?i:a+?)A$/.test('aaA')&&!/^(?i:a+?)A$/.test('aaa');",
            "r"
        ),
        "true"
    );
}

/** Failed matching cannot retain the mode of a previously attempted scoped branch or execution. */
#[test]
fn failed_alternatives_and_executions_restore_outer_modes() {
    assert_eq!(
        common::eval(
            "const re=/^(?:(?i:a)c|b)d$/;let r=!re.test('Bd')&&!re.test('aCd')&&re.test('bd')&&re.test('Acd');for(let i=0;i<20;i++)r=r&&!re.test('BD')&&re.test('bd');",
            "r"
        ),
        "true"
    );
}

/** Added multiline permits searching later lines, while removal still anchors only the affected group. */
#[test]
fn multiline_modes_control_the_actual_anchor_search() {
    assert_eq!(
        common::eval(
            "const re=/(?m:^foo$)/,match=re.exec('x\nfoo\ny');const r=match.index===2&&match[0]==='foo'&&!/(?m:^foo$)bar/.test('x\nfoo\nbar')&&/(?m:^foo$)|bar/.test('x\nfoo')&&!/(?-m:^foo$)/m.test('x\nfoo\ny')&&/(?-m:^foo$)/m.test('foo');",
            "r"
        ),
        "true"
    );
}

/** A scoped multiline assertion in positive lookahead must not trigger an offset-zero-only optimization. */
#[test]
fn scoped_multiline_lookahead_keeps_later_search_candidates() {
    assert_eq!(
        common::eval(
            "const match=/(?=(?m:^foo$))foo/.exec('x\nfoo\ny');const r=match.index===2&&match[0]==='foo'&&!/^(?m:foo$)/.test('x\nfoo')&&/(?m:^foo)(?-m:$)/.test('x\nfoo');",
            "r"
        ),
        "true"
    );
}

/** DotAll changes only scoped dots and composes with independent multiline and case modes. */
#[test]
fn dot_all_scopes_preserve_outer_dots_and_other_flags() {
    assert_eq!(
        common::eval(
            "const r=/^(?s:.)$/.test('\n')&&!/^(?-s:.)$/s.test('\n')&&/^(?s:.).$/.test('\nX')&&!/^(?s:.).$/.test('\n\n')&&/(?ms-i:^f.o$)/i.test('x\nf\ro\ny')&&!/(?ms-i:^f.o$)/i.test('x\nF\ro\ny');",
            "r"
        ),
        "true"
    );
}

/** Positive and negative lookarounds restore mode and direction before matching outer continuations. */
#[test]
fn lookarounds_keep_local_case_modes_without_leaking() {
    assert_eq!(
        common::eval(
            "const r=/^(?=(?i:a))a$/.test('a')&&!/^(?=(?i:a))a$/.test('A')&&/^(?!(?-i:A))a$/i.test('a')&&!/^(?!(?-i:A))a$/i.test('A')&&/(?<=(?i:a))b/.test('Ab')&&!/(?<=(?i:a))b/.test('AB');",
            "r"
        ),
        "true"
    );
}

/** Backreferences use the mode at their own occurrence, independently of the capture's mode. */
#[test]
fn backreference_comparisons_use_the_reference_scope() {
    assert_eq!(
        common::eval(
            r#"const r=/^(a)(?i:\1)$/.test('aA')&&/^(?<x>a)(?i:\k<x>)$/.test('aA')&&!/^(?i:(a))\1$/.test('Aa')&&/^(?i:(a))\1$/.test('AA')&&/^(a)(?-i:\1)$/i.test('aa')&&!/^(a)(?-i:\1)$/i.test('aA');"#,
            "r"
        ),
        "true"
    );
}

/** Unicode simple folding affects words, boundaries, and complemented class escapes under lexical scoped i. */
#[test]
fn unicode_folding_and_word_classes_follow_local_modes() {
    assert_eq!(
        common::eval(
            r#"const kelvin='\u212a';const r=/^(?i:k)$/u.test(kelvin)&&!/^(?i:k)$/.test(kelvin)&&/^(?i:\w)$/u.test(kelvin)&&!/^(?-i:\w)$/iu.test(kelvin)&&/^(?i:\b\u212a\b)$/u.test(kelvin)&&!/^(?-i:\b\u212a\b)$/iu.test(kelvin)&&!/^(?i:[\W])$/u.test(kelvin)&&/^(?-i:[\W])$/iu.test(kelvin);"#,
            "r"
        ),
        "true"
    );
}

/** Modifier groups never become capture groups or change public flags, names, or match indices. */
#[test]
fn capture_numbering_and_public_metadata_remain_unchanged() {
    assert_eq!(
        common::eval(
            "const re=/(?i:(a))(?<named>b)(?-i:(c))/dg,match=re.exec('Abc');const r=[re.flags,re.ignoreCase,re.multiline,re.dotAll,match.length,match[1],match.groups.named,match[3],match.indices[1].join(),re.lastIndex].join();",
            "r"
        ),
        "dg,false,false,false,4,A,b,c,0,1,3"
    );
}

/** Scoped flags leave sticky/global position handling and failed-match resets intact. */
#[test]
fn global_and_sticky_execution_keep_actual_last_index_behavior() {
    assert_eq!(
        common::eval(
            "const re=/(?i:a)/gy;const first=re.exec('Aa');const second=re.exec('Aa');const failed=re.exec('Aa');const r=[first.index,second.index,failed===null,re.lastIndex,re.ignoreCase].join();",
            "r"
        ),
        "0,1,true,0,false"
    );
}

/** Runtime constructors reject exact invalid grammar without accepting escaped or Unicode-folded flag letters. */
#[test]
fn invalid_modifier_grammar_throws_at_construction() {
    assert_eq!(
        common::eval(
            r#"let rejected=0;const invalid=['(?-:a)','(?--:a)','(?ii:a)','(?mm:a)','(?ss:a)','(?-ii:a)','(?i-i:a)','(?ims-s:a)','(?g:a)','(?u:a)','(?v:a)','(?d:a)','(?y:a)','(?I:a)','(?\u0131:a)','(?\\u0069:a)','(?i--m:a)','(?i=a)','(?i!a)','(?i<a)','(?i:a'];for(const pattern of invalid)try{new RegExp(pattern)}catch(error){if(error instanceof SyntaxError)rejected++}const r=rejected===invalid.length;"#,
            "r"
        ),
        "true"
    );
}

/** The grammar allows an empty removal list when the addition list is nonempty, and vice versa. */
#[test]
fn nonempty_modifiers_allow_empty_body_and_empty_removal_side() {
    assert_eq!(
        common::eval(
            "const r=new RegExp('(?i-:a)').test('A')&&new RegExp('(?-i:a)','i').test('a')&&!new RegExp('(?-i:a)','i').test('A')&&new RegExp('(?im-s:)').test('')&&new RegExp('(?i-:)').test('')&&new RegExp('(?-i:)').test('');",
            "r"
        ),
        "true"
    );
}

/** Saved patterns recompile scoped nodes under their real flags after a public state round trip. */
#[test]
fn scoped_patterns_survive_state_transport() {
    let sources=vec!["const re=/(?m:^(?i:foo)$)/g;document.getElementById('go').onclick=()=>{const match=re.exec('x\nFoO\ny');console.log(match[0],match.index,re.lastIndex,re.ignoreCase,re.multiline)};".to_owned()];
    let host = common::host();
    let environment = common::environment();
    let loaded = mars_v9::load_page(common::input(&sources, &host, &environment)).unwrap();
    assert!(loaded.failed.is_empty(), "{:?}", common::failure(&loaded));
    let clicked = mars_v9::click_page(
        common::input(&sources, &host, &environment),
        common::round_trip(loaded.state),
        3,
    )
    .unwrap();
    assert!(clicked.failed.is_empty(), "{:?}", common::failure(&clicked));
    assert_eq!(common::console(&clicked), ["FoO 2 5 false false"]);
}
