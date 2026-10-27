/** Literal early errors keep raw lexical restrictions and object-pattern cover grammar distinct. */
mod common;

/** A second literal prototype setter rejects the whole script before any side effects. */
#[test]
fn duplicate_literal_prototypes_are_early_errors() {
    for source in [
        "const o={__proto__:{},__proto__:{}};",
        "const o={__proto__:{},'__proto__':null};",
        "if(false){({__proto__:null,__proto__:null})}",
        "function unused(){return {__proto__:null,__proto__:null}}",
    ] {
        assert_eq!(
            mars_v9::check(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
    let source = "let value=0;try{eval('value=1;({__proto__:value++,__proto__:value++})')}\
        catch(e){if(!(e instanceof SyntaxError))throw e}const r=value;";
    assert_eq!(common::eval(source, "r"), "0");
}

/** Computed, shorthand and method properties named __proto__ are ordinary data properties. */
#[test]
fn ordinary_proto_properties_do_not_count_as_setters() {
    let source = "const __proto__=7;const a={__proto__,__proto__:null};\
        const b={['__proto__']:8,__proto__:null};const c={__proto__(){return 9},__proto__:null};\
        const r=[Object.getPrototypeOf(a)===null,a.__proto__,b.__proto__,c.__proto__()].join();";
    assert_eq!(common::eval(source, "r"), "true,7,8,9");
}

/** Repeated __proto__ destructuring keys are valid and bind the source's ordinary properties. */
#[test]
fn duplicate_proto_pattern_keys_are_valid() {
    let source = "const {__proto__:a,__proto__:b}={['__proto__']:7};let c,d;\
        ({__proto__:c,__proto__:d}={['__proto__']:9});const r=[a,b,c,d].join();";
    assert_eq!(common::eval(source, "r"), "7,7,9,9");
}

/** Legacy integer spelling and decimal/octal escapes are rejected only in strict source. */
#[test]
fn strict_code_rejects_legacy_numeric_and_string_literals() {
    for literal in [
        "010", "08", "09", "'\\1'", "'\\01'", "'\\08'", "'\\8'", "'\\9'",
    ] {
        assert!(
            mars_v9::check(&format!("void {literal}")).is_ok(),
            "{literal}"
        );
        let strict = format!("'use strict';void {literal}");
        assert_eq!(
            mars_v9::check(&strict).unwrap_err().code,
            "script_syntax_error",
            "{literal}"
        );
    }
}

/** A later strict directive applies even to legacy escapes in earlier prologue strings. */
#[test]
fn strict_directives_validate_the_complete_prologue() {
    let source = "function f(){'before\\01';'use strict';}";
    assert_eq!(
        mars_v9::check(source).unwrap_err().code,
        "script_syntax_error"
    );
    assert!(mars_v9::check("'before\\01';function strict(){'use strict';}").is_ok());
}

/** Literal keys obey strict lexical grammar while computed ordinary strings keep their content. */
#[test]
fn strict_property_keys_preserve_raw_lexical_rules() {
    for source in [
        "'use strict';({010:1});",
        "'use strict';({'\\01':1});",
        "class C{010(){}}",
    ] {
        assert_eq!(
            mars_v9::check(source).unwrap_err().code,
            "script_syntax_error",
            "{source}"
        );
    }
    for source in [
        "'use strict';({0o10:1,0x10:2,0b10:3});",
        "'use strict';'\\0';",
        "'use strict';'\\\\1';",
    ] {
        assert!(mars_v9::check(source).is_ok(), "{source}");
    }
}

/** A direct eval inherits its caller's lexical restrictions before nested functions compile. */
#[test]
fn direct_eval_inherits_strict_literal_grammar() {
    let source = "function f(){'use strict';const out=[];for(const code of ['010',\"'\\\\01'\",'function g(){return 010}']){\
        try{eval(code);out.push(false)}catch(e){out.push(e instanceof SyntaxError)}}return out.join()}const r=f();";
    assert_eq!(common::eval(source, "r"), "true,true,true");
}

/** A strict body also restricts a function's own name, parsed before the directive was known. */
#[test]
fn strict_body_rechecks_the_function_name() {
    for name in [
        "implements",
        "interface",
        "let",
        "package",
        "private",
        "protected",
        "public",
        "static",
        "yield",
        "eval",
        "arguments",
    ] {
        for source in [
            format!("function {name}(){{'use strict'}}"),
            format!("(function {name}(){{'use strict'}})"),
        ] {
            assert_eq!(
                mars_v9::check(&source).unwrap_err().code,
                "script_syntax_error",
                "{source}"
            );
        }
        assert!(
            mars_v9::check(&format!("({{ {name}(){{'use strict'}} }})")).is_ok(),
            "method {name}"
        );
    }
}

/** Strict code names each legacy spelling the way V8 does, and sloppy code still accepts them. */
#[test]
fn strict_legacy_spellings_report_v8_messages() {
    for (source, message) in [
        (
            "'use strict';010",
            "Octal literals are not allowed in strict mode.",
        ),
        (
            "'use strict';08",
            "Decimals with leading zeros are not allowed in strict mode.",
        ),
        (
            "'use strict';'\\8'",
            "\\8 and \\9 are not allowed in strict mode.",
        ),
        (
            "'use strict';'\\12'",
            "Octal escape sequences are not allowed in strict mode.",
        ),
    ] {
        let error = mars_v9::check(source).unwrap_err();
        assert!(
            error.message.contains(message),
            "{source}: {}",
            error.message
        );
    }
    assert!(mars_v9::check("'\\8';010;'\\12'").is_ok());
}
