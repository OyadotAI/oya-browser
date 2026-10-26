/** Template escapes preserve delimiters and distinguish tagged cooking from early syntax errors. */
mod common;

/** An incomplete escape never eats the substitution or backtick that ends its chunk. */
#[test]
fn incomplete_tagged_escapes_preserve_raw_chunks_and_substitutions() {
    let source = r#"function tag(strings,...values){return [strings[0]===undefined,strings.raw[0],strings[1],values[0]].join('|')}const r=[tag`\u0${7}tail`,tag`\x${8}tail`,tag`\u{${9}tail`,tag`\u0`].join(';');"#;
    assert_eq!(
        common::eval(source, "r"),
        "true|\\u0|tail|7;true|\\x|tail|8;true|\\u{|tail|9;true|\\u0||"
    );
}

/** Octal and decimal escapes are invalid in templates, while a standalone null escape is legal. */
#[test]
fn tagged_legacy_escapes_have_undefined_cooked_values() {
    let source = r#"function tag(strings){return strings[0]===undefined}const r=[tag`\01`,tag`\1`,tag`\8`,tag`\9`,tag`\00`,tag`\0`,String.raw`\u0`].join();"#;
    assert_eq!(
        common::eval(source, "r"),
        "true,true,true,true,true,false,\\u0"
    );
}

/** Invalid untagged escapes reject the entire source before its statements can execute. */
#[test]
fn untagged_invalid_escapes_are_early_errors_even_in_dead_code() {
    let source = r#"const inputs=['`\\u0`','`\\x`','`\\u{`','`\\1`','`\\8`','`\\09`','if(false){`\\u0`}'];const r=inputs.every(text=>{try{Function(text);return false}catch(e){return e instanceof SyntaxError}});"#;
    assert_eq!(common::eval(source, "r"), "true");
}

/** An inner tagged template with an invalid escape does not invalidate its ordinary outer template. */
#[test]
fn nested_tagged_invalid_escapes_do_not_poison_outer_templates() {
    let source =
        r#"function tag(strings){return strings[0]===undefined}const r=`outer:${tag`\u0`}:end`;"#;
    assert_eq!(common::eval(source, "r"), "outer:true:end");
}
