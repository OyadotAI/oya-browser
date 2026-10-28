/** RegExp v-flag (ES2024 unicodeSets): set operators, nested classes, \q{} string alternatives and their early errors. */
mod common;

/** Every Unicode string-property family matches real sequences and rejects unrelated text. */
#[test]
fn unicode_emoji_string_properties_match_real_sequences() {
    let source = "const cases=[['Basic_Emoji','⚽'],['Emoji_Keycap_Sequence','4️⃣'],['RGI_Emoji','👨🏾‍⚕️'],['RGI_Emoji_Flag_Sequence','🇧🇪'],['RGI_Emoji_Modifier_Sequence','👍🏽'],['RGI_Emoji_Tag_Sequence','🏴󠁧󠁢󠁳󠁣󠁴󠁿'],['RGI_Emoji_ZWJ_Sequence','👨🏾‍⚕️']];const r=cases.map(([name,text])=>{let re=new RegExp('^\\\\p{'+name+'}$','v');return re.test(text)&&!re.test('xyz')}).join();";
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,true,true"
    );
}

/** String-set algebra retains longest-match behavior and case folding after data expansion. */
#[test]
fn emoji_properties_mix_with_string_sets_and_subtraction() {
    let source = "const r=[/^[\\p{RGI_Emoji_Flag_Sequence}--\\q{🇧🇪}]$/v.test('🇧🇪'),/^[\\p{RGI_Emoji_Flag_Sequence}--\\q{🇧🇪}]$/v.test('🇨🇭'),/^[\\p{Emoji_Keycap_Sequence}\\q{Ab|abc}]$/vi.test('ABC'),/[\\p{Emoji_Keycap_Sequence}\\q{4|4️⃣}]/v.exec('4️⃣')[0]].join();";
    assert_eq!(common::eval(source, "r"), "false,true,true,4️⃣");
}

/** Repeated emoji must not scan every property member at each input position. */
#[test]
fn emoji_properties_match_repeated_characters_and_sequences() {
    let source = "const r=[/^\\p{RGI_Emoji}+$/v.test('⚽'.repeat(300)),/^\\p{RGI_Emoji}+$/v.test('👨🏾‍⚕️'.repeat(30))].join();";
    assert_eq!(common::eval(source, "r"), "true,true");
}

/** Trie alternatives backtrack to a shorter match and also run backward in lookbehind. */
#[test]
fn string_set_tries_preserve_backtracking_and_lookbehind() {
    let source = "const r=[/[\\q{ab|abc}]c/v.exec('abc')[0],/(?<=[\\q{ab|abc}])x/v.exec('abcx')[0],/[\\q{Ab|abc}]c/vi.exec('ABC')[0]].join();";
    assert_eq!(common::eval(source, "r"), "abc,x,ABC");
}

/** Repeated large generated properties remain subject to the existing regex-node budget. */
#[test]
fn repeated_emoji_properties_respect_the_regex_node_limit() {
    let source = "new RegExp('\\\\p{RGI_Emoji}\\\\p{RGI_Emoji}', 'v');";
    let execution = common::load(source);
    let message = common::failure(&execution).expect("two expanded properties exceed the bound");
    assert!(message.contains("too large"), "{message}");
}

/** String properties participate in static set algebra even before matching is supported. */
#[test]
fn string_property_early_errors_are_checked_before_engine_gaps() {
    for source in [
        r"/[^\p{Basic_Emoji}]/v",
        r"/[^[[\p{Basic_Emoji}]]]/v",
        r"/[^\p{Basic_Emoji}&&\p{RGI_Emoji}]/v",
        r"/[^\p{Basic_Emoji}--a]/v",
        r"/[^\p{RGI_Emoji}&&\p{RGI_Emoji}]/v",
        r"/\P{Basic_Emoji}/v",
        r"/\p{Basic_Emoji}/u",
        r"/\P{Basic_Emoji}/u",
        r"/\p{Basic_Emoji}(/v",
    ] {
        assert!(
            mars_v9::check(source).is_err(),
            "{source} must be an early error"
        );
    }
    for source in [
        r"/[^\p{Basic_Emoji}&&a]/v",
        r"/[^a--\p{Basic_Emoji}]/v",
        r"/\p{Basic_Emoji}/v",
    ] {
        assert!(mars_v9::check(source).is_ok(), "{source} must parse");
    }
    assert_eq!(
        common::eval(
            "const bad=['/[^\\\\p{Basic_Emoji}]/v','/\\\\p{Basic_Emoji}(/v'];const r=bad.map(s=>{try{new Function(s);return 'bad'}catch(e){return e instanceof SyntaxError}}).join();",
            "r"
        ),
        "true,true"
    );
    assert_eq!(
        common::eval(
            "let caught=false;try{new RegExp('[^\\\\p{Basic_Emoji}]','v')}catch(e){caught=e instanceof SyntaxError}const r=caught;",
            "r"
        ),
        "true"
    );
    assert!(mars_v9::check(r"if(false){ /[^\p{Basic_Emoji}]/v }").is_err());
}

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

/** Unicode string properties parse and execute, while invalid names remain early errors. */
#[test]
fn unicode_properties_parse_and_invalid_properties_throw() {
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
        common::eval("const r=/^\\p{RGI_Emoji}$/v.test('⚽');", "r"),
        "true"
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
