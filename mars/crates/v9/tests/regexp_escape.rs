/** RegExp.escape produces literal Unicode patterns while preserving native admission and resource limits. */
use crate::common;

/** A leading ASCII alphanumeric is hexadecimal, while subsequent letters and digits stay literal. */
#[test]
fn only_initial_ascii_alphanumerics_are_hexadecimal() {
    assert_eq!(
        common::eval(
            r#"let r=true;for(const c of '0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz'){r=r&&RegExp.escape(c+'a9')==='\\x'+c.charCodeAt(0).toString(16)+'a9'}r=r&&RegExp.escape('')===''&&RegExp.escape('_a9')==='_a9'&&RegExp.escape('éa9')==='éa9';"#,
            "r"
        ),
        "true"
    );
}

/** Syntax characters, including the literal delimiter, use grammar-valid backslash escapes. */
#[test]
fn all_syntax_characters_and_solidus_are_backslashed() {
    assert_eq!(
        common::eval(
            r#"const syntax='^$\\.*+?()[]{}|/';let r=true;for(const c of syntax){r=r&&RegExp.escape(c)==='\\'+c}r=r&&RegExp.escape(syntax)===Array.from(syntax,c=>'\\'+c).join('');"#,
            "r"
        ),
        "true"
    );
}

/** The five controls use their named escapes; unrelated controls remain exact raw code units. */
#[test]
fn control_table_does_not_overescape_nul_or_backspace() {
    assert_eq!(
        common::eval(
            r#"const controls='\t\n\v\f\r',other='\0\b\x01\x1f\x7f\x85';let r=RegExp.escape(controls)==='\\t\\n\\v\\f\\r'&&RegExp.escape(other)===other;"#,
            "r"
        ),
        "true"
    );
}

/** Punctuation that would form invalid identity escapes in Unicode mode uses two hex digits. */
#[test]
fn other_punctuators_use_lowercase_byte_escapes() {
    assert_eq!(
        common::eval(
            r#"const chars=",-=<>#&!%:;@~'`\"";let r=RegExp.escape(chars)===Array.from(chars,c=>'\\x'+c.charCodeAt(0).toString(16)).join('');"#,
            "r"
        ),
        "true"
    );
}

/** ECMAScript whitespace uses byte or Unicode escapes, without treating all Unicode spacing alike. */
#[test]
fn whitespace_and_line_terminators_follow_ecmascript_categories() {
    assert_eq!(
        common::eval(
            r#"const chars=' \xa0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000\ufeff';let r=true;for(const c of chars){const n=c.charCodeAt(0);r=r&&RegExp.escape(c)===(n<=255?'\\x':'\\u')+n.toString(16).padStart(n<=255?2:4,'0')}const unchanged='\u180e\u200b\u0085';r=r&&RegExp.escape(unchanged)===unchanged;"#,
            "r"
        ),
        "true"
    );
}

/** Unmatched surrogate halves are escaped, but actual pairs remain the original two code units. */
#[test]
fn surrogate_pairs_and_lone_halves_keep_distinct_encodings() {
    assert_eq!(
        common::eval(
            r#"const pair='\ud800\udc00',mixed='\ud800x\udc00'+pair+'\udfff';let r=RegExp.escape(mixed)==='\\ud800x\\udc00'+pair+'\\udfff'&&RegExp.escape(pair)===pair&&RegExp.escape('你好é_')==='你好é_';for(const n of [0xd800,0xdbff,0xdc00,0xdfff])r=r&&RegExp.escape(String.fromCharCode(n))==='\\u'+n.toString(16);"#,
            "r"
        ),
        "true"
    );
}

/** No object, boxed primitive, revoked proxy, or conversion hook is consulted on type rejection. */
#[test]
fn nonprimitive_strings_are_rejected_without_conversion() {
    assert_eq!(
        common::eval(
            r#"let hooks=0,r=true;const object={toString(){hooks++;return 'x'},[Symbol.toPrimitive](){hooks++;return 'x'}},revoked=Proxy.revocable({},{});revoked.revoke();for(const x of [undefined,null,false,1,1n,Symbol(),[],object,new String('x'),revoked.proxy]){try{RegExp.escape(x);r=false}catch(e){r=r&&e instanceof TypeError}}r=r&&hooks===0;"#,
            "r"
        ),
        "true"
    );
}

/** Standard property metadata and nonconstructability come from the normal native method installation. */
#[test]
fn native_function_has_standard_descriptors_and_is_not_a_constructor() {
    assert_eq!(
        common::eval(
            r#"const f=RegExp.escape,d=Object.getOwnPropertyDescriptor(RegExp,'escape'),n=Object.getOwnPropertyDescriptor(f,'name'),l=Object.getOwnPropertyDescriptor(f,'length');let rejected=false;try{new f('x')}catch(e){rejected=e instanceof TypeError}let r=typeof f==='function'&&f.name==='escape'&&f.length===1&&d.writable&&!d.enumerable&&d.configurable&&!n.writable&&!n.enumerable&&n.configurable&&!l.writable&&!l.enumerable&&l.configurable&&!Object.hasOwn(f,'prototype')&&rejected;"#,
            "r"
        ),
        "true"
    );
}

/** Escaped user text matches literally even after a capture reference or inside a Unicode pattern. */
#[test]
fn escaped_text_composes_with_actual_patterns_and_backreferences() {
    assert_eq!(
        common::eval(
            r#"const text='a+b-[x]/ 😀\n';const pattern=new RegExp('^'+RegExp.escape(text)+'$','u');const reference=new RegExp('^(x)\\1'+RegExp.escape('12')+'$','u');let r=pattern.test(text)&&!pattern.test('ab-x')&&reference.test('xx12')&&!reference.test('x12');"#,
            "r"
        ),
        "true"
    );
}

/** Primitive input makes an exact expansion preflight safe, and the existing string ceiling still applies. */
#[test]
fn expansion_rejects_overflow_under_the_existing_string_limit() {
    let count = mars_v9::constants::JS_STRING_UNITS / 4 + 1;
    let execution = common::load(&format!("RegExp.escape('-'.repeat({count}));"));
    assert_eq!(execution.failed.len(), 1);
    assert_eq!(execution.failed[0].error.code, "limit_exceeded");
    assert!(
        execution.failed[0]
            .error
            .message
            .contains("JavaScript string UTF-16 units")
    );
}

/** Exact-ceiling output remains valid rather than rejecting a conservative worst-case estimate. */
#[test]
fn exact_expansion_boundary_is_accepted() {
    let count = mars_v9::constants::JS_STRING_UNITS / 4;
    assert_eq!(
        common::eval(
            &format!("const r=RegExp.escape('-'.repeat({count})).length;"),
            "r"
        ),
        mars_v9::constants::JS_STRING_UNITS.to_string()
    );
}

/** Foreign native calls use the callee's realm for TypeErrors and retain receiver independence. */
#[cfg(feature = "conformance")]
#[test]
fn foreign_escape_uses_its_creation_realm_and_ignores_receiver() {
    assert_eq!(
        common::eval(
            r#"const realm=Realm.create(),f=Realm.eval(realm,'RegExp.escape'),Type=Realm.eval(realm,'TypeError');let foreign=false;try{f.call(null,new String('x'))}catch(e){foreign=e instanceof Type&&!(e instanceof TypeError)}let r=f!==RegExp.escape&&f.call({},'ab+')===RegExp.escape('ab+')&&foreign;"#,
            "r"
        ),
        "true"
    );
}

/** Transported references still dispatch the real builtin without caching escaped text or native handles. */
#[test]
fn escaped_function_and_surrogate_input_survive_transport() {
    let sources=vec![r#"const escape=RegExp.escape,input='\ud800-';document.getElementById('go').onclick=()=>console.log(escape(input)==='\\ud800\\x2d',input.length);"#.to_owned()];
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
    assert_eq!(common::console(&clicked), ["true 2"]);
}
