/** RegExp v-flag (ES2024 unicodeSets): set operators, nested classes, \q{} string alternatives and their early errors. */
mod common;

/** Source-level early errors: every malformed v-mode class is a SyntaxError at construction. The snippets run through the engine's own eval, which is the parser under test, not untrusted input. */
#[test]
fn malformed_v_classes_are_early_syntax_errors() {
    assert_eq!(
        common::eval(
            "const bad=['/[(]/v','/[)]/v','/[[]/v','/[]]/v','/[{]/v','/[}]/v','/[/]/v','/[-]/v','/[\\\\]/v','/[|]/v','/[&&]/v','/[!!]/v','/[a&&&]/v','/[&&&a]/v','/[\\\\q{foo]/v','/[\\\\q{foo|]/v','/[^\\\\q{foo}]/v','/[^\\\\q{}]/v','/[^[\\\\q{foo}]]/v','/[^\\\\q{foo}&&\\\\q{bar}]/v','/[\\\\q{\\\\w}]/v','/[a--b&&c]/v','/[a&&b--c]/v','/[\\\\d-z]/v','/[z-\\\\d]/v','/[z-a]/v','/[a-c&&b]/v','/.+/uv','/\\\\P{RGI_Emoji}/v'];const bads=bad.map(s=>{try{eval(s);return 'ok'}catch(e){return e instanceof SyntaxError?'E':'X'}});const ok=['/[^\\\\q{a}]/v','/[^\\\\q{a|b|c}]/v','/[\\\\q{}]/v','/[[a]--[b]]/v'].map(s=>{try{eval(s);return 'ok'}catch(e){return 'X'}});const flagsOk=new RegExp('.','v').flags;const r=[bads.join(','),ok.join(','),flagsOk].join(' ');",
            "r"
        ),
        "E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E,E ok,ok,ok,ok v"
    );
}

/** Set operators: intersection and subtraction over nested classes, with the operands in either order. */
#[test]
fn set_operators_intersect_and_subtract_code_points() {
    assert_eq!(
        common::eval(
            "const r=[/^[\\d&&[02468]]$/v.test('4'),/^[\\d&&[02468]]$/v.test('5'),/^[\\d--[0-9]]$/v.test('4'),/^[[3-7]--[5-9]]$/v.test('4'),/^[[3-7]--[5-9]]$/v.test('6'),/^[[2-8]--[0-3]--5--[7-9]]$/v.test('6'),/^[[2-8]--[0-3]--5--[7-9]]$/v.test('5'),/^[\\w&&[^a-zA-Z_]]$/v.test('5'),/^[\\w&&[^a-zA-Z_]]$/v.test('a'),/^[^\\w&&[a-zA-Z_]]$/v.test('!'),/^[[a-c]--\\0]$/v.test('a')].join();",
            "r"
        ),
        "true,false,false,true,false,true,false,true,false,true,true"
    );
}

/** Case closure under i: operands fold before they meet, so K, k and the Kelvin sign are one character. */
#[test]
fn case_closure_applies_to_each_operand_under_v_and_i() {
    assert_eq!(
        common::eval(
            "const r=[/[K&&K]/vi.test('k'),/[K&&\\u{212A}]/vi.test('k'),/[K--K]/vi.test('k'),/[K--k]/vi.test('K'),/[\\d--[A-Z]]/vi.test('a'),/[\\d--[A-Z]]/vi.test('5'),/^\\P{ASCII}$/iv.test('K'),/^\\P{Lowercase}$/iv.test('A'),/[\\w&&[^a-z_]]/vi.test('a')].join();",
            "r"
        ),
        "true,true,false,false,false,true,false,false,false"
    );
}

/** String alternatives: \q{} members match longest first, then characters, then the empty string. */
#[test]
fn string_alternatives_match_longest_first() {
    assert_eq!(
        common::eval(
            "const r=[/[a-c\\q{W|xy|xyz}]/v.exec('xyzabc')[0],/[a-c\\q{W|xyz|xy}]/v.exec('xyzabc')[0],/[\\q{W|xyz|xy}a-c]/v.exec('xyzabc')[0],/[\\q{W|}a-c]/v.exec('abc')[0],/[\\q{}]/v.exec('foo')[0].length,/^[\\q{foo|bar|0|5}--\\d]$/v.test('foo'),/^[\\q{foo|bar|0|5}--\\d]$/v.test('5'),/^[\\q{foo|bar|0|5}&&\\d]$/v.test('5'),/^[\\q{foo|bar}[05]]$/v.test('bar'),/^[\\q{foo|bar|3|2|0}--\\d]$/v.test('bar')].join();",
            "r"
        ),
        "xyz,xyz,xyz,a,0,true,false,true,true,true"
    );
}

/** Strings take part in set operators and compare case-insensitively under i. */
#[test]
fn string_members_follow_set_operators_and_case_folding() {
    assert_eq!(
        common::eval(
            "const r=[/^[\\q{foo|bar|0|5}&&\\q{bar}]$/v.test('bar'),/^[\\q{foo|bar|0|5}&&\\q{bar}]$/v.test('foo'),/^[\\q{foo|bar|0|5}--\\q{FoO}]$/vi.test('foo'),/^[\\q{foo|bar|0|5}--\\q{FoO}]$/vi.test('bar'),/^[\\q{foo|bar|0|5}&&\\q{bAr}]$/vi.test('BAR'),/^[\\q{ĀĂĄĆ|AaAc}&&\\q{āăąć}]$/vi.test('āăąć'),/^[\\q{ĀĂĄĆ|AaAc}&&\\q{āăąć}]$/vi.test('AaAc'),/^[\\q{ĀĂĄĆ|AaAc}--\\q{āăąć}]$/vi.test('aAaC'),/^[\\q{ĀĂĄĆ|AaAc}--\\q{āăąć}]$/vi.test('ĀĂĄĆ')].join();",
            "r"
        ),
        "true,false,false,true,true,true,false,true,false"
    );
}

/** Negation: a negated class may not contain strings, but a negated class of characters inverts. */
#[test]
fn negated_classes_invert_only_character_members() {
    assert_eq!(
        common::eval(
            "const r=[/^[^[a-c]]$/v.test('b'),/^[^[a-c]]$/v.test('d'),/^[^\\q{a}]$/v.test('b'),/^[^\\q{a}]$/v.test('a'),/^[^[\\q{a}--a]]$/v.test('a')].join();",
            "r"
        ),
        "false,true,true,false,true"
    );
}

/** Parse-time regex checks throw only for ECMAScript early errors. V9's own gaps (a property of strings, a size bound) must parse and fail at run time, so real pages still load. */
#[test]
fn engine_gaps_parse_and_real_early_errors_still_throw() {
    for source in [
        "/\\p{RGI_Emoji}/v",
        "/[\\p{Basic_Emoji}]/v",
        "/\\p{RGI_Emoji_Flag_Sequence}/v",
    ] {
        assert!(mars_v9::check(source).is_ok(), "{source} should parse");
    }
    for source in ["/\\p{NotAProperty}/u", "/[(]/v"] {
        assert!(
            mars_v9::check(source).is_err(),
            "{source} should fail at parse"
        );
    }
    assert_eq!(
        common::eval(
            "let r;try{eval('/\\\\p{RGI_Emoji}/v');r='no error'}catch(e){r=e instanceof SyntaxError?'runtime SyntaxError':'other'}r",
            "r"
        ),
        "runtime SyntaxError"
    );
}

/** Deep nesting of v-mode classes is a limit, not a native stack overflow: 1000 levels fail as an
engine limit (limits are uncatchable, so the page load reports it), and a shallow class compiles. */
#[test]
fn deeply_nested_v_mode_classes_fail_as_a_limit() {
    let deep = format!("{}a{}", "[".repeat(1000), "]".repeat(1000));
    let environment = common::environment();
    let host = common::host();
    let source = format!("new RegExp('{deep}', 'v');");
    let input = mars_v9::PageInput {
        sources: &[source],
        source_keys: &[],
        document: common::page(),
        storage: mars_v9::StorageState::default(),
        host: &host,
        environment: &environment,
        modules: mars_v9::ModuleInput::NONE,
    };
    let execution = mars_v9::load_page(input).expect("page loads");
    let message = common::failure(&execution).expect("the deep class must fail");
    assert!(
        message.contains("SyntaxError: Invalid regular expression"),
        "{message}"
    );
    assert_eq!(
        common::eval(
            "let r = new RegExp('[[a]]', 'v').test('a') ? 'ok' : 'no';",
            "r"
        ),
        "ok"
    );
}
